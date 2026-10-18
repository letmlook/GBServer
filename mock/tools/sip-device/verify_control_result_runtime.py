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
PTZ_CHANNEL_ID = f"{DEVICE_ID[:-4]}0002"
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


def ptz_command(token, wait_result=True):
    query = urllib.parse.urlencode({
        "deviceId": DEVICE_ID, "channelId": PTZ_CHANNEL_ID, "command": "right",
        "speed": "3", "waitResult": str(wait_result).lower(),
    })
    response = api("/api/device/control/ptz?" + query, token=token)
    if response.get("code") != 0 or not response.get("data", {}).get("sipSent"):
        raise AssertionError("PTZ DeviceControl was not sent: " + json.dumps(response, ensure_ascii=False))
    return response["data"]["deviceResult"]


def batch_command(token, command, wait_result=True):
    payload = {"deviceIds": [DEVICE_ID], "command": command,
               "waitResult": wait_result}
    if command == "ptzStop":
        payload["channelId"] = PTZ_CHANNEL_ID
    response = api("/api/device/batch/control", token=token, post=True, payload=payload)
    data = response.get("data") or {}
    results = data.get("results") or []
    if response.get("code") != 0 or data.get("success") != 1 or len(results) != 1:
        raise AssertionError("Batch control was not sent: " + json.dumps(response, ensure_ascii=False))
    result = results[0]
    if not result.get("sipSent"):
        raise AssertionError("Batch control lacks SIP send status: " + json.dumps(response, ensure_ascii=False))
    return result["deviceResult"]


def batch_silent_commands_run_concurrently(token):
    started = time.monotonic()
    response = api("/api/device/batch/control", token=token, post=True,
                   payload={"deviceIds": [DEVICE_ID, DEVICE_ID],
                            "command": "reboot", "waitResult": True})
    elapsed = time.monotonic() - started
    data = response.get("data") or {}
    results = data.get("results") or []
    assert response["code"] == 0 and data.get("success") == 2 and len(results) == 2, response
    assert all(r["sipSent"] and r["deviceResult"]["status"] == "unconfirmed" for r in results), response
    assert results[0]["deviceResult"]["sn"] != results[1]["deviceResult"]["sn"], response
    assert elapsed < 8, f"batch waited serially for two 5s timeouts: {elapsed:.2f}s"
    print(f"silent batch two requests: unconfirmed in {elapsed:.2f}s")


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
    sync_data = sync.get("data") or {}
    if (sync.get("code") != 0 or sync_data.get("syncState") != "done"
            or sync_data.get("totalChannels") != 17
            or sync_data.get("receivedChannels") != 17
            or sync_data.get("receivedPackets") != 3
            or sync_data.get("error") is not None):
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
        data = result.get("data") or {}
        assert data.get("sipSent") is True and data.get("deviceResult", {}).get("status") == "sent", result
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            output.flush()
            output.seek(0)
            if expected in output.read().decode(errors="replace"):
                break
            time.sleep(0.05)
        else:
            raise AssertionError("Common channel command missing " + expected)

    home = api("/api/common/channel/front-end/home_position?" + urllib.parse.urlencode({
        "channelId": channel_id, "enabled": "true", "resetTime": 30, "presetIndex": 3,
    }), token=token)
    assert home.get("code") == 0 and "命令已发送" in home.get("msg", ""), home
    assert home.get("data", {}).get("sipSent") is True, home
    assert home["data"]["deviceResult"]["status"] == "sent", home

    invalid = api("/api/common/channel/front-end/preset/call?" + urllib.parse.urlencode({
        "channel_id": channel_id, "preset_id": 256,
    }), token=token)
    assert invalid.get("code") != 0 and "1-255" in invalid.get("msg", ""), invalid


def playback_send_only_commands(token):
    stream = f"playback_{DEVICE_ID}_{DEVICE_ID}_1700000000"
    public = api(f"/api/playback/pause/{stream}", token=token)
    data = public.get("data") or {}
    assert public.get("code") == 0 and data.get("sipSent") is True, public
    assert data.get("deviceResult", {}).get("status") == "sent", public
    assert data.get("statusSource") == "local_requested", public

    query = urllib.parse.urlencode({"stream": stream, "speed": "2"})
    compatible = api("/api/common/channel/playback/speed?" + query, token=token)
    assert compatible.get("code") == 0 and compatible.get("sipSent") is True, compatible
    assert compatible.get("deviceResult", {}).get("status") == "sent", compatible
    assert "命令已发送" in compatible.get("msg", ""), compatible
    print("playback pause and compatible speed: SIP sent, device result pending")


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
                "--password", SIP_PASSWORD, "--channels", "17",
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
                immediate_ptz = ptz_command(token, wait_result=False)
                assert immediate_ptz["status"] == "sent", f"{name} immediate PTZ: {immediate_ptz}"
                ptz = ptz_command(token)
                assert ptz["status"] == expected, f"{name} PTZ: {ptz}"
                assert ptz["sn"] != preset["sn"], "PTZ reused preset request SN"
                print(f"{name} PTZ: {ptz['status']} sn={ptz['sn']}")
                for batch_type in ("ptzStop", "reboot"):
                    immediate_batch = batch_command(token, batch_type, wait_result=False)
                    assert immediate_batch["status"] == "sent", f"{name} immediate batch {batch_type}: {immediate_batch}"
                    batch_result = batch_command(token, batch_type)
                    assert batch_result["status"] == expected, f"{name} batch {batch_type}: {batch_result}"
                    print(f"{name} batch {batch_type}: {batch_result['status']} sn={batch_result['sn']}")
                if name == "silent":
                    batch_silent_commands_run_concurrently(token)
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
                    playback_send_only_commands(token)
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
                first_ptz = ptz_command(token)
                next_ptz = ptz_command(token)
                assert first_ptz["status"] == expected, f"{name} first PTZ: {first_ptz}"
                assert next_ptz["status"] == expected, f"{name} second PTZ: {next_ptz}"
                assert next_ptz["sn"] != first_ptz["sn"], "new PTZ request reused the previous SN"
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
    for command_name in ("syncCatalog", "queryDeviceInfo", "queryDeviceStatus"):
        response = api("/api/device/batch/control", token=token, post=True,
                       payload={"deviceIds": [DEVICE_ID], "command": command_name,
                                "waitResult": True})
        assert response["code"] != 0, f"{command_name} incorrectly accepted waitResult: {response}"
    response = api("/api/device/batch/control", token=token, post=True,
                   payload={"deviceIds": ["34020000001320000099"], "command": "reboot"})
    data = response.get("data") or {}
    assert response["code"] == 0 and data.get("success") == 0 and data.get("failed") == 1, response
    result = data["results"][0]
    assert result["sipSent"] is False and result["deviceResult"]["status"] == "not_sent", response
    print("batch query waitResult rejected; offline device not_sent")


if __name__ == "__main__":
    main()
