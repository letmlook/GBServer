import { defineConfig, devices } from '@playwright/test';

const PORT = process.env.E2E_BASE_URL || 'http://127.0.0.1:9528';

export default defineConfig({
  testDir: './tests',
  // 登录并落盘鉴权状态（artifacts/.auth.json）。
  // 修复：此前该文件由 smoke.spec.ts 的第 3 个用例顺手写出，而按文件名字母序
  // live.spec.ts 先执行，导致干净检出上 live 的 5 个用例必定因
  // "Error reading storage state ... ENOENT" 全部失败。
  globalSetup: './global-setup.ts',
  timeout: 30_000,
  expect: { timeout: 8_000 },
  fullyParallel: false,    // run serially so screenshots don't clobber each other
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: 'playwright-report' }],
  ],
  outputDir: 'test-results',
  use: {
    baseURL: PORT,
    // 默认 page fixture 即为已登录态（状态由 globalSetup 写入）。
    // 需要"未登录"的用例（登录页/登录流程）显式 newContext 一个空 storageState。
    storageState: 'artifacts/.auth.json',
    headless: true,                 // sandbox has no $DISPLAY; switch to false to see window
    viewport: { width: 1440, height: 900 },
    locale: 'zh-CN',
    ignoreHTTPSErrors: true,
    screenshot: 'only-on-failure',
    video: 'off',
    trace: 'retain-on-failure',
    actionTimeout: 8_000,
    navigationTimeout: 15_000,
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
});
