/**
 * API Key 管理页（`/#/apiKey`）端到端测试。
 *
 * 这条链路的价值不在"页面能打开"，而在**密钥真的能用**：
 * 新建的 API Key 应当能直接通过 `X-API-Key` 头调用受保护接口；
 * 停用 / 重置 / 删除之后，旧密钥必须立刻失效。
 *
 * 页面只在新增与重置时展示一次完整密钥（后端列表只给掩码），所以用例
 * 必须从那个弹窗里取密钥 —— 这也正是真实操作者的路径。
 *
 * 每次运行用**唯一**的应用标识，并在开始前清掉上一轮遗留的行：
 * 否则同库多次运行会留下多行同名数据，Playwright 的严格模式会直接报
 * "resolved to N elements"，把真正的断言盖掉。
 */

import { test, expect, type Page } from '@playwright/test'
import { apiPath } from '../test-helpers/api-path'

/** 本轮用例专用的应用标识前缀（用于定位与清理）。 */
const APP_PREFIX = 'e2e-apikey'

/** 用 X-API-Key 调受保护接口，返回 HTTP 状态码。 */
async function statusWithApiKey(page: Page, apiKey: string, path: string): Promise<number> {
  return page.evaluate(
    async ({ key, url }) => {
      const res = await fetch(url, { headers: { 'X-API-Key': key }, credentials: 'omit' })
      return res.status
    },
    { key: apiKey, url: path }
  )
}

/** 消息框里的确认按钮。必须限定在消息框内 —— 表格行里也有同名按钮。 */
function messageBoxButton(page: Page, name: string) {
  return page.locator('.el-message-box').getByRole('button', { name, exact: true })
}

/** 从"请立即复制密钥"弹窗里取出完整密钥。 */
async function readSecretFromDialog(page: Page): Promise<string> {
  const dialog = page.getByRole('dialog', { name: '请立即复制密钥' })
  await expect(dialog).toBeVisible()
  const value = await dialog.locator('input').inputValue()
  expect(value).toMatch(/^[0-9a-f]{32}$/)
  return value
}

/** 清掉上一轮遗留的用例数据，避免多行同名导致严格模式失败。 */
async function cleanupLeftovers(page: Page) {
  await page.evaluate(async (listUrl) => {
    const token = (document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/) || [])[1]
    const headers = { 'access-token': decodeURIComponent(token || '') }
    const list = await (await fetch(listUrl, { headers, credentials: 'omit' })).json()
    const rows: any[] = list?.data?.list ?? []
    for (const row of rows) {
      if (String(row.app || '').startsWith('e2e-apikey')) {
        await fetch(listUrl.replace('/userApiKeys', '/delete'), {
          method: 'DELETE',
          headers,
          credentials: 'omit'
        })
      }
    }
  }, apiPath('/userApiKey/userApiKeys?page=1&count=100'))
}

/** 打开页面并确认有管理员权限（否则跳过）。 */
async function openPage(page: Page): Promise<boolean> {
  await page.goto('/#/apiKey', { waitUntil: 'domcontentloaded' })
  const adminGuard = page.getByText('需要管理员权限才能管理 API Key')
  await expect(page.getByRole('button', { name: '新增 API Key' })).toBeVisible({ timeout: 15_000 })
  return (await adminGuard.count()) === 0
}

