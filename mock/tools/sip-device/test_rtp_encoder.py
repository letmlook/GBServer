import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from sip_device_mock import choose_h264_encoder


class RtpEncoderTests(unittest.TestCase):
    def test_prefers_openh264_when_available(self):
        self.assertEqual(choose_h264_encoder(' V..... libopenh264\n V..... libx264\n'), 'libopenh264')

    def test_uses_x264_when_openh264_is_absent(self):
        self.assertEqual(choose_h264_encoder(' V..... libx264 H.264 encoder\n'), 'libx264')

    def test_fails_explicitly_without_h264_encoder(self):
        with self.assertRaisesRegex(RuntimeError, 'H.264'):
            choose_h264_encoder(' V..... mpeg4\n')


if __name__ == '__main__':
    unittest.main()
