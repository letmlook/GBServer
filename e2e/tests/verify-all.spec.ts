import { test, expect } from '@playwright/test';

test('管理员：用户管理页完整可用', async ({ page }) => {
  const errors: string[] = [];
  const bad: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('response', (r) => {
    if (r.url().includes('/api/') && r.status() >= 400) bad.push(`${r.status()} ${r.url().replace(/^https?:\/\/[^/]+/, '')}`);
  });

  await page.goto('/#/user', { waitUntil: 'domcontentloaded' });
  await page.locator('.el-table__header th', { hasText: '用户名' }).waitFor({ timeout: 12000 });
  await page.waitForTimeout(1200);

  const rows = page.locator('.el-table__body tbody tr');
  const n = await rows.count();
  console.log('\n[管理员] 行数:', n, '| 搜索框:', await page.locator('.search input').count() > 0);
  for (let i = 0; i < n; i++) {
    const name = ((await rows.nth(i).locator('td').nth(1).textContent()) || '').trim();
    const btns = (await rows.nth(i).locator('button').allTextContents()).map((b) => b.trim()).filter(Boolean);
    console.log(`  ${name.padEnd(8)} -> ${JSON.stringify(btns)}`);
  }
  console.log('  4xx/5xx:', bad.length ? bad : '无');
  console.log('  控制台错误:', errors.length ? errors.slice(0, 3) : '无');

  // 搜索实测
  await page.locator('.search input').fill('viewer');
  await page.getByRole('button', { name: '搜索' }).click();
  await page.waitForTimeout(1200);
  const searched = await page.locator('.el-table__body tbody tr').count();
  console.log('  搜索 viewer 后行数:', searched);
  await page.screenshot({ path: 'artifacts/verify-admin.png', fullPage: true });
  expect(n).toBeGreaterThan(0);
});