test.describe('API Key 管理', () => {
  test('创建后可鉴权调用，停用即失效，重置换新密钥，删除后彻底不可用', async ({ page }) => {
    if (!(await openPage(page))) {
      test.skip(true, '当前登录账号不是管理员，跳过 API Key 管理用例')
    }
    await cleanupLeftovers(page)
    await page.reload({ waitUntil: 'domcontentloaded' })

    const appName = `${APP_PREFIX}-main-${Date.now()}`

    await page.getByRole('button', { name: '新增 API Key' }).click()
    const addDialog = page.getByRole('dialog', { name: '新增 API Key' })
    await expect(addDialog).toBeVisible()
    await addDialog.getByLabel('应用标识').fill(appName)
    await addDialog.getByLabel('备注').fill('端到端回归创建的密钥')
    await addDialog.getByRole('button', { name: '创建' }).click()

    // 完整密钥只在这一次出现。
    const apiKey = await readSecretFromDialog(page)
    await page.getByRole('button', { name: '我已复制' }).click()

    // 列表里必须是掩码，不能把密钥回显到页面上。
    const row = page.getByRole('row', { name: new RegExp(appName) })
    await expect(row).toBeVisible()
    await expect(row).toContainText('******')
    await expect(row).not.toContainText(apiKey)
    await expect(row).toContainText('启用')
    await expect(row).toContainText('永不过期')

    // 密钥真的能调通受保护接口。
    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(200)

    // 停用后旧密钥必须立刻失效。
    await row.getByRole('button', { name: '停用' }).click()
    await expect(page.getByRole('row', { name: new RegExp(appName) })).toContainText('停用')
    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(401)

    // 重新启用后恢复可用。
    await page
      .getByRole('row', { name: new RegExp(appName) })
      .getByRole('button', { name: '启用' })
      .click()
    await expect(page.getByRole('row', { name: new RegExp(appName) })).toContainText('启用')
    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(200)

    // 重置 → 旧密钥失效、新密钥生效。
    await page
      .getByRole('row', { name: new RegExp(appName) })
      .getByRole('button', { name: '重置密钥' })
      .click()
    await messageBoxButton(page, '重置').click()
    const newKey = await readSecretFromDialog(page)
    expect(newKey).not.toBe(apiKey)
    await page.getByRole('button', { name: '我已复制' }).click()

    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(401)
    expect(await statusWithApiKey(page, newKey, apiPath('/server/version'))).toBe(200)

    // 删除 → 新密钥也不可用。
    await page
      .getByRole('row', { name: new RegExp(appName) })
      .getByRole('button', { name: '删除' })
      .click()
    await messageBoxButton(page, '删除').click()
    await expect(page.getByRole('row', { name: new RegExp(appName) })).toHaveCount(0)
    expect(await statusWithApiKey(page, newKey, apiPath('/server/version'))).toBe(401)
  })

  test('过期时间的密钥在过期后不再被接受', async ({ page }) => {
    if (!(await openPage(page))) {
      test.skip(true, '当前登录账号不是管理员，跳过 API Key 管理用例')
    }
    await cleanupLeftovers(page)
    await page.reload({ waitUntil: 'domcontentloaded' })

    const appName = `${APP_PREFIX}-expired-${Date.now()}`

    await page.getByRole('button', { name: '新增 API Key' }).click()
    const addDialog = page.getByRole('dialog', { name: '新增 API Key' })
    await expect(addDialog).toBeVisible()
    await addDialog.getByLabel('应用标识').fill(appName)
    // 后端接受 `YYYY-MM-DD HH:mm:ss`，这里给一个已经过去的时间。
    // 过期时间用的是 el-date-picker：填完要按回车把面板收掉，
    // 否则浮层还在，"创建"按钮会一直被判为"不稳定"而点不动。
    const expiryInput = addDialog.getByPlaceholder('留空表示永不过期')
    await expiryInput.fill('2020-01-01 00:00:00')
    await expiryInput.press('Enter')
    await expect(page.locator('.el-picker__popper')).toBeHidden()
    await addDialog.getByRole('button', { name: '创建' }).click()

    const apiKey = await readSecretFromDialog(page)
    await page.getByRole('button', { name: '我已复制' }).click()

    // 过期即拒绝 —— 这条才是契约。
    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(401)

    // 过期记录会被后台每小时清理一次，所以行可能已经不在列表里了；
    // 只要还在，就必须标成"已过期"而不是"启用"。
    const row = page.getByRole('row', { name: new RegExp(appName) })
    if (await row.count()) {
      await expect(row).toContainText('已过期')
      await row.getByRole('button', { name: '删除' }).click()
      await messageBoxButton(page, '删除').click()
    }
  })
})
