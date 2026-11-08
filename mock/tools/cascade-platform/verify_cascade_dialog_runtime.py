#!/usr/bin/env python3
"""Verify cascade response, ACK and peer BYE using an isolated backend and ZLM.

The selected test platform must point to 127.0.0.1:15062 and be disabled to
avoid registration. GBSERVER_TEST_PLATFORM_ID and _GB_ID identify it.
"""
import hashlib
import os
import socket
import threading
import time
import urllib.parse

from verify_cascade_invite_failure_runtime import BASE, ZLM, get


def main():
    platform_id = os.environ["GBSERVER_TEST_PLATFORM_ID"]
    platform_gb = os.environ["GBSERVER_TEST_PLATFORM_GB_ID"]
    secret = os.environ["GBSERVER_TEST_ZLM_SECRET"]
    channel = "34020000001310009876"
    reject = os.environ.get("GBSERVER_TEST_REJECT") == "1"
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
            sock.sendto(response.encode(), addr)
            if reject:
                outcome.append((addr, headers))
                return
            ack, _ = sock.recvfrom(65535)
            assert ack.startswith(b"ACK "), ack[:80]
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
        if reject:
            assert result["data"]["pushedCount"] == 0 and any("486" in error for error in result["data"]["errors"]), result
            assert stream not in servers(), "rejected INVITE leaked RTP server"
            print("cascade 486 rejection reported and real ZLM RTP server removed")
            return
        assert result["data"]["pushedCount"] == 1 and not result["data"]["errors"], result
        assert stream in servers(), "accepted session has no RTP server"
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
        print("cascade accepted: ACK received; peer BYE closed real ZLM RTP server")
    finally:
        thread.join(10)
        sock.close()


if __name__ == "__main__":
    main()
