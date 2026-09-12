#!/usr/bin/env python3
"""对讲音频上行探针：用浏览器同样的方式把 PCM 推给 `/api/talk/audio/...`。

存在的意义：`/api/talk/start` 返回 200 只证明**信令**通了。真正"对讲可用"
还要求浏览器（或第三方客户端）通过 WebSocket 把麦克风 PCM 送上去，平台再
编码成 G.711A/RTP 发到设备 `m=audio` 端口。本脚本用**标准库**实现
WebSocket 客户端（不依赖 websockets 库），按 20ms/帧发送 8kHz 小端 i16 PCM，
并打印服务端回报的 `{packets,bytes}`，配合 sip-device mock 的
`SIP_MOCK_TALK_REPORT` 即可给出报文级证据。

用法：
    python3 mock/tools/talk-audio-probe.py --device 34020000001320000001 \
        --channel 34020000001320000001 --token <JWT> [--seconds 1]
"""

from __future__ import annotations

import argparse
import base64
import json
import math
import os
import socket
import struct
import sys
import time

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def build_handshake(host: str, port: int, path: str) -> bytes:
    key = base64.b64encode(os.urandom(16)).decode()
    return (
        f"GET {path} HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        "\r\n"
    ).encode()


def read_until_headers(sock: socket.socket) -> bytes:
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = sock.recv(4096)
        if not chunk:
            break
        buf += chunk
    return buf


def send_frame(sock: socket.socket, opcode: int, payload: bytes) -> None:
    header = bytearray([0x80 | opcode])
    mask = os.urandom(4)
    n = len(payload)
    if n < 126:
        header.append(0x80 | n)
    elif n < (1 << 16):
        header.append(0x80 | 126)
        header += struct.pack("!H", n)
    else:
        header.append(0x80 | 127)
        header += struct.pack("!Q", n)
    header += mask
    masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    sock.sendall(bytes(header) + masked)


def recv_frames(sock: socket.socket, timeout: float) -> list:
    """读取若干帧（非阻塞式，超时即返回）。只处理文本/二进制/关闭。"""
    frames = []
    sock.settimeout(timeout)
    try:
        while True:
            head = sock.recv(2)
            if len(head) < 2:
                break
            opcode = head[0] & 0x0F
            masked = bool(head[1] & 0x80)
            length = head[1] & 0x7F
            if length == 126:
                length = struct.unpack("!H", sock.recv(2))[0]
            elif length == 127:
                length = struct.unpack("!Q", sock.recv(8))[0]
            mask = sock.recv(4) if masked else b""
            data = b""
            while len(data) < length:
                chunk = sock.recv(length - len(data))
                if not chunk:
                    break
                data += chunk
            if masked:
                data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
            if opcode == 0x8:
                break
            frames.append((opcode, data))
            if len(frames) >= 8:
                break
    except socket.timeout:
        pass
    return frames


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=18080)
    ap.add_argument("--device", required=True)
    ap.add_argument("--channel", required=True)
    ap.add_argument("--token", required=True)
    ap.add_argument("--seconds", type=float, default=1.0, help="发送时长（秒）")
    ap.add_argument("--tone", type=float, default=440.0, help="测试音频率 Hz")
    args = ap.parse_args()

    path = f"/api/talk/audio/{args.device}/{args.channel}?token={args.token}"
    sock = socket.create_connection((args.host, args.port), timeout=10)
    sock.sendall(build_handshake(args.host, args.port, path))
    resp = read_until_headers(sock)
    first = resp.split(b"\r\n", 1)[0].decode(errors="replace")
    if "101" not in first:
        print(f"WebSocket 握手失败: {first}", file=sys.stderr)
        print(resp.decode(errors="replace")[:400], file=sys.stderr)
        return 1
    print(f"WS 已连接: {first}")

    # 8kHz 单声道，20ms = 160 采样 = 320 字节
    sample_rate = 8000
    frame_samples = 160
    total_frames = int(args.seconds * sample_rate / frame_samples)
    for i in range(total_frames):
        pcm = bytearray()
        for n in range(frame_samples):
            t = (i * frame_samples + n) / sample_rate
            v = int(12000 * math.sin(2 * math.pi * args.tone * t))
            pcm += struct.pack("<h", v)
        send_frame(sock, 0x2, bytes(pcm))
        time.sleep(0.02)

    # 给服务端一点时间发统计回报
    for opcode, data in recv_frames(sock, 1.5):
        if opcode == 0x1:
            print("服务端回报:", data.decode(errors="replace"))
    send_frame(sock, 0x8, b"")
    sock.close()
    print(f"已发送 {total_frames} 帧 PCM（{total_frames * frame_samples / sample_rate:.1f}s）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
