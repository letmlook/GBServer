import { test, expect } from '@playwright/test'

test('接口文档提供分组导航、鉴权和接口调试', async ({ page }) => {
  await page.goto('/api-docs', { waitUntil: 'domcontentloaded' })
  await expect(page.getByText('GBServer API', { exact: true }).first()).toBeVisible()
  const login = page.getByText('POST /api/user/login?username=xx&password=xx', { exact: true }).first()
  await expect(login).toBeVisible()
  await login.click()
  await expect(page.getByText('Open API Client', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('Auth Type').first()).toBeVisible()
})
