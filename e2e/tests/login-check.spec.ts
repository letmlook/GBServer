import { test, expect } from '@playwright/test';

test('登录流程是否可用（前端 GET vs 后端 POST）', async ({ page }) => {
  const calls: string[] = [];
  page.on('response', (r) => {
    if (r.url().includes('/api/user/login')) calls.push(`${r.request().method()} ${r.status()} ${r.url()}`);
  });
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.context().clearCookies();
  await page.goto('/#/login', { waitUntil: 'domcontentloaded' });
  await page.waitForTimeout(800);
  const inputs = page.locator('input');
  await inputs.nth(0).fill('admin');
  await inputs.nth(1).fill('admin');
  await page.getByRole('button', { name: /登\s*录/ }).click();
  await page.waitForTimeout(3000);
  console.log('\n登录请求:', calls);
  console.log('当前 URL:', page.url());
  const body = (await page.locator('body').innerText()) || '';
  console.log('是否进入控制台:', body.includes('控制台') || body.includes('实时直播'));
  console.log('是否仍在登录页:', body.includes('欢迎回来'));
  const msg = await page.locator('.el-message').last().textContent().catch(() => null);
  console.log('提示消息:', JSON.stringify(msg));
  await page.screenshot({ path: 'artifacts/login-check.png', fullPage: true });
});
