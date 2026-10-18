#!/usr/bin/env python3
"""Verify GB28181 download INVITE, RTP arrival, MP4 recording and stop.

Requires a running backend at GBSERVER_TEST_URL and reachable ZLM. The SIP
Mock owns port 15060 by default; pass SIP_MOCK_LOCAL_PORT to change it.
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


def api(path, token=None, post=False, timeout=35):
    headers = {"access-token": token} if token else {}
    request = urllib.request.Request(BASE + path, headers=headers, data=b"" if post else None)
    with HTTP.open(request, timeout=timeout) as response:
        return json.load(response)


def main():
    query = urllib.parse.urlencode({
        "username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
        "password": hashlib.md5(os.environ.get("GBSERVER_TEST_PASSWORD", "admin").encode()).hexdigest(),
    })
    token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]
    status = int(os.environ.get("SIP_MOCK_INVITE_STATUS", "200"))
    send_rtp = os.environ.get("SIP_MOCK_SEND_RTP", "1") != "0"
    if status not in (200, 486, 503):
        raise ValueError("SIP_MOCK_INVITE_STATUS must be 200, 486 or 503")

    with tempfile.TemporaryDirectory() as temp:
        report = Path(temp) / "bye.json"
        log = Path(temp) / "mock.log"
        with log.open("wb+") as output:
            args = [
                sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                "--channels", "1", "--auto-keepalive", "5", "--auto-bye-secs", "0",
                "--local-port", os.environ.get("SIP_MOCK_LOCAL_PORT", "15060"),
                "--invite-status", str(status),
            ]
            if status == 200 and send_rtp:
                args.append("--send-rtp")
            proc = subprocess.Popen(
                args, stdout=output, stderr=subprocess.STDOUT,
                env={**os.environ, "SIP_MOCK_BYE_REPORT": str(report)},
            )
            try:
                deadline = time.monotonic() + 12
                while time.monotonic() < deadline:
                    if proc.poll() is not None:
                        raise AssertionError("SIP Mock exited before registration")
                    online = api("/api/device/query/sync_status?deviceId=" + DEVICE_ID, token=token)
                    if online.get("data", {}).get("online"):
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError("SIP Mock did not register")

                sync = api(f"/api/device/query/devices/{DEVICE_ID}/sync", token=token)
                if sync.get("code") != 0 or sync.get("data", {}).get("syncState") != "done":
                    raise AssertionError("Catalog sync failed: " + json.dumps(sync, ensure_ascii=False))
                listing = api("/api/common/channel/list?count=100", token=token)
                channel = next(row["channelId"] for row in listing["data"]["list"]
                               if row["deviceId"] == DEVICE_ID)
                period = urllib.parse.urlencode({"startTime": "2026-10-18T10:00:00", "endTime": "2026-10-18T10:01:00"})
                started = api(
                    f"/api/gb_record/download/start/{DEVICE_ID}/{channel}?{period}",
                    token=token, timeout=40,
                )
                if status != 200:
                    if started.get("code") == 0 or str(status) not in started.get("msg", ""):
                        raise AssertionError("Rejected INVITE was accepted: " + json.dumps(started, ensure_ascii=False))
                    print("download rejection reported:", started["msg"])
                    return
                if not send_rtp:
                    if started.get("code") == 0 or "Download media timeout" not in started.get("msg", ""):
                        raise AssertionError("Missing RTP was accepted: " + json.dumps(started, ensure_ascii=False))
                    deadline = time.monotonic() + 3
                    while time.monotonic() < deadline and not report.exists():
                        time.sleep(0.1)
                    if not report.exists() or json.loads(report.read_text()).get("valid", 0) < 1:
                        raise AssertionError("No valid BYE after media timeout")
                    print("download media timeout reported and BYE sent")
                    return
                if started.get("code") != 0 or started.get("data", {}).get("status") != "downloading":
                    raise AssertionError("Download did not start after media ready: " + json.dumps(started, ensure_ascii=False))
                stream_id = started["data"]["streamId"]
                time.sleep(3)
                wrong_channel = api(
                    f"/api/gb_record/download/stop/{DEVICE_ID}/00000000000000000000/{stream_id}",
                    token=token,
                )
                if wrong_channel.get("code") == 0:
                    raise AssertionError("Wrong channel stopped the download")
                stopped = api(f"/api/gb_record/download/stop/{DEVICE_ID}/{channel}/{stream_id}", token=token)
                if stopped.get("code") != 0:
                    raise AssertionError("Download stop failed: " + json.dumps(stopped, ensure_ascii=False))
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    progress = api(f"/api/gb_record/download/progress/{DEVICE_ID}/{channel}/{stream_id}", token=token)
                    data = progress.get("data") or {}
                    if data.get("status") == "completed" and data.get("currentBytes", 0) > 0:
                        break
                    time.sleep(0.3)
                else:
                    raise AssertionError("MP4 completion not observed: " + json.dumps(progress, ensure_ascii=False))
                file_request = urllib.request.Request(
                    BASE + f"/api/gb_record/download/file/{stream_id}",
                    headers={"access-token": token, "Range": "bytes=0-31"},
                )
                with HTTP.open(file_request, timeout=15) as response:
                    prefix = response.read(32)
                    if response.status not in (200, 206) or len(prefix) != 32 or b"ftyp" not in prefix:
                        raise AssertionError("Download URL did not serve an MP4 header")
                if not report.exists() or json.loads(report.read_text()).get("valid", 0) < 1:
                    raise AssertionError("No valid in-dialog BYE: " + report.read_text() if report.exists() else "No BYE report")
                print("download completed:", stream_id, data.get("currentBytes"), "bytes")
            except Exception:
                output.flush()
                print(log.read_text(errors="replace")[-6000:], file=sys.stderr)
                raise
            finally:
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()


if __name__ == "__main__":
    main()
