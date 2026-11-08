#!/usr/bin/env python3
"""Verify cascade response, ACK and peer BYE using an isolated backend and ZLM.

The selected test platform must point to 127.0.0.1:15062 and be disabled to
avoid registration. GBSERVER_TEST_PLATFORM_ID and _GB_ID identify it.
"""
import hashlib
import os
import socket
import sys
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path

from verify_cascade_invite_failure_runtime import BASE, HTTP, ZLM, get

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "sip-device"))
from sip_device_mock import DeviceConfig, RtpSender, SipDeviceMock


def main():
    platform_id = os.environ["GBSERVER_TEST_PLATFORM_ID"]
    platform_gb = os.environ["GBSERVER_TEST_PLATFORM_GB_ID"]
    secret = os.environ["GBSERVER_TEST_ZLM_SECRET"]
    channel = "34020000001310009876"
    reject = os.environ.get("GBSERVER_TEST_REJECT") == "1"
    media = os.environ.get("GBSERVER_TEST_MEDIA") == "1"
    late = os.environ.get("GBSERVER_TEST_LATE") == "1"
    stream = f"cascade_{platform_gb}_{channel}"
    query = urllib.parse.urlencode({"secret": secret})

    def servers():
        result = get(ZLM + "/index/api/listRtpServer?" + query)
        assert result["code"] == 0
        return {entry["stream_id"] for entry in result.get("data", [])}

    assert stream not in servers(), "test stream already exists"
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 15062))
    sock.settimeout(8)
    outcome = []
    senders = []

    def peer():
        try:
            message, addr = sock.recvfrom(65535)
            text = message.decode()
            assert text.startswith("INVITE "), text[:80]
            headers = {}
            for line in text.split("\r\n"):
                if ":" in line:
                    key, value = line.split(":", 1)
                    headers[key.lower()] = value.strip()
            response = (("SIP/2.0 486 Busy Here\r\n" if reject else "SIP/2.0 200 OK\r\n") + f"Via: {headers['via']}\r\n"
                        + f"From: {headers['from']}\r\nTo: {headers['to']};tag=peer123\r\n"
                        + f"Call-ID: {headers['call-id']}\r\nCSeq: 1 INVITE\r\nContent-Length: 0\r\n\r\n")
            if late:
                time.sleep(17)
            sock.sendto(response.encode(), addr)
            if reject:
                ack, _ = sock.recvfrom(65535)
                ack_text = ack.decode()
                assert ack_text.startswith("ACK "), ack_text[:80]
                assert f"Via: {headers['via']}" in ack_text, ack_text
                assert f"To: {headers['to']};tag=peer123" in ack_text, ack_text
                assert f"Call-ID: {headers['call-id']}" in ack_text and "CSeq: 1 ACK" in ack_text, ack_text
                time.sleep(0.3)
                sock.sendto(response.encode(), addr)
                repeated_ack, _ = sock.recvfrom(65535)
                assert repeated_ack == ack, "retransmitted rejection did not receive the same ACK"
                outcome.append((addr, headers))
                return
            ack, _ = sock.recvfrom(65535)
            assert ack.startswith(b"ACK "), ack[:80]
            if late:
                assert f"To: {headers['to']};tag=peer123" in ack.decode()
                assert "CSeq: 1 ACK" in ack.decode()
                bye, _ = sock.recvfrom(65535)
                assert bye.startswith(b"BYE "), bye[:80]
                assert f"Call-ID: {headers['call-id']}" in bye.decode()
                assert f"To: {headers['to']};tag=peer123" in bye.decode()
                assert "CSeq: 2 BYE" in bye.decode()
                sock.sendto(response.encode(), addr)
                repeated_ack, _ = sock.recvfrom(65535)
                assert repeated_ack.startswith(b"ACK "), repeated_ack[:80]
                sock.settimeout(0.5)
                try:
                    extra, _ = sock.recvfrom(65535)
                except socket.timeout:
                    pass
                else:
                    raise AssertionError(f"unexpected duplicate late BYE: {extra[:80]!r}")
                outcome.append((addr, headers))
                return
            if media:
                ip, port, ssrc = SipDeviceMock._parse_platform_media(text.split("\r\n\r\n", 1)[1])
                assert ip and port and ssrc.isdecimal(), (ip, port, ssrc)
                sender = RtpSender(DeviceConfig())
                assert sender.start(ip, port, int(ssrc)), "RTP sender failed to start"
                senders.append(sender)
            outcome.append((addr, headers))
        except Exception as error:
            outcome.append(error)

    thread = threading.Thread(target=peer)
    thread.start()
    try:
        login = urllib.parse.urlencode({"username": "admin", "password": hashlib.md5(b"admin").hexdigest()})
        token = get(BASE + "/api/user/login?" + login, post=True)["data"]["accessToken"]
        params = urllib.parse.urlencode({"platformId": platform_id, "channelIdList": channel})
        result = get(BASE + "/api/platform/channel/push?" + params, {"access-token": token})
        thread.join(10)
        assert len(outcome) == 1 and not isinstance(outcome[0], Exception), outcome
        if late:
            assert result["data"]["pushedCount"] == 0 and result["data"]["errors"], result
            assert stream not in servers(), "timed-out INVITE leaked RTP server"
            print("late 200 acknowledged and terminated without RTP leak")
            return
        if reject:
            assert result["data"]["pushedCount"] == 0 and any("486" in error for error in result["data"]["errors"]), result
            assert stream not in servers(), "rejected INVITE leaked RTP server"
            print("cascade 486 rejection reported and real ZLM RTP server removed")
            return
        assert result["data"]["pushedCount"] == 1 and not result["data"]["errors"], result
        assert stream in servers(), "accepted session has no RTP server"
        if media:
            params = urllib.parse.urlencode({"app": "rtp", "stream": stream})
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                media_info = get(BASE + "/api/media/stream_info_by_app_and_stream?" + params, {"access-token": token})
                if media_info["code"] == 0 and media_info["data"]["count"] > 0:
                    break
                time.sleep(0.2)
            else:
                raise AssertionError("cascade RTP did not produce a ZLM media stream")
            with HTTP.open(urllib.request.Request(f"{ZLM}/rtp/{stream}.live.flv"), timeout=10) as response:
                media_bytes = response.read(1024)
            assert len(media_bytes) == 1024 and media_bytes.startswith(b"FLV"), "cascade FLV data unavailable"
        addr, headers = outcome[0]
        bye = (f"BYE sip:platform@127.0.0.1:5060 SIP/2.0\r\n"
               f"Via: SIP/2.0/UDP 127.0.0.1:15062;branch=z9hG4bKbye\r\n"
               f"From: {headers['to']};tag=peer123\r\nTo: {headers['from']}\r\n"
               f"Call-ID: {headers['call-id']}\r\nCSeq: 2 BYE\r\nContent-Length: 0\r\n\r\n")
        sock.sendto(bye.encode(), addr)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and stream in servers():
            time.sleep(0.1)
        assert stream not in servers(), "peer BYE did not close RTP server"
        print(f"cascade accepted: ACK received; FLV bytes={len(media_bytes) if media else 0}; peer BYE closed real ZLM RTP server")
    finally:
        thread.join(10)
        for sender in senders:
            sender.stop()
        sock.close()
        try:
            if stream in servers():
                params = urllib.parse.urlencode({"secret": secret, "stream_id": stream})
                get(ZLM + "/index/api/closeRtpServer?" + params)
        except Exception:
            pass


if __name__ == "__main__":
    main()
