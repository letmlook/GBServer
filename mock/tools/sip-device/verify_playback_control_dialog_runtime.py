#!/usr/bin/env python3
"""Verify playback INFO uses the active INVITE dialog with real ZLM and SIP Mock."""

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
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

    with tempfile.TemporaryDirectory() as temp:
        with (Path(temp) / "mock.log").open("wb+") as output:
            bye_report = Path(temp) / "bye.json"
            proc = subprocess.Popen([
                sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                "--channels", "1", "--auto-keepalive", "0",
                "--auto-bye-secs", "0", "--send-rtp",
            ], stdout=output, stderr=subprocess.STDOUT,
                env={**os.environ, "SIP_MOCK_BYE_REPORT": str(bye_report)})
            stream = None
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if proc.poll() is not None:
                        raise AssertionError("SIP Mock exited before registration")
                    status = api("/api/device/query/sync_status?deviceId=" + DEVICE_ID, token)
                    if status["data"]["online"]:
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError("SIP Mock did not register")

                sync = api(f"/api/device/query/devices/{DEVICE_ID}/sync", token)
                assert sync["code"] == 0 and sync["data"]["syncState"] == "done", sync
                listing = api("/api/common/channel/list?count=100", token)
                channel = next(row["channelId"] for row in listing["data"]["list"]
                               if row["deviceId"] == DEVICE_ID)
                period = urllib.parse.urlencode({"startTime": "2026-10-18T10:00:00",
                                                 "endTime": "2026-10-18T10:01:00"})
                started = api(f"/api/playback/start/{DEVICE_ID}/{channel}?{period}", token)
                assert started["code"] == 0, started
                stream = started["data"]["streamId"]
                paused = api(f"/api/playback/pause/{stream}", token)
                assert paused["code"] == 0 and paused["data"]["sipSent"], paused
                with ThreadPoolExecutor(max_workers=4) as executor:
                    results = list(executor.map(
                        lambda speed: api(f"/api/playback/speed/{stream}/{speed}", token),
                        (1, 2, 3, 4),
                    ))
                assert all(result["code"] == 0 for result in results), results

                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    output.flush()
                    output.seek(0)
                    log = output.read().decode(errors="replace")
                    matches = re.findall(r"Playback INFO dialog valid=True call_id=(\S+) cseq=(\d+)", log)
                    if len(matches) >= 5:
                        break
                    time.sleep(0.1)
                else:
                    raise AssertionError("Mock did not receive five valid in-dialog INFO messages")
                assert "Playback INFO dialog valid=False" not in log, log[-1800:]
                assert len({call_id for call_id, _ in matches}) == 1, matches
                cseqs = [int(cseq) for _, cseq in matches]
                assert cseqs == sorted(set(cseqs)), cseqs
                stopped = api(f"/api/playback/stop/{DEVICE_ID}/{channel}/{stream}", token)
                assert stopped["code"] == 0, stopped
                stream = None
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline and not bye_report.exists():
                    time.sleep(0.1)
                assert bye_report.exists(), "Mock did not receive playback BYE"
                bye = json.loads(bye_report.read_text())
                assert bye["valid"] == 1 and bye["invalid"] == 0, bye
                print(f"playback controls: five valid INFO messages with increasing CSeq {cseqs}")
                print("playback stop: BYE CSeq greater than the final INFO")
            except Exception:
                output.flush()
                output.seek(0)
                print(output.read().decode(errors="replace")[-2000:])
                raise
            finally:
                if stream:
                    try:
                        api(f"/api/playback/stop/{DEVICE_ID}/{channel}/{stream}", token)
                    except Exception:
                        pass
                proc.terminate()
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait(timeout=3)


if __name__ == "__main__":
    main()
