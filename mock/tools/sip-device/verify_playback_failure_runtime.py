#!/usr/bin/env python3
"""Exercise playback rejection, missing media and device BYE with real ZLM."""

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

from verify_playback_control_dialog_runtime import DEVICE_ID, HTTP, MOCK, api
from verify_concurrent_playback_runtime import media_count


def rtp_servers():
    secret = os.environ["GBSERVER_TEST_ZLM_SECRET"]
    base = os.environ.get("GBSERVER_TEST_ZLM_URL", "http://127.0.0.1:8080")
    request = urllib.request.Request(base + "/index/api/listRtpServer?" + urllib.parse.urlencode({"secret": secret}))
    with HTTP.open(request, timeout=5) as response:
        result = json.load(response)
    assert result.get("code") == 0, "ZLM listRtpServer failed"
    return {item["stream_id"] for item in result.get("data", [])}


def run_case(token, status, send_rtp, auto_bye):
    with tempfile.TemporaryDirectory() as temp:
        with (Path(temp) / "mock.log").open("wb+") as output:
            args = [sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                    "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                    "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                    "--channels", "1", "--auto-keepalive", "0", "--auto-bye-secs", str(auto_bye),
                    "--invite-status", str(status)]
            if send_rtp:
                args.append("--send-rtp")
            bye_report = Path(temp) / "bye.json"
            proc = subprocess.Popen(args, stdout=output, stderr=subprocess.STDOUT,
                                    env={**os.environ, "SIP_MOCK_BYE_REPORT": str(bye_report)})
            stream = None
            channel = None
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    output.flush()
                    output.seek(0)
                    log = output.read().decode(errors="replace")
                    if "REGISTER" in log and "200 OK" in log:
                        break
                    if proc.poll() is not None:
                        raise AssertionError("SIP Mock exited before registration")
                    time.sleep(0.2)
                else:
                    raise AssertionError("SIP Mock did not register")
                assert api(f"/api/device/query/devices/{DEVICE_ID}/sync", token)["code"] == 0
                listing = api("/api/common/channel/list?count=100", token)
                channel = next(row["channelId"] for row in listing["data"]["list"] if row["deviceId"] == DEVICE_ID)
                period = urllib.parse.urlencode({"startTime": "2026-10-18T10:00:00", "endTime": "2026-10-18T10:01:00"})
                baseline = rtp_servers()
                result = api(f"/api/playback/start/{DEVICE_ID}/{channel}?{period}", token, timeout=40)
                if status != 200 or not send_rtp:
                    assert result["code"] != 0, result
                    if status != 200:
                        assert str(status) in result["msg"], result
                    else:
                        deadline = time.monotonic() + 3
                        while time.monotonic() < deadline and not bye_report.exists():
                            time.sleep(0.1)
                        assert bye_report.exists() and json.loads(bye_report.read_text())["valid"] >= 1, "timeout did not send valid BYE"
                else:
                    assert result["code"] == 0, result
                    stream = result["data"]["streamId"]
                    assert media_count(stream, token) > 0
                    assert stream in rtp_servers(), "active RTP server missing"
                    deadline = time.monotonic() + 12
                    while time.monotonic() < deadline and media_count(stream, token) > 0:
                        time.sleep(0.2)
                    assert media_count(stream, token) == 0, "device BYE did not close stream"
                    paused = api(f"/api/playback/pause/{stream}", token)
                    assert paused["code"] != 0, paused
                    stopped = api(f"/api/playback/stop/{DEVICE_ID}/{channel}/{stream}", token)
                    assert stopped["code"] == 0, stopped
                    stream = None
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline and rtp_servers() - baseline:
                    time.sleep(0.2)
                assert not (rtp_servers() - baseline), "RTP server leaked"
                print(f"playback status={status} media={send_rtp} device_bye={auto_bye}: cleanup verified")
            except Exception:
                output.flush()
                output.seek(0)
                print(output.read().decode(errors="replace")[-4000:])
                raise
            finally:
                if stream and channel:
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


def main():
    query = urllib.parse.urlencode({"username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
                                    "password": hashlib.md5(os.environ.get("GBSERVER_TEST_PASSWORD", "admin").encode()).hexdigest()})
    token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]
    run_case(token, 486, False, 0)
    run_case(token, 200, False, 0)
    run_case(token, 200, True, 12)


if __name__ == "__main__":
    main()
