#!/usr/bin/env python3
"""
JT1078 部标终端模拟器

独立可运行的 Python 进程，模拟 JT/T 1078-2016 车辆终端。
无第三方依赖（仅 Python 标准库）。

实现的消息（部分）：
  - 0x0100 终端注册
  - 0x0101 注册应答
  - 0x0002 心跳
  - 0x0001 通用应答
  - 0x0102 实时音视频（仅应答占位，不发实际流）
  - 0x0801 录像列表查询应答
  - 0x8300 文本信息下发（响应）
  - 0x1006-0x100B 报警上报
  - 0x0200 位置上报
  - 重传请求 0x0005（响应）

特性：
  - 启动时模拟丢失率（--simulate-loss）
  - 启动时模拟乱序率（--simulate-reorder）
  - 收到重传请求时按序号重发指定帧

详细文档见 mock/TEST_PLAN.md §5.2
"""

from __future__ import annotations

import argparse
import logging
import os
import random
import signal
import socket
import struct
import sys
import threading
import time
import uuid
from collections import deque
from typing import Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
log = logging.getLogger("jt1078-mock")


# ---------------- JT1078 协议常量 ----------------

# 真实 JT/T 808-2013 消息头（去转义后）：
#   msg_id(2) + 体属性(2) + 终端手机号(6,BCD) + 流水号(2) [+ 分包(4)]
# 帧边界为 0x7E，校验码为 XOR（1 字节），紧跟在消息体之后。
#
# 此前这里用的是**自造格式**：SYNC(1)+msg_id+attr+phone+seq+total+packet_no+
# reserved(6)+CRC16(2)，且转义用 `byte ^ 0x20`、校验用 CRC16 —— 真实终端
# 一条消息都发不出来（这也让平台侧"解析不出来"的缺陷被掩盖）。现已改为国标格式。
HEAD_FMT = "!HH6sH"          # msg_id, body_attr, phone, seq
HEAD_LEN = struct.calcsize(HEAD_FMT)  # 12（不含分包字段）

# 帧起始/转义
SYNC = 0x7E
ESCAPE = 0x7D

# 常用消息 ID
MSG_REGISTER = 0x0100
# JT/T 808-2013：0x8100 = 终端注册应答（此前写 0x0101，平台回的是 0x8100，
# 于是平台明明回了"注册成功"，mock 却永远认为注册超时）。
MSG_REGISTER_ACK = 0x8100
MSG_HEARTBEAT = 0x0002
# 0x0001 = 终端通用应答（终端→平台）；0x8001 = 平台通用应答（平台→终端）
MSG_COMMON_ACK = 0x0001
MSG_PLATFORM_ACK = 0x8001
# 0x9101 = 平台下发的实时音视频传输请求（此前写 0x0102，那是终端鉴权）
MSG_LIVE_STREAM = 0x9101
MSG_RECORD_QUERY_ACK = 0x0801
MSG_TEXT_MSG = 0x8300
MSG_ALARM = 0x1006
MSG_LOCATION = 0x0200
MSG_RETRANSMIT = 0x0005

# 报警子类型
ALARM_SUBTYPES = {
    0x1006: "紧急报警",
    0x1007: "超速报警",
    0x1008: "疲劳驾驶",
    0x1009: "危险驾驶",
    0x100A: "GNSS 故障",
    0x100B: "GNSS 天线断开",
}


# ---------------- 帧编解码 ----------------

def escape_bytes(data: bytes) -> bytes:
    """按 JT/T 808-2013 §4.4.2 转义：0x7E → 0x7D 0x02，0x7D → 0x7D 0x01。"""
    out = bytearray()
    for byte in data:
        if byte == 0x7E:
            out.append(0x7D)
            out.append(0x02)
        elif byte == 0x7D:
            out.append(0x7D)
            out.append(0x01)
        else:
            out.append(byte)
    return bytes(out)


def unescape_bytes(data: bytes) -> bytes:
    """反转义：0x7D 0x02 → 0x7E，0x7D 0x01 → 0x7D。"""
    out = bytearray()
    i = 0
    while i < len(data):
        if data[i] == 0x7D and i + 1 < len(data):
            code = data[i + 1]
            out.append(0x7E if code == 0x02 else (0x7D if code == 0x01 else code))
            i += 2
        else:
            out.append(data[i])
            i += 1
    return bytes(out)


