#!/usr/bin/env python3
"""
Webhook 接收器（mock）

独立可运行的 Python 进程，接收 GBServer 主动回调的 Webhook 请求，
将其记录到本地日志文件，便于断言与回溯。

监听端点：
  POST /hook/<name>      通用：所有 webhook 都接受
  POST /hook/on_publish / on_play / on_record_mp4 / on_stream_changed
  POST /hook/jt1078/retransmit
  POST /hook/cloud_record/status

启动示例：
  python3 webhook_receiver.py --port 9090 --log mock/logs/webhook.log

详细文档见 mock/TEST_PLAN.md §5.5
"""

from __future__ import annotations

import argparse
import http.server
import json
import logging
import os
import socketserver
import sys
import threading
import time
from datetime import datetime
from typing import Any

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
log = logging.getLogger("webhook-receiver")

_received: list = []
_lock = threading.Lock()
_log_path: str = ""


class WebhookHandler(http.server.BaseHTTPRequestHandler):
    """记录所有 POST 请求的 Handler"""

    server_version = "WebhookReceiver/1.0"

    def log_message(self, fmt: str, *args):
        log.debug(fmt, *args)

    def do_POST(self):  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b""
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"_raw": raw.decode(errors="replace")}
        record = {
            "ts": datetime.utcnow().isoformat() + "Z",
            "path": self.path,
            "headers": dict(self.headers),
            "payload": payload,
        }
        with _lock:
            _received.append(record)
            # 持久化到日志
            if _log_path:
                try:
                    with open(_log_path, "a", encoding="utf-8") as f:
                        f.write(json.dumps(record, ensure_ascii=False) + "\n")
                except OSError as e:
                    log.warning("写日志失败: %s", e)
        log.info("RX %s body=%d", self.path, len(raw))
        body = json.dumps({"code": 0, "msg": "received", "data": None}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):  # noqa: N802
        # 方便查询已接收列表
        if self.path == "/received":
            with _lock:
                payload = list(_received)
            body = json.dumps(payload, ensure_ascii=False, indent=2).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/received/clear":
            with _lock:
                _received.clear()
            self.send_response(200)
            self.end_headers()
            return
        if self.path == "/healthz":
            self.send_response(200)
            self.end_headers()
            return
        self.send_response(404)
        self.end_headers()


class ThreadingHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def main():
    parser = argparse.ArgumentParser(description="Webhook Receiver Mock")
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=9090)
    parser.add_argument("--log", default="mock/logs/webhook.log",
                        help="持久化日志文件路径")
    parser.add_argument("--log-level", default="INFO",
                        choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    args = parser.parse_args()

    log.setLevel(args.log_level)
    global _log_path
    _log_path = args.log

    os.makedirs(os.path.dirname(_log_path) or ".", exist_ok=True)

    server = ThreadingHTTPServer((args.host, args.port), WebhookHandler)
    log.info("Webhook receiver listening on http://%s:%d", args.host, args.port)
    log.info("日志: %s", _log_path)
    log.info("GET /received 查看已接收列表；GET /received/clear 清空")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log.info("Shutting down...")
        server.shutdown()


if __name__ == "__main__":
    main()