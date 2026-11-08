#!/usr/bin/env python3
"""Verify cancellation after real playback media arrives but before HTTP response.

Requires an isolated backend configured with ZLM HTTP port 18081, ZLM itself
on 8080, SIP on 5060 and no other request using this test proxy. Start this
script before the backend; it waits for the backend to become available.
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from verify_playback_control_dialog_runtime import BASE, DEVICE_ID, HTTP, MOCK, api

paused = threading.Event()
release = threading.Event()


class Proxy(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        path = urllib.parse.urlsplit(self.path)
        query = urllib.parse.parse_qs(path.query)
        if (path.path == "/index/api/getMediaList" and query.get("app") == ["rtp"]
                and query.get("stream", [""])[0].startswith("playback_")):
            paused.set()
            release.wait(20)
        try:
            with HTTP.open("http://127.0.0.1:8080" + self.path, timeout=20) as upstream:
                body = upstream.read()
                self.send_response(upstream.status)
                self.send_header("Content-Type", upstream.headers.get("Content-Type", "application/json"))
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass


def main():
    proxy = ThreadingHTTPServer(("127.0.0.1", 18081), Proxy)
    thread = threading.Thread(target=proxy.serve_forever, daemon=True)
    thread.start()
    print("ZLM proxy ready on 18081", flush=True)
    proc = None
    try:
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                query = urllib.parse.urlencode({"username": "admin", "password": hashlib.md5(b"admin").hexdigest()})
                token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]
                break
            except Exception:
                time.sleep(0.2)
        else:
            raise AssertionError("isolated backend did not start")
        with tempfile.TemporaryDirectory() as temp:
            report = Path(temp) / "bye.json"
            with (Path(temp) / "mock.log").open("wb+") as log:
                proc = subprocess.Popen([sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                    "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                    "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                    "--channels", "1", "--auto-keepalive", "0", "--auto-bye-secs", "0", "--send-rtp"],
                    stdout=log, stderr=subprocess.STDOUT, env={**os.environ, "SIP_MOCK_BYE_REPORT": str(report)})
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if api("/api/device/query/sync_status?deviceId=" + DEVICE_ID, token)["data"]["online"]:
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError("mock did not register")
                assert api(f"/api/device/query/devices/{DEVICE_ID}/sync", token)["code"] == 0
                listing = api("/api/common/channel/list?count=100", token)
                channel = next(row["channelId"] for row in listing["data"]["list"] if row["deviceId"] == DEVICE_ID)
                secret = os.environ["GBSERVER_TEST_ZLM_SECRET"]
                query = urllib.parse.urlencode({"secret": secret})
                def servers():
                    with HTTP.open("http://127.0.0.1:8080/index/api/listRtpServer?" + query, timeout=5) as response:
                        result = json.load(response)
                    return {item["stream_id"] for item in result.get("data", [])}
                baseline = servers()
                period = urllib.parse.urlencode({"startTime": "2026-10-18T10:00:00", "endTime": "2026-10-18T10:01:00"})
                url = urllib.parse.urlsplit(BASE)
                with socket.create_connection((url.hostname, url.port or 80), timeout=5) as client:
                    request = (f"GET /api/playback/start/{DEVICE_ID}/{channel}?{period} HTTP/1.1\r\n"
                               f"Host: {url.netloc}\r\naccess-token: {token}\r\nConnection: close\r\n\r\n")
                    client.sendall(request.encode())
                    assert paused.wait(20), "media-ready response did not reach delayed ZLM query"
                    allocated = servers() - baseline
                    assert len(allocated) == 1, f"expected one RTP server, got {allocated}"
                    stream = allocated.pop()
                    media_query = urllib.parse.urlencode({"secret": secret, "app": "rtp", "stream": stream})
                    with HTTP.open("http://127.0.0.1:8080/index/api/getMediaList?" + media_query, timeout=5) as response:
                        media = json.load(response)
                    assert media.get("code") == 0 and media.get("data"), "ZLM media not ready before disconnect"
                deadline = time.monotonic() + 8
                while time.monotonic() < deadline:
                    if report.exists() and json.loads(report.read_text()).get("valid", 0) and not (servers() - baseline):
                        break
                    time.sleep(0.1)
                else:
                    raise AssertionError("ready playback disconnect did not send BYE and close RTP")
                stopped = api(f"/api/playback/stop/{DEVICE_ID}/{channel}/{stream}", token)
                assert stopped["code"] != 0 and "无活动回放会话" in stopped["msg"], stopped
                print("media-ready HTTP disconnect: valid BYE, RTP cleanup and session removal verified")
    finally:
        release.set()
        if proc:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=3)
        proxy.shutdown()
        proxy.server_close()


if __name__ == "__main__":
    main()