def xor_checksum(data: bytes) -> int:
    """JT/T 808 校验码 = 起始符之后、校验码之前所有字节的异或。"""
    acc = 0
    for b in data:
        acc ^= b
    return acc


def build_frame(msg_id: int, phone: bytes, seq: int, body: bytes,
                total_packets: int = 1, packet_no: int = 1) -> bytes:
    """构造完整 JT/T 808 帧（国标格式 + XOR 校验）。

    转义只作用于起始符之间；起始符本身不转义。
    """
    body_attr = len(body) & 0x03FF
    if total_packets > 1:
        body_attr |= (1 << 13)  # 分包标志
    inner = struct.pack(HEAD_FMT, msg_id, body_attr, phone, seq)
    if total_packets > 1:
        inner += struct.pack("!HH", total_packets, packet_no)
    inner += body
    checksum = xor_checksum(inner)
    return bytes([SYNC]) + escape_bytes(inner + bytes([checksum])) + bytes([SYNC])


def parse_frame(raw: bytes) -> Optional[dict]:
    """解析单帧；未处理分包（packet_no != 1）"""
    if raw[0] != SYNC:
        for i, b in enumerate(raw):
            if b == SYNC:
                raw = raw[i:]
                break
        else:
            return None
    if raw[-1] != SYNC:
        return None
    inner = unescape_bytes(raw[1:-1])
    if len(inner) < HEAD_LEN + 1:
        return None
    # 校验码 = 最后一字节
    checksum = inner[-1]
    if xor_checksum(inner[:-1]) != checksum:
        log.warning("校验码不匹配（按国标 XOR 校验）")
        return None
    msg_id, body_attr, phone, seq = struct.unpack(HEAD_FMT, inner[:HEAD_LEN])
    has_sub = bool(body_attr & 0x2000)
    pos = HEAD_LEN
    total, packet_no = 1, 1
    if has_sub:
        if len(inner) < HEAD_LEN + 4 + 1:
            return None
        total, packet_no = struct.unpack("!HH", inner[pos:pos + 4])
        pos += 4
    body = inner[pos:-1]
    return {
        "msg_id": msg_id,
        "phone": phone,
        "seq": seq,
        "total": total,
        "packet_no": packet_no,
        "body": body[: body_attr & 0x03FF],
    }


# ---------------- 业务构造 ----------------

def build_register_body(province: int, city: int, manufacturer_id: bytes, model: bytes,
                        device_id: bytes, plate_color: int, plate: str) -> bytes:
    """构造 0x0100 注册消息体（JT/T 808-2013 §8.8 真实字段布局）。

    省域ID(2) + 市县域ID(2) + 制造商ID(5) + 终端型号(20) + 终端ID(7)
    + 车牌颜色(1) + 车牌(GBK, 剩余)

    此前这里是自造的 88 字节布局（制造商 11 / 型号 30 / 终端ID 30 / 车牌 12），
    平台按国标偏移解析必然失败 —— 于是"终端永远注册不上"被 mock 掩盖。
    """
    body = bytearray()
    body += struct.pack("!HH", province, city)
    body += manufacturer_id.ljust(5, b"\x00")[:5]
    body += model.ljust(20, b"\x00")[:20]
    body += device_id.ljust(7, b"\x00")[:7]
    body += struct.pack("!B", plate_color)
    body += plate.encode("gbk")
    return bytes(body)


def build_register_ack_body(seq: int, result: int, auth_code: str = "MOCK-AUTH") -> bytes:
    """构造 0x0101 注册应答"""
    body = struct.pack("!HH", seq, result)  # 应答流水号 + 结果
    body += auth_code.encode("ascii")[:64].ljust(64, b"\x00")
    return body


def build_common_ack_body(seq: int, msg_id: int, result: int) -> bytes:
    """构造 0x0001 通用应答"""
    return struct.pack("!HHH", seq, msg_id, result)


def build_location_body(alarm_flag: int, status: int, lat: int, lon: int, speed: int) -> bytes:
    """构造 0x0200 位置上报（占位实现）"""
    body = struct.pack("!I", alarm_flag)         # 报警标志
    body += struct.pack("!I", status)            # 状态
    body += struct.pack("!I", lat)               # 纬度（*1e6）
    body += struct.pack("!I", lon)               # 经度
    body += struct.pack("!H", speed)            # 速度（km/h）
    body += struct.pack("!H", 0)                 # 方向
    body += b"\x00" * 12                         # 时间 + 预留
    return body


