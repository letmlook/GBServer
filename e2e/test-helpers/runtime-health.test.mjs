import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { checkRuntimeHealth } from './runtime-health.ts';

test('reports a dead managed process with PID and log location', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'gbserver-health-'));
  try {
    const pidFile = join(dir, 'mock.pid');
    const logFile = join(dir, 'mock.log');
    writeFileSync(pidFile, '99999999\n');
    await assert.rejects(
      checkRuntimeHealth([{ name: 'SIP Mock', pidFile, logFile }]),
      (error) => /测试环境失效/.test(error.message) && /99999999/.test(error.message) &&
        error.message.includes(logFile)
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('reports a stopped HTTP dependency instead of a product failure', async () => {
  const server = createServer((_req, res) => { res.writeHead(200); res.end('ok'); });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  const url = `http://127.0.0.1:${address.port}/health`;
  try {
    await checkRuntimeHealth([{ name: 'backend', url }]);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
  await assert.rejects(checkRuntimeHealth([{ name: 'backend', url }]), /测试环境失效.*backend/);
});
