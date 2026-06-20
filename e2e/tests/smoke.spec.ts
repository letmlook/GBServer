/**
 * GBServer admin SPA smoke test.
 *
 * Walks the core admin paths the Vue 2 + Element UI frontend exposes,
 * after logging in as `admin / admin`. For each page we:
 *  1. Navigate to its path.
 *  2. Wait for the SPA to mount.
 *  3. Take a screenshot to artifacts/<page>.png.
 *  4. Surface any console errors collected during navigation.
 *
 * Auth state is persisted via Playwright storageState so we only log in once
 * and every per-page test sees the real layout (not a login redirect).
 */

import { test, expect, type ConsoleMessage, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ARTIFACT_DIR = path.resolve(__dirname, '../artifacts');
fs.mkdirSync(ARTIFACT_DIR, { recursive: true });

// Subset of menu routes that should always render. Excludes dynamic :id routes
// (would need real data) and the 404 page.
const ADMIN_PAGES: ReadonlyArray<{ name: string; path: string; waitFor?: string }> = [
  { name: 'dashboard',    path: '/dashboard' },
  { name: 'live',         path: '/live' },
  { name: 'channel',      path: '/channel' },
  { name: 'map',          path: '/map' },
  { name: 'device',       path: '/device' },
  { name: 'push',         path: '/push' },
  { name: 'proxy',        path: '/proxy' },
  { name: 'commonChannel',path: '/commonChannel' },
  { name: 'recordPlan',   path: '/recordPlan' },
  { name: 'cloudRecord',  path: '/cloudRecord' },
  { name: 'mediaServer',  path: '/mediaServer' },
  { name: 'platform',     path: '/platform' },
  { name: 'user',         path: '/user' },
  { name: 'operations',   path: '/operations' },
  { name: 'alarm',        path: '/alarm' },
];

const ADMIN = {
  username: 'admin',
  // The Vue store MD5-hashes the password before sending
  // (see web/src/store/modules/user.js::login). So the form value
  // must be the *plaintext* 'admin', not its MD5 digest.
  password: 'admin',
};

const AUTH_FILE = path.resolve(__dirname, '../artifacts/.auth.json');

test.describe.configure({ mode: 'serial' });

test.describe('GBServer admin UI smoke', () => {
  test('backend health probe', async ({ request }) => {
    const r = await request.get('http://127.0.0.1:18080/api/health');
    expect(r.status(), 'backend /api/health').toBe(200);
  });

  test('login page renders', async ({ page }) => {
    await page.goto('/login');
    await expect(page.locator('input[placeholder="用户名"]')).toBeVisible();
    await expect(page.locator('input[placeholder="密码"]')).toBeVisible();
    await expect(page.getByRole('button', { name: '登录' })).toBeVisible();
    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'login.png'), fullPage: true });
  });

  test('admin login → dashboard (and persist auth)', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
    page.on('console', (m: ConsoleMessage) => {
      if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
    });

    await page.goto('/login');
    await page.locator('input[placeholder="用户名"]').fill(ADMIN.username);
    await page.locator('input[placeholder="密码"]').fill(ADMIN.password);
    await page.getByRole('button', { name: '登录' }).click();

    // Login redirects to '/' which is a Vue-router redirect to '/dashboard'
    await page.waitForURL(/\/(dashboard)?$/, { timeout: 15_000 });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    await expect(page).toHaveURL(/\/(dashboard)?$/);

    // Persist auth state for the per-page tests.
    await page.context().storageState({ path: AUTH_FILE });

    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'dashboard-after-login.png'), fullPage: true });
  });

  for (const p of ADMIN_PAGES) {
    test(`page renders: ${p.name} (${p.path})`, async ({ browser }) => {
      // Load persisted auth from the previous login test.
      const context = await browser.newContext({ storageState: AUTH_FILE });
      const page = await context.newPage();
      try {
        const errors: string[] = [];
        page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
        page.on('console', (m: ConsoleMessage) => {
          if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
        });

        // The Vue app uses hash-based routing. Going to /<path> via plain
        // navigation lands on the SPA shell but the router then defaults to
        // '/'. We need to anchor into the hash so the router picks up the
        // desired route on first paint.
        await page.goto(`/#${p.path}`, { waitUntil: 'domcontentloaded' });
        await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});

        // We must not be on the login page after navigation.
        const url = page.url();
        if (/\/login$/.test(url) || url.endsWith('/#/login')) {
          throw new Error(`Auth was lost: redirected to ${url} when visiting ${p.path}`);
        }

        const hasBody = await page.locator('body').isVisible();
        expect(hasBody, `body visible for ${p.name}`).toBe(true);

        await page.screenshot({ path: path.join(ARTIFACT_DIR, `${p.name}.png`), fullPage: true });

        // Filter expected noise. Real issues are surfaced via console.warn so
        // they show up in test output without turning the run red.
        const real = errors.filter(
          (e) => !/favicon|net::ERR_ABORTED|Loading chunk \d+ failed/i.test(e),
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
