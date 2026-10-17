#!/usr/bin/env python3
"""Against a running GBServer, verify SIP 200 vs DeviceControl execution results.

This starts and stops a local SIP Mock on UDP 15060. Run only against an isolated
test backend whose SIP listener is 127.0.0.1:5060 and whose test device may register.
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
PASSWORD = os.environ.get("GBSERVER_TEST_PASSWORD", "admin")
SIP_PASSWORD = os.environ.get("GBSERVER_TEST_SIP_PASSWORD", "admin123")
MOCK = Path(__file__).with_name("sip_device_mock.py")
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def api(path, token=None, post=False, payload=None):
    headers = {"access-token": token} if token else {}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    data = json.dumps(payload).encode() if payload is not None else (b"" if post else None)
    request = urllib.request.Request(BASE + path, headers=headers, data=data)
    with HTTP.open(request, timeout=12) as response:
        return json.load(response)


def login():
    query = urllib.parse.urlencode({
        "username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
        "password": hashlib.md5(PASSWORD.encode()).hexdigest(),
    })
    response = api("/api/user/login?" + query, post=True)
    if response.get("code") != 0:
        raise RuntimeError("test backend login failed")
    return response["data"]["accessToken"]


def wait_registered(proc, output, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            break
        output.flush()
        output.seek(0)
        if "设备已注册" in output.read().decode(errors="replace"):
            return
        time.sleep(0.1)
    output.seek(0)
    raise RuntimeError("SIP Mock did not register: " + output.read().decode(errors="replace")[-1500:])


def command(token):
    path = f"/api/device/query/transport/{DEVICE_ID}/UDP?waitResult=true"
    response = api(path, token=token, post=True)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def guard_command(token):
    query = urllib.parse.urlencode({
        "deviceId": DEVICE_ID, "guardCmd": "SetGuard", "waitResult": "true",
    })
    response = api("/api/device/control/guard?" + query, token=token)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("Guard DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def reboot_command(token, wait_result=True):
    query = urllib.parse.urlencode({"deviceId": DEVICE_ID, "waitResult": str(wait_result).lower()})
    response = api("/api/device/control/reboot?" + query, token=token)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("Reboot DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def preset_command(token, wait_result=True):
    query = urllib.parse.urlencode({
        "deviceId": DEVICE_ID, "channelId": DEVICE_ID, "command": "GOTO_PRESET",
        "presetIndex": "3", "waitResult": str(wait_result).lower(),
    })
    response = api("/api/device/control/preset?" + query, token=token)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("Preset DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def sy_preset_command(token):
    query = urllib.parse.urlencode({
        "deviceId": DEVICE_ID, "channelId": DEVICE_ID,
        "command": "GOTO_PRESET", "preset": "3",
    })
    response = api("/api/sy/camera/control/ptz?" + query, token=token)
    data = response.get("data") or {}
    if response.get("code") != 0 or not data.get("sipSent") or data.get("presetIndex") != 3:
        raise AssertionError("SY preset command was not sent with index 3: " + json.dumps(response, ensure_ascii=False))
    assert data["deviceResult"]["status"] == "sent", data


def common_channel_commands(token, output):
    sync = api(f"/api/device/query/devices/{DEVICE_ID}/sync", token=token)
    if sync.get("code") != 0 or (sync.get("data") or {}).get("syncState") != "done":
        raise AssertionError("Mock catalog sync failed: " + json.dumps(sync, ensure_ascii=False))
    response = api("/api/common/channel/list?count=100", token=token)
    channels = (response.get("data") or {}).get("list") or []
    channel = next((item for item in channels if item.get("deviceId") == DEVICE_ID), None)
    if response.get("code") != 0 or channel is None:
        raise AssertionError("Mock channel was not listed: " + json.dumps(response, ensure_ascii=False))
    channel_id = channel["id"]

    cases = [
        ("/api/common/channel/front-end/fi/focus", {"command": "on", "speed": 40},
         "PTZCmd=A50F01422800001F"),
        ("/api/common/channel/front-end/preset/call", {"preset_id": 3},
         "PTZCmd=A50F01820003003A"),
    ]
    for path, params, expected in cases:
        query = urllib.parse.urlencode({"channel_id": channel_id, **params})
        result = api(path + "?" + query, token=token)
        if result.get("code") != 0 or "已发送" not in result.get("msg", ""):
            raise AssertionError("Common channel command failed: " + json.dumps(result, ensure_ascii=False))
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            output.flush()
            output.seek(0)
            if expected in output.read().decode(errors="replace"):
                break
            time.sleep(0.05)
        else:
            raise AssertionError("Common channel command missing " + expected)

    invalid = api("/api/common/channel/front-end/preset/call?" + urllib.parse.urlencode({
        "channel_id": channel_id, "preset_id": 256,
    }), token=token)
    assert invalid.get("code") != 0 and "1-255" in invalid.get("msg", ""), invalid


def record_command(token, action="Record"):
    query = urllib.parse.urlencode({
        "deviceId": DEVICE_ID, "channelId": DEVICE_ID, "recordCmdStr": action,
    })
    response = api("/api/device/control/record?" + query, token=token)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("Record DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]


def config_update_command(token, wait_result=True):
    response = api("/api/device/config/update", token=token, post=True, payload={
        "deviceId": DEVICE_ID, "configType": "BasicParam",
        "configData": {"sipServerId": DEVICE_ID, "sipServerDomain": "test&domain"},
        "waitResult": wait_result,
    })
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("DeviceConfig was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def scenario(token, name, mode, expected, delay_ms=0, repeats=1, second=False):
    with tempfile.TemporaryFile() as output:
        args = [sys.executable, str(MOCK), "--server", "127.0.0.1:5060",
                "--device-id", DEVICE_ID, "--username", DEVICE_ID,
                "--password", SIP_PASSWORD, "--channels", "1",
                "--auto-keepalive", "0", "--control-result", mode,
                "--control-result-delay-ms", str(delay_ms),
                "--control-result-repeats", str(repeats)]
        proc = subprocess.Popen(args, stdout=output, stderr=subprocess.STDOUT)
        try:
            wait_registered(proc, output)
            first = command(token)
            assert first["status"] == expected, f"{name}: {first}"
            if expected == "confirmed":
                assert first["resultCode"] == "OK", f"{name}: {first}"
            if expected == "rejected":
                assert first["resultCode"] == "ERROR", f"{name}: {first}"
            if name in ("ok", "reject", "silent"):
                immediate = config_update_command(token, wait_result=False)
                assert immediate["status"] == "sent", f"{name} immediate config: {immediate}"
                config = config_update_command(token)
                assert config["status"] == expected, f"{name} config: {config}"
                assert config["sn"] != first["sn"], "config reused transport request SN"
                print(f"{name} config: {config['status']} sn={config['sn']}")
                immediate_reboot = reboot_command(token, wait_result=False)
                assert immediate_reboot["status"] == "sent", f"{name} immediate reboot: {immediate_reboot}"
                reboot = reboot_command(token)
                assert reboot["status"] == expected, f"{name} reboot: {reboot}"
                assert reboot["sn"] != config["sn"], "reboot reused config request SN"
                print(f"{name} reboot: {reboot['status']} sn={reboot['sn']}")
                immediate_preset = preset_command(token, wait_result=False)
                assert immediate_preset["status"] == "sent", f"{name} immediate preset: {immediate_preset}"
                preset = preset_command(token)
                assert preset["status"] == expected, f"{name} preset: {preset}"
                assert preset["sn"] != reboot["sn"], "preset reused reboot request SN"
                print(f"{name} preset: {preset['status']} sn={preset['sn']}")
                if name == "ok":
                    sy_preset_command(token)
                    deadline = time.monotonic() + 2
                    while time.monotonic() < deadline:
                        output.flush()
                        output.seek(0)
                        if "PTZCmd=A50F01820003003A" in output.read().decode(errors="replace"):
                            break
                        time.sleep(0.05)
                    else:
                        raise AssertionError("SY GOTO_PRESET did not send PTZCmd for preset 3")
                    common_channel_commands(token, output)
                guard = guard_command(token)
                assert guard["status"] == expected, f"{name} guard: {guard}"
                assert guard["sn"] != first["sn"], "guard reused transport request SN"
                print(f"{name} guard: {guard['status']} sn={guard['sn']}")
                record = record_command(token)
                result = record["deviceResult"]
                assert result["status"] == expected, f"{name} record: {record}"
                expected_recording = True if expected == "confirmed" else None
                assert record["recording"] is expected_recording, f"{name} recording: {record}"
                assert result["sn"] != guard["sn"], "record reused guard request SN"
                print(f"{name} record: {result['status']} sn={result['sn']}")
                if name == "ok":
                    stopped = record_command(token, "StopRecord")
                    assert stopped["deviceResult"]["status"] == "confirmed", stopped
                    assert stopped["recording"] is False, stopped
                    print(f"{name} stop record: confirmed sn={stopped['deviceResult']['sn']}")
            if second:
                next_result = command(token)
                assert next_result["status"] == expected, f"{name} second: {next_result}"
                assert next_result["sn"] != first["sn"], "new request reused the previous SN"
            print(f"{name}: {first['status']} sn={first['sn']}")
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=3)
            time.sleep(0.2)


def main():
    token = login()
    scenario(token, "ok", "ok", "confirmed")
    scenario(token, "reject", "reject", "rejected")
    scenario(token, "silent", "silent", "unconfirmed")
    scenario(token, "duplicate", "ok", "confirmed", repeats=2)
    scenario(token, "late", "ok", "unconfirmed", delay_ms=5500, second=True)


if __name__ == "__main__":
    main()
