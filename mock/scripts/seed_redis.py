#!/usr/bin/env python3
"""
seed_redis.py — 用 Python 标准库（socket + Redis RESP 协议）把种子 JSON 灌入 Redis
无第三方依赖；仅在 redis-cli 不可用时作为备用方案

用法：
  python3 seed_redis.py                       # 默认 127.0.0.1:6379 db=0
  REDIS_URL=redis://:pass@host:6379/1 python3 seed_redis.py
"""

from __future__ import annotations

import json
import os
import socket
import sys
from typing import List, Tuple
from urllib.parse import urlparse


def parse_redis_url(url: str) -> Tuple[str, int, str, int]:
    """解析 redis://[:pass@]host:port/db"""
    u = urlparse(url)
    host = u.hostname or "127.0.0.1"
    port = u.port or 6379
    password = u.password
    db = int(u.path.lstrip("/") or "0")
    return host, port, password, db


class RedisClient:
    """最小 Redis RESP 客户端，支持 PING / SET / SCAN"""

    def __init__(self, host: str, port: int, password: str = "", db: int = 0):
        self.host = host
        self.port = port
        self.password = password
        self.db = db
        self.sock: socket.socket = None  # type: ignore

    def _connect(self):
        self.sock = socket.create_connection((self.host, self.port), timeout=3)
        if self.password:
            self._send_command(["AUTH", self.password])
            self._read_reply()
        if self.db:
            self._send_command(["SELECT", str(self.db)])
            self._read_reply()

    def _send_command(self, args: List[str]):
        # RESP array
        msg = f"*{len(args)}\r\n"
        for a in args:
            b = a.encode("utf-8")
            msg += f"${len(b)}\r\n{a}\r\n"
        self.sock.sendall(msg.encode("utf-8"))

    def _read_reply(self) -> str:
        # 简单解析：只支持 simple string / integer / bulk string
        line = self._readline()
        if not line:
            return ""
        first = line[0:1]
        if first == b"+":
            return line[1:].decode(errors="replace").rstrip("\r\n")
        elif first == b"-":
            err = line[1:].decode(errors="replace").rstrip("\r\n")
            raise RuntimeError(f"Redis error: {err}")
        elif first == b":":
            return line[1:].decode(errors="replace").rstrip("\r\n")
        elif first == b"$":
            length = int(line[1:].strip())
            if length < 0:
                return ""
            data = self.sock.recv(length + 2)  # 含 \r\n
            return data[:length].decode(errors="replace")
        elif first == b"*":
            length = int(line[1:].strip())
            items = []
            for _ in range(length):
                items.append(self._read_reply())
            return items
        return ""

    def _readline(self) -> bytes:
        buf = bytearray()
        while True:
            ch = self.sock.recv(1)
            if not ch:
                return bytes(buf)
            buf += ch
            if buf.endswith(b"\r\n"):
                return bytes(buf)

    def ping(self) -> str:
        self._connect()
        self._send_command(["PING"])
        return self._read_reply()

    def set(self, key: str, value: str):
        self._send_command(["SET", key, value])
        return self._read_reply()

    def scan(self, count: int = 100) -> List[str]:
        """简单 SCAN 实现（游标版本），返回所有 key（限制 count 数量）"""
        self._send_command(["SCAN", "0", "COUNT", str(count)])
        # 返回 [cursor, [keys]]
        result = self._read_reply()
        if isinstance(result, list) and len(result) >= 2:
            return result[1]
        return []

    def close(self):
        if self.sock:
            self.sock.close()


def main():
    redis_url = os.environ.get("REDIS_URL", "redis://127.0.0.1:6379/0")
    json_file = os.environ.get(
        "SEED_FILE",
        os.path.join(os.path.dirname(__file__), "..", "seed", "redis", "seed-test-data.json"),
    )

    if not os.path.exists(json_file):
        print(f"❌ seed 文件不存在: {json_file}")
        sys.exit(1)

    host, port, password, db = parse_redis_url(redis_url)
    print(f"==== Redis {host}:{port} db={db} ====")

    client = RedisClient(host, port, password, db)
    try:
        pong = client.ping()
        print(f"  PING → {pong}")
    except Exception as e:
        print(f"❌ Redis 不可达: {e}")
        sys.exit(1)

    with open(json_file, encoding="utf-8") as f:
        data = json.load(f)

    count = 0
    for group_name, group in data.items():
        if group_name.startswith("_") or not isinstance(group, dict):
            continue
        for key, value in group.items():
            if key.startswith("_"):
                continue
            if isinstance(value, (str, int, float)):
                client.set(key, str(value))
            else:
                client.set(key, json.dumps(value, ensure_ascii=False))
            count += 1
            print(f"  ✓ SET {key}")

    print(f"\n==== 已灌入 {count} 个 key ====")
    keys = client.scan(100)
    print(f"==== 当前 Redis 中部分 key（前 {len(keys)} 个）====")
    for k in keys[:20]:
        print(f"  - {k}")

    client.close()


if __name__ == "__main__":
    main()