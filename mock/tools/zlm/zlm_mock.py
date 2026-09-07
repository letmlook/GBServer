#!/usr/bin/env python3
"""
ZLMediaKit HTTP API 模拟器

独立可运行的 Python 进程，模拟 ZLMediaKit 2024+ master 的 `/index/api/*` HTTP API 子集。
无第三方依赖（仅 Python 标准库）。

支持的 API：
  - getServerConfig
  - getApiList（返回已实现 API 列表）
  - getMediaList / getMediaInfo / isMediaExist
  - addStreamProxy / delStreamProxy
  - openRtpServer / closeRtpServer / listRtpServer
  - getRtpInfo / pushStream
  - 通用触发器端点（POST /trigger/<hook_name>，用于主动触发 Webhook）：
      /trigger/on_publish /trigger/on_play /trigger/on_stream_changed
      /trigger/on_record_mp4 /trigger/on_record_hls /trigger/on_server_started

启动示例：
  python3 zlm_mock.py \
    --port 8080 \
    --secret demo \
    --hook-url http://127.0.0.1:18080/api/zlm/hook \
    --trigger-on-publish

详细文档见 mock/TEST_PLAN.md §5.3
"""

from __future__ import annotations

import argparse
import http.server
import json
import logging
import os
import random
import socketserver
import sys
import threading
import time
import urllib.parse
from typing import Any, Optional

# ---------------- 常量与日志 ----------------

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
log = logging.getLogger("zlm-mock")

SERVER_VERSION = "ZLMediaKit-Mock-1.0"
BUILD_TIME = __import__("datetime").datetime.now().strftime("%Y-%m-%d %H:%M:%S")

# ---------------- 内存状态 ----------------

_state = {
    "media_list": [],          # list[dict]
    "rtp_servers": {},         # key -> dict
    "stream_proxies": {},      # key -> dict
    "streams": {},             # (app, stream) -> dict
    "media_secret": "demo",
}


def _ok(data: Any = None) -> dict:
    return {"code": 0, "msg": "success", "data": data}


def _err(code: int, msg: str) -> dict:
    return {"code": code, "msg": msg}


# ---------------- HTTP Handler ----------------

