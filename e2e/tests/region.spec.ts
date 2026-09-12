/**
 * 行政区划 / 业务分组页（/#/region）契约端到端测试。
 *
 * 这是本轮**新补的一个页面**：此前 `web/src/api/region.ts` 里 12 个函数只有
 * `getRegionTreeList` 有调用方（地图页），区域/分组的增删改在界面上完全不可达。
 *
 * 同时修掉的三处契约错误也在本 spec 覆盖：
 *   - `deleteRegion` / `deleteGroup` 用 GET，后端只注册 DELETE → 405（用响应断言）；
 *   - `tree/query` 的前端返回类型声明成数组，实际是 `{total, list}`；
 *   - `RegionUpdate`/`GroupUpdate` 收不到 camelCase → 只改名字会把节点抬到根级。
 */

import { test, expect, type Page } from '@playwright/test';

const STAMP = String(Date.now()).slice(-10);
const REGION_DEV = `3402000000${STAMP}`;
const CHILD_DEV = `3402000001${STAMP}`;
const GROUP_DEV = `3402000002${STAMP}`;
const REGION_NAME = `e2e省-${STAMP}`;
const CHILD_NAME = `e2e市-${STAMP}`;
const GROUP_NAME = `e2e组-${STAMP}`;

async function gotoRegion(page: Page) {
  await page.goto('/#/region', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

/** 打开新增/编辑弹窗并填写（kind 决定占位符文案） */
async function fillDialog(
  page: Page,
  opts: { name: string; deviceId: string; kind: 'region' | 'group' }
) {
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  const devPlaceholder = opts.kind === 'group' ? '分组国标编码（20 位）' : '行政区划国标编码（20 位）';
  await dialog.getByPlaceholder(devPlaceholder).fill(opts.deviceId);
  await dialog.getByPlaceholder('节点名称').fill(opts.name);
  const [resp] = await Promise.all([
    page.waitForResponse(
      (r) =>
        (r.url().includes(`/${opts.kind}/add`) || r.url().includes(`/${opts.kind}/update`)) &&
        r.request().method() === 'POST',
      { timeout: 10_000 }
    ),
    dialog.getByRole('button', { name: '保存' }).click()
  ]);
  expect(((await resp.json()) as { code: number }).code).toBe(0);
  await expect(dialog).toBeHidden({ timeout: 10_000 });
}

function nodeRow(page: Page, name: string) {
  return page.locator('.el-tree-node__content', { hasText: name }).first();
}

test.describe.serial('Region / group page (/region)', () => {
  test('新增顶级行政区划 + 其子节点', async ({ page }) => {
    await gotoRegion(page);
    await expect(page.locator('#tab-region')).toBeVisible();

    await page.getByRole('button', { name: '新增', exact: true }).click();
    await fillDialog(page, { name: REGION_NAME, deviceId: REGION_DEV, kind: 'region' });
    await expect(nodeRow(page, REGION_NAME)).toBeVisible({ timeout: 10_000 });

    // 在该节点下新增子节点（上级由 defaultParentId 带出）
    await nodeRow(page, REGION_NAME).getByRole('button', { name: '新增子节点' }).click();
    await fillDialog(page, { name: CHILD_NAME, deviceId: CHILD_DEV, kind: 'region' });
    await expect(nodeRow(page, CHILD_NAME)).toBeVisible({ timeout: 10_000 });
    // 子节点必须嵌在父节点之下
    const parentNode = page.locator('.el-tree-node', { hasText: REGION_NAME }).first();
    await expect(parentNode).toContainText(CHILD_NAME);
  });

  test('只改名字不会把子节点搬到根级（parentId 保留）', async ({ page }) => {
    await gotoRegion(page);
    await nodeRow(page, CHILD_NAME).getByRole('button', { name: '编辑' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    const renamed = `${CHILD_NAME}-改`;
    await dialog.getByPlaceholder('节点名称').fill(renamed);
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/region/update'), { timeout: 10_000 }),
      dialog.getByRole('button', { name: '保存' }).click()
    ]);
    expect(((await resp.json()) as { code: number }).code).toBe(0);

    const parentNode = page.locator('.el-tree-node', { hasText: REGION_NAME }).first();
    await expect(parentNode).toContainText(renamed, { timeout: 10_000 });
    // 若是被搬到根级，父节点下就不会再有它
    await expect(parentNode).toContainText(renamed);
    await expect(nodeRow(page, `${CHILD_NAME}-改`)).toBeVisible();
  });

  test('搜索按名称过滤', async ({ page }) => {
    await gotoRegion(page);
    const search = page.locator('#pane-region').getByPlaceholder('按名称 / 国标编码搜索');
    await search.fill(REGION_NAME);
    // el-tree 的 filter 是本地过滤，直接断言可见性
    await expect(nodeRow(page, REGION_NAME)).toBeVisible();
    await search.fill('不存在的名字-zzz');
    await expect(nodeRow(page, REGION_NAME)).toBeHidden({ timeout: 5_000 });
  });

  test('删除子节点走 DELETE（此前 GET → 405）', async ({ page }) => {
    await gotoRegion(page);
    const row = nodeRow(page, `${CHILD_NAME}-改`);
    await expect(row).toBeVisible({ timeout: 10_000 });

    const [resp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/region/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await row.getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    expect(resp.status()).toBe(200);
    await expect(nodeRow(page, `${CHILD_NAME}-改`)).toHaveCount(0, { timeout: 10_000 });
  });

  test('业务分组：新增后可见，删除走 DELETE', async ({ page }) => {
    await gotoRegion(page);
    await page.locator('#tab-group').click();

    await page.getByRole('button', { name: '新增', exact: true }).click();
    await fillDialog(page, { name: GROUP_NAME, deviceId: GROUP_DEV, kind: 'group' });
    await expect(nodeRow(page, GROUP_NAME)).toBeVisible({ timeout: 10_000 });

    const [resp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/group/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await nodeRow(page, GROUP_NAME).getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    expect(resp.status()).toBe(200);
    await expect(nodeRow(page, GROUP_NAME)).toHaveCount(0, { timeout: 10_000 });

    // 清理：删掉本 spec 建的顶级区域（自清理，保持共享开发库可用）
    await page.locator('#tab-region').click();
    const rootRow = nodeRow(page, REGION_NAME);
    await expect(rootRow).toBeVisible({ timeout: 10_000 });
    await rootRow.getByRole('button', { name: '删除' }).click();
    await page.locator('.el-message-box').getByRole('button', { name: '确定' }).click();
    await expect(nodeRow(page, REGION_NAME)).toHaveCount(0, { timeout: 10_000 });
  });
});
