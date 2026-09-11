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
import json
import re
import threading
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


def compute_digest(username: str, realm: str, password: str, method: str,
                   uri: str, nonce: str, qop: Optional[str] = None,
                   nc: Optional[str] = None, cnonce: Optional[str] = None) -> str:
    """RFC 2617 Digest 响应（与真实上级平台一致的算法）。"""
    ha1 = hashlib.md5(f"{username}:{realm}:{password}".encode()).hexdigest()
    ha2 = hashlib.md5(f"{method}:{uri}".encode()).hexdigest()
    if qop:
        nc = nc or "00000001"
        cnonce = cnonce or uuid.uuid4().hex[:8]
        secret = f"{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}"
    else:
        secret = f"{ha1}:{nonce}:{ha2}"
    return hashlib.md5(secret.encode()).hexdigest()


def parse_auth_params(header: str) -> dict:
    out = {}
    for part in re.split(r",(?=\s*[a-zA-Z]+=)", header):
        if "=" not in part:
            continue
        k, v = part.split("=", 1)
        out[k.strip().lower()] = v.strip().strip('"')
    return out


class CascadePlatform:
    """上级平台：监听 UDP，接收 GBServer 的下行消息"""

    def __init__(self, port: int, server_id: str, realm: str = "3402000000",
                 require_digest: bool = False, password: str = "admin123"):
        self.port = port
        self.server_id = server_id
        self.realm = realm
        self.require_digest = require_digest
        self.password = password
        self.transport: Optional[asyncio.DatagramTransport] = None
        # 已注册的下级设备
        self.registered_devices: dict[str, dict] = {}
        # 每个设备的挑战 nonce
        self.challenges: dict[str, str] = {}
        # 断言用的记账
        self.report = {
            "register_ok": 0,
            "register_challenged": 0,
            "register_rejected": 0,
            "unregister": 0,
            "messages": [],
            "invites": 0,
            "devices": [],
        }

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
        if first_line.startswith(f"{SIP_VERSION} "):
            # 收到的响应（我们发 INVITE 后下级的 200 OK / 4xx / 5xx）
            self.report["last_response_status"] = first_line
            parts = msg.split("\r\n\r\n", 1)
            if first_line.startswith(f"{SIP_VERSION} 200") and len(parts) > 1:
                self.report["last_answer_sdp"] = parts[1]
            return
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

    def _common_response(self, msg: str, addr: tuple, status: str) -> str:
        cseq_line = next((l for l in msg.splitlines() if l.lower().startswith("cseq:")), "CSeq: 1 REGISTER")
        cseq_val = cseq_line.split(":", 1)[1].strip().split()[0]
        branch = ""
        for line in msg.splitlines():
            if line.lower().startswith("via:") and "branch=" in line:
                start = line.index("branch=") + 7
                end = line.find(";", start)
                branch = line[start:end if end != -1 else len(line)]
        from_tag = ""
        for line in msg.splitlines():
            if line.lower().startswith("from:") and "tag=" in line:
                start = line.index("tag=") + 4
                end = line.find(";", start)
                from_tag = line[start:end if end != -1 else len(line)]
        device_id = ""
        for line in msg.splitlines():
            if line.lower().startswith("from:") and "<sip:" in line:
                device_id = line.split("<sip:", 1)[1].split("@", 1)[0]
        call_id = next((l.split(":", 1)[1].strip() for l in msg.splitlines()
                        if l.lower().startswith("call-id:")), "")
        to_tag = uuid.uuid4().hex[:8]
        return (
            f"{SIP_VERSION} {status}\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch}\r\n"
            f"From: <sip:{device_id}@{self.realm}>;tag={from_tag or uuid.uuid4().hex[:8]}\r\n"
            f"To: <sip:{device_id}@{self.realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq_val} REGISTER\r\n"
            f"User-Agent: Cascade-Mock/1.0\r\n"
            f"Content-Length: 0\r\n\r\n"
        )

    def _send_401(self, msg: str, addr: tuple, nonce: str):
        base = self._common_response(msg, addr, "401 Unauthorized")
        base = base.replace(
            "User-Agent:",
            f'WWW-Authenticate: Digest realm="{self.realm}", nonce="{nonce}", algorithm=MD5\r\nUser-Agent:',
            1,
        )
        self._send(base.encode(), addr)

    def _send_status(self, msg: str, addr: tuple, code: int, reason: str):
        self._send(self._common_response(msg, addr, f"{code} {reason}").encode(), addr)

    def _send(self, payload: bytes, addr: tuple):
        if self.transport:
            self.transport.sendto(payload, addr)
            log.info("TX -> %s:%d: %s", addr[0], addr[1], payload.decode().splitlines()[0][:120])

    # ----- 主动发起 INVITE（模拟上级平台点播本级） -----

    def send_invite(
        self,
        target_addr: tuple,
        channel_id: str,
        recv_ip: str,
        recv_port: int,
        ssrc: str,
        our_id: Optional[str] = None,
    ) -> str:
        """向上级（GBServer）发起 INVITE，SDP 里给出**本平台的收流地址**。

        真实上级平台点播本级就是这个动作：`c=`/`m=` 指向上级自己的收流端口，
        并要求下级把流推过来。此前 mock 只会应答 INVITE，无法验证这条链路。
        """
        call_id = f"{uuid.uuid4().hex}@cascade-pull"
        branch = make_branch()
        from_tag = uuid.uuid4().hex[:8]
        local_id = our_id or self.server_id
        body = (
            "v=0\r\n"
            f"o={local_id} 0 0 IN IP4 {recv_ip}\r\n"
            "s=Play\r\n"
            f"c=IN IP4 {recv_ip}\r\n"
            "t=0 0\r\n"
            f"m=video {recv_port} RTP/AVP 96\r\n"
            "a=recvonly\r\n"
            "a=rtpmap:96 PS/90000\r\n"
            f"y={ssrc}\r\n"
        )
        subject = f"{channel_id}:{ssrc},{local_id}:0"
        msg = (
            f"INVITE sip:{channel_id}@{target_addr[0]}:{target_addr[1]} SIP/2.0\r\n"
            f"Via: SIP/2.0/UDP {recv_ip}:{self.port};rport;branch={branch}\r\n"
            f"From: <sip:{local_id}@{self.realm}>;tag={from_tag}\r\n"
            f"To: <sip:{channel_id}@{target_addr[0]}:{target_addr[1]}>\r\n"
            f"Call-ID: {call_id}\r\n"
            "CSeq: 1 INVITE\r\n"
            f"Contact: <sip:{local_id}@{recv_ip}:{self.port}>\r\n"
            f"Subject: {subject}\r\n"
            "Content-Type: application/sdp\r\n"
            f"Content-Length: {len(body.encode())}\r\n\r\n{body}"
        )
        self.report["invites"] += 1
        self._send(msg.encode(), target_addr)
        return call_id

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
        # Digest 鉴权：真实上级平台先回 401 挑战，下级必须带
        # Authorization 重发（RFC 2617）。mock 也要照做，否则
        # "预置假 nonce 的 Proxy-Authenticate"这种假实现会被放过。
        auth_header = next(
            (l.split(":", 1)[1].strip() for l in msg.splitlines()
             if l.lower().startswith("authorization:")), "")
        if self.require_digest and expires != 0:
            if not auth_header:
                nonce = uuid.uuid4().hex
                self.challenges[device_id] = nonce
                self.report["register_challenged"] += 1
                log.info("设备注册挑战(401): %s", device_id)
                self._send_401(msg, addr, nonce)
                return
            params = parse_auth_params(auth_header.replace("Digest", "", 1))
            username = params.get("username", device_id)
            uri = params.get("uri", "")
            expected = compute_digest(
                username, params.get("realm", self.realm), self.password,
                "REGISTER", uri, params.get("nonce", ""),
                qop=params.get("qop"), nc=params.get("nc"),
                cnonce=params.get("cnonce"),
            )
            if not uri or params.get("response") != expected:
                self.report["register_rejected"] += 1
                log.warning(
                    "设备注册鉴权失败: %s（应带正确的 Authorization；uri=%r 挑战 nonce=%r）",
                    device_id, uri, params.get("nonce"),
                )
                self._send_status(msg, addr, 403, "Forbidden")
                return

        if expires == 0:
            # 注销
            self.registered_devices.pop(device_id, None)
            self.report["unregister"] += 1
            log.info("设备注销: %s", device_id)
        else:
            self.registered_devices[device_id] = {"addr": addr, "ts": time.time()}
            self.report["register_ok"] += 1
            if device_id not in self.report["devices"]:
                self.report["devices"].append(device_id)
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
        cmd = ""
        m = re.search(r"<(CmdType)>\s*([^<]+)\s*</\1>", body)
        if m:
            cmd = m.group(2)
        self.report["messages"].append(cmd or "unknown")
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
        self.report["invites"] += 1
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
    parser.add_argument(
        "--require-digest", action="store_true",
        help="REGISTER 需要 Digest 鉴权（先回 401 挑战，校验 Authorization）",
    )
    parser.add_argument("--password", default="admin123", help="下级平台口令（Digest 校验用）")
    parser.add_argument(
        "--control-port", type=int, default=0,
        help="HTTP 控制口（0=关闭）。/trigger/invite 让 mock 主动发 INVITE 模拟上级点播",
    )
    parser.add_argument(
        "--report", default=None,
        help="把注册/消息记账写到该 JSON 文件（供验证脚本断言）",
    )
    args = parser.parse_args()
    log.setLevel(args.log_level)

    platform = CascadePlatform(
        args.port, args.server_id, args.realm,
        require_digest=args.require_digest, password=args.password,
    )

    async def dump_report():
        if args.report:
            try:
                with open(args.report, "w", encoding="utf-8") as fh:
                    json.dump(platform.report, fh, ensure_ascii=False, indent=2)
            except OSError as e:
                log.warning("写报告失败: %s", e)

    _orig_send = platform._send

    def _send_with_report(payload: bytes, addr: tuple):
        _orig_send(payload, addr)
        asyncio.create_task(dump_report())

    platform._send = _send_with_report  # type: ignore[assignment]

    def start_control_server(control_port: int):
        """极简 HTTP 控制口：`/trigger/invite?...` 让 mock 主动发 INVITE。"""
        import http.server
        import urllib.parse

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, fmt, *args):  # noqa: A003
                log.debug("control: " + fmt, *args)

            def do_GET(self):  # noqa: N802
                parsed = urllib.parse.urlparse(self.path)
                params = urllib.parse.parse_qs(parsed.query)
                if parsed.path == "/trigger/invite":
                    channel = params.get("channel", [""])[0]
                    target = params.get("target", ["127.0.0.1:5060"])[0]
                    host, _, port = target.rpartition(":")
                    recv_ip = params.get("recv_ip", ["127.0.0.1"])[0]
                    recv_port = int(params.get("recv_port", ["20000"])[0])
                    ssrc = params.get("ssrc", ["0200000001"])[0]
                    call_id = platform.send_invite(
                        (host, int(port)), channel, recv_ip, recv_port, ssrc
                    )
                    body = json.dumps({"code": 0, "call_id": call_id}).encode()
                elif parsed.path == "/report":
                    body = json.dumps(platform.report, ensure_ascii=False).encode()
                else:
                    self.send_response(404)
                    self.end_headers()
                    return
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        server = http.server.ThreadingHTTPServer(("0.0.0.0", control_port), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        log.info("级联 mock 控制口: http://127.0.0.1:%d/trigger/invite?channel=...", control_port)

    async def run():
        if args.control_port:
            start_control_server(args.control_port)
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