class ZlmMockHandler(http.server.BaseHTTPRequestHandler):
    """ZLMediaKit 模拟 HTTP Handler"""

    server_version = "ZLMediaKit-Mock/1.0"

    # 静音默认 access log
    def log_message(self, format: str, *args):
        log.debug("HTTP %s - %s", self.address_string(), format % args)

    def _send_json(self, code: int, body: dict):
        body_bytes = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body_bytes)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body_bytes)

    def _check_secret(self, params: dict) -> bool:
        expected = self.server.secret  # type: ignore
        return params.get("secret", [""])[0] == expected

    def do_GET(self):  # noqa: N802
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        params = urllib.parse.parse_qs(parsed.query)
        # 触发器端点
        if path.startswith("/trigger/"):
            self._handle_trigger(path[len("/trigger/"):])
            return
        # 真实 ZLM API
        if path == "/index/api/getServerConfig":
            self._handle_get_server_config(params)
        elif path == "/index/api/getApiList":
            self._handle_get_api_list()
        elif path == "/index/api/getMediaList":
            self._handle_get_media_list(params)
        elif path == "/index/api/getMediaInfo":
            self._handle_get_media_info(params)
        elif path == "/index/api/isMediaExist":
            self._handle_is_media_exist(params)
        elif path == "/index/api/listRtpServer":
            self._handle_list_rtp_server(params)
        elif path == "/index/api/getRtpInfo":
            self._handle_get_rtp_info(params)
        elif path == "/index/api/getStatistic":
            self._handle_get_statistic()
        elif path == "/healthz":
            self._send_json(200, _ok({"alive": True}))
        else:
            log.warning("未实现 API: %s", path)
            self._send_json(404, _err(-1, f"unknown api: {path}"))

    def do_POST(self):  # noqa: N802
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b""
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {}
        params = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
        if not self._check_secret(params):
            self._send_json(401, _err(-100, "secret invalid"))
            return
        if path == "/index/api/addStreamProxy":
            self._handle_add_stream_proxy(params)
        elif path == "/index/api/delStreamProxy":
            self._handle_del_stream_proxy(params)
        elif path == "/index/api/openRtpServer":
            self._handle_open_rtp_server(params)
        elif path == "/index/api/closeRtpServer":
            self._handle_close_rtp_server(params)
        elif path == "/index/api/pushStream":
            self._handle_push_stream(params, payload)
        else:
            log.warning("未实现 POST: %s", path)
            self._send_json(404, _err(-1, f"unknown api: {path}"))

    # ----- 具体 handler -----

    def _handle_get_server_config(self, params: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        self._send_json(200, _ok({
            "api.secret": _state["media_secret"],
            "protocol.enable_rtsp": 1,
            "protocol.enable_rtmp": 1,
            "protocol.enable_hls": 1,
            "protocol.enable_http": 1,
            "protocol.enable_ws": 1,
            "protocol.enable_rtp": 1,
            "general.mediaServerId": "zlmediakit-mock-1",
        }))

    def _handle_get_api_list(self):
        apis = [
            "getServerConfig", "getApiList", "getMediaList", "getMediaInfo",
            "isMediaExist", "addStreamProxy", "delStreamProxy",
            "openRtpServer", "closeRtpServer", "listRtpServer", "getRtpInfo",
            "pushStream", "getStatistic",
        ]
        self._send_json(200, _ok(apis))

    def _handle_get_media_list(self, params: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        schema = params.get("schema", [None])[0]
        app = params.get("app", [None])[0]
        stream = params.get("stream", [None])[0]
        result = []
        for m in _state["media_list"]:
            if schema and m.get("schema") != schema:
                continue
            if app and m.get("app") != app:
                continue
            if stream and m.get("stream") != stream:
                continue
            result.append(m)
        self._send_json(200, _ok(result))

    def _handle_get_media_info(self, params: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        key = (params.get("schema", [None])[0],
               params.get("vhost", ["__defaultVhost"])[0],
               params.get("app", [None])[0],
               params.get("stream", [None])[0])
        for m in _state["media_list"]:
            if (m.get("schema"), m.get("vhost"), m.get("app"), m.get("stream")) == key:
                self._send_json(200, _ok(m))
                return
        self._send_json(200, _ok(None))

    def _handle_is_media_exist(self, params: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        key = (params.get("schema", [None])[0],
               params.get("vhost", ["__defaultVhost"])[0],
               params.get("app", [None])[0],
               params.get("stream", [None])[0])
        for m in _state["media_list"]:
            if (m.get("schema"), m.get("vhost"), m.get("app"), m.get("stream")) == key:
                self._send_json(200, _ok({"exist": True}))
                return
        self._send_json(200, _ok({"exist": False}))

    def _handle_add_stream_proxy(self, params: dict):
        url = params.get("url", [""])[0]
        app = params.get("app", ["proxy"])[0]
        stream = params.get("stream", ["proxy"])[0]
        vhost = params.get("vhost", ["__defaultVhost"])[0]
        if not url:
            return self._send_json(200, _err(-1, "url required"))
        key = f"{app}/{stream}"
        _state["stream_proxies"][key] = {"key": key, "url": url, "app": app, "stream": stream, "vhost": vhost}
        # 同步加入 media_list，模拟流就绪
        _state["media_list"].append({
            "schema": "rtsp",
            "vhost": vhost,
            "app": app,
            "stream": stream,
            "duration": 0,
            "bytes_speed": 0,
        })
        log.info("addStreamProxy: %s -> %s", url, key)
        self._send_json(200, _ok({"key": key}))

    def _handle_del_stream_proxy(self, params: dict):
        key = params.get("key", [""])[0]
        if key in _state["stream_proxies"]:
            del _state["stream_proxies"][key]
            log.info("delStreamProxy: %s", key)
            self._send_json(200, _ok(None))
        else:
            self._send_json(200, _err(-1, "key not found"))

    def _handle_open_rtp_server(self, params: dict):
        port = int(params.get("port", [0])[0])
        if port == 0:
            port = random.randint(30000, 30100)
        stream_id = params.get("stream_id", [f"rtp-{port}"])[0]
        tcp = params.get("tcp_mode", ["0"])[0] == "1"
        _state["rtp_servers"][stream_id] = {
            "stream_id": stream_id,
            "port": port,
            "tcp": tcp,
        }
        log.info("openRtpServer: port=%d stream_id=%s", port, stream_id)
        # ZLM 真实行为：成功时 port/cookie 在顶层
        body = {
            "code": 0,
            "msg": "success",
            "port": port,
            "cookie": f"cookie-{stream_id}",
        }
        self._send_json(200, body)

    def _handle_close_rtp_server(self, params: dict):
        stream_id = params.get("stream_id", [""])[0]
        if stream_id in _state["rtp_servers"]:
            del _state["rtp_servers"][stream_id]
            self._send_json(200, _ok(None))
        else:
            self._send_json(200, _err(-1, "stream_id not found"))

    def _handle_list_rtp_server(self, params: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        self._send_json(200, _ok(list(_state["rtp_servers"].values())))

    def _handle_get_rtp_info(self, params: dict):
        stream_id = params.get("stream_id", [""])[0]
        info = _state["rtp_servers"].get(stream_id)
        self._send_json(200, _ok(info))

    def _handle_get_statistic(self):
        self._send_json(200, _ok({
            "MediaSource": {"total": len(_state["media_list"])},
            "MultiMediaSourceMaps": {"total": len(_state["media_list"])},
        }))

    def _handle_push_stream(self, params: dict, payload: dict):
        schema = payload.get("schema", "rtsp")
        app = payload.get("app", "live")
        stream = payload.get("stream", "demo")
        vhost = payload.get("vhost", "__defaultVhost")
        _state["media_list"].append({
            "schema": schema, "app": app, "stream": stream, "vhost": vhost,
            "duration": 0, "bytes_speed": 0,
        })
        self._send_json(200, _ok({"key": f"{app}/{stream}"}))

    # ----- 触发器 -----

    def _handle_trigger(self, name: str):
        """主动触发 Webhook，便于测试"""
        if not self.server.hook_url:  # type: ignore
            log.warning("未配置 --hook-url，无法触发 Webhook")
            self._send_json(200, _ok({"triggered": False, "reason": "no hook_url"}))
            return
        # 构造对应 hook 负载
        base = {"hook_name": name, "mediaServerId": "zlmediakit-mock-1"}
        if name in ("on_publish", "on_play"):
            base.update({
                "schema": "rtsp", "app": "rtp", "stream": "34020000001320000001",
                "ip": "127.0.0.1", "port": 554, "vhost": "__defaultVhost",
            })
        elif name in ("on_stream_changed",):
            base.update({
                "schema": "rtsp", "app": "rtp", "stream": "34020000001320000001",
                "vhost": "__defaultVhost", "register": True,
            })
        elif name in ("on_record_mp4", "on_record_hls"):
            base.update({
                "schema": "rtsp", "app": "rtp", "stream": "34020000001320000001",
                "vhost": "__defaultVhost",
                "file_name": "2026-08-23-001.mp4",
                "file_path": "/opt/media/bin/2026/08/23/001.mp4",
                "folder": "/opt/media/bin/2026/08/23/",
                "file_size": 1024 * 1024,
                "file_duration": 60.0,
                "file_create_time": time.strftime("%Y-%m-%d %H:%M:%S"),
            })
        elif name == "on_server_started":
            base.update({
                "port": 554, "hook_port": 0, "rtsp_port": 554, "rtmp_port": 1935,
                "http_port": 8080, "https_port": 8443,
            })
        # 异步 POST（不阻塞 HTTP 响应）
        threading.Thread(target=_post_hook, args=(self.server.hook_url, base), daemon=True).start()  # type: ignore
        log.info("触发 Webhook: %s -> %s", name, self.server.hook_url)  # type: ignore
        self._send_json(200, _ok({"triggered": True, "hook_name": name}))


def _post_hook(url: str, payload: dict):
    """简易 POST 到 Webhook 接收器（用 urllib）"""
    import urllib.request
    data = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=3) as resp:
            log.info("Hook POST %s -> %d", url, resp.getcode())
    except Exception as e:
        log.warning("Hook POST 失败: %s", e)


# ---------------- 启动 / 停止助手 ----------------

class ThreadedHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    """多线程 HTTP Server"""
    daemon_threads = True
    allow_reuse_address = True
    secret: str = "demo"
    hook_url: Optional[str] = None


def main():
    parser = argparse.ArgumentParser(description="ZLMediaKit HTTP API Mock")
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--secret", default="demo")
    parser.add_argument("--hook-url", default="",
                        help="ZLM 主动触发 Webhook 时回调到此 URL")
    parser.add_argument("--log-level", default="INFO",
                        choices=["DEBUG", "INFO", "WARNING", "ERROR"])
    args = parser.parse_args()

    log.setLevel(args.log_level)
    _state["media_secret"] = args.secret

    server = ThreadedHTTPServer((args.host, args.port), ZlmMockHandler)
    server.secret = args.secret
    server.hook_url = args.hook_url or None

    log.info("ZLM mock listening on http://%s:%d (secret=%s)",
             args.host, args.port, args.secret)
    if args.hook_url:
        log.info("主动触发 Webhook 将 POST 到 %s", args.hook_url)
    log.info("触发器端点: GET /trigger/on_publish 等")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log.info("Shutting down...")
        server.shutdown()


if __name__ == "__main__":
    main()