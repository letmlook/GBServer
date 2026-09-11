#!/usr/bin/env python3
"""
SIP GB28181 设备模拟器

独立可运行的 Python 进程，模拟 GB/T 28181 国标设备（NVR / IPC / 平台下级）。
无第三方依赖（仅 Python 标准库）。

支持的能力：
  - REGISTER / 401 Digest 鉴权 / 200 OK
  - 周期性 Keepalive (MESSAGE / Notify)
  - 取消注册（Expires: 0）
  - 响应 SUBSCRIBE/MESSAGE Catalog 查询（多包聚合）
  - 响应 DeviceInfo / DeviceStatus 查询
  - 响应 INVITE 实时/回放/对讲 → 200 OK + SDP + ACK + BYE
  - 响应 MANSRTSP PTZ 控制命令

启动示例：
  python3 sip_device_mock.py \
    --server 127.0.0.1:5060 \
    --device-id 34020000001320000001 \
    --username admin \
    --password admin123 \
    --channels 32 \
    --auto-register \
    --auto-keepalive 30

详细文档见 mock/TEST_PLAN.md §5.1
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import logging
import os
import random
import re
import signal
import socket
import sys
import time
import uuid
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from typing import Optional

# ---------------- 常量 ----------------

DEFAULT_PORT = 15060  # 默认本地监听端口（避开 5060，方便与真实设备同机调试）
DEFAULT_KEEPALIVE = 30
DEFAULT_CHANNELS = 4
SIP_VERSION = "SIP/2.0"
USER_AGENT = "GBServer-MockDevice/1.0"

# ---------------- 日志 ----------------

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
log = logging.getLogger("sip-device-mock")


# ---------------- 配置 ----------------

@dataclass
class DeviceConfig:
    """模拟设备配置"""
    device_id: str = "34020000001320000001"
    device_name: str = "MockCamera-01"
    manufacturer: str = "MockVendor"
    model: str = "MOCK-IPC-100"
    firmware: str = "1.0.0-mock"
    channel_count: int = DEFAULT_CHANNELS
    username: str = "admin"
    password: str = "admin123"
    realm: str = "3402000000"
    expires_secs: int = 3600
    # 语音对讲时设备侧收发音频的 RTP 端口（200 OK 的 m=audio 里上报）
    talk_port: int = 10002


@dataclass
class DeviceState:
    """模拟设备运行时状态"""
    registered: bool = False
    keepalive_count: int = 0
    catalog_sn: int = 0
    cseq: int = 0
    invite_count: int = 0
    last_keepalive_ts: float = 0.0

    def next_cseq(self) -> int:
        self.cseq += 1
        return self.cseq


# ---------------- 工具函数 ----------------

def realm_from_device_id(device_id: str) -> str:
    """从 20 位设备 ID 中提取域（中间 8 位）"""
    if len(device_id) >= 18:
        return device_id[0:10]
    return "3402000000"


def make_branch() -> str:
    return f"z9hG4bK{uuid.uuid4().hex[:16]}"


def make_call_id(prefix: str) -> str:
    return f"{prefix}-{uuid.uuid4().hex[:12]}"


def compute_digest_response(
    username: str,
    realm: str,
    password: str,
    method: str,
    uri: str,
    nonce: str,
    qop: Optional[str] = None,
    nc: Optional[str] = None,
    cnonce: Optional[str] = None,
) -> str:
    """
    计算 Digest 鉴权 response（RFC 2617）
    HA1 = MD5(username:realm:password)
    HA2 = MD5(method:uri)
    response = MD5(HA1:nonce:nc:cnonce:qop:HA2)  # 有 qop
    或
    response = MD5(HA1:nonce:HA2)                # 无 qop
    """
    ha1 = hashlib.md5(f"{username}:{realm}:{password}".encode()).hexdigest()
    ha2 = hashlib.md5(f"{method}:{uri}".encode()).hexdigest()
    if qop:
        if not nc:
            nc = "00000001"
        if not cnonce:
            cnonce = uuid.uuid4().hex[:8]
        secret = f"{ha1}:{nonce}:{nc}:{cnonce}:{qop}:{ha2}"
    else:
        secret = f"{ha1}:{nonce}:{ha2}"
    return hashlib.md5(secret.encode()).hexdigest()


def parse_authorization(auth_header: str) -> dict:
    """简单解析 Authorization: Digest ... 头部"""
    result = {}
    auth_header = auth_header.strip()
    if auth_header.startswith("Digest "):
        auth_header = auth_header[7:]
    # 用 ; 分隔键值对
    parts = []
    buf = ""
    in_quote = False
    for ch in auth_header:
        if ch == '"':
            in_quote = not in_quote
            buf += ch
        elif ch == ',' and not in_quote:
            parts.append(buf.strip())
            buf = ""
        else:
            buf += ch
    if buf.strip():
        parts.append(buf.strip())
    for part in parts:
        if "=" not in part:
            continue
        k, _, v = part.partition("=")
        v = v.strip()
        if v.startswith('"') and v.endswith('"'):
            v = v[1:-1]
        result[k.strip()] = v
    return result


def parse_cseq(msg: str) -> int:
    for line in msg.splitlines():
        if line.lower().startswith("cseq:"):
            try:
                return int(line.split(":", 1)[1].strip().split()[0])
            except (ValueError, IndexError):
                return 1
    return 1


# ---------------- 消息构建 ----------------

def build_register(
    cfg: DeviceConfig,
    local_addr: tuple,
    server_addr: tuple,
    cseq: int,
    expires: int,
    authorization_header: Optional[str] = None,
) -> bytes:
    """构造 REGISTER 请求"""
    realm = realm_from_device_id(cfg.device_id)
    branch = make_branch()
    call_id = make_call_id("sim-reg")
    auth_hdr = authorization_header or ""

    msg = (
        f"REGISTER sip:{realm}@{server_addr[0]}:{server_addr[1]} {SIP_VERSION}\r\n"
        f"Via: {SIP_VERSION}/UDP {local_addr[0]}:{local_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
        f"To: <sip:{cfg.device_id}@{realm}>\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {cseq} REGISTER\r\n"
        f"Contact: <sip:{cfg.device_id}@{local_addr[0]}:{local_addr[1]}>\r\n"
        f"Expires: {expires}\r\n"
        f"Max-Forwards: 70\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: 0\r\n"
    )
    if auth_hdr:
        msg += f"Authorization: {auth_hdr}\r\n"
    msg += "\r\n"
    return msg.encode()


def build_keepalive(cfg: DeviceConfig, local_addr: tuple, server_addr: tuple, cseq: int) -> bytes:
    """构造 Keepalive（MESSAGE / Notify）"""
    realm = realm_from_device_id(cfg.device_id)
    sn = f"{cseq:010d}"
    body = (
        '<?xml version="1.0" encoding="UTF-8"?>\r\n'
        '<Notify>\r\n'
        '<CmdType>Keepalive</CmdType>\r\n'
        f'<SN>{sn}</SN>\r\n'
        f'<DeviceID>{cfg.device_id}</DeviceID>\r\n'
        '<Status>OK</Status>\r\n'
        '</Notify>\r\n'
    )
    branch = make_branch()
    call_id = make_call_id("sim-ka")
    msg = (
        f"MESSAGE sip:{realm}@{server_addr[0]}:{server_addr[1]} {SIP_VERSION}\r\n"
        f"Via: {SIP_VERSION}/UDP {local_addr[0]}:{local_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
        f"To: <sip:{realm}@{realm}>\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {cseq} MESSAGE\r\n"
        f"Content-Type: Application/MANSCDP+XML\r\n"
        f"Max-Forwards: 70\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: {len(body.encode())}\r\n"
        f"\r\n"
        f"{body}"
    )
    return msg.encode()


def build_catalog_response(
    cfg: DeviceConfig,
    local_addr: tuple,
    server_addr: tuple,
    cseq: int,
    sn: str,
    sum_num: int,
    chunk: list,
) -> bytes:
    """构造 Catalog 响应（单包；多包逻辑在外层循环）"""
    realm = realm_from_device_id(cfg.device_id)
    items_xml = ""
    for i, ch in enumerate(chunk, start=1):
        channel_id = f"{cfg.device_id[:-4]}{i:04d}" if len(cfg.device_id) >= 20 else f"{cfg.device_id}{i:02d}"
        items_xml += (
            f'<Item>\r\n'
            f'<DeviceID>{channel_id}</DeviceID>\r\n'
            f'<Name>{cfg.device_name}-通道{i}</Name>\r\n'
            f'<Manufacturer>{cfg.manufacturer}</Manufacturer>\r\n'
            f'<Model>{cfg.model}</Model>\r\n'
            f'<Owner>Admin</Owner>\r\n'
            f'<CivilCode>{realm[:6]}</CivilCode>\r\n'
            f'<Address>Mock-Addr-{i}</Address>\r\n'
            f'<Parental>0</Parental>\r\n'
            f'<ParentID>{cfg.device_id}</ParentID>\r\n'
            f'<SafetyWay>0</SafetyWay>\r\n'
            f'<RegisterWay>1</RegisterWay>\r\n'
            f'<CertNum>CERT{i:06d}</CertNum>\r\n'
            f'<Certifiable>0</Certifiable>\r\n'
            f'<ErrCode>0</ErrCode>\r\n'
            f'<PTZType>2</PTZType>\r\n'
            f'<Status>ON</Status>\r\n'
            f'<Longitude>116.397128</Longitude>\r\n'
            f'<Latitude>39.916527</Latitude>\r\n'
            f'</Item>\r\n'
        )
    body = (
        '<?xml version="1.0" encoding="UTF-8"?>\r\n'
        '<Response>\r\n'
        '<CmdType>Catalog</CmdType>\r\n'
        f'<SN>{sn}</SN>\r\n'
        f'<DeviceID>{cfg.device_id}</DeviceID>\r\n'
        f'<SumNum>{sum_num}</SumNum>\r\n'
        f'<Num>{len(chunk)}</Num>\r\n'
        f'<DeviceList Name="Catalog" Num="{len(chunk)}">\r\n'
        f'{items_xml}'
        '</DeviceList>\r\n'
        '</Response>\r\n'
    )
    branch = make_branch()
    call_id = make_call_id("sim-cat")
    msg = (
        f"MESSAGE sip:{realm}@{server_addr[0]}:{server_addr[1]} {SIP_VERSION}\r\n"
        f"Via: {SIP_VERSION}/UDP {local_addr[0]}:{local_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
        f"To: <sip:{realm}@{realm}>\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {cseq} MESSAGE\r\n"
        f"Content-Type: Application/MANSCDP+XML\r\n"
        f"Max-Forwards: 70\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: {len(body.encode())}\r\n"
        f"\r\n"
        f"{body}"
    )
    return msg.encode()


def build_device_info_response(cfg: DeviceConfig, local_addr: tuple, server_addr: tuple, cseq: int, sn: str, call_id: str) -> bytes:
    """构造 DeviceInfo 响应"""
    realm = realm_from_device_id(cfg.device_id)
    body = (
        '<?xml version="1.0" encoding="UTF-8"?>\r\n'
        '<Response>\r\n'
        '<CmdType>DeviceInfo</CmdType>\r\n'
        f'<SN>{sn}</SN>\r\n'
        f'<DeviceID>{cfg.device_id}</DeviceID>\r\n'
        f'<DeviceName>{cfg.device_name}</DeviceName>\r\n'
        f'<Manufacturer>{cfg.manufacturer}</Manufacturer>\r\n'
        f'<Model>{cfg.model}</Model>\r\n'
        f'<FirmwareVersion>{cfg.firmware}</FirmwareVersion>\r\n'
        f'<Channel>{cfg.channel_count}</Channel>\r\n'
        '</Response>\r\n'
    )
    branch = make_branch()
    msg = (
        f"MESSAGE sip:{realm}@{server_addr[0]}:{server_addr[1]} {SIP_VERSION}\r\n"
        f"Via: {SIP_VERSION}/UDP {local_addr[0]}:{local_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
        f"To: <sip:{realm}@{realm}>\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {cseq} MESSAGE\r\n"
        f"Content-Type: Application/MANSCDP+XML\r\n"
        f"Max-Forwards: 70\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: {len(body.encode())}\r\n"
        f"\r\n"
        f"{body}"
        f"\r\n"
        f"{body}"
    )
    return msg.encode()


def build_invite_ok(
    cfg: DeviceConfig,
    local_addr: tuple,
    server_addr: tuple,
    invite_cseq: int,
    call_id: str,
    branch: str,
    from_tag: str,
    to_tag: str,
    ssrc: str,
    request_body: Optional[str] = None,
    talk_port: int = 10002,
) -> bytes:
    """构造 INVITE 200 OK + SDP

    按请求 SDP 里的业务类型回应：
      * `s=Talk` / `m=audio`  → 回 `m=audio`（PCMA）+ `a=sendrecv`
        —— 语音对讲时设备要在自己的端口上**收发**音频，平台据此把
        麦克风音频发到 `c=IN IP4 <device>` + `m=audio <port>`。
      * 其它（实时/回放/下载）→ 回 `m=video`（PS）
    """
    realm = realm_from_device_id(cfg.device_id)
    req = request_body or ""
    is_talk = ("s=Talk" in req) or ("m=audio" in req)
    if is_talk:
        sdp = (
            f"v=0\r\n"
            f"o={cfg.device_id} 0 0 IN IP4 {local_addr[0]}\r\n"
            f"s=Talk\r\n"
            f"c=IN IP4 {local_addr[0]}\r\n"
            f"t=0 0\r\n"
            f"m=audio {talk_port} RTP/AVP 8 0 101\r\n"
            f"a=rtpmap:8 PCMA/8000\r\n"
            f"a=rtpmap:0 PCMU/8000\r\n"
            f"a=rtpmap:101 telephone-event/8000\r\n"
            f"a=sendrecv\r\n"
            f"y={ssrc}\r\n"
        )
    else:
        sdp = (
            f"v=0\r\n"
            f"o={cfg.device_id} 0 0 IN IP4 {local_addr[0]}\r\n"
            f"s=Play\r\n"
            f"c=IN IP4 {local_addr[0]}\r\n"
            f"t=0 0\r\n"
            f"m=video 10000 RTP/AVP 96 97 98\r\n"
            f"a=recvonly\r\n"
            f"a=rtpmap:96 PS/90000\r\n"
            f"a=rtpmap:97 MPEG4/90000\r\n"
            f"a=rtpmap:98 H264/90000\r\n"
            f"y={ssrc}\r\n"
        )
    msg = (
        f"{SIP_VERSION} 200 OK\r\n"
        f"Via: {SIP_VERSION}/UDP {server_addr[0]}:{server_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={from_tag}\r\n"
        f"To: <sip:{realm}@{realm}>;tag={to_tag}\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {invite_cseq} INVITE\r\n"
        f"Contact: <sip:{cfg.device_id}@{local_addr[0]}:{local_addr[1]}>\r\n"
        f"Content-Type: application/sdp\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: {len(sdp.encode())}\r\n"
        f"\r\n"
        f"{sdp}"
    )
    return msg.encode()


def build_bye(cfg: DeviceConfig, local_addr: tuple, server_addr: tuple, cseq: int, call_id: str, branch: str, from_tag: str, to_tag: str) -> bytes:
    """构造 BYE"""
    realm = realm_from_device_id(cfg.device_id)
    msg = (
        f"BYE sip:{realm}@{server_addr[0]}:{server_addr[1]} {SIP_VERSION}\r\n"
        f"Via: {SIP_VERSION}/UDP {local_addr[0]}:{local_addr[1]};rport;branch={branch}\r\n"
        f"From: <sip:{cfg.device_id}@{realm}>;tag={from_tag}\r\n"
        f"To: <sip:{realm}@{realm}>;tag={to_tag}\r\n"
        f"Call-ID: {call_id}\r\n"
        f"CSeq: {cseq} BYE\r\n"
        f"Max-Forwards: 70\r\n"
        f"User-Agent: {USER_AGENT}\r\n"
        f"Content-Length: 0\r\n"
        f"\r\n"
    )
    return msg.encode()


# ---------------- 主类 ----------------

class SipDeviceMock:
    """SIP GB28181 设备模拟器"""

    def __init__(
        self,
        cfg: DeviceConfig,
        server_addr: tuple,
        local_port: int = DEFAULT_PORT,
        auto_register: bool = True,
        auto_keepalive: int = 0,
    ):
        self.cfg = cfg
        self.server_addr = server_addr
        self.local_port = local_port
        self.auto_register = auto_register
        self.auto_keepalive_interval = auto_keepalive

        self.state = DeviceState()
        self.transport: Optional[asyncio.DatagramTransport] = None
        self.server_nonce: Optional[str] = None
        # 服务端 401 里宣告的 qop；为 "auth" 时按 RFC 2617 用 qop 计算 digest
        self.server_qop: Optional[str] = None
        self.server_realm: Optional[str] = None
        self.last_register_ts = 0.0
        self._keepalive_task: Optional[asyncio.Task] = None
        self._register_task: Optional[asyncio.Task] = None
        self._invite_state: dict = {}  # call_id -> {from_tag, to_tag, branch}

    # ----- UDP 收发 -----

    async def start(self):
        loop = asyncio.get_running_loop()
        transport, _ = await loop.create_datagram_endpoint(
            lambda: self,
            local_addr=("0.0.0.0", self.local_port),
            allow_broadcast=False,
        )
        self.transport = transport
        local = transport.get_extra_info("sockname")
        log.info("SIP mock listening on %s:%d, targeting %s:%d",
                 local[0], local[1], self.server_addr[0], self.server_addr[1])
        if self.auto_register:
            self._register_task = asyncio.create_task(self._auto_register_loop())

    def connection_made(self, transport):
        pass

    def datagram_received(self, data: bytes, addr: tuple):
        msg = data.decode(errors="replace")
        first_line = msg.splitlines()[0] if msg else ""
        log.info("RX <- %s:%d: %s", addr[0], addr[1], first_line[:120])
        # 分发
        if first_line.startswith(SIP_VERSION) and " 401 " in first_line:
            asyncio.create_task(self._on_401(msg, addr))
        elif first_line.startswith(SIP_VERSION) and " 200 " in first_line:
            asyncio.create_task(self._on_200(msg, addr))
        elif first_line.startswith(SIP_VERSION) and " 407 " in first_line:
            asyncio.create_task(self._on_407(msg, addr))
        elif first_line.startswith("MESSAGE "):
            asyncio.create_task(self._on_message(msg, addr))
        elif first_line.startswith("INVITE "):
            asyncio.create_task(self._on_invite(msg, addr))
        elif first_line.startswith("BYE "):
            asyncio.create_task(self._on_bye(msg, addr))
        elif first_line.startswith("INFO "):
            asyncio.create_task(self._on_info(msg, addr))
        elif first_line.startswith("SUBSCRIBE "):
            asyncio.create_task(self._on_subscribe(msg, addr))
        else:
            log.debug("Unhandled: %s", first_line[:200])

    # ----- 处理响应 -----

    async def _on_401(self, msg: str, addr: tuple):
        # 提取 nonce 与 realm，重新发送带 Digest 的 REGISTER
        realm = self._extract_auth_param(msg, "WWW-Authenticate", "realm")
        nonce = self._extract_auth_param(msg, "WWW-Authenticate", "nonce")
        if not realm or not nonce:
            log.warning("401 缺少 realm/nonce，跳过重试")
            return
        self.server_realm = realm
        self.server_nonce = nonce
        # 服务端宣告 qop 时按 RFC 2617 用 qop=auth 计算（覆盖服务端的 qop 分支）
        qop_raw = self._extract_auth_param(msg, "WWW-Authenticate", "qop") or ""
        self.server_qop = "auth" if "auth" in qop_raw.lower() else None
        await asyncio.sleep(0.05)
        await self._send_register_with_digest(addr)

    async def _on_200(self, msg: str, addr: tuple):
        cseq_method = self._extract_header(msg, "CSeq", "")
        log.info("200 OK for %s", cseq_method)
        if "REGISTER" in cseq_method:
            self.state.registered = True
            log.info("设备已注册到 %s", addr)
            if self.auto_keepalive_interval > 0 and self._keepalive_task is None:
                self._keepalive_task = asyncio.create_task(self._keepalive_loop())

    async def _on_407(self, msg: str, addr: tuple):
        # 同 401，代理鉴权
        await self._on_401(msg, addr)

    async def _on_407_proxy(self, msg: str, addr: tuple):
        realm = self._extract_auth_param(msg, "Proxy-Authenticate", "realm")
        nonce = self._extract_auth_param(msg, "Proxy-Authenticate", "nonce")
        if realm and nonce:
            self.server_realm = realm
            self.server_nonce = nonce

    # ----- 处理请求（来自 GBServer） -----

    async def _on_message(self, msg: str, addr: tuple):
        """处理 MESSAGE 请求（GBServer 发来的查询）"""
        body = msg.split("\r\n\r\n", 1)[1] if "\r\n\r\n" in msg else ""
        if "<CmdType>Catalog</CmdType>" in body:
            sn = self._extract_xml_value(body, "SN") or "1"
            sum_num = self.cfg.channel_count
            # 多包聚合：按 8 个一分包
            chunk_size = 8
            chunks = [list(range(i, min(i + chunk_size, sum_num))) for i in range(0, sum_num, chunk_size)]
            for idx, chunk in enumerate(chunks):
                cseq = self.state.next_cseq()
                local = self.transport.get_extra_info("sockname")
                payload = build_catalog_response(
                    self.cfg, local, self.server_addr, cseq, sn, sum_num, chunk
                )
                self.transport.sendto(payload, addr)
                await asyncio.sleep(0.05)
        elif "<CmdType>DeviceInfo</CmdType>" in body:
            sn = self._extract_xml_value(body, "SN") or "1"
            call_id = self._extract_header(msg, "Call-ID", "")
            cseq = self.state.next_cseq()
            local = self.transport.get_extra_info("sockname")
            payload = build_device_info_response(self.cfg, local, self.server_addr, cseq, sn, call_id)
            self.transport.sendto(payload, addr)
        elif "<CmdType>DeviceStatus</CmdType>" in body:
            await self._reply_device_status(msg, addr)
        else:
            log.debug("未识别 MESSAGE body: %s", body[:200])

    async def _reply_device_status(self, msg: str, addr: tuple):
        sn = self._extract_xml_value(msg.split("\r\n\r\n", 1)[1], "SN") or "1"
        call_id = self._extract_header(msg, "Call-ID", "")
        cseq = self.state.next_cseq()
        realm = realm_from_device_id(self.cfg.device_id)
        body = (
            '<?xml version="1.0" encoding="UTF-8"?>\r\n'
            '<Response>\r\n'
            '<CmdType>DeviceStatus</CmdType>\r\n'
            f'<SN>{sn}</SN>\r\n'
            f'<DeviceID>{self.cfg.device_id}</DeviceID>\r\n'
            '<Result>OK</Result>\r\n'
            '<Online>ONLINE</Online>\r\n'
            '<Status>OK</Status>\r\n'
            '</Response>\r\n'
        )
        branch = make_branch()
        payload = (
            f"MESSAGE sip:{realm}@{self.server_addr[0]}:{self.server_addr[1]} {SIP_VERSION}\r\n"
            f"Via: {SIP_VERSION}/UDP {self.transport.get_extra_info('sockname')[0]}:{self.transport.get_extra_info('sockname')[1]};rport;branch={branch}\r\n"
            f"From: <sip:{self.cfg.device_id}@{realm}>;tag={uuid.uuid4().hex[:8]}\r\n"
            f"To: <sip:{realm}@{realm}>\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq} MESSAGE\r\n"
            f"Content-Type: Application/MANSCDP+XML\r\n"
            f"Content-Length: {len(body.encode())}\r\n"
            f"\r\n{body}"
        )
        self.transport.sendto(payload.encode(), addr)

    async def _on_invite(self, msg: str, addr: tuple):
        """处理 INVITE，发送 200 OK + SDP，3 秒后 BYE"""
        invite_cseq = parse_cseq(msg)
        call_id = self._extract_header(msg, "Call-ID", "") or make_call_id("sim-inv")
        branch = self._extract_via_branch(msg)
        from_tag = self._extract_from_tag(msg)
        to_tag = uuid.uuid4().hex[:8]
        ssrc = f"{random.randint(0, 0xFFFFFFFF):08X}"
        self._invite_state[call_id] = {
            "from_tag": from_tag, "to_tag": to_tag, "branch": branch,
            "cseq": invite_cseq, "started": time.time(),
        }
        local = self.transport.get_extra_info("sockname")
        req_body = msg.split("\r\n\r\n", 1)[1] if "\r\n\r\n" in msg else ""
        payload = build_invite_ok(
            self.cfg, local, addr, invite_cseq, call_id, branch, from_tag, to_tag, ssrc,
            request_body=req_body, talk_port=self.cfg.talk_port,
        )
        self.transport.sendto(payload, addr)
        log.info(
            "INVITE 200 OK sent for call %s, ssrc=%s (%s)",
            call_id, ssrc, "audio/Talk" if ("s=Talk" in req_body or "m=audio" in req_body) else "video/Play",
        )
        # 模拟媒体会话：3 秒后主动 BYE
        async def delayed_bye():
            await asyncio.sleep(3)
            state = self._invite_state.get(call_id)
            if not state:
                return
            cseq = self.state.next_cseq()
            payload = build_bye(self.cfg, local, addr, cseq, call_id, make_branch(), state["from_tag"], state["to_tag"])
            self.transport.sendto(payload, addr)
            log.info("BYE sent for call %s", call_id)
        asyncio.create_task(delayed_bye())

    async def _on_bye(self, msg: str, addr: tuple):
        # 回应 200 OK
        cseq = parse_cseq(msg)
        call_id = self._extract_header(msg, "Call-ID", "")
        branch = make_branch()
        from_tag = self._extract_from_tag(msg)
        to_tag = self._extract_to_tag(msg)
        realm = realm_from_device_id(self.cfg.device_id)
        local = self.transport.get_extra_info("sockname")
        payload = (
            f"{SIP_VERSION} 200 OK\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch}\r\n"
            f"From: <sip:{self.cfg.device_id}@{realm}>;tag={from_tag}\r\n"
            f"To: <sip:{realm}@{realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq} BYE\r\n"
            f"Content-Length: 0\r\n"
            f"\r\n"
        )
        self.transport.sendto(payload.encode(), addr)
        self._invite_state.pop(call_id, None)

    async def _on_info(self, msg: str, addr: tuple):
        """处理 INFO（MANSRTSP PTZ）"""
        cseq = parse_cseq(msg)
        call_id = self._extract_header(msg, "Call-ID", "")
        branch = make_branch()
        from_tag = self._extract_from_tag(msg)
        to_tag = self._extract_to_tag(msg)
        realm = realm_from_device_id(self.cfg.device_id)
        local = self.transport.get_extra_info("sockname")
        payload = (
            f"{SIP_VERSION} 200 OK\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch}\r\n"
            f"From: <sip:{self.cfg.device_id}@{realm}>;tag={from_tag}\r\n"
            f"To: <sip:{realm}@{realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq} INFO\r\n"
            f"Content-Length: 0\r\n"
            f"\r\n"
        )
        self.transport.sendto(payload.encode(), addr)
        body = msg.split("\r\n\r\n", 1)[1] if "\r\n\r\n" in msg else ""
        log.info("PTZ INFO 收到: %s", body[:80])

    async def _on_subscribe(self, msg: str, addr: tuple):
        """处理 SUBSCRIBE（目录订阅等）"""
        cseq = parse_cseq(msg)
        branch = self._extract_via_branch(msg)
        call_id = self._extract_header(msg, "Call-ID", "")
        from_tag = self._extract_from_tag(msg)
        to_tag = uuid.uuid4().hex[:8]
        realm = realm_from_device_id(self.cfg.device_id)
        local = self.transport.get_extra_info("sockname")
        payload = (
            f"{SIP_VERSION} 200 OK\r\n"
            f"Via: {SIP_VERSION}/UDP {addr[0]}:{addr[1]};rport;branch={branch}\r\n"
            f"From: <sip:{self.cfg.device_id}@{realm}>;tag={from_tag}\r\n"
            f"To: <sip:{realm}@{realm}>;tag={to_tag}\r\n"
            f"Call-ID: {call_id}\r\n"
            f"CSeq: {cseq} SUBSCRIBE\r\n"
            f"Expires: 3600\r\n"
            f"Content-Length: 0\r\n"
            f"\r\n"
        )
        self.transport.sendto(payload.encode(), addr)

    # ----- REGISTER / Keepalive -----

    async def _auto_register_loop(self):
        """周期性重注册"""
        while True:
            await self._send_register()
            await asyncio.sleep(max(self.cfg.expires_secs - 60, 30))

    async def _send_register(self):
        if not self.transport:
            return
        cseq = self.state.next_cseq()
        local = self.transport.get_extra_info("sockname")
        payload = build_register(
            self.cfg, local, self.server_addr, cseq, self.cfg.expires_secs,
        )
        self.transport.sendto(payload, self.server_addr)
        log.info("REGISTER sent (cseq=%d)", cseq)

    async def _send_register_with_digest(self, addr: tuple):
        if not self.transport:
            return
        cseq = self.state.next_cseq()
        local = self.transport.get_extra_info("sockname")
        # 计算 Digest
        uri = f"sip:{self.server_realm}@{self.server_addr[0]}:{self.server_addr[1]}"
        qop = getattr(self, "server_qop", None)
        nc = "00000001" if qop else None
        cnonce = uuid.uuid4().hex[:16] if qop else None
        resp = compute_digest_response(
            self.cfg.username, self.server_realm, self.cfg.password,
            "REGISTER", uri, self.server_nonce,
            qop=qop, nc=nc, cnonce=cnonce,
        )
        auth = (
            f'Digest username="{self.cfg.username}", '
            f'realm="{self.server_realm}", '
            f'nonce="{self.server_nonce}", '
            f'uri="{uri}", '
            f'response="{resp}", '
            f'algorithm=MD5'
        )
        if qop:
            auth += f', qop={qop}, nc={nc}, cnonce="{cnonce}"'
        payload = build_register(
            self.cfg, local, self.server_addr, cseq, self.cfg.expires_secs,
            authorization_header=auth,
        )
        self.transport.sendto(payload, self.server_addr)
        log.info("REGISTER (with Digest) sent (cseq=%d)", cseq)

    async def _keepalive_loop(self):
        """周期性 Keepalive"""
        while self.state.registered:
            await asyncio.sleep(self.auto_keepalive_interval)
            if not self.transport or not self.state.registered:
                continue
            cseq = self.state.next_cseq()
            local = self.transport.get_extra_info("sockname")
            payload = build_keepalive(self.cfg, local, self.server_addr, cseq)
            self.transport.sendto(payload, self.server_addr)
            self.state.keepalive_count += 1
            log.info("Keepalive #%d sent", self.state.keepalive_count)

    # ----- Header / XML 解析辅助 -----

    def _extract_header(self, msg: str, name: str, default: str = "") -> str:
        for line in msg.splitlines():
            if line.lower().startswith(name.lower() + ":"):
                return line.split(":", 1)[1].strip()
        return default

    def _extract_auth_param(self, msg: str, header: str, param: str) -> Optional[str]:
        """从 WWW-Authenticate / Proxy-Authenticate 头里取单个参数。

        历史 bug：调用方写的是
        `self._extract_header(msg, "WWW-Authenticate", "realm")`，
        但 `_extract_header` 的第三个参数是 **default**，没有按键取值的语义，
        于是 realm 与 nonce 都被赋成了**整个头值**
        （'Digest realm="...", nonce="...", algorithm=MD5, qop="auth"'）。
        结果 HA1/HA2 全错，任何 REGISTER 都必然被服务端 403 拒绝
        —— 这个模拟器此前从未成功注册过。
        """
        raw = self._extract_header(msg, header, "")
        if not raw:
            return None
        m = re.search(rf'{re.escape(param)}\s*=\s*"?([^",]+)"?', raw, re.IGNORECASE)
        return m.group(1).strip() if m else None

    def _extract_via_branch(self, msg: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith("via:"):
                if "branch=" in line:
                    start = line.index("branch=") + 7
                    end = line.find(";", start)
                    if end == -1:
                        end = len(line)
                    return line[start:end]
        return make_branch()

    def _extract_from_tag(self, msg: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith("from:"):
                if "tag=" in line:
                    start = line.index("tag=") + 4
                    end = line.find(";", start)
                    if end == -1:
                        end = len(line)
                    return line[start:end].strip()
        return uuid.uuid4().hex[:8]

    def _extract_to_tag(self, msg: str) -> str:
        for line in msg.splitlines():
            if line.lower().startswith("to:"):
                if "tag=" in line:
                    start = line.index("tag=") + 4
                    end = line.find(";", start)
                    if end == -1:
                        end = len(line)
                    return line[start:end].strip()
        return ""

    def _extract_xml_value(self, body: str, tag: str) -> Optional[str]:
        try:
            root = ET.fromstring(body)
            for child in root.iter():
                if child.tag == tag:
                    return child.text
        except ET.ParseError:
            pass
        return None


# ---------------- CLI ----------------

def main():
    parser = argparse.ArgumentParser(
        description="GB28181 SIP Device Mock",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--server", default="127.0.0.1:5060", help="GBServer SIP address (host:port)")
    parser.add_argument("--local-port", type=int, default=DEFAULT_PORT, help="local UDP port")
    parser.add_argument("--device-id", default="34020000001320000001", help="20-digit device ID")
    parser.add_argument("--device-name", default="MockCamera-01")
    parser.add_argument("--manufacturer", default="MockVendor")
    parser.add_argument("--model", default="MOCK-IPC-100")
    parser.add_argument("--firmware", default="1.0.0-mock")
    parser.add_argument("--channels", type=int, default=DEFAULT_CHANNELS)
    parser.add_argument("--username", default="admin")
    parser.add_argument("--password", default="admin123")
    parser.add_argument("--realm", default=None)
    parser.add_argument("--expires", type=int, default=3600)
    parser.add_argument("--auto-register", action="store_true", default=True)
    parser.add_argument("--no-auto-register", dest="auto_register", action="store_false")
    parser.add_argument("--auto-keepalive", type=int, default=30, help="keepalive interval seconds (0=off)")
    parser.add_argument("--log-level", default="INFO", choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    args = parser.parse_args()

    log.setLevel(args.log_level)

    server_host, _, server_port = args.server.partition(":")
    server_addr = (server_host, int(server_port))

    cfg = DeviceConfig(
        device_id=args.device_id,
        device_name=args.device_name,
        manufacturer=args.manufacturer,
        model=args.model,
        firmware=args.firmware,
        channel_count=args.channels,
        username=args.username,
        password=args.password,
        realm=args.realm or realm_from_device_id(args.device_id),
        expires_secs=args.expires,
    )

    mock = SipDeviceMock(cfg, server_addr, args.local_port, args.auto_register, args.auto_keepalive)

    async def run():
        await mock.start()
        stop = asyncio.Event()
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, stop.set)
        await stop.wait()
        log.info("Shutting down...")
        if mock.transport:
            mock.transport.close()

    try:
        asyncio.run(run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()