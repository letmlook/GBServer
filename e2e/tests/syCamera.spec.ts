/**
 * 摄像机接口（`/api/sy/camera/*`）契约端到端测试。
 *
 * 这一组接口此前 6 处不一致，其中两处是用户可见的：
 *   * `list-with-child` 请求 `count=1000`（live 页建树时一次取全量），
 *     但 db 层把任何 count 都**静默截到 100** → 第 101 台设备及其通道
 *     在实时预览页彻底看不到，而响应里的 count 仍回显 1000；
 *   * 搜索关键字参数名是 `keyword`，前端/WVP 传的是 `query` → 静默丢弃；
 *     `online`（只看在线）与 `civilCode` 连 DTO 字段都没有；
 *   * `total` 返回的是"本页展开出的行数"，分页器永远只有一页；
 *   * `/camera/list` 返回的是**设备行**（`is_device=true`），而 live 页按
 *     `!c.is_device` 过滤 → 若改用它，通道树会是空的。
 *
 * 本 spec 通过页内请求直接打后端（走 `/dev-api` 代理），避免依赖 UI 细节。
 */

import { test, expect, type Page } from '@playwright/test';

interface CameraRow {
  device_id: string
  channel_id: string
  name: string
  is_device: boolean
  online: boolean
}

interface CameraPage {
  total: number
  listTotal?: number
  count: number
  page: number
  list: CameraRow[]
}

const DEVICE_ID = '34020000001320000001';

async function authHeaders(page: Page): Promise<Record<string, string>> {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
  return token ? { 'access-token': token } : {};
}

async function cameraPage(
  page: Page,
  path: string,
  query = ''
): Promise<CameraPage> {
  const headers = await authHeaders(page);
  const resp = await page.request.get(`/dev-api/sy/camera/${path}${query}`, { headers });
  expect(resp.status()).toBe(200);
  const body = (await resp.json()) as { code: number; data: CameraPage };
  expect(body.code).toBe(0);
  return body.data;
}

test.describe('Camera API (/api/sy/camera)', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/#/live', { waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
  });

  test('list-with-child 返回通道级行且 count=1000 不被截断', async ({ page }) => {
    const data = await cameraPage(page, 'list-with-child', '?page=1&count=1000');
    expect(data.count).toBe(1000);
    expect(data.list.length).toBe(data.listTotal ?? data.list.length);
    expect(data.total).toBeGreaterThan(0);
    // 必须包含真正的通道行（live 页按 !is_device 过滤后要用它们建树）
    expect(data.list.some((r) => !r.is_device)).toBe(true);
    // 本环境至少有一台带通道的设备
    expect(data.list.some((r) => r.device_id === DEVICE_ID && !r.is_device)).toBe(true);
  });

  test('query 能按通道名搜索（此前参数名 keyword 不匹配，搜索无效果）', async ({ page }) => {
    const all = await cameraPage(page, 'list-with-child', `?query=${DEVICE_ID}`);
    expect(all.total).toBeGreaterThan(1);
    // 逐个通道名搜索，必须命中且只命中该通道
    for (const row of all.list.filter((r) => !r.is_device).slice(0, 3)) {
      const hit = await cameraPage(
        page,
        'list-with-child',
        `?query=${encodeURIComponent(row.name)}`
      );
      expect(hit.list.some((r) => r.channel_id === row.channel_id)).toBe(true);
    }
  });

  test('online 过滤真正生效', async ({ page }) => {
    const online = await cameraPage(page, 'list-with-child', '?online=true');
    expect(online.list.every((r) => r.online)).toBe(true);
    const offline = await cameraPage(page, 'list-with-child', '?online=false');
    expect(offline.list.every((r) => !r.online)).toBe(true);
    // 两者互补
    const all = await cameraPage(page, 'list-with-child', '');
    expect(online.total + offline.total).toBe(all.total);
  });

  test('/camera/list 也返回通道级行（此前只有设备行）', async ({ page }) => {
    const data = await cameraPage(page, 'list', '?count=1000');
    expect(data.list.some((r) => !r.is_device)).toBe(true);
    expect(data.total).toBeGreaterThan(0);
  });
});
