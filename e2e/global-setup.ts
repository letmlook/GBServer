/**
 * Playwright 全局前置：登录一次并把鉴权状态写到 `artifacts/.auth.json`。
 *
 * 为什么必须有它：
 * `tests/smoke.spec.ts` 原来是在第 3 个用例里**顺手**把 storageState 写出来的，
 * 而 `tests/live.spec.ts` 的 beforeEach 又直接读这个文件。Playwright 按文件名的
 * 字母序执行，`live.spec.ts` **排在 `smoke.spec.ts` 前面** —— 于是在干净检出上
 * live 的 5 个用例必定因为
 *   `Error reading storage state ... ENOENT: artifacts/.auth.json`
 * 全部失败。也就是说这套 e2e 在 CI/新环境上从来没整体绿过。
 *
 * 现在把"登录并落盘鉴权状态"提升为全局前置，与任何用例的执行顺序解耦。
 */
import { chromium, type FullConfig } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const AUTH_FILE = path.resolve(__dirname, 'artifacts/.auth.json');

const USERNAME = process.env.E2E_USERNAME ?? 'admin';
const PASSWORD = process.env.E2E_PASSWORD ?? 'admin';
const LOGIN_USERNAME_PLACEHOLDER = '用户名 / SIP 编号';
const LOGIN_PASSWORD_PLACEHOLDER = '登录密码';
const LOGIN_BUTTON_TEXT = '登 录';

export default async function globalSetup(config: FullConfig): Promise<void> {
  const baseURL =
    config.projects[0]?.use?.baseURL ?? process.env.E2E_BASE_URL ?? 'http://127.0.0.1:9528';

  fs.mkdirSync(path.dirname(AUTH_FILE), { recursive: true });

  const browser = await chromium.launch();
  const context = await browser.newContext({ baseURL, locale: 'zh-CN' });
  const page = await context.newPage();
  try {
    await page.goto('/login');
    await page.locator(`input[placeholder="${LOGIN_USERNAME_PLACEHOLDER}"]`).fill(USERNAME);
    await page.locator(`input[placeholder="${LOGIN_PASSWORD_PLACEHOLDER}"]`).fill(PASSWORD);
    await page.getByRole('button', { name: LOGIN_BUTTON_TEXT }).click();
    await page.waitForURL(/\/(dashboard)?$/, { timeout: 20_000 });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    await context.storageState({ path: AUTH_FILE });
    console.log(`[e2e] 已写入鉴权状态: ${AUTH_FILE}`);
  } finally {
    await context.close();
    await browser.close();
  }
}
