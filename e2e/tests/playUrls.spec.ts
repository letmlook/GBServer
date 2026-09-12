/**
 * 实时点播返回的**播放地址必须真的能用**（`/api/play/start`）。
 *
 * 这里曾经有两个「给了地址但 404」的坑，而且正好是浏览器唯一能用的两条路：
 *   1. `flvUrl` 拼成了 `/<app>/<stream>.flv`，而 ZLM 的 HTTP-FLV 地址后缀固定是
 *      `<app>/<stream>.live.flv`（`.flv` 返回 404 的 HTML 页面）；
 *   2. `hls` 无条件返回，但 ZLM 并不会给 GB28181 的 RTP/PS 流生成 hls 源
 *      → `/rtp/<stream>/hls.m3u8` 404。
 *
 * 实时预览页的兜底顺序是 `hls → flvUrl → playUrl`，而 `playUrl` 是 rtsp
 * （浏览器播不了）—— 于是**画面永远出不来，还没有任何报错线索**。
 *
 * 本 spec 直接校验"回来的地址能不能取到字节"，并要求 hls 要么不给、要么可用。
 */
import { test, expect, type Page } from '@playwright/test';
import * as http from 'node:http';

/**
 * 读一个"永不结束"的 HTTP 流的前 N 个字节。
 *
 * 直播 FLV 不会自己结束，用 Playwright 的 `request.get` 会一直等到超时
 * （拿不到 body）；所以这里直接用 node:http 读首块后主动断开。
 */
function readStreamHead(url: string, bytes = 16, timeoutMs = 5_000): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      const chunks: Buffer[] = [];
      let total = 0;
      res.on('data', (c: Buffer) => {
        chunks.push(c);
        total += c.length;
        if (total >= bytes) {
          req.destroy();
          resolve(Buffer.concat(chunks).subarray(0, bytes));
        }
      });
      res.on('end', () => resolve(Buffer.concat(chunks).subarray(0, bytes)));
      res.on('error', reject);
    });
    req.setTimeout(timeoutMs, () => {
      req.destroy();
      reject(new Error(`读取 ${url} 超时（未收到数据）`));
    });
    req.on('error', (e) => {
      if ((e as { code?: string }).code === 'ECONNRESET') return; // 主动断开
      reject(e);
    });
  });
}

const DEVICE = '34020000001320000001';
const CHANNEL = '34020000001320000001';

async function authHeaders(page: Page): Promise<Record<string, string>> {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
  return token ? { 'access-token': token } : {};
}

test('实时点播返回的 flvUrl 真的能取到 FLV 流，且不会给出 404 的 hls', async ({ page }) => {
  await page.goto('/#/live', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});

  const headers = await authHeaders(page);
  const resp = await page.request.get(`/dev-api/play/start/${DEVICE}/${CHANNEL}`, { headers });
  expect(resp.status()).toBe(200);
  const body = (await resp.json()) as {
    code: number;
    data?: { flvUrl?: string; hls?: string; playUrl?: string; hlsAvailable?: boolean };
  };
  expect(body.code).toBe(0);
  const data = body.data ?? {};

  // 1) FLV 地址必须是 .live.flv 且真的返回 FLV 头
  expect(data.flvUrl, '必须给出 flvUrl').toBeTruthy();
  expect(data.flvUrl!).toContain('.live.flv');
  const flvBytes = await readStreamHead(data.flvUrl!);
  expect(flvBytes.subarray(0, 3).toString('latin1'), `${data.flvUrl} 应返回 FLV 流`).toBe('FLV');

  // 2) hls 要么不给、要么真的可取（不能给 404 地址）
  if (data.hls) {
    const hls = await page.request.get(data.hls, { failOnStatusCode: false });
    expect(hls.status(), `${data.hls} 被返回了就必须可用`).toBeLessThan(400);
    expect(await hls.text()).toContain('#EXTM3U');
  } else {
    expect(data.hlsAvailable).toBe(false);
  }

  // 3) rtsp 地址仍然给出（非浏览器客户端用）
  expect(data.playUrl ?? '').toMatch(/^rtsp:\/\//);
});
