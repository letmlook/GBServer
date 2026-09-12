/**
 * 拉流代理页（/#/streamProxy）契约端到端测试。
 *
 * 这一页此前**整个功能都是坏的**（字段名全部对不上后端）：
 *   1. 新增/编辑弹窗提交 `{url, enabled, destUrl}`，后端 DTO 只认
 *      `src_url`/`srcUrl` → 新增必报"Stream and src_url are required"；
 *   2. 编辑保存时 `src_url` 为 NULL + `COALESCE` → "源 URL"静默改不动；
 *   3. 列表读 `row.url` / `row.enabled` / `row.status`，后端返回的是
 *      `srcUrl` / `enable` / `pulling` → "源 URL"整列空白、启用开关恒关、
 *      状态恒为"停止"（"停止"按钮因此永远禁用）；
 *   4. 搜索框的 `query` 后端收了从不使用 → 搜什么都返回全量。
 *
 * 本 spec 会真起一条源流（ffmpeg → ZLM），再从界面启动/停止代理，
 * 校验 ZLM 侧确实产生了代理流 —— 不是只看 DOM。
 */

import { test, expect, type Page } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';

const ZLM_API = 'http://127.0.0.1:8080/index/api';
const ZLM_SECRET = 'EWHpCV2W2SYa8SnpE2whVlUUlJJKhaiw';
const SRC_APP = 'live';
const SRC_STREAM = `e2eProxySrc`;
const NAME = `e2e代理-${Date.now()}`;
const STREAM = `e2ePx${Date.now()}`;
const SRC_URL = `rtsp://127.0.0.1:554/${SRC_APP}/${SRC_STREAM}`;

let ffmpeg: ChildProcess | null = null;
let zlmUp = false;

async function zlmHasStream(app: string, stream: string): Promise<boolean> {
  try {
    const r = await fetch(`${ZLM_API}/getMediaList?secret=${ZLM_SECRET}&app=${app}&stream=${stream}`);
    const body = (await r.json()) as { code: number; data?: unknown[] };
    return body.code === 0 && (body.data?.length ?? 0) > 0;
  } catch {
    return false;
  }
}

async function gotoProxy(page: Page) {
  // hash 路由：必须带 `#/streamProxy`，否则会被重定向到 dashboard
  await page.goto('/#/streamProxy', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe.serial('Stream proxy page (/streamProxy)', () => {
  test.beforeAll(async () => {
    // 没有真实 ZLM 就跳过（本页的"启动/停止"必须打到 ZLM 才算验证）
    try {
      const r = await fetch(`${ZLM_API}/getServerConfig?secret=${ZLM_SECRET}`);
      zlmUp = r.ok;
    } catch {
      zlmUp = false;
    }
    if (!zlmUp) return;

    ffmpeg = spawn(
      'ffmpeg',
      [
        '-hide_banner', '-loglevel', 'error', '-re',
        '-f', 'lavfi', '-i', 'testsrc=size=320x240:rate=15',
        '-c:v', 'libx264', '-preset', 'ultrafast', '-tune', 'zerolatency', '-g', '15',
        '-f', 'flv', `rtmp://127.0.0.1:1935/${SRC_APP}/${SRC_STREAM}`
      ],
      { stdio: 'ignore' }
    );

    // 等源流在 ZLM 上出现（最多 15s）
    for (let i = 0; i < 30; i++) {
      if (await zlmHasStream(SRC_APP, SRC_STREAM)) return;
      await new Promise((r) => setTimeout(r, 500));
    }
  });

  test.afterAll(async () => {
    ffmpeg?.kill('SIGKILL');
    ffmpeg = null;
  });

  test('新增代理后列表展示源地址/代理方式/启用状态', async ({ page }) => {
    test.skip(!zlmUp, '本地没有可用的 ZLMediaKit，跳过');
    await gotoProxy(page);

    await page.getByRole('button', { name: '新增代理' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByPlaceholder('代理名称').fill(NAME);
    await dialog.getByPlaceholder('live / proxy').fill('proxy');
    await dialog.getByPlaceholder('流 ID').fill(STREAM);
    await dialog.getByPlaceholder('rtsp://... / rtmp://... / http://...m3u8').fill(SRC_URL);

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/proxy/add'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);

    const row = page.locator('tr', { hasText: STREAM }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    // 源地址列必须真的有值（此前读 row.url → 恒空）
    await expect(row).toContainText(SRC_URL);
    // 代理方式 / 启用状态（此前传 enabled → 后端收不到，恒显示未启用）
    await expect(row).toContainText('默认');
    await expect(row).toContainText('已启用');
    await expect(row).toContainText('尚未拉流');
  });

  test('搜索关键字会真的传给后端 query', async ({ page }) => {
    test.skip(!zlmUp, '本地没有可用的 ZLMediaKit，跳过');
    await gotoProxy(page);

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/proxy/list') && r.url().includes('query='), {
        timeout: 10_000
      }),
      (async () => {
        await page.getByPlaceholder('名称 / App / Stream / 源地址').fill(STREAM);
        await page.getByRole('button', { name: '查询' }).click();
      })()
    ]);
    const body = (await resp.json()) as { code: number; data: { total: number } };
    expect(body.code).toBe(0);
    expect(body.data.total).toBeGreaterThan(0);
    await expect(page.locator('tr', { hasText: STREAM }).first()).toBeVisible();
  });

  test('启动/停止代理：ZLM 侧真的产生并关闭代理流', async ({ page }) => {
    test.skip(!zlmUp, '本地没有可用的 ZLMediaKit，跳过');
    await gotoProxy(page);

    const row = page.locator('tr', { hasText: STREAM }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    // 启动
    const [startResp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/proxy/start'), { timeout: 20_000 }),
      row.getByRole('button', { name: '播放' }).click()
    ]);
    expect(((await startResp.json()) as { code: number }).code).toBe(0);

    // ZLM 上必须真的出现 proxy/<stream>
    let exists = false;
    for (let i = 0; i < 20; i++) {
      if (await zlmHasStream('proxy', STREAM)) {
        exists = true;
        break;
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    expect(exists, 'ZLM 上应出现拉流代理的流').toBe(true);

    // UI 状态列必须变成"正在拉流"（此前恒为"停止"）
    const liveRow = page.locator('tr', { hasText: STREAM }).first();
    await expect(liveRow).toContainText('正在拉流', { timeout: 10_000 });
    await page.screenshot({ path: 'artifacts/streamProxy-start.png', fullPage: true });

    // 停止
    const [stopResp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/proxy/stop'), { timeout: 20_000 }),
      liveRow.getByRole('button', { name: '停止' }).click()
    ]);
    expect(((await stopResp.json()) as { code: number }).code).toBe(0);
    await expect(page.locator('tr', { hasText: STREAM }).first()).toContainText('尚未拉流', {
      timeout: 10_000
    });
  });

  test('删除代理（DELETE /proxy/delete）', async ({ page }) => {
    test.skip(!zlmUp, '本地没有可用的 ZLMediaKit，跳过');
    await gotoProxy(page);

    const row = page.locator('tr', { hasText: STREAM }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    const [resp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/proxy/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await row.getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);
    await expect(page.locator('tr', { hasText: STREAM })).toHaveCount(0, { timeout: 10_000 });
  });
});
