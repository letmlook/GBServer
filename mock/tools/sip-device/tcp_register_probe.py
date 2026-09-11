#!/usr/bin/env python3
"""TCP 信令探针：验证平台把**出站请求**发回设备登记的 TCP 连接。

背景（2026-09-12 第十三/十四轮）：本平台此前只有**响应**按 RFC 3261
§18.2.2 回到同一 TCP 连接，所有**出站请求**（INVITE / ACK / BYE / MESSAGE /
INFO / SUBSCRIBE）都硬编码走 UDP。以 TCP 注册的国标设备只在 TCP 上监听，
走 UDP 的请求会被直接丢弃 —— 现象是"平台发了目录查询/PTZ/BYE，设备毫无反应"，
而平台日志显示已发送。

本探针只做一件事：**只用 TCP** 与平台通信，然后断言平台发起的请求
（心跳 MESSAGE、目录查询等）确实出现在这条 TCP 连接上。

它复用 `sip_device_mock.py` 的消息构造函数，避免第二套 SIP 实现漂移。

用法：
    python3 tcp_register_probe.py \
        --server 127.0.0.1:5061 \
        --device-id 34020000001320000001 \
        --username 34020000001320000001 \
        --password admin123 \
        --listen-seconds 20
"""

from __future__ import annotations

import argparse
import asyncio
import os
import re
import socket
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import sip_device_mock as sdm  # noqa: E402


def parse_host_port(value: str) -> tuple[str, int]:
    host, _, port = value.rpartition(":")
    return host, int(port)


def extract_header(msg: str, name: str, default: str = "") -> str:
    pattern = re.compile(rf"^{re.escape(name)}\s*:\s*(.*)$", re.IGNORECASE | re.MULTILINE)
    match = pattern.search(msg)
    return match.group(1).strip() if match else default


def content_length(msg: str) -> int:
    try:
        return int(extract_header(msg, "Content-Length", "0") or "0")
    except ValueError:
        return 0


