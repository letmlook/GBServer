/**
 * 媒体节点页（/#/mediaServer）契约端到端测试。
 *
 * 这一页此前两处硬伤：
 *   1. 「检测」按钮 `checkMediaServer(row.id)` 只发 `id`，而后端 DTO 里**没有 `id`**
 *      → 无论点哪个节点都在探测兜底的 `127.0.0.1:80`；且前端按 `data.code === 0`
 *      判断，而后端返回的是节点信息 payload（没有 code/msg）→ **必然弹「检测失败」**。
 *   2. 编辑弹窗的「启用」开关提交 `enabled`，后端没有这个字段、库里也没有这一列
 *      → 保存后静默丢弃，刷新即还原，开关不能停用节点。
 */

import { test, expect, type Page } from '@playwright/test';

const NODE_ID = 'zlmediakit-1';

async function gotoMediaServer(page: Page) {
  await page.goto('/#/mediaServer', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

function nodeRow(page: Page) {
  return page.locator('tr', { hasText: NODE_ID }).first();
}

test.describe.serial('Media server page (/mediaServer)', () => {
  test('检测：探测的是被点击的节点，且成功后给出明确提示', async ({ page }) => {
    await gotoMediaServer(page);
    const row = nodeRow(page);
    await expect(row).toBeVisible({ timeout: 10_000 });

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/media_server/check'), { timeout: 15_000 }),
      row.getByRole('button', { name: '检测' }).click()
    ]);
    // 后端按 id 查到节点真实 ip/端口/secret 再探测
    expect(resp.url()).toContain(`id=${NODE_ID}`);
    const body = (await resp.json()) as { code: number; data?: { reachable?: boolean } };
    expect(body.code).toBe(0);
    expect(body.data?.reachable).toBe(true);
    // 此前无论成功与否都弹「检测失败」
    await expect(page.locator('.el-message').last()).toContainText('连通正常', {
      timeout: 10_000
    });
  });

  test('启用开关真的落库（停用后节点不再出现在可选节点里）', async ({ page }) => {
    await gotoMediaServer(page);
    const row = nodeRow(page);
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText('启用中');

    // 停用
    await row.getByRole('button', { name: '编辑' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    const enableItem = dialog.locator('.el-form-item', { hasText: '启用' }).first();
    await enableItem.locator('.el-switch').click();
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/media_server/save'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);

    await expect(nodeRow(page)).toContainText('已停用', { timeout: 10_000 });

    // 停用后 online/list（拉流代理 / 录像计划的节点下拉、负载均衡都用它）不再包含该节点
    const online = await page.request.get('/dev-api/server/media_server/online/list', {
      headers: await authHeaders(page)
    });
    const onlineBody = (await online.json()) as { data?: Array<{ id: string }> };
    expect((onlineBody.data ?? []).some((m) => m.id === NODE_ID)).toBe(false);
  });

  test('重新启用，恢复节点可用性（收尾）', async ({ page }) => {
    await gotoMediaServer(page);
    const row = nodeRow(page);
    await expect(row).toBeVisible({ timeout: 10_000 });

    await row.getByRole('button', { name: '编辑' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    const enableItem = dialog.locator('.el-form-item', { hasText: '启用' }).first();
    await enableItem.locator('.el-switch').click();
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/media_server/save'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);
    await expect(nodeRow(page)).toContainText('启用中', { timeout: 10_000 });

    const online = await page.request.get('/dev-api/server/media_server/online/list', {
      headers: await authHeaders(page)
    });
    const onlineBody = (await online.json()) as { data?: Array<{ id: string }> };
    expect((onlineBody.data ?? []).some((m) => m.id === NODE_ID)).toBe(true);
  });
});

/** 前端的 token 存在 cookie `gbserver_token`，axios 把它放进 `access-token` 头 */
async function authHeaders(page: Page): Promise<Record<string, string>> {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
  return token ? { 'access-token': token } : {};
}
