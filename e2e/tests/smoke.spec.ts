/**
 * GBServer admin SPA smoke test (Vue 3 + Element Plus 前端版本)。
 *
 * 与 web-legacy-vue2 的 smoke.spec.ts 不同：本文件选择器全部对齐
 * web/src/views/（Vue 3）当前实现。
 *
 * 覆盖流程：
 *   1. 后端健康探测（/api/health）
 *   2. 登录页元素渲染
 *   3. admin/admin 登录 → dashboard
 *   4. 17 个业务路由逐页加载（截图到 artifacts/）
 *
 * 鉴权状态通过 storageState 持久化到 artifacts/.auth.json，
 * 后续每页测试读取该状态，避免重复登录。
 */

import { test, expect, type ConsoleMessage, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ARTIFACT_DIR = path.resolve(__dirname, '../artifacts');
fs.mkdirSync(ARTIFACT_DIR, { recursive: true });

// 与 web/src/router/index.ts 实际注册路径保持一致
// （不含 :id 动态路由与 404）
const ADMIN_PAGES: ReadonlyArray<{ name: string; path: string }> = [
  { name: 'dashboard',    path: '/dashboard' },
  { name: 'device',       path: '/device' },
  { name: 'channel',      path: '/channel' },
  { name: 'live',         path: '/live' },
  { name: 'playback',     path: '/playback' },
  { name: 'cloudRecord',  path: '/cloudRecord' },
  { name: 'mediaServer',  path: '/mediaServer' },
  { name: 'recordPlan',   path: '/recordPlan' },
  { name: 'platform',     path: '/platform' },
  { name: 'streamProxy',  path: '/streamProxy' },
  { name: 'streamPush',   path: '/streamPush' },
  { name: 'map',          path: '/map' },
  { name: 'alarm',        path: '/alarm' },
  { name: 'jtDevice',     path: '/jtDevice' },
  { name: 'user',         path: '/user' },
  { name: 'operations',   path: '/operations' },
];

// Vue 3 新登录页 placeholder（见 web/src/views/login/index.vue）
const LOGIN_USERNAME_PLACEHOLDER = '用户名 / SIP 编号';
const LOGIN_PASSWORD_PLACEHOLDER = '登录密码';
const LOGIN_BUTTON_TEXT = '登 录';

const ADMIN = { username: 'admin', password: 'admin' };
const AUTH_FILE = path.resolve(__dirname, '../artifacts/.auth.json');

test.describe.configure({ mode: 'serial' });

test.describe('GBServer admin UI smoke (Vue 3)', () => {
  test('backend health probe', async ({ request }) => {
    const r = await request.get('http://127.0.0.1:18080/api/health');
    expect(r.status(), 'backend /api/health').toBe(200);
  });

  test('login page renders', async ({ browser }) => {
    // 项目级 storageState 是已登录态，而已登录访问 /login 会被守卫重定向到 /，
    // 因此这里显式开一个空状态的 context。
    const context = await browser.newContext({ storageState: { cookies: [], origins: [] } });
    const page = await context.newPage();
    await page.goto('/login');
    await expect(page.locator(`input[placeholder="${LOGIN_USERNAME_PLACEHOLDER}"]`)).toBeVisible();
    await expect(page.locator(`input[placeholder="${LOGIN_PASSWORD_PLACEHOLDER}"]`)).toBeVisible();
    await expect(page.getByRole('button', { name: LOGIN_BUTTON_TEXT })).toBeVisible();
    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'login.png'), fullPage: true });
    await context.close();
  });

  test('admin login → dashboard', async ({ browser }) => {
    const context = await browser.newContext({ storageState: { cookies: [], origins: [] } });
    const page = await context.newPage();
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
    page.on('console', (m: ConsoleMessage) => {
      if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
    });

    await page.goto('/login');
    await page.locator(`input[placeholder="${LOGIN_USERNAME_PLACEHOLDER}"]`).fill(ADMIN.username);
    await page.locator(`input[placeholder="${LOGIN_PASSWORD_PLACEHOLDER}"]`).fill(ADMIN.password);
    await page.getByRole('button', { name: LOGIN_BUTTON_TEXT }).click();

    await page.waitForURL(/\/(dashboard)?$/, { timeout: 15_000 });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    await expect(page).toHaveURL(/\/(dashboard)?$/);

    // 鉴权状态已由 globalSetup 统一落盘（e2e/global-setup.ts），此处不再重复写
    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'dashboard-after-login.png'), fullPage: true });

    const real = errors.filter(
      (e) => !/favicon|net::ERR_ABORTED|Loading chunk \d+ failed/i.test(e),
    );
    if (real.length > 0) {
      console.warn(`[dashboard] ${real.length} non-fatal console issue(s):`);
      for (const r of real.slice(0, 3)) console.warn(`  - ${r.slice(0, 200)}`);
    }
    await context.close();
  });

  for (const p of ADMIN_PAGES) {
    test(`page renders: ${p.name} (${p.path})`, async ({ browser }) => {
      const context = await browser.newContext({ storageState: AUTH_FILE });
      const page = await context.newPage();
      try {
        const errors: string[] = [];
        page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
        page.on('console', (m: ConsoleMessage) => {
          if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
        });

        // 路由是 **hash 模式**（`createWebHashHistory`，见 src/router/index.ts）。
        // 此前这里用 history 路径 `page.goto(p.path)` 直接访问 —— 浏览器请求
        // `/live` 时 hash 为空，vue-router 解析成 `/` 并重定向到 `#/dashboard`，
        // 于是这 17 个"page renders"用例**每个都在渲染控制台页并全部通过**，
        // 从来没有真正渲染过被断言的那些页面（截图也都是 dashboard）。
        // 现在走 hash 路径，并断言 hash 确实切换到了目标路由。
        await page.goto(`/#${p.path}`, { waitUntil: 'domcontentloaded' });
        await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});

        const url = page.url();
        if (/\/login$/.test(url) || url.includes('#/login')) {
          throw new Error(`Auth was lost: redirected to ${url} when visiting ${p.path}`);
        }
        await expect(page, `hash 应切到 ${p.path}（实际 ${url}）`).toHaveURL(
          new RegExp(`#${p.path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`)
        );

        const hasBody = await page.locator('body').isVisible();
        expect(hasBody, `body visible for ${p.name}`).toBe(true);

        await page.screenshot({ path: path.join(ARTIFACT_DIR, `${p.name}.png`), fullPage: true });

        const real = errors.filter(
          (e) => !/favicon|net::ERR_ABORTED|Loading chunk \d+ failed|AbortError/i.test(e),
        );
        if (real.length > 0) {
          console.warn(`[${p.name}] ${real.length} non-fatal console issue(s):`);
          for (const r of real.slice(0, 3)) console.warn(`  - ${r.slice(0, 200)}`);
        }
      } finally {
        await context.close();
      }
    });
  }
});