class TcpSipProbe:
    def __init__(self, args: argparse.Namespace) -> None:
        self.args = args
        self.host, self.port = parse_host_port(args.server)
        self.local_ip = args.local_ip
        self.reader: asyncio.StreamReader | None = None
        self.writer: asyncio.StreamWriter | None = None
        self.buffer = b""
        self.received: list[str] = []
        self.cseq = 0
        self.registered = False
        self.realm = args.realm or sdm.realm_from_device_id(args.device_id)
        self.local_port = 0

    # ---------- 底层收发 ----------

    async def connect(self) -> None:
        self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
        sock = self.writer.get_extra_info("sockname")
        self.local_port = sock[1] if sock else 0
        print(f"[probe] TCP connected to {self.host}:{self.port} from port {self.local_port}")

    def send(self, message: str) -> None:
        assert self.writer is not None
        self.writer.write(message.encode())
        print(f"[probe] -> {message.splitlines()[0]}")

    def next_cseq(self) -> int:
        self.cseq += 1
        return self.cseq

    async def read_message(self, timeout: float) -> str | None:
        """按 Content-Length 从 TCP 流里切出一条完整的 SIP 消息。"""
        assert self.reader is not None
        header_end = self.buffer.find(b"\r\n\r\n")
        while True:
            if header_end != -1:
                head = self.buffer[:header_end].decode(errors="replace")
                length = content_length(head)
                total = header_end + 4 + length
                if len(self.buffer) >= total:
                    raw = self.buffer[:total]
                    self.buffer = self.buffer[total:]
                    return raw.decode(errors="replace")
            try:
                chunk = await asyncio.wait_for(self.reader.read(4096), timeout=timeout)
            except asyncio.TimeoutError:
                return None
            if not chunk:
                return None
            self.buffer += chunk
            header_end = self.buffer.find(b"\r\n\r\n")

    # ---------- REGISTER ----------

    async def register(self) -> bool:
        """TCP REGISTER（含 401 摘要挑战往返）。"""
        uri = f"sip:{self.realm}@{self.host}:{self.port}"
        self.send(self._build_register(uri, None))
        response = await self.read_message(timeout=5)
        if response is None:
            print("[probe] REGISTER 无响应")
            return False
        print(f"[probe] <- {response.splitlines()[0]}")
        if "401" not in response.splitlines()[0]:
            self.registered = "200" in response.splitlines()[0]
            return self.registered
        auth = extract_header(response, "WWW-Authenticate")
        params = sdm.parse_authorization(auth)
        self.send(self._build_register(uri, params))
        response = await self.read_message(timeout=5)
        if response is None:
            print("[probe] REGISTER(带鉴权) 无响应")
            return False
        print(f"[probe] <- {response.splitlines()[0]}")
        self.registered = "200" in response.splitlines()[0]
        return self.registered

    def _build_register(self, uri: str, challenge: dict | None) -> str:
        cseq = self.next_cseq()
        call_id = f"{self.args.device_id}@{self.local_ip}"
        branch = sdm.make_branch()
        tag = sdm.make_tag() if hasattr(sdm, "make_tag") else sdm.uuid.uuid4().hex[:8]
        headers = [
            f"Via: SIP/2.0/TCP {self.local_ip}:{self.local_port};branch={branch};rport",
            f"From: <sip:{self.args.device_id}@{self.realm}>;tag={tag}",
            f"To: <sip:{self.args.device_id}@{self.realm}>",
            f"Call-ID: {call_id}",
            f"CSeq: {cseq} REGISTER",
            f"Contact: <sip:{self.args.device_id}@{self.local_ip}:{self.local_port};transport=TCP>",
            "Max-Forwards: 70",
            f"Expires: {self.args.expires}",
            "User-Agent: tcp-register-probe/1.0",
        ]
        if challenge:
            nonce = challenge.get("nonce", "")
            qop = challenge.get("qop")
            nc = "00000001" if qop else None
            cnonce = sdm.uuid.uuid4().hex[:8] if qop else None
            digest = sdm.compute_digest_response(
                self.args.username, challenge.get("realm", self.realm),
                self.args.password, "REGISTER", uri, nonce,
                qop=qop, nc=nc, cnonce=cnonce,
            )
            auth = (
                f'Digest username="{self.args.username}", realm="{challenge.get("realm", self.realm)}", '
                f'nonce="{nonce}", uri="{uri}", response="{digest}"'
            )
            if qop:
                auth += f', qop={qop}, nc={nc}, cnonce="{cnonce}"'
            if challenge.get("opaque"):
                auth += f', opaque="{challenge["opaque"]}"'
            headers.append(f"Authorization: {auth}")
        return (
            f"REGISTER {uri} SIP/2.0\r\n"
            + "\r\n".join(headers)
            + "\r\nContent-Length: 0\r\n\r\n"
        )

    # ---------- 接收平台请求 ----------

    async def listen(self, seconds: float) -> list[str]:
        """接收平台发来的请求/响应，返回收到的请求起始行列表。"""
        deadline = time.monotonic() + seconds
        methods: list[str] = []
        while time.monotonic() < deadline:
            remaining = max(0.1, deadline - time.monotonic())
            message = await self.read_message(timeout=remaining)
            if message is None:
                continue
            first = message.splitlines()[0] if message.splitlines() else ""
            self.received.append(message)
            print(f"[probe] <- {first}")
            # 平台请求：需要回 200 OK，否则事务层会重传
            if re.match(r"^(MESSAGE|INVITE|BYE|INFO|SUBSCRIBE|NOTIFY|OPTIONS|CANCEL)\s", first):
                methods.append(first.split()[0])
                self._ack_request(message)
        return methods

    def _ack_request(self, message: str) -> None:
        first = message.splitlines()[0]
        method = first.split()[0]
        via = extract_header(message, "Via")
        from_h = extract_header(message, "From")
        to_h = extract_header(message, "To")
        call_id = extract_header(message, "Call-ID")
        cseq = extract_header(message, "CSeq")
        to_tagged = to_h if ";tag=" in to_h else f"{to_h};tag={sdm.uuid.uuid4().hex[:8]}"
        ok = (
            f"SIP/2.0 200 OK\r\nVia: {via}\r\nFrom: {from_h}\r\nTo: {to_tagged}\r\n"
            f"Call-ID: {call_id}\r\nCSeq: {cseq}\r\nUser-Agent: tcp-register-probe/1.0\r\n"
            f"Content-Length: 0\r\n\r\n"
        )
        assert self.writer is not None
        self.writer.write(ok.encode())
        print(f"[probe] -> 200 OK for {method}")

    async def keepalive_loop(self) -> None:
        """周期性发送 TCP 心跳，维持设备在线。"""
        while True:
            await asyncio.sleep(self.args.keepalive)
            if not self.writer or self.writer.is_closing():
                return
            sn = f"{int(time.time()) % 100000:06d}"
            body = (
                '<?xml version="1.0" encoding="GB2312"?>\r\n'
                "<Notify>\r\n<CmdType>Keepalive</CmdType>\r\n"
                f"<SN>{sn}</SN>\r\n<DeviceID>{self.args.device_id}</DeviceID>\r\n"
                "<Status>OK</Status>\r\n</Notify>\r\n"
            )
            cseq = self.next_cseq()
            branch = sdm.make_branch()
            message = (
                f"MESSAGE sip:{self.realm}@{self.host}:{self.port} SIP/2.0\r\n"
                f"Via: SIP/2.0/TCP {self.local_ip}:{self.local_port};branch={branch};rport\r\n"
                f"From: <sip:{self.args.device_id}@{self.realm}>;tag={sdm.uuid.uuid4().hex[:8]}\r\n"
                f"To: <sip:{self.realm}@{self.realm}>\r\n"
                f"Call-ID: {sdm.uuid.uuid4().hex}@keepalive\r\n"
                f"CSeq: {cseq} MESSAGE\r\n"
                f"Content-Type: Application/MANSCDP+xml\r\n"
                f"Content-Length: {len(body.encode())}\r\n\r\n{body}"
            )
            self.send(message)


