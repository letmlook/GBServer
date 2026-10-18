#!/usr/bin/env python3
"""Check play stop's SIP BYE result against an isolated GBServer and real ZLM.

The backend must listen on 127.0.0.1:5060/UDP, serve BASE, and have a reachable
ZLM node. The script owns a local SIP Mock, requires ffmpeg, and cleans it up.
"""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

BASE = os.environ.get("GBSERVER_TEST_URL", "http://127.0.0.1:18080").rstrip("/")
DEVICE_ID = os.environ.get("GBSERVER_TEST_DEVICE_ID", "34020000001320000001")
MOCK = Path(__file__).with_name("sip_device_mock.py")
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def api(path, token=None, post=False, timeout=25):
    headers = {"access-token": token} if token else {}
    request = urllib.request.Request(BASE + path, headers=headers, data=b"" if post else None)
    with HTTP.open(request, timeout=timeout) as response:
        return json.load(response)


def main():
    password = os.environ.get("GBSERVER_TEST_PASSWORD", "admin")
    query = urllib.parse.urlencode({
        "username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
        "password": hashlib.md5(password.encode()).hexdigest(),
    })
    token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]

    # Without an active INVITE, ZLM cleanup may still be attempted, but the
    # endpoint must not claim a BYE was sent.
    stop_path = f"/api/play/stop/{DEVICE_ID}/00000000000000000000"
    absent = api(stop_path, token=token)
    if absent.get("code") == 0 or "BYE 未发送" not in absent.get("msg", ""):
        raise AssertionError("stop without session falsely succeeded: " + json.dumps(absent, ensure_ascii=False))
    print("stop without session: BYE not sent")

    with tempfile.TemporaryDirectory() as temp:
        report = Path(temp) / "bye.json"
        with (Path(temp) / "mock.log").open("wb+") as output:
            env = {**os.environ, "SIP_MOCK_BYE_REPORT": str(report)}
            proc = subprocess.Popen([
                sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                "--channels", "1", "--auto-keepalive", "0",
                "--auto-bye-secs", "0", "--send-rtp",
            ], stdout=output, stderr=subprocess.STDOUT, env=env)
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if proc.poll() is not None:
                        raise AssertionError("SIP Mock exited before registration")
                    status = api("/api/device/query/sync_status?deviceId=" + DEVICE_ID, token=token)
                    if status["data"]["online"]:
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError("SIP Mock did not register")

                sync = api(f"/api/device/query/devices/{DEVICE_ID}/sync", token=token)
                if sync.get("code") != 0 or sync["data"]["syncState"] != "done":
                    raise AssertionError("Catalog sync failed: " + json.dumps(sync, ensure_ascii=False))
                listing = api("/api/common/channel/list?count=100", token=token)
                channel = next(row["channelId"] for row in listing["data"]["list"]
                               if row["deviceId"] == DEVICE_ID)

                started = api(f"/api/play/start/{DEVICE_ID}/{channel}", token=token, timeout=35)
                if started.get("code") != 0:
                    raise AssertionError("play start failed: " + json.dumps(started, ensure_ascii=False))
                stopped = api(f"/api/play/stop/{DEVICE_ID}/{channel}", token=token)
                if stopped.get("code") != 0 or not (stopped.get("data") or {}).get("callId"):
                    raise AssertionError("play stop failed: " + json.dumps(stopped, ensure_ascii=False))

                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    if report.exists():
                        try:
                            bye = json.loads(report.read_text())
                        except json.JSONDecodeError:
                            pass  # Mock may still be replacing the report file.
                        else:
                            if bye.get("valid", 0) > 0:
                                break
                    time.sleep(0.1)
                else:
                    raise AssertionError("SIP Mock did not report a valid BYE")
                if bye["invalid"] != 0 or bye["late"] != 0:
                    raise AssertionError("BYE dialog failed: " + json.dumps(bye, ensure_ascii=False))
                print("play stop: BYE dialog valid, callId=" + stopped["data"]["callId"])

                # Snapshot capture retries after 2+3+5 seconds. None may
                # resurrect a stopped stream via ZLM on_stream_not_found.
                time.sleep(11)
                again = api(f"/api/play/stop/{DEVICE_ID}/{channel}", token=token)
                if again.get("code") == 0:
                    raise AssertionError("stopped stream was restarted: " + json.dumps(again, ensure_ascii=False))
                print("stopped stream stayed stopped through snapshot retries")
            except Exception:
                output.flush()
                output.seek(0)
                print(output.read().decode(errors="replace")[-1600:])
                raise
            finally:
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=3)


if __name__ == "__main__":
    main()