def build_alarm_body(alarm_seq: int, lat: int, lon: int, speed: int) -> bytes:
    """构造 0x1006 紧急报警"""
    body = struct.pack("!I", alarm_seq)
    body += build_location_body(0xFFFFFFFF, 0x03, lat, lon, speed)
    return body


def build_text_ack_body(seq: int, msg_id: int, result: int = 0) -> bytes:
    """构造 0x8300 文本应答"""
    return build_common_ack_body(seq, msg_id, result)


# ---------------- 主类 ----------------

class Jt1078TerminalMock:
    """JT1078 终端模拟器"""

    def __init__(
        self,
        server_addr: tuple,
        local_port: int,
        phone: bytes,
        plate: str,
        manufacturer: str = "MOCK",
        model: str = "MOCK-V100",
        device_id: str = "MOCK-DEVICE-001",
        simulate_loss: float = 0.0,
        simulate_reorder: float = 0.0,
        auto_keepalive: int = 30,
        auto_location: int = 10,
    ):
        self.server_addr = server_addr
        self.local_port = local_port
        self.phone = phone
        self.plate = plate
        self.manufacturer = manufacturer.encode("ascii")
        self.model = model.encode("ascii")
        self.device_id = device_id.encode("ascii")
        self.simulate_loss = simulate_loss
        self.simulate_reorder = simulate_reorder
        self.auto_keepalive = auto_keepalive
        self.auto_location = auto_location

        self.seq = 0
        self.registered = False
        self.running = False
        self.sock: Optional[socket.socket] = None
        self.history_frames: dict = {}  # (msg_id, seq) -> bytes（用于重传）
        self.history_lock = threading.Lock()

    def next_seq(self) -> int:
        self.seq = (self.seq + 1) & 0xFFFF
        if self.seq == 0:
            self.seq = 1
        return self.seq

    # ----- 网络 -----

    def start(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.settimeout(1.0)
        self.sock.bind(("0.0.0.0", self.local_port))
        self.sock.connect(self.server_addr)
        self.running = True
        log.info("JT1078 mock listening on UDP port %d, target %s:%d",
                 self.local_port, self.server_addr[0], self.server_addr[1])

        # 注册
        threading.Thread(target=self._register, daemon=True).start()
        # 心跳
        if self.auto_keepalive > 0:
            threading.Thread(target=self._keepalive_loop, daemon=True).start()
        # 位置上报
        if self.auto_location > 0:
            threading.Thread(target=self._location_loop, daemon=True).start()

        # 主循环
        while self.running:
            try:
                data, addr = self.sock.recvfrom(4096)
            except socket.timeout:
                continue
            except OSError:
                break
            self._on_frame(data, addr)

    def stop(self):
        self.running = False
        if self.sock:
            self.sock.close()

    def _on_frame(self, raw: bytes, addr: tuple):
        # 模拟丢包：仅对收到的入站丢（罕见）
        if random.random() < self.simulate_loss * 0.5:
            log.debug("模拟入站丢包: %d bytes", len(raw))
            return
        # 转义在解析时已经处理；这里直接解析
        frame = parse_frame(raw)
        if not frame:
            log.warning("无法解析的入站帧: %s", raw[:60].hex())
            return
        log.info("RX msg_id=0x%04x seq=%d body_len=%d", frame["msg_id"], frame["seq"], len(frame["body"]))
        # 处理
        if frame["msg_id"] == MSG_COMMON_ACK:
            log.info("  通用应答")
        elif frame["msg_id"] == MSG_REGISTER_ACK:
            self._handle_register_ack(frame)
        elif frame["msg_id"] == MSG_LIVE_STREAM:
            self._handle_live_stream(frame)
        elif frame["msg_id"] == MSG_RETRANSMIT:
            self._handle_retransmit(frame)
        elif frame["msg_id"] == 0x9201:  # 录像列表查询
            self._handle_record_query(frame)
        elif frame["msg_id"] == MSG_TEXT_MSG:
            self._handle_text_msg(frame)
        elif 0x8000 <= frame["msg_id"] <= 0x8FFF and frame["msg_id"] != MSG_PLATFORM_ACK:
            # 平台下发的其它命令：真实终端一律回通用应答 0x0001（结果 0），
            # 否则平台的 `*_and_wait` 只能等到超时。此前 mock 只处理极少数
            # 消息，"平台能不能收到终端应答"这条链路无法验证。
            log.info("  平台命令 0x%04x → 回通用应答", frame["msg_id"])
            self._send_common_ack(frame["seq"], frame["msg_id"], 0)
        else:
            log.debug("未处理 msg_id=0x%04x", frame["msg_id"])

    def _send_common_ack(self, reply_seq: int, reply_msg_id: int, result: int):
        """0x0001 终端通用应答：<应答流水号><应答ID><结果>"""
        body = struct.pack("!HHB", reply_seq, reply_msg_id, result)
        self._send(MSG_COMMON_ACK, body)

    # ----- 上行 -----

    def _send(self, msg_id: int, body: bytes):
        if not self.sock:
            return
        # 模拟丢包
        if random.random() < self.simulate_loss:
            log.warning("模拟丢包: msg_id=0x%04x", msg_id)
            return
        seq = self.next_seq()
        frame = build_frame(msg_id, self.phone, seq, body)
        try:
            self.sock.send(frame)
            log.info("TX msg_id=0x%04x seq=%d body_len=%d", msg_id, seq, len(body))
            with self.history_lock:
                self.history_frames[(msg_id, seq)] = frame
                # 保留最近 256 帧
                if len(self.history_frames) > 256:
                    self.history_frames.pop(next(iter(self.history_frames)))
        except OSError as e:
            log.error("send failed: %s", e)

    def _register(self):
        time.sleep(0.5)
        body = build_register_body(
            province=34, city=200,
            manufacturer_id=self.manufacturer[:11],
            model=self.model[:30],
            device_id=self.device_id[:30],
            plate_color=2,  # 黄牌
            plate=self.plate,
        )
        self._send(MSG_REGISTER, body)
        # 等待注册应答；超时则重试
        for _ in range(3):
            if self.registered:
                return
            time.sleep(2)
            if not self.registered:
                log.info("注册超时，重新注册")
                self._send(MSG_REGISTER, body)

    def _keepalive_loop(self):
        while self.running:
            time.sleep(self.auto_keepalive)
            self._send(MSG_HEARTBEAT, b"")

    def _location_loop(self):
        # 模拟北京三环附近移动
        lat = int(39.916527 * 1e6)
        lon = int(116.397128 * 1e6)
        speed = 30
        seq = 0
        while self.running:
            time.sleep(self.auto_location)
            # 随机偏移
            lat += random.randint(-50, 50)
            lon += random.randint(-50, 50)
            speed = max(0, speed + random.randint(-5, 5))
            seq += 1
            body = build_location_body(
                alarm_flag=0, status=0x03,
                lat=lat, lon=lon, speed=speed,
            )
            self._send(MSG_LOCATION, body)

    def trigger_alarm(self, subtype: int = MSG_ALARM):
        """外部触发报警"""
        body = build_alarm_body(alarm_seq=int(time.time()),
                                lat=int(39.916527 * 1e6),
                                lon=int(116.397128 * 1e6),
                                speed=80)
        self._send(subtype, body)
        log.info("触发报警 0x%04x (%s)", subtype, ALARM_SUBTYPES.get(subtype, "未知"))

    # ----- 下行处理 -----

    def _handle_register_ack(self, frame: dict):
        # 0x8100 应答体：应答流水号(WORD) + 结果(BYTE) + 鉴权码(不定长)
        # 此前按 `!HH` 读 4 字节，把"结果(1 字节)+鉴权码首字节"当成 WORD，
        # 于是平台回了 result=0 的成功应答，终端也永远认为注册超时。
        if len(frame["body"]) < 3:
            return
        result = frame["body"][2]
        if result == 0:
            self.registered = True
            log.info("✅ 注册成功")
        else:
            log.warning("注册失败 result=%d", result)

    def _handle_live_stream(self, frame: dict):
        # 实时音视频控制；仅应答
        self._send(MSG_COMMON_ACK, build_common_ack_body(frame["seq"], frame["msg_id"], 0))

    def _handle_retransmit(self, frame: dict):
        # 重传请求：根据 SEQ 重发指定消息
        if len(frame["body"]) < 4:
            return
        target_msg_id = (frame["body"][0] << 8) | frame["body"][1]
        target_seq = struct.unpack("!H", frame["body"][2:4])[0]
        with self.history_lock:
            cached = self.history_frames.get((target_msg_id, target_seq))
        if cached:
            self.sock.send(cached)
            log.info("重传消息: msg_id=0x%04x seq=%d", target_msg_id, target_seq)
        else:
            log.warning("未找到可重传的消息: msg_id=0x%04x seq=%d", target_msg_id, target_seq)

    def _handle_record_query(self, frame: dict):
        # 录像列表查询应答
        ack_body = struct.pack("!HHH", frame["seq"], 0, 1)  # 流水号 + 结果 + 总数
        # 添加一个录像项（占位）
        item = (
            b"\x00\x00\x00\x00"  # 通道
            + b"\x00" * 6         # 开始时间
            + b"\x00" * 6         # 结束时间
            + struct.pack("!I", 1024 * 1024)  # 大小
            + b"\x00" * 64        # 路径
        )
        ack_body += item
        self._send(MSG_RECORD_QUERY_ACK, ack_body)

    def _handle_text_msg(self, frame: dict):
        # 文本下发，回应通用应答
        self._send(MSG_TEXT_MSG, build_text_ack_body(frame["seq"], frame["msg_id"], 0))


# ---------------- CLI ----------------

def main():
    parser = argparse.ArgumentParser(description="JT1078 部标终端模拟器")
    parser.add_argument("--server", default="127.0.0.1:60000")
    parser.add_argument("--local-port", type=int, default=16000)
    parser.add_argument("--phone", default=b"\x00\x00\x00\x00\x00\x01",
                        help="6 字节手机号 BCD；默认 13912345678 → 0x13912345678")
    parser.add_argument("--phone-decimal", default="13912345678",
                        help="十进制手机号（自动转 BCD）")
    parser.add_argument("--plate", default="京A12345")
    parser.add_argument("--manufacturer", default="MOCK")
    parser.add_argument("--model", default="MOCK-V100")
    parser.add_argument("--device-id", default="MOCK-DEVICE-001")
    parser.add_argument("--simulate-loss", type=float, default=0.0,
                        help="丢包率 0.0~1.0（仅上行）")
    parser.add_argument("--simulate-reorder", type=float, default=0.0)
    parser.add_argument("--auto-keepalive", type=int, default=30)
    parser.add_argument("--auto-location", type=int, default=10)
    parser.add_argument("--log-level", default="INFO",
                        choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    args = parser.parse_args()

    log.setLevel(args.log_level)

    host, _, port = args.server.partition(":")
    server_addr = (host, int(port))

    # 解析手机号：**BCD 编码**（JT/T 808 规定终端手机号为 6 字节 BCD）。
    #
    # 此前用 `int(x).to_bytes(6, "big")` 写成**二进制**，而平台按 BCD 解码，
    # 得到的是 "00033=3=8<4>" 这种乱码 —— 终端注册必然失败。
    if hasattr(args, "phone_decimal") and args.phone_decimal:
        digits = [int(c) for c in args.phone_decimal if c.isdigit()][:12]
        while len(digits) < 12:
            digits.append(0)
        phone = bytes(
            (digits[i * 2] << 4) | digits[i * 2 + 1] for i in range(6)
        )
    elif isinstance(args.phone, bytes):
        phone = args.phone
    else:
        phone = args.phone.encode()

    mock = Jt1078TerminalMock(
        server_addr=server_addr,
        local_port=args.local_port,
        phone=phone,
        plate=args.plate,
        manufacturer=args.manufacturer,
        model=args.model,
        device_id=args.device_id,
        simulate_loss=args.simulate_loss,
        simulate_reorder=args.simulate_reorder,
        auto_keepalive=args.auto_keepalive,
        auto_location=args.auto_location,
    )

    def shutdown(signum, frame):
        log.info("Shutting down...")
        mock.stop()

    signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGTERM, shutdown)

    try:
        mock.start()
    except KeyboardInterrupt:
        pass
    finally:
        mock.stop()


if __name__ == "__main__":
    main()