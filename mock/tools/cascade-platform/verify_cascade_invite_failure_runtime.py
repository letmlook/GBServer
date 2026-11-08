#!/usr/bin/env python3
"""Verify failed cascade INVITE releases the RTP server on a real ZLM.

Requires an isolated backend with a platform whose id is supplied by
GBSERVER_TEST_PLATFORM_ID and whose server_ip is deliberately invalid.
"""
import hashlib
import json
import os
import urllib.parse
import urllib.request


BASE = os.environ.get("GBSERVER_TEST_URL", "http://127.0.0.1:18080").rstrip("/")
ZLM = os.environ.get("GBSERVER_TEST_ZLM_URL", "http://127.0.0.1:8080").rstrip("/")
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def get(url, headers=None, post=False):
    with HTTP.open(urllib.request.Request(url, headers=headers or {}, data=b"" if post else None), timeout=20) as response:
        return json.load(response)


def main():
    platform_id = os.environ["GBSERVER_TEST_PLATFORM_ID"]
    platform_gb = os.environ["GBSERVER_TEST_PLATFORM_GB_ID"]
    secret = os.environ["GBSERVER_TEST_ZLM_SECRET"]
    channel = "34020000001310009876"
    prefix = f"cascade_pull_{platform_gb}_{channel}_"
    query = urllib.parse.urlencode({"secret": secret})

    def rtp_servers():
        result = get(ZLM + "/index/api/listRtpServer?" + query)
        assert result.get("code") == 0, "ZLM RTP listing failed"
        return {item["stream_id"] for item in result.get("data", [])}

    baseline = rtp_servers()
    login = urllib.parse.urlencode({"username": os.environ.get("GBSERVER_TEST_USERNAME", "admin"),
                                    "password": hashlib.md5(os.environ.get("GBSERVER_TEST_PASSWORD", "admin").encode()).hexdigest()})
    token = get(BASE + "/api/user/login?" + login, post=True)["data"]["accessToken"]
    params = urllib.parse.urlencode({"platformId": platform_id, "channelIdList": channel})
    result = get(BASE + "/api/platform/channel/push?" + params, {"access-token": token})
    assert result["code"] == 0 and result["data"]["pushedCount"] == 0 and result["data"]["errors"], result
    assert any("invalid socket address" in error for error in result["data"]["errors"]), result
    assert not {stream for stream in rtp_servers() - baseline if stream.startswith(prefix)}, "cascade RTP server leaked after failed INVITE"
    print("failed cascade INVITE reported; real ZLM RTP server removed")


if __name__ == "__main__":
    main()
