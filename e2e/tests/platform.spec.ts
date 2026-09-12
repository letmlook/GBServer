/**
 * 级联平台页（/#/platform）契约端到端测试。
 *
 * 这一页此前有三个"用户看得见"的硬伤：
 *   1. 前端字段名写成 `serverGbId`（小写 b），而 WVP 与后端都是 `serverGBId`
 *      → 新增平台时国标ID 绑不上、库里写空串（接口还回"成功"，平台实际不可用）；
 *      列表"国标ID"列空白；「注销」按钮被 `if (!row.serverGbId) return` 静默拦掉。
 *   2. `expires` 用 el-input-number 提交 JSON 数字，而后端 DTO 只认字符串
 *      → 反序列化阶段 422，**新增平台直接报错**。
 *   3. 「域名」写的是 `realm`（后端字段是 `serverGBDomain`）、
 *      「注册间隔/心跳间隔/心跳次数」三个输入框是凭空发明的字段（WVP 与数据库
 *      都没有）→ 填了静默丢弃，重新打开又变回默认值。
 */

import { test, expect, type Page } from '@playwright/test';

// 20 位国标ID：3402000000 + 时间戳后 10 位，保证多次运行不撞
const GB_ID = `3402000000${String(Date.now()).slice(-10)}`;
const DOMAIN = '3402000199';
const NAME = `e2e平台-${Date.now()}`;

async function gotoPlatform(page: Page) {
  await page.goto('/#/platform', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe.serial('Platform page (/platform)', () => {
  test('新增平台：国标ID/国标域/注册周期/心跳周期都真实落库并回显', async ({ page }) => {
    await gotoPlatform(page);

    await page.getByRole('button', { name: '新增平台' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByPlaceholder('上级平台名称').fill(NAME);
    await dialog.getByPlaceholder('20 位国标ID').fill(GB_ID);
    await dialog.getByPlaceholder('如 3402000000（10 位域编码）').fill(DOMAIN);
    await dialog.getByPlaceholder('上级平台 SIP IP').fill('10.99.0.1');

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/platform/add'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    const body = (await resp.json()) as { code: number };
    expect(body.code, '新增平台不能 422/400').toBe(0);

    const row = page.locator('tr', { hasText: GB_ID }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    // 国标ID 必须显示出来（此前读 row.serverGbId → 恒空）
    await expect(row).toContainText(GB_ID);
    await expect(row).toContainText(DOMAIN);
    // 注册周期/心跳周期来自真实的 expires / keepTimeout
    await expect(row).toContainText('3600 s');
    await expect(row).toContainText('60 s');
    await expect(row).toContainText('已启用');
  });

  test('编辑：心跳周期改动会落库', async ({ page }) => {
    await gotoPlatform(page);
    const row = page.locator('tr', { hasText: GB_ID }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await row.getByRole('button', { name: '编辑' }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // 心跳周期用了 el-input-number，没有 placeholder，按 label 定位
    const hbItem = dialog.locator('.el-form-item', { hasText: '心跳周期' }).first();
    await hbItem.locator('input').fill('45');
    await hbItem.locator('input').blur();

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/platform/update'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);

    await expect(page.locator('tr', { hasText: GB_ID }).first()).toContainText('45 s', {
      timeout: 10_000
    });
  });

  test('注销：按 serverGBId 定位并真的置为停用（此前静默 return）', async ({ page }) => {
    await gotoPlatform(page);
    const row = page.locator('tr', { hasText: GB_ID }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText('已启用');

    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/platform/exit/'), { timeout: 10_000 }),
      (async () => {
        await row.getByRole('button', { name: '注销' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    const body = (await resp.json()) as { code: number; data: { exited?: boolean } };
    expect(body.code).toBe(0);
    expect(body.data.exited).toBe(true);

    const after = page.locator('tr', { hasText: GB_ID }).first();
    await expect(after).toContainText('未启用', { timeout: 10_000 });
    await expect(after).toContainText('离线');
  });

  test('删除平台（DELETE /platform/delete）', async ({ page }) => {
    await gotoPlatform(page);
    const row = page.locator('tr', { hasText: GB_ID }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    const [resp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/platform/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await row.getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);
    await expect(page.locator('tr', { hasText: GB_ID })).toHaveCount(0, { timeout: 10_000 });
  });
});
