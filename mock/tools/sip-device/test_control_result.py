import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from sip_device_mock import DeviceConfig, build_device_control_response


class DeviceControlResultTests(unittest.TestCase):
    def test_result_message_echoes_request_context(self):
        cfg = DeviceConfig()
        payload = build_device_control_response(
            cfg, ('127.0.0.1', 15060), ('127.0.0.1', 5060),
            27, '8123', 'request-call-id', cfg.device_id, 'OK',
        )
        head, body = payload.split(b'\r\n\r\n', 1)
        self.assertIn(b'Call-ID: request-call-id', head)
        self.assertIn(b'CSeq: 27 MESSAGE', head)
        self.assertIn(f'Content-Length: {len(body)}'.encode(), head)
        root = ET.fromstring(body)
        self.assertEqual(root.tag, 'Response')
        self.assertEqual(root.findtext('CmdType'), 'DeviceControl')
        self.assertEqual(root.findtext('SN'), '8123')
        self.assertEqual(root.findtext('DeviceID'), cfg.device_id)
        self.assertEqual(root.findtext('Result'), 'OK')

    def test_rejected_result_is_distinct_from_sip_200(self):
        payload = build_device_control_response(
            DeviceConfig(), ('127.0.0.1', 15060), ('127.0.0.1', 5060),
            28, '8124', 'request-call-id', 'channel-1', 'ERROR',
        )
        body = payload.split(b'\r\n\r\n', 1)[1]
        root = ET.fromstring(body)
        self.assertEqual(root.findtext('DeviceID'), 'channel-1')
        self.assertEqual(root.findtext('Result'), 'ERROR')


if __name__ == '__main__':
    unittest.main()
