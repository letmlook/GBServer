import unittest
from unittest.mock import MagicMock, patch

from sip_device_mock import DeviceConfig, SipDeviceMock


def invite(call_id, ssrc, port):
    return (f"INVITE sip:channel@127.0.0.1 SIP/2.0\r\n"
            f"Via: SIP/2.0/UDP 127.0.0.1:5060;branch=z9hG4bK{call_id}\r\n"
            f"From: <sip:platform@127.0.0.1>;tag=from{call_id}\r\n"
            f"To: <sip:channel@127.0.0.1>\r\nCall-ID: {call_id}\r\n"
            f"CSeq: 1 INVITE\r\n\r\ns=Playback\r\nc=IN IP4 127.0.0.1\r\n"
            f"m=video {port} RTP/AVP 96\r\ny={ssrc}\r\n")


class ConcurrentRtpTests(unittest.IsolatedAsyncioTestCase):
    async def test_two_invites_have_independent_decimal_ssrc_and_bye(self):
        with patch("sip_device_mock.RtpSender") as sender:
            first, second = MagicMock(), MagicMock()
            sender.side_effect = [first, second]
            device = SipDeviceMock(DeviceConfig(send_rtp=True, auto_bye_secs=0), ("127.0.0.1", 5060))
            device.transport = MagicMock()
            device.transport.get_extra_info.return_value = ("127.0.0.1", 15060)
            await device._on_invite(invite("one", "1200000001", 30001), ("127.0.0.1", 5060))
            await device._on_invite(invite("two", "1200000002", 30002), ("127.0.0.1", 5060))
            self.assertEqual(sender.call_count, 2)
            first.start.assert_called_once_with("127.0.0.1", 30001, 1200000001)
            second.start.assert_called_once_with("127.0.0.1", 30002, 1200000002)
            tag = device._invite_state["one"]["to_tag"]
            bye = (f"BYE sip:channel@127.0.0.1 SIP/2.0\r\n"
                   f"From: <sip:platform@127.0.0.1>;tag=fromone\r\n"
                   f"To: <sip:channel@127.0.0.1>;tag={tag}\r\n"
                   f"Call-ID: one\r\nCSeq: 2 BYE\r\n\r\n")
            await device._on_bye(bye, ("127.0.0.1", 5060))
            first.stop.assert_called_once()
            second.stop.assert_not_called()

