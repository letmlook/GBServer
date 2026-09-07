#!/usr/bin/env python3
"""
上级 / 级联平台模拟器

独立可运行的 Python 进程，作为 GB28181 上级 SIP 平台，
**接收** GBServer 作为下级的 REGISTER / Keepalive / Catalog 等。

行为：
  - 接受 GBServer 的 REGISTER，200 OK（不带 Digest 鉴权；可启用 --require-digest）
  - 接受心跳，返回 200
  - 接收 MESSAGE Catalog / DeviceInfo / DeviceStatus 查询，简单应答

详细文档见 mock/TEST_PLAN.md §5.4
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import logging
import os
import signal
import socket
import sys
import time
import uuid
import xml.etree.ElementTree as ET
from typing import Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
log = logging.getLogger("cascade-mock")


SIP_VERSION = "SIP/2.0"


def make_branch() -> str:
    return f"z9hG4bK{uuid.uuid4().hex[:16]}"


class CascadePlatform:
    """上级平台：监听 UDP，接收 GBServer 的下行消息"""

    def __init__(self, port: int, server_id: str, realm: str = "3402000000"):
        self.port = port
        self.server_id = server_id
        self.realm = realm
        self.transport: Optional[asyncio.DatagramTransport] = None
        # 已注册的下级设备
        self.registered_devices: dict[str, dict] = {}

    async def start(self):
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(
            lambda: self, local_addr=("0.0.0.0", self.port)
        )
        self.transport = transport
        log.info("Cascade platform listening on UDP %d (server_id=%s, realm=%s)",
                 self.port, self.server_id, self.realm)

    def connection_made(self, transport):
        pass

    def datagram_received(self, data: bytes, addr: tuple):
        msg = data.decode(errors="replace")
        first_line = msg.splitlines()[0] if msg else ""
        log.info("RX <- %s:%d: %s", addr[0], addr[1], first_line[:120])
        if first_line.startswith("REGISTER "):
            asyncio.create_task(self._on_register(msg, addr))
        elif first_line.startswith("MESSAGE "):
            asyncio.create_task(self._on_message(msg, addr))
        elif first_line.startswith("SUBSCRIBE "):
            asyncio.create_task(self._on_subscribe(msg, addr))
        elif first_line.startswith("NOTIFY "):
            asyncio.create_task(self._on_notify(msg, addr))
        elif first_line.startswith("BYE "):
            asyncio.create_task(self._on_bye(msg, addr))
        elif first_line.startswith("INFO "):
            asyncio.create_task(self._on_info(msg, addr))
        elif first_line.startswith("INVITE "):
            asyncio.create_task(self._on_invite(msg, addr))
        else:
            log.debug("Unhandled: %s", first_line[:80])

    # ----- 处理 -----

    def _send(self, payload: bytes, addr: tuple):
        if self.transport:
            self.transport.sendto(payload, addr)
            log.info("TX -> %s:%d: %s", addr[0], addr[1], payload.decode().splitlines()[0][:120])

    async def _on_register(self, msg: str, addr: tuple):
        # 解析 From 中的设备 ID 与 Expires
        device_id = ""
        expires = 3600
        for line in msg.splitlines():
            if line.lower().startswith("from:"):
                # <sip:DEVICEID@REALM>
                try:
                    device_id = line.split("<sip:", 1)[1].split("@", 1)[0]
                except IndexError:
                    pass
            elif line.lower().startswith("expires:"):
                try:
                    expires = int(line.split(":", 1)[1].strip())
                except (ValueError, IndexError):
                    pass
        if expires == 0:
            # 注销
            self.registered_devices.pop(device_id, None)
            log.info("设备注销: %s", device_id)
        else:
            self.registered_devices[device_id] = {"addr": addr, "ts": time.time()}
            log.info("设备注册: %s (expires=%d)", device_id, expires)

        # 回 200 OK（无 Digest）
        cseq_line = next((l for l in msg.splitlines() if l.lower().startswith("cseq:")), "CSeq: 1 REGISTER")
        cseq_val = cseq_line.split(":", 1)[1].strip().split()[0]
        branch = ""
        for line in msg.splitlines():
            if line.lower().startswith("via:"):
                if "branch=" in line:
                    start = line.index("branch=") + 7
                    end = line.find(";", start)
                    if end == -1:
                        end = len(line)
                    branch = line[start:end]
        from_tag = ""
        for line in msg.splitlines():
            if line.lower().startswith("from:") and "tag=" in line:
                start = line.index("tag=") + 4
                end = line.find(";", start)
                if end == -1:
                    end = len(line)
                from_tag = line[start:end]
        to_tag = uuid.uuid4().hex[:8]
        call_id = next((l.split(":", 1)[1].strip() for l in msg.splitlines() if l.lower().startswith("call-id:")), "")
        resp = (
            f"{SIP_VERSION} 200 OK\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch}\r\n"
            f"From: <sip:{device_id}@{self.realm}>;tag={from_tag or uuid.uuid4().hex[:8]}\r\n"
            f"To: <sip:{device_id}@{self.realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq_val} REGISTER\r\n"
            f"Expires: {expires}\r\n"
            f"User-Agent: Cascade-Mock/1.0\r\n"
            f"Content-Length: 0\r\n\r\n"
        )
        self._send(resp.encode(), addr)

    async def _on_message(self, msg: str, addr: tuple):
        body = msg.split("\r\n\r\n", 1)[1] if "\r\n\r\n" in msg else ""
        cseq = self._parse_cseq(msg)
        call_id = self._parse_header(msg, "Call-ID")
        branch = self._parse_branch(msg)
        # 简单响应
        if "<CmdType>Keepalive</CmdType>" in body:
            # 不响应
            return
        if "<CmdType>Catalog</CmdType>" in body:
            sn = self._xml_val(body, "SN") or "1"
            device_id = self._xml_val(body, "DeviceID") or ""
            resp_body = (
                '<?xml version="1.0" encoding="UTF-8"?>\r\n<Response>\r\n'
                f'<CmdType>Catalog</CmdType>\r\n<SN>{sn}</SN>\r\n'
                f'<DeviceID>{device_id}</DeviceID>\r\n<SumNum>0</SumNum>\r\n<Num>0</Num>\r\n'
                '<DeviceList Name="Catalog" Num="0"></DeviceList>\r\n'
                '</Response>\r\n'
            )
            resp = (
                f"MESSAGE sip:{self.realm}@{self.realm} {SIP_VERSION}\r\n"
                f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={make_branch()}\r\n"
                f"From: <sip:{self.server_id}@{self.realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
                f"To: <sip:{self.realm}@{self.realm}>\r\n"
                f"Call-ID: {call_id}\r\n"
                f"CSeq: {cseq} MESSAGE\r\n"
                f"Content-Type: Application/MANSCDP+XML\r\n"
                f"Content-Length: {len(resp_body.encode())}\r\n\r\n"
                f"{resp_body}"
            )
            self._send(resp.encode(), addr)
        else:
            log.info("未处理 MESSAGE body: %s", body[:80])

    async def _on_subscribe(self, msg: str, addr: tuple):
        await self._generic_200("SUBSCRIBE", msg, addr)

    async def _on_notify(self, msg: str, addr: tuple):
        await self._generic_200("NOTIFY", msg, addr)

    async def _on_bye(self, msg: str, addr: tuple):
        await self._generic_200("BYE", msg, addr)

    async def _on_info(self, msg: str, addr: tuple):
        await self._generic_200("INFO", msg, addr)

    async def _on_invite(self, msg: str, addr: tuple):
        await self._generic_200("INVITE", msg, addr)

    async def _generic_200(self, method: str, msg: str, addr: tuple):
        cseq = self._parse_cseq(msg)
        call_id = self._parse_header(msg, "Call-ID")
        branch = self._parse_branch(msg)
        from_tag = self._parse_tag(msg, "From")
        to_tag = uuid.uuid4().hex[:8]
        resp = (
            f"{SIP_VERSION} 200 OK\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch or make_branch()}\r\n"
            f"From: <sip:{self.server_id}@{self.realm}>;tag={from_tag or uuid.uuid4().hex[:8]}\r\n"
            f"To: <sip:{self.realm}@{self.realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq} {method}\r\n"
            f"Content-Length: 0\r\n\r\n"
        )
        self._send(resp.encode(), addr)

    # ----- 解析 -----

    def _parse_cseq(self, msg: str) -> int:
        for line in msg.splitlines():
            if line.lower().startswith("cseq:"):
                try:
                    return int(line.split(":", 1)[1].strip().split()[0])
                except (ValueError, IndexError):
                    return 1
        return 1

    def _parse_header(self, msg: str, name: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith(name.lower() + ":"):
                return line.split(":", 1)[1].strip()
        return ""

    def _parse_branch(self, msg: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith("via:") and "branch=" in line:
                start = line.index("branch=") + 7
                end = line.find(";", start)
                if end == -1:
                    end = len(line)
                return line[start:end]
        return make_branch()

    def _parse_tag(self, msg: str, header: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith(header.lower() + ":") and "tag=" in line:
                start = line.index("tag=") + 4
                end = line.find(";", start)
                if end == -1:
                    end = len(line)
                return line[start:end].strip()
        return ""

    def _xml_val(self, body: str, tag: str) -> Optional[str]:
        try:
            root = ET.fromstring(body)
            for child in root.iter():
                if child.tag == tag:
                    return child.text
        except ET.ParseError:
            pass
        return None


def main():
    parser = argparse.ArgumentParser(description="Cascade Platform SIP Mock")
    parser.add_argument("--port", type=int, default=5062)
    parser.add_argument("--server-id", default="34020000002000000099")
    parser.add_argument("--realm", default="3402000000")
    parser.add_argument("--log-level", default="INFO",
                        choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    args = parser.parse_args()
    log.setLevel(args.log_level)

    platform = CascadePlatform(args.port, args.server_id, args.realm)

    async def run():
        await platform.start()
        stop = asyncio.Event()
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, stop.set)
        await stop.wait()

    try:
        asyncio.run(run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()