/**
 * JT/T 1078 页（/#/jtDevice）端到端契约测试。
 *
 * 覆盖的缺陷（2026-09-12 第 43 轮修复）：
 *   1. 「新增终端」此前**必然 422**：前端表单里车牌颜色是 `el-select` 的
 *      **数字**（0..4）、省域/市域是 `el-input` 的**字符串**，而后端 DTO 恰好相反
 *      （颜色要 String、省域要 i32）→
 *      `invalid type: integer 0, expected a string`。UI 上表现为"保存失败"。
 *   2. postgres 下「车牌颜色」列是 varchar 而 Rust 读 i32 → 只要有非空值，
 *      `/api/jt1078/terminal/list` 整表 500。这里顺带断言列表能读出来。
 *   3. 圆形/多边形/路线围栏的读写接口在 postgres 下用了 `?` 占位符 →
 *      `syntax error at or near ","`，页面「新增」同样失败。
 */

import { test, expect, type Page } from '@playwright/test';

const PHONE = `139${Date.now().toString().slice(-8)}`;
const PLATE = `E2E-${Date.now().toString().slice(-5)}`;
const PLATE2 = `${PLATE}-X`;

async function gotoJt(page: Page) {
  await page.goto('/#/jtDevice', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe.serial('JT1078 page (/jtDevice)', () => {
  test('新增终端：车牌颜色(数字)/省域编码(文本) 与后端 DTO 对齐并真的落库', async ({ page }) => {
    await gotoJt(page);

    // 锁定请求体形状：车牌颜色必须是数字，省域/市域编码必须是字符串
    // 注意：dev 下 axios baseURL 是 `/dev-api`（vite 代理到后端 :18080），
    // 所以这里匹配的是 `/dev-api/jt1078/terminal/add`，不能写死 `/api/...`。
    const addReq = page.waitForRequest(
      (r) => r.url().includes('/jt1078/terminal/add') && r.method() === 'POST'
    );
    await page.getByRole('button', { name: '新增终端' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByLabel('手机号').fill(PHONE);
    await dialog.getByLabel('车牌号').fill(PLATE);
    // 车牌颜色：默认「蓝」= 0（数字）
    await expect(dialog.locator('.el-select')).toHaveCount(1);
    await dialog.getByLabel('省域编码').fill('340200');
    await dialog.getByLabel('市域编码').fill('340201');
    await dialog.getByRole('button', { name: '保存' }).click();

    const req = await addReq;
    const body = req.postDataJSON() as Record<string, unknown>;
    expect(body.phoneNumber).toBe(PHONE);
    expect(typeof body.plateColor, 'plateColor 必须是数字（el-select 的值）').toBe('number');
    expect(body.plateColor).toBe(0);
    expect(typeof body.provinceId, 'provinceId 必须是字符串（el-input 的值）').toBe('string');
    expect(body.cityId).toBe('340201');

    // 保存成功后列表出现该行（能出现即说明后端解码 plate_color 没炸）
    const row = page.locator('tr', { hasText: PHONE }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText(PLATE);
  });

  test('编辑终端：改车牌号后列表刷新', async ({ page }) => {
    await gotoJt(page);
    const row = page.locator('tr', { hasText: PHONE }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await row.getByRole('button', { name: '编辑' }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByLabel('车牌号').fill(PLATE2);
    await dialog.getByRole('button', { name: '保存' }).click();

    const updated = page.locator('tr', { hasText: PHONE }).first();
    await expect(updated).toContainText(PLATE2, { timeout: 10_000 });
  });

  test('圆形区域：新增（围栏接口在 postgres 下曾因 ? 占位符 500）+ 删除', async ({ page }) => {
    await gotoJt(page);
    await page.getByRole('tab', { name: '圆形区域' }).click();
    // Element Plus 的 tab-pane 有稳定 id：`#pane-<name>`；几个围栏页的
    // 筛选框 placeholder 相同（"筛选 phone"），必须限定在激活的 pane 内。
    const pane = page.locator('#pane-circle');
    await pane.getByPlaceholder('筛选 phone').fill(PHONE);
    await pane.getByRole('button', { name: '新增' }).click();

    const prompt = page.getByRole('dialog').last();
    await prompt.locator('input').first().fill(PHONE);
    await prompt.getByRole('button', { name: '确定' }).click();

    // 必须限定在圆形区域 pane 内：终端列表里也有同一个手机号的行，
    // 而它在非激活 pane 里（display:none），`.first()` 会命中不可见的那一行。
    const row = pane.locator('tr', { hasText: PHONE }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText('未命名');

    await row.getByRole('button', { name: '删除' }).click();
    await expect(pane.locator('tr', { hasText: PHONE })).toHaveCount(0, { timeout: 10_000 });
  });

  test('删除终端（清理）', async ({ page }) => {
    await gotoJt(page);
    const pane = page.locator('#pane-terminal');
    const row = pane.locator('tr', { hasText: PHONE }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await row.getByRole('button', { name: '删除' }).click();
    const confirm = page.getByRole('dialog').last();
    await confirm.getByRole('button', { name: '确定' }).click();
    await expect(pane.locator('tr', { hasText: PHONE })).toHaveCount(0, { timeout: 10_000 });
  });
});
