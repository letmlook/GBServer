#!/usr/bin/env python3
"""Verify SIP registration against an isolated SQLite backend with limit 2.

The backend must use a freshly initialized test database (one seeded device)
and listen on 127.0.0.1:5060.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

MOCK = Path(__file__).with_name('sip_device_mock.py')
FIRST = '34020000001320000001'
SECOND = '34020000001320000999'


def register(device, expected):
    with tempfile.TemporaryFile(mode='w+') as output:
        proc = subprocess.Popen([
            sys.executable, str(MOCK), '--server', '127.0.0.1:5060',
            '--device-id', device, '--username', device,
            '--password', os.environ.get('GBSERVER_TEST_SIP_PASSWORD', 'admin123'),
            '--channels', '1', '--auto-keepalive', '0', '--auto-bye-secs', '0',
        ], stdout=output, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                output.flush()
                output.seek(0)
                log = output.read()
                if expected in log:
                    print(f'{device}: {expected}')
                    return
                if proc.poll() is not None:
                    break
                time.sleep(0.1)
            raise AssertionError(f'{device}: expected {expected}, got {log[-1500:]}')
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=3)


if __name__ == '__main__':
    register(FIRST, '设备已注册')
    register(SECOND, '503 Service Unavailable')
    register(FIRST, '设备已注册')
