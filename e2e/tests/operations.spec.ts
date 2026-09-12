/**
 * 系统信息 / 历史日志（运维中心）契约端到端测试。
 *
 * 这一组页面此前全部读不到值：
 *   * 「系统信息」页 CPU 卡片把**历史采样数组** `cpu` 当数字渲染
 *     → 显示成 "[object Object]"，进度条 percentage 收到数组而失效；
 *   * 内存卡片读 `memory.total/used`，而后端根本没有 `memory` 键 → 恒 0%；
 *   * 磁盘卡片读 `disk[].total/used`，后端只有 `use`(GB) → 恒 0%；
 *   * 版本/构建时间恒 `-`；资源统计三行恒 0；
 *   * 「历史日志」页「导出」发 `?format=csv`，后端把 `format` 静默丢弃、
 *     返回 JSON，前端却把它存成 `.csv` —— 用户拿到一个内容是 JSON 的 CSV。
 */

import { test, expect, type Page } from '@playwright/test';

async function authHeaders(page: Page): Promise<Record<string, string>> {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
  return token ? { 'access-token': token } : {};
}

test.describe('Operations pages', () => {
  test('系统信息页显示真实的 CPU/内存/磁盘/版本/资源统计', async ({ page }) => {
    await page.goto('/#/operations/systemInfo', { waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});

    const cpuCard = page.locator('.metric-card', { hasText: 'CPU' }).first();
    await expect(cpuCard).toBeVisible({ timeout: 10_000 });
    // 此前显示 "[object Object],..."
    await expect(cpuCard).not.toContainText('[object Object]');
    await expect(cpuCard.locator('.metric-value')).toHaveText(/^\d+%$/);

    const memCard = page.locator('.metric-card', { hasText: '内存' }).first();
    await expect(memCard.locator('.metric-value')).toHaveText(/^\d+%$/);
    // 明细行是 "used / total"，两者都必须是已格式化的容量而不是 '-'
    await expect(memCard.locator('.metric-detail')).not.toContainText('- / -');

    const diskCard = page.locator('.metric-card', { hasText: '磁盘' }).first();
    await expect(diskCard.locator('.metric-detail')).not.toContainText('- / -');

    // 版本 / 构建时间不再恒为 '-'，副标题不再是「加载中...」
    await expect(page.locator('.page-subtitle')).not.toContainText('加载中');
    const build = page.locator('.el-descriptions').first();
    await expect(build).toContainText('版本');
    await expect(build).not.toContainText('构建时间\n-');

    // 资源统计：设备总数/在线（本环境至少有 2 个设备）
    const rows = page.locator('.el-table__row');
    await expect(rows.first()).toBeVisible();
    await expect(page.locator('.el-table')).toContainText('/');
  });

  test('历史日志导出：?format=csv 返回真正的 CSV 附件', async ({ page }) => {
    await page.goto('/#/operations/historyLog', { waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});

    const headers = await authHeaders(page);
    const resp = await page.request.get('/dev-api/log/list?format=csv&level=INFO', { headers });
    expect(resp.status()).toBe(200);
    expect(resp.headers()['content-type']).toContain('text/csv');
    expect(resp.headers()['content-disposition'] ?? '').toContain('gbserver-log-');
    const text = await resp.text();
    // UTF-8 BOM + 表头（Excel 打开不乱码）
    expect(text.startsWith('\uFEFF')).toBe(true);
    expect(text).toContain('id,time,level,logger,thread,source,message');

    // 不带 format 时仍是 JSON 分页契约
    const jsonResp = await page.request.get('/dev-api/log/list?page=1&count=5', { headers });
    expect(jsonResp.headers()['content-type']).toContain('application/json');
    const body = (await jsonResp.json()) as { code: number; data: { list: unknown[] } };
    expect(body.code).toBe(0);
    expect(Array.isArray(body.data.list)).toBe(true);
  });
});
