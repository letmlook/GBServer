/**
 * 通道页（/#/channel）契约端到端测试。
 *
 * 这一页此前「新增通道」是 100% 失败的：
 *   1. 前端发 camelCase（deviceId/channelId/...），后端 DTO 只有 snake_case
 *      且没有任何 `#[serde(alias)]` → device_id/channel_id 为空 → 400「必填」；
 *   2. 后端 INSERT 里有 `custom_name` 列，而 schema 里根本没有该列
 *      （只在运行时 ALTER 过）→ 即使字段绑上了也会 500；
 *   3. 编辑时 manufacturer/streamIdentification/channelType/address 四列
 *      后端 DTO 里不存在，改完静默丢失；
 *   4. 行业/类型/网络标识三个下拉的接口返回 `{name, code}` 对象，
 *      前端当 string[] 用 → 选项显示 "[object Object]"。
 *
 * 所以这里必须走真实写库 + 读回，而不是只看"保存成功"的提示。
 */

import { test, expect, type Page } from '@playwright/test';

const stamp = Date.now();
const CHANNEL_ID = `3402000000131${String(stamp).slice(-7)}`;
const DEVICE_ID = `3402000000132${String(stamp).slice(-7)}`;
const NAME = `E2E通道_${stamp}`;

async function gotoChannel(page: Page) {
  await page.goto('/#/channel', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe.serial('Channel page (/channel)', () => {
  test('新增通道：camelCase 字段与下拉码值都能落库', async ({ page }) => {
    await gotoChannel(page);
    await page.getByRole('button', { name: '新增通道' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByPlaceholder('20 位国标ID').fill(CHANNEL_ID);
    await dialog.getByPlaceholder('父设备国标ID').fill(DEVICE_ID);
    await dialog.getByPlaceholder('通道名称').fill(NAME);
    await dialog.getByPlaceholder('6 位行政区划码').fill('340200');

    // 行业下拉：必须是可读文本，而不是 "[object Object]"
    // el-select 的 placeholder 不是 input 的 placeholder 属性，按顺序取表单里的下拉
    const selects = dialog.locator('.el-select')
    await selects.nth(0).click();
    const industryOption = page.locator('.el-select-dropdown__item', { hasText: '煤矿' }).first();
    await expect(industryOption).toBeVisible();
    await expect(page.locator('.el-select-dropdown__item', { hasText: '[object Object]' })).toHaveCount(0);
    await industryOption.click();

    // 类型下拉（表单里第 3 个 select：行业 / 网络标识 / 类型）
    await selects.nth(2).click();
    const typeOption = page.locator('.el-select-dropdown__item', { hasText: '快球' }).first();
    await expect(typeOption).toBeVisible();
    await typeOption.click();

    // 捕获真实请求体：必须是 camelCase（后端靠 serde alias 绑定）
    const [req] = await Promise.all([
      page.waitForRequest((r) => r.url().includes('/common/channel/add') && r.method() === 'POST'),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    const body = req.postDataJSON() as Record<string, unknown>;
    expect(body.deviceId).toBe(DEVICE_ID);
    expect(body.channelId).toBe(CHANNEL_ID);
    expect(body.manufacturer).toBe('02'); // 煤矿
    expect(body.channelType).toBe(3); // 快球

    // 列表里应能查到（说明真的写库了）
    await page.getByPlaceholder('国标ID / 名称').fill(NAME);
    await page.getByRole('button', { name: '查询' }).click();
    const row = page.locator('tr', { hasText: NAME }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });
    await expect(row).toContainText(CHANNEL_ID);
  });

  test('编辑通道：只改名称不会清空其它字段', async ({ page }) => {
    await gotoChannel(page);
    await page.getByPlaceholder('国标ID / 名称').fill(NAME);
    await page.getByRole('button', { name: '查询' }).click();
    const row = page.locator('tr', { hasText: NAME }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    await row.getByRole('button', { name: '编辑' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    const nameInput = dialog.getByPlaceholder('通道名称');
    await expect(nameInput).toHaveValue(NAME);
    const newName = `${NAME}_改`;
    await nameInput.fill(newName);
    await dialog.getByRole('button', { name: '保存' }).click();

    await page.getByPlaceholder('国标ID / 名称').fill(newName);
    await page.getByRole('button', { name: '查询' }).click();
    const edited = page.locator('tr', { hasText: newName }).first();
    await expect(edited).toBeVisible({ timeout: 10_000 });
    // 行业列（manufacturer）必须还在，没有被 COALESCE 之外的空值清掉
    await expect(edited).toContainText('02');

    // ---- 清理：本用例造的通道不能留在共享开发库里 ----
    // 否则它会被直播页的通道树选中，而 SIP mock 并不认识这个通道，
    // 导致 live.spec.ts 的起流用例失败（跨用例污染）。
    await edited.getByRole('button', { name: '删除' }).click();
    await page.getByRole('button', { name: '确定' }).click();
    await expect(page.locator('tr', { hasText: newName })).toHaveCount(0, { timeout: 10_000 });
  });
});
