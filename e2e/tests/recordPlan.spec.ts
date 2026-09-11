/**
 * 录像计划页（/#/recordPlan）端到端测试。
 *
 * 这一页此前**完全是坏的**，而且坏得毫无提示：
 *
 *   1. 前端把计划存成 `startTime/endTime/mon..sun`，后端只认 `planItemList`
 *      （WVP 契约）→ "新增计划"返回成功，但库里一条时段都没有，计划永不录像；
 *   2. 列表渲染 `planType/startTime/endTime` 等后端从不返回的字段 → 整列空白；
 *   3. 删除用 HTTP GET 调 `/api/record/plan/delete`，而后端只注册了 DELETE
 *      → 稳定 405，删除功能不可用。
 *
 * 所以这里必须测"真实数据往返"，而不是"页面能渲染"：
 *   - 断言**发出的请求体**是 WVP 契约（`planItemList` + 分钟 + ISO 星期）；
 *   - 断言列表能读回时段（说明后端确实存下来了）；
 *   - 断言删除真的成功（走 DELETE）。
 */

import { test, expect, type Page } from '@playwright/test';

const PLAN_NAME = `E2E录像计划_${Date.now()}`;
const PLAN_NAME_EDITED = `${PLAN_NAME}_改`;

async function gotoRecordPlan(page: Page) {
  // hash 路由：直接 goto('/recordPlan') 会被解析成无 hash 而重定向到 dashboard
  await page.goto('/#/recordPlan', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe('Record plan page (/recordPlan)', () => {
  test('add → list → edit → delete 全链路使用 WVP 契约', async ({ page }) => {
    await gotoRecordPlan(page);
    await expect(page.getByRole('button', { name: '新增计划' })).toBeVisible();

    // ---------- 新增 ----------
    await page.getByRole('button', { name: '新增计划' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    // 新增时默认 7 天全天，共 7 条时段
    await expect(dialog.getByText('周三')).toBeVisible();
    await dialog.getByPlaceholder('例如：机房全天录像').fill(PLAN_NAME);

    // 捕获真实请求体：必须是 planItemList（不是 startTime/endTime/mon..sun）
    const [addReq] = await Promise.all([
      page.waitForRequest((r) => r.url().includes('/record/plan/add') && r.method() === 'POST'),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    const body = addReq.postDataJSON() as {
      name: string;
      planItemList: { start: number; stop: number; weekDay: number }[];
    };
    expect(body.name).toBe(PLAN_NAME);
    expect(Array.isArray(body.planItemList)).toBe(true);
    expect(body.planItemList.length).toBe(7);
    // 分钟口径（0..1440）+ ISO 星期（1=周一）
    const days = body.planItemList.map((i) => i.weekDay).sort((a, b) => a - b);
    expect(days).toEqual([1, 2, 3, 4, 5, 6, 7]);
    for (const item of body.planItemList) {
      expect(item.start).toBe(0);
      expect(item.stop).toBe(1440);
    }

    // ---------- 列表能读回时段 ----------
    const row = page.locator('tr', { hasText: PLAN_NAME }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row.getByText('周一 00:00-24:00')).toBeVisible();
    await expect(row.getByText('周日 00:00-24:00')).toBeVisible();

    // ---------- 编辑 ----------
    await row.getByRole('button', { name: '编辑' }).click();
    const editDialog = page.getByRole('dialog');
    await expect(editDialog).toBeVisible();
    const nameInput = editDialog.getByPlaceholder('例如：机房全天录像');
    await expect(nameInput).toHaveValue(PLAN_NAME);
    await nameInput.fill(PLAN_NAME_EDITED);
    await editDialog.getByRole('button', { name: '保存' }).click();
    await expect(page.locator('tr', { hasText: PLAN_NAME_EDITED }).first()).toBeVisible({
      timeout: 10_000
    });

    // ---------- 删除（必须走 DELETE；旧实现用 GET 会 405） ----------
    const editedRow = page.locator('tr', { hasText: PLAN_NAME_EDITED }).first();
    let deleteMethod = '';
    page.on('request', (r) => {
      if (r.url().includes('/record/plan/delete')) deleteMethod = r.method();
    });
    await editedRow.getByRole('button', { name: '删除' }).click();
    await page.getByRole('button', { name: '确定' }).click();
    await expect(page.locator('tr', { hasText: PLAN_NAME_EDITED })).toHaveCount(0, {
      timeout: 10_000
    });
    expect(deleteMethod).toBe('DELETE');
  });

  test('保存非法时段被拦截（不产生"永不触发的计划"）', async ({ page }) => {
    await gotoRecordPlan(page);
    await page.getByRole('button', { name: '新增计划' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByPlaceholder('例如：机房全天录像').fill(`${PLAN_NAME}_非法`);

    // 周一清空后不加任何时段 → 全局只剩其它 6 天，仍然合法；
    // 因此先全部清空，触发"至少需要安排一个录像时段"。
    await dialog.getByRole('button', { name: '全部清空' }).click();
    await dialog.getByRole('button', { name: '保存' }).click();
    // ElMessage 渲染在 body 上的 toast 容器里，不在 dialog 内部
    await expect(page.getByText('至少需要安排一个录像时段')).toBeVisible();
    // 对话框保持打开，未被误当成保存成功
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: '取消' }).click();
  });
});
