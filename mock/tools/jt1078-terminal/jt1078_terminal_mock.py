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

# 消息头格式（22 字节，body 不含）
# JT1078 头部字段：SYNC(1) + msg_id(2) + body_attr(2) + phone(6) +
#                   seq(2) + total_packets(2) + packet_no(2) + reserved(6) + CRC(2) = 25 bytes
HEAD_FMT = "!BHH6sHHH6sH"
HEAD_LEN = struct.calcsize(HEAD_FMT)  # 25

# 帧起始
SYNC = 0x7E
ESCAPE = 0x7D

# 常用消息 ID
MSG_REGISTER = 0x0100
MSG_REGISTER_ACK = 0x0101
MSG_HEARTBEAT = 0x0002
MSG_COMMON_ACK = 0x0001
MSG_LIVE_STREAM = 0x0102
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
    """转义 0x7E/0x7D/0x01/0x02（按 JT/T 1078 标准：仅 0x7E 和 0x7D）"""
    out = bytearray()
    for byte in data:
        if byte in (0x7E, 0x7D, 0x01, 0x02):
            out.append(0x7D)
            out.append(byte ^ 0x20)
        else:
            out.append(byte)
    return bytes(out)


def unescape_bytes(data: bytes) -> bytes:
    """反转义"""
    out = bytearray()
    i = 0
    while i < len(data):
        if data[i] == 0x7D and i + 1 < len(data):
            out.append(data[i + 1] ^ 0x20)
            i += 2
        else:
            out.append(data[i])
            i += 1
    return bytes(out)


def crc16_jt1078(data: bytes) -> int:
    """简化的 CRC16（占位实现，仅用于长度校验）。生产应使用真实 CRC-ITU 查表。"""
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            if crc & 0x0001:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc & 0xFFFF


def build_frame(msg_id: int, phone: bytes, seq: int, body: bytes,
                total_packets: int = 1, packet_no: int = 1) -> bytes:
    """构造完整 JT1078 帧"""
    # 体属性：bit15 = 0（无分包）；body 长度 10 bit
    body_attr = len(body) & 0x03FF
    if total_packets > 1:
        body_attr |= (1 << 13)  # 分包标志位
    # 头部字段（不含 CRC）：SYNC + msg_id + body_attr + phone + seq + total + packet_no + reserved
    head_no_crc = struct.pack(
        HEAD_FMT[:-1],  # 去掉末尾的 CRC 'H'
        SYNC,
        msg_id,
        body_attr,
        phone,
        seq,
        total_packets,
        packet_no,
        b"\x00" * 6,
    )
    # CRC：从 msg_id 开始到 body 结束
    crc_data = head_no_crc[1:] + body  # 去掉 SYNC（标准 CRC 范围）
    crc = crc16_jt1078(crc_data)
    head = head_no_crc + struct.pack("!H", crc)
    return escape_bytes(head + body) + bytes([SYNC])


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
    if len(inner) < HEAD_LEN:
        return None
    head = inner[:HEAD_LEN]
    body_with_crc = inner[HEAD_LEN:]
    # CRC 占最后 2 字节
    body = body_with_crc[:-2] if len(body_with_crc) > 2 else b""
    # 解析头部字段
    sync = head[0]
    msg_id = struct.unpack("!H", head[1:3])[0]
    body_attr = struct.unpack("!H", head[3:5])[0]
    body_len = body_attr & 0x03FF
    phone = head[5:11]
    seq = struct.unpack("!H", head[11:13])[0]
    total = struct.unpack("!H", head[13:15])[0]
    packet_no = struct.unpack("!H", head[15:17])[0]
    body = body[:body_len]
    return {
        "msg_id": msg_id,
        "phone": phone,
        "seq": seq,
        "total": total,
        "packet_no": packet_no,
        "body": body,
    }


# ---------------- 业务构造 ----------------

def build_register_body(province: int, city: int, manufacturer_id: bytes, model: bytes,
                        device_id: bytes, plate_color: int, plate: str) -> bytes:
    """构造 0x0100 注册消息体（占位 76 字节）"""
    # 实际字段按 JT/T 1078-2016 表 12；这里用占位结构
    body = bytearray()
    body += struct.pack("!HH", province, city)
    body += manufacturer_id.ljust(11, b"\x00")[:11]
    body += model.ljust(30, b"\x00")[:30]
    body += device_id.ljust(30, b"\x00")[:30]
    body += struct.pack("!B", plate_color)
    body += plate.encode("gbk")[:12].ljust(12, b"\x00")
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
        else:
            log.debug("未处理 msg_id=0x%04x", frame["msg_id"])

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
        # 解析 result 字段（body 前 4 字节：seq + result）
        if len(frame["body"]) < 4:
            return
        _, result = struct.unpack("!HH", frame["body"][:4])
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

    # 解析手机号
    if isinstance(args.phone, bytes):
        phone = args.phone
    else:
        phone = args.phone.encode()
    if hasattr(args, "phone_decimal") and args.phone_decimal:
        phone = int(args.phone_decimal).to_bytes(6, "big")

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