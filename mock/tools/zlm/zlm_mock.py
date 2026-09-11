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
    "send_rtp": {},            # (app, stream) -> dict
    "recordings": {},          # (app, stream) -> dict
    "media_secret": "demo",
    # getServerConfig 返回的基础配置；setServerConfig 会覆盖其中的键
    "server_config": {},
    "rtp_port_start": 30000,
    "rtp_port_end": 30100,
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
        params = urllib.parse.parse_qs(parsed.query)
        self._dispatch(parsed.path, params, {})

    def do_POST(self):  # noqa: N802
        parsed = urllib.parse.urlparse(self.path)
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b""
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {}
        params = urllib.parse.parse_qs(parsed.query)
        # 真实 ZLM 的部分 API 同时接受 query 与 JSON body（setServerConfig 走 JSON），
        # 这里把 body 里的标量并入 params，调用方不必关心用哪种方式传参。
        if isinstance(payload, dict):
            for k, v in payload.items():
                params.setdefault(k, [str(v)])
        self._dispatch(parsed.path, params, payload if isinstance(payload, dict) else {})

    def _dispatch(self, path: str, params: dict, payload: dict) -> None:
        """统一分发：GET/POST 共用一张路由表。

        真实 ZLM 的 API 大多接受 GET+query，只有 setServerConfig 是 POST+JSON；
        此前 mock 把 GET/POST 各写一套 if-else，两边不一致（例如 setServerConfig
        只在 POST 里、而 close_streams 根本没实现），调用方一不小心就打到 404。
        """
        if path.startswith("/trigger/"):
            self._handle_trigger(path[len("/trigger/"):])
            return
        if path == "/healthz":
            self._send_json(200, _ok({"alive": True}))
            return
        handler = _ROUTES.get(path)
        if handler is None:
            log.warning("未实现 API: %s", path)
            self._send_json(404, _err(-1, f"unknown api: {path}"))
            return
        handler(self, params, payload)

    def _fire_hook(self, hook_name: str, data: dict) -> None:
        """按**真实 ZLM 的形态**异步投递一个 hook（未配置 `--hook-url` 则跳过）。

        两点必须与真实 ZLM 一致（见
        https://docs.zlmediakit.com/guide/media_server/web_hook_api.html 的
        `[hook]` 配置预览与各事件示例 body）：

        1. **事件类型由 URL 决定**：`on_play` / `on_publish` / …
           各自有独立地址，所以这里 POST 到 `{base}/api/hook/{hook_name}`。
        2. **请求体是扁平 JSON，且不含 `hook_name`**：body 里只有
           `mediaServerId` / `app` / `stream` / `schema` … 这些业务字段。

        此前 mock 把字段嵌在 `data` 里、还带着 `hook_name`，因此既掩盖了
        "后端靠 body 里的 hook_name 分派、真实 ZLM 根本不发它"这个致命缺陷，
        也让数据相关的事件分支全部取不到值。
        """
        url = getattr(self.server, "hook_url", None)
        if not url:
            return
        base = _hook_base_url(url)
        target = f"{base}/api/hook/{hook_name}"
        payload = dict(data)
        payload.setdefault("mediaServerId", "zlmediakit-mock-1")
        threading.Thread(target=_post_hook, args=(target, payload), daemon=True).start()

    # ----- 具体 handler -----

    def _handle_get_server_config(self, params: dict, payload: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        # 两处都必须与真实 ZLM 对齐，否则 GBServer 的 ZlmClient 反序列化会失败
        # （`ApiResponse<Vec<Resp>>`，且 Resp 把所有字段 flatten 进
        # HashMap<String,String>）：
        #   1) `data` 是**单元素数组**，不是对象；
        #   2) 所有配置值都是**字符串**，不是数字/布尔。
        # 此前 mock 返回 `data: {..整数..}`，导致 getServerConfig 必然解析失败
        # → 健康检查永远判 ZLM offline、media_server/check 丢掉全部探测字段。
        cfg = {
            "api.apiDebug": "0",
            "api.secret": _state["media_secret"],
            "general.mediaServerId": "zlmediakit-mock-1",
            "general.enableVhost": "1",
            "hook.enable": "0",
            "hook.hookIp": "127.0.0.1",
            "protocol.enable_rtsp": "1",
            "protocol.enable_rtmp": "1",
            "protocol.enable_hls": "1",
            "protocol.enable_http": "1",
            "protocol.enable_ws": "1",
            "protocol.enable_rtp": "1",
            "protocol.enable_ts": "1",
            "protocol.enable_fmp4": "1",
            "rtp.port_range": "30000-30100",
            "rtp_proxy.port_range": "30000-30100",
            "record.appName": "record",
            "record.filePath": "./www/record/",
        }
        cfg.update(_state["server_config"])
        self._send_json(200, _ok([cfg]))

    def _handle_get_api_list(self, params: dict, payload: dict):
        apis = [
            "getServerConfig", "setServerConfig", "getApiList", "getMediaList",
            "getMediaInfo", "isMediaExist", "addStreamProxy", "delStreamProxy",
            "openRtpServer", "closeRtpServer", "connectRtpServer",
            "listRtpServer", "getRtpInfo", "pushStream", "getStatistic",
            "getServerStats", "getNetWorkApi", "close_streams", "close_stream",
            "kick_session", "kick_sessions",
            "startSendRtp", "stopSendRtp", "sendRtpInfo",
            "startRecord", "stopRecord", "isRecording", "getMp4RecordFile",
            "getSnap", "createDownload", "getDownloadList", "close_download",
            "deleteRecord", "addFfmpegSource", "restartServer", "version",
        ]
        self._send_json(200, _ok(apis))

    def _handle_get_media_list(self, params: dict, payload: dict):
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

    def _handle_get_media_info(self, params: dict, payload: dict):
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

    def _handle_is_media_exist(self, params: dict, payload: dict):
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

    def _handle_add_stream_proxy(self, params: dict, payload: dict):
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

    def _handle_del_stream_proxy(self, params: dict, payload: dict):
        key = params.get("key", [""])[0]
        if key in _state["stream_proxies"]:
            del _state["stream_proxies"][key]
            log.info("delStreamProxy: %s", key)
            self._send_json(200, _ok(None))
        else:
            self._send_json(200, _err(-1, "key not found"))

    def _handle_open_rtp_server(self, params: dict, payload: dict):
        port = int(params.get("port", [0])[0])
        if port == 0:
            # 确定性分配：随机端口会与已分配的撞车，测试因此不稳定
            used = {int(v["port"]) for v in _state["rtp_servers"].values()}
            port = next(
                (p for p in range(_state["rtp_port_start"], _state["rtp_port_end"] + 1)
                 if p not in used),
                _state["rtp_port_start"],
            )
        stream_id = params.get("stream_id", [f"rtp-{port}"])[0]
        tcp = params.get("tcp_mode", ["0"])[0] == "1"
        _state["rtp_servers"][stream_id] = {
            "stream_id": stream_id,
            "port": port,
            "tcp": tcp,
        }
        # 真实 ZLM 在 openRtpServer 成功时**随即创建该流**，因此紧接着的
        # getMediaList / isMediaExist 都能看到它。此前 mock 只登记了 RTP server，
        # 导致 GBServer 里"先起流、再取地址"的第二个请求会误判为"流尚未建立"。
        app = params.get("app", ["rtp"])[0]
        vhost = params.get("vhost", ["__defaultVhost__"])[0]
        _state["media_list"] = [
            m for m in _state["media_list"]
            if not (m.get("app") == app and m.get("stream") == stream_id)
        ]
        _state["media_list"].append({
            "schema": "rtsp",
            "vhost": vhost,
            "app": app,
            "stream": stream_id,
            "duration": 0,
            "bytes_speed": 0,
        })
        log.info("openRtpServer: port=%d stream_id=%s", port, stream_id)
        self._fire_hook("on_rtp_server_started", {
            "stream_id": stream_id,
            "port": port,
        })
        # ZLM 真实行为：成功时 port/cookie 在顶层
        body = {
            "code": 0,
            "msg": "success",
            "port": port,
            "cookie": f"cookie-{stream_id}",
        }
        self._send_json(200, body)

    def _handle_close_rtp_server(self, params: dict, payload: dict):
        stream_id = params.get("stream_id", [""])[0]
        if stream_id in _state["rtp_servers"]:
            del _state["rtp_servers"][stream_id]
            # 与真实 ZLM 一致：关掉 RTP server 时对应流也随之消失
            _state["media_list"] = [
                m for m in _state["media_list"] if m.get("stream") != stream_id
            ]
            self._send_json(200, _ok(None))
        else:
            self._send_json(200, _err(-1, "stream_id not found"))

    def _handle_list_rtp_server(self, params: dict, payload: dict):
        if not self._check_secret(params):
            return self._send_json(401, _err(-100, "secret invalid"))
        self._send_json(200, _ok(list(_state["rtp_servers"].values())))

    def _handle_get_rtp_info(self, params: dict, payload: dict):
        stream_id = params.get("stream_id", [""])[0]
        info = _state["rtp_servers"].get(stream_id)
        self._send_json(200, _ok(info))

    def _handle_get_statistic(self, params: dict, payload: dict):
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
        # 与真实 ZLM 一致：POST 到该事件自己的 URL、body 扁平且不含 hook_name
        base.pop("hook_name", None)
        target = f"{_hook_base_url(self.server.hook_url)}/api/hook/{name}"  # type: ignore
        threading.Thread(target=_post_hook, args=(target, base), daemon=True).start()
        log.info("触发 Webhook: %s -> %s", name, target)
        self._send_json(200, _ok({"triggered": True, "hook_name": name}))


    # ----- 补齐的端点（此前 mock 未实现，调用方只能拿到 404） -----

    def _handle_set_server_config(self, params: dict, payload: dict):
        """setServerConfig：GET+query 或 POST+JSON 都要支持。

        真实 ZLM 用 POST+JSON；GBServer 的 ZlmClient 也走 POST。
        返回形状：`{code, changed}`（changed=1 表示确有变更）。
        """
        key = params.get("key", [""])[0]
        value = str(params.get("value", [""])[0])
        if not key:
            return self._send_json(200, _err(-1, "key required"))
        old = _state["server_config"].get(key)
        _state["server_config"][key] = value
        log.info("setServerConfig: %s=%s", key, value)
        self._send_json(200, {"code": 0, "msg": "success", "changed": 0 if old == value else 1})

    def _handle_version(self, params: dict, payload: dict):
        """版本端点。同时兼容 `data.version`（真实 ZLM）与顶层 `version`
        （GBServer `get_api_version` 读的是顶层）。"""
        body = {
            "code": 0,
            "msg": "success",
            "version": SERVER_VERSION,
            "data": {"version": SERVER_VERSION, "branchName": "mock", "buildTime": BUILD_TIME},
        }
        self._send_json(200, body)

    def _handle_connect_rtp_server(self, params: dict, payload: dict):
        """connectRtpServer：让 ZLM 主动连到设备端口。

        真实 ZLM 在 `stream_id` 不对应已知流时返回
        `{"code":-1,"msg":"can not find the stream"}` —— 这正是
        `handlers/play.rs` 里注释提到的那个坑，mock 必须复现，
        否则测不出调用方的错误处理。
        """
        stream_id = params.get("stream_id", [""])[0]
        if stream_id not in _state["rtp_servers"] and stream_id not in _state["streams"]:
            return self._send_json(200, _err(-1, "can not find the stream"))
        log.info(
            "connectRtpServer: stream_id=%s dst_url=%s dst_port=%s",
            stream_id, params.get("dst_url", [""])[0], params.get("dst_port", [""])[0],
        )
        self._send_json(200, _ok(None))

    def _handle_close_streams(self, params: dict, payload: dict):
        """close_streams：按 app/stream/schema 批量关流。

        返回 `CloseStreamsResponse{count_hit,count_closed}`。
        """
        app = params.get("app", [None])[0]
        stream = params.get("stream", [None])[0]
        schema = params.get("schema", [None])[0]
        before = len(_state["media_list"])
        _state["media_list"] = [
            m for m in _state["media_list"]
            if not (
                (app is None or m.get("app") == app)
                and (stream is None or m.get("stream") == stream)
                and (schema is None or m.get("schema") == schema)
            )
        ]
        hit = before - len(_state["media_list"])
        log.info("close_streams: app=%s stream=%s hit=%d", app, stream, hit)
        self._send_json(200, _ok({"count_hit": hit, "count_closed": hit}))

    def _handle_close_stream(self, params: dict, payload: dict):
        return self._handle_close_streams(params, payload)

    def _handle_kick_sessions(self, params: dict, payload: dict):
        self._send_json(200, _ok({"count_hit": 0, "count_closed": 0}))

    def _handle_kick_session(self, params: dict, payload: dict):
        self._send_json(200, _ok(None))

    def _handle_start_send_rtp(self, params: dict, payload: dict):
        app = params.get("app", [""])[0]
        stream = params.get("stream", [""])[0]
        _state["send_rtp"][(app, stream)] = {
            "app": app,
            "stream": stream,
            "ssrc": params.get("ssrc", [""])[0],
            "dst_url": params.get("dst_url", [""])[0],
            "dst_port": params.get("dst_port", [""])[0],
            "is_udp": params.get("is_udp", ["0"])[0],
            "use_ps": params.get("use_ps", ["0"])[0],
        }
        log.info(
            "startSendRtp: %s/%s ssrc=%s -> %s:%s",
            app, stream, params.get("ssrc", [""])[0],
            params.get("dst_url", [""])[0], params.get("dst_port", [""])[0],
        )
        self._send_json(200, {"code": 0, "msg": "success", "local_port": 40000 + len(_state["send_rtp"])})

    def _handle_stop_send_rtp(self, params: dict, payload: dict):
        app = params.get("app", [""])[0]
        stream = params.get("stream", [""])[0]
        existed = _state["send_rtp"].pop((app, stream), None)
        log.info("stopSendRtp: %s/%s existed=%s", app, stream, bool(existed))
        self._send_json(200, _ok(None))

    def _handle_send_rtp_info(self, params: dict, payload: dict):
        app = params.get("app", [""])[0]
        stream = params.get("stream", [""])[0]
        info = _state["send_rtp"].get((app, stream))
        if info is None:
            return self._send_json(200, _err(-1, "can not find the send rtp"))
        self._send_json(200, _ok(info))

    def _handle_start_record(self, params: dict, payload: dict):
        key = (params.get("app", [""])[0], params.get("stream", [""])[0])
        _state["recordings"][key] = {"type": params.get("type", ["1"])[0]}
        self._send_json(200, _ok({"result": True}))

    def _handle_stop_record(self, params: dict, payload: dict):
        key = (params.get("app", [""])[0], params.get("stream", [""])[0])
        _state["recordings"].pop(key, None)
        self._send_json(200, _ok({"result": True}))

    def _handle_is_recording(self, params: dict, payload: dict):
        key = (params.get("app", [""])[0], params.get("stream", [""])[0])
        status = _state["recordings"].get(key)
        self._send_json(200, _ok({"status": bool(status)}))

    def _handle_get_mp4_record_file(self, params: dict, payload: dict):
        # GBServer 读的是 `data.list`（Mp4RecordResponse）
        self._send_json(200, _ok({"rootPath": "", "paths": [], "list": []}))

    def _handle_get_snap(self, params: dict, payload: dict):
        """getSnap：GBServer 的 ZlmClient 按 JSON 解析（`ApiResponse<SnapResponse>`），
        因此这里返回 `data.path`；真实 ZLM 默认直接回图片字节，若要核对需同时支持
        两种形态（用 `snap=1` 之类的开关区分），此处按调用方期望的形状返回。"""
        self._send_json(200, _ok({"path": "mock://snap/1.jpg"}))

    def _handle_get_server_stats(self, params: dict, payload: dict):
        # 真实 ZLM：data 是**对象**（不是数组）
        self._send_json(200, _ok({
            "MediaSource": len(_state["media_list"]),
            "MultiMediaSourceMuxer": len(_state["media_list"]),
            "TcpSession": 0,
            "UdpSession": 0,
            "TcpSessionCount": 0,
            "UdpSessionCount": 0,
        }))

    def _handle_get_network_api(self, params: dict, payload: dict):
        self._send_json(200, _ok({"interface": [], "dns": []}))

    def _handle_create_download(self, params: dict, payload: dict):
        self._send_json(200, _ok(None))

    def _handle_get_download_list(self, params: dict, payload: dict):
        self._send_json(200, _ok({"data": [], "total": 0}))

    def _handle_close_download(self, params: dict, payload: dict):
        self._send_json(200, _ok(None))

    def _handle_delete_record(self, params: dict, payload: dict):
        self._send_json(200, _ok(None))

    def _handle_add_ffmpeg_source(self, params: dict, payload: dict):
        self._send_json(200, _ok({"key": f"ffmpeg-{int(time.time())}"}))

    def _handle_restart_server(self, params: dict, payload: dict):
        self._send_json(200, _ok(None))

def _hook_base_url(configured: str) -> str:
    """把配置里的单个 hook 地址归一化成服务器根地址。"""
    trimmed = configured.strip().rstrip("/")
    for suffix in ("/api/zlm/hook", "/api/hook"):
        pos = trimmed.rfind(suffix)
        if pos != -1:
            return trimmed[:pos].rstrip("/")
    return trimmed


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


# 路由表：GET/POST 共用
_ROUTES = {
    "/index/api/getServerConfig": ZlmMockHandler._handle_get_server_config,
    "/index/api/setServerConfig": ZlmMockHandler._handle_set_server_config,
    "/index/api/getApiList": ZlmMockHandler._handle_get_api_list,
    "/index/api/getMediaList": ZlmMockHandler._handle_get_media_list,
    "/index/api/getMediaInfo": ZlmMockHandler._handle_get_media_info,
    "/index/api/isMediaExist": ZlmMockHandler._handle_is_media_exist,
    "/index/api/addStreamProxy": ZlmMockHandler._handle_add_stream_proxy,
    "/index/api/delStreamProxy": ZlmMockHandler._handle_del_stream_proxy,
    "/index/api/openRtpServer": ZlmMockHandler._handle_open_rtp_server,
    "/index/api/closeRtpServer": ZlmMockHandler._handle_close_rtp_server,
    "/index/api/connectRtpServer": ZlmMockHandler._handle_connect_rtp_server,
    "/index/api/listRtpServer": ZlmMockHandler._handle_list_rtp_server,
    "/index/api/getRtpInfo": ZlmMockHandler._handle_get_rtp_info,
    "/index/api/pushStream": ZlmMockHandler._handle_push_stream,
    "/index/api/getStatistic": ZlmMockHandler._handle_get_statistic,
    "/index/api/getServerStats": ZlmMockHandler._handle_get_server_stats,
    "/index/api/getNetWorkApi": ZlmMockHandler._handle_get_network_api,
    "/index/api/close_streams": ZlmMockHandler._handle_close_streams,
    "/index/api/close_stream": ZlmMockHandler._handle_close_stream,
    "/index/api/kick_session": ZlmMockHandler._handle_kick_session,
    "/index/api/kick_sessions": ZlmMockHandler._handle_kick_sessions,
    "/index/api/startSendRtp": ZlmMockHandler._handle_start_send_rtp,
    "/index/api/stopSendRtp": ZlmMockHandler._handle_stop_send_rtp,
    "/index/api/sendRtpInfo": ZlmMockHandler._handle_send_rtp_info,
    "/index/api/startRecord": ZlmMockHandler._handle_start_record,
    "/index/api/stopRecord": ZlmMockHandler._handle_stop_record,
    "/index/api/isRecording": ZlmMockHandler._handle_is_recording,
    "/index/api/getMp4RecordFile": ZlmMockHandler._handle_get_mp4_record_file,
    "/index/api/getSnap": ZlmMockHandler._handle_get_snap,
    "/index/api/createDownload": ZlmMockHandler._handle_create_download,
    "/index/api/getDownloadList": ZlmMockHandler._handle_get_download_list,
    "/index/api/close_download": ZlmMockHandler._handle_close_download,
    "/index/api/deleteRecord": ZlmMockHandler._handle_delete_record,
    "/index/api/addFfmpegSource": ZlmMockHandler._handle_add_ffmpeg_source,
    "/index/api/restartServer": ZlmMockHandler._handle_restart_server,
    # ZlmClient::get_api_version 打的是 `/api/version`（不带 /index），
    # 真实 ZLM 是 `/index/api/version`；两个都提供，便于同时验证两种路径。
    "/index/api/version": ZlmMockHandler._handle_version,
    "/api/version": ZlmMockHandler._handle_version,
}


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