async def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default="127.0.0.1:5061", help="平台 SIP **TCP** 地址")
    parser.add_argument("--device-id", default="34020000001320000001")
    parser.add_argument("--username", default="34020000001320000001")
    parser.add_argument("--password", default="admin123")
    parser.add_argument("--realm", default=None)
    parser.add_argument("--expires", type=int, default=3600)
    parser.add_argument("--keepalive", type=int, default=15)
    parser.add_argument("--local-ip", default=None, help="本机用于 Via/Contact 的 IP")
    parser.add_argument("--listen-seconds", type=float, default=20)
    parser.add_argument("--expect", default="MESSAGE", help="期望在 TCP 上收到的请求方法（逗号分隔）")
    args = parser.parse_args()

    if not args.local_ip:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            s.connect((parse_host_port(args.server)[0], parse_host_port(args.server)[1]))
            args.local_ip = s.getsockname()[0]
        finally:
            s.close()

    probe = TcpSipProbe(args)
    await probe.connect()
    if not await probe.register():
        print("[probe] FAIL: TCP REGISTER 未成功")
        return 2
    print("[probe] TCP REGISTER OK（平台应已登记该 TCP 连接）")

    asyncio.create_task(probe.keepalive_loop())
    methods = await probe.listen(args.listen_seconds)

    expected = [m.strip().upper() for m in args.expect.split(",") if m.strip()]
    missing = [m for m in expected if m not in methods]
    print(f"[probe] TCP 上收到的平台请求: {sorted(set(methods)) or '（无）'}")
    if missing:
        print(f"[probe] FAIL: 期望的 {missing} 未出现在 TCP 连接上（说明平台走了 UDP）")
        return 1
    print(f"[probe] PASS: 平台出站请求确实走了 TCP（收到 {sorted(set(methods))}）")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
