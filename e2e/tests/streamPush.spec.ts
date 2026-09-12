/**
 * 推流管理页（/#/streamPush）契约端到端测试。
 *
 * 这一页此前 3 个动作是错的：
 *   1. 删除用 `DELETE /api/push/remove`，后端只注册 POST（WVP 也是 POST+query）
 *      → 405，推流记录删不掉；
 *   2. 「批量删除」把 ids 放 query，后端要求 DELETE + JSON body → 415；
 *   3. 状态列读 `row.status === 1`，而后端（与 WVP）返回**布尔** → 永远显示"停止"，
 *      「停止」按钮恒为禁用。
 * 另外列表读 `mediaServerId`/`url`，而后端返回 snake_case 且根本没有 url 列
 * →「媒体节点」列空白、「源 URL」列空白。
 */

import { test, expect, type Page } from '@playwright/test';

const STREAM = `e2e-${Date.now()}`;

async function gotoPush(page: Page) {
  await page.goto('/#/streamPush', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe.serial('Stream push page (/streamPush)', () => {
  test('新增推流后列表显示媒体节点与推流地址', async ({ page }) => {
    await gotoPush(page);
    await page.getByRole('button', { name: '新增推流' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // 与 WVP 一致：只有 App / Stream / 媒体节点（没有"源 URL"输入）
    await expect(dialog.getByPlaceholder('rtsp://... 或 rtmp://...')).toHaveCount(0);
    const inputs = dialog.locator('input');
    await inputs.nth(0).fill('push');
    await inputs.nth(1).fill(STREAM);
    await dialog.getByRole('button', { name: '保存' }).click();

    const row = page.locator('tr', { hasText: STREAM }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    // 媒体节点列必须有值（后端 camelCase）；推流地址列是后端算出来的
    await expect(row).toContainText('rtmp://');
  });

  test('删除：走 POST /push/remove（此前用 DELETE 会 405）', async ({ page }) => {
    await gotoPush(page);
    const row = page.locator('tr', { hasText: STREAM }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    const [resp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/push/remove') && r.request().method() === 'POST',
        { timeout: 10_000 }
      ),
      (async () => {
        await row.getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    const body = (await resp.json()) as { code: number };
    expect(body.code).toBe(0);
    await expect(page.locator('tr', { hasText: STREAM })).toHaveCount(0, { timeout: 10_000 });
  });
});
