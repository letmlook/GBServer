#!/usr/bin/env python3
"""Exercise two playback sessions on one mock device against GBServer and real ZLM."""

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from verify_playback_control_dialog_runtime import DEVICE_ID, MOCK, api


def media_count(stream, token):
    query = urllib.parse.urlencode({"app": "rtp", "stream": stream})
    response = api("/api/media/stream_info_by_app_and_stream?" + query, token)
    assert response["code"] == 0, response
    return response["data"]["count"]


def main():
    query = urllib.parse.urlencode({
        "username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
        "password": hashlib.md5(os.environ.get("GBSERVER_TEST_PASSWORD", "admin").encode()).hexdigest(),
    })
    token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]
    with tempfile.TemporaryDirectory() as temp:
        with (Path(temp) / "mock.log").open("wb+") as output:
            proc = subprocess.Popen([
                sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
                "--channels", "1", "--auto-keepalive", "0", "--auto-bye-secs", "0", "--send-rtp",
            ], stdout=output, stderr=subprocess.STDOUT)
            streams = []
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    if proc.poll() is not None:
                        raise AssertionError("SIP Mock exited before registration")
                    if api("/api/device/query/sync_status?deviceId=" + DEVICE_ID, token)["data"]["online"]:
                        break
                    time.sleep(0.2)
                else:
                    raise AssertionError("SIP Mock did not register")
                assert api(f"/api/device/query/devices/{DEVICE_ID}/sync", token)["code"] == 0
                listing = api("/api/common/channel/list?count=100", token)
                channel = next(row["channelId"] for row in listing["data"]["list"] if row["deviceId"] == DEVICE_ID)
                period = urllib.parse.urlencode({"startTime": "2026-10-18T10:00:00", "endTime": "2026-10-18T10:01:00"})
                path = f"/api/playback/start/{DEVICE_ID}/{channel}?{period}"
                with ThreadPoolExecutor(max_workers=2) as pool:
                    futures = [pool.submit(api, path, token) for _ in range(2)]
                    results = []
                    errors = []
                    for future in futures:
                        try:
                            result = future.result()
                            results.append(result)
                            if result.get("code") == 0:
                                streams.append(result["data"]["streamId"])
                        except Exception as error:
                            errors.append(error)
                    if errors:
                        raise errors[0]
                assert len(streams) == 2 and len(set(streams)) == 2, results
                assert all(media_count(stream, token) > 0 for stream in streams), streams
                output.flush()
                output.seek(0)
                log = output.read().decode(errors="replace")
                ssrcs = re.findall(r"INVITE 收到: Subject=\S+ s=Playback y=(\d+)", log)
                assert len(ssrcs) == 2 and len(set(ssrcs)) == 2, ssrcs
                stop = api(f"/api/playback/stop/{DEVICE_ID}/{channel}/{streams[0]}", token)
                assert stop["code"] == 0, stop
                streams.pop(0)
                # A control request proves the surviving dialog is still active after the first BYE.
                paused = api(f"/api/playback/pause/{streams[0]}", token)
                assert paused["code"] == 0 and paused["data"]["sipSent"], paused
                time.sleep(1)
                assert media_count(streams[0], token) > 0, "surviving ZLM stream disappeared"
                output.flush()
                output.seek(0)
                log = output.read().decode(errors="replace")
                assert len(re.findall(r"RTP 推流结束", log)) == 1, log[-3000:]
                print("two playback streams arrived; distinct stream IDs and SSRCs")
                print("stopping one stream leaves the other ZLM stream, sender and dialog active")
            except Exception:
                output.flush()
                output.seek(0)
                print(output.read().decode(errors="replace")[-5000:])
                raise
            finally:
                for stream in streams:
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
