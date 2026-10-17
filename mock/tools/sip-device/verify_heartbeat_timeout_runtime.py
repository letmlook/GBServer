#!/usr/bin/env python3
"""Verify a running isolated backend expires a registered SIP device.

Start GBServer with SIP keepalive_timeout=3, heartbeat timeout_multiplier=1,
and heartbeat check_interval_secs=1. This script owns a local SIP Mock and
requires the test backend to listen on 127.0.0.1:5060.
"""

import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

BASE = os.environ.get("GBSERVER_TEST_URL", "http://127.0.0.1:18080").rstrip("/")
DEVICE_ID = os.environ.get("GBSERVER_TEST_DEVICE_ID", "34020000001320000001")
MOCK = Path(__file__).with_name("sip_device_mock.py")
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def api(path, token=None, post=False):
    headers = {"access-token": token} if token else {}
    request = urllib.request.Request(BASE + path, headers=headers, data=b"" if post else None)
    with HTTP.open(request, timeout=5) as response:
        return json.load(response)


def online(token):
    response = api("/api/device/query/sync_status?" + urllib.parse.urlencode({
        "deviceId": DEVICE_ID,
    }), token=token)
    if response.get("code") != 0:
        raise AssertionError("status query failed: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["online"]


def main():
    password = os.environ.get("GBSERVER_TEST_PASSWORD", "admin")
    query = urllib.parse.urlencode({
        "username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
        "password": hashlib.md5(password.encode()).hexdigest(),
    })
    token = api("/api/user/login?" + query, post=True)["data"]["accessToken"]
    mock_args = [
        sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
        "--device-id", DEVICE_ID, "--username", DEVICE_ID,
        "--password", os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123"),
        "--channels", "1", "--auto-keepalive", "0",
    ]
    proc = subprocess.Popen(mock_args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise AssertionError("SIP Mock exited before registration")
            if online(token):
                break
            time.sleep(0.2)
        else:
            raise AssertionError("device did not become online after REGISTER")
        print("online after REGISTER")

        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if not online(token):
                break
            time.sleep(0.5)
        else:
            raise AssertionError("device did not become offline after keepalive timeout")
        print("offline after keepalive timeout")

        query = urllib.parse.urlencode({
            "deviceId": DEVICE_ID, "channelId": DEVICE_ID, "command": "UP",
        })
        response = api("/api/device/control/ptz?" + query, token=token)
        if response.get("code") == 0 or "not online" not in response.get("msg", "").lower():
            raise AssertionError("offline device accepted PTZ: " + json.dumps(response, ensure_ascii=False))
        print("offline PTZ rejected")

        proc.terminate()
        proc.wait(timeout=3)
        proc = subprocess.Popen(mock_args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise AssertionError("SIP Mock exited before re-registration")
            if online(token):
                break
            time.sleep(0.2)
        else:
            raise AssertionError("device did not return online after re-registration")
        print("online after re-registration")
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=3)


if __name__ == "__main__":
    main()
