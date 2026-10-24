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

import { test, expect, type Page, type Locator } from '@playwright/test'
import { apiPath } from '../test-helpers/api-path'

/** 本轮用例专用的应用标识前缀（用于定位与清理）。 */
const APP_PREFIX = 'e2e-apikey'

/**
 * 在 el-date-picker 面板里选一个日期。
 *
 * **为什么不能直接 fill 输入框**：日期选择器已改成 `:editable="false"`
 * （输入框只读），否则用户能打出非法日期 —— 而 element-plus 的用户输入解析
 * 走 dayjs 的**非严格**模式，"2026-02-30" 会被**归一化**成 "2026-03-02"，
 * 解析不出来的又会被**静默丢弃**成"永不过期"。两条路都会让后端新加的 400
 * 保护根本触发不到。只读之后，非法日期在浏览器里就输不出来。
 *
 * 面板的默认时间是 **00:00:00**，所以：
 * - 选「今天」→ 今天午夜 → **已经过去**（用于验证过期即拒绝）；
 * - 选「下个月」→ 将来（用于验证设置了过期时间后仍然可用）。
 *
 * 返回输入框里的最终值。
 */
async function pickDate(
  page: Page,
  dialog: Locator,
  opts: { monthOffset: number; day?: number }
): Promise<string> {
  const popper = page.locator('.el-picker__popper')
  const input = dialog.getByPlaceholder('留空表示永不过期')
  await input.click()
  // 先断言浮层**真的打开**了 —— 否则后面的 toBeHidden 会变成"元素不存在也算过"的
  // 空断言，交互写坏了测试照样绿。
  await expect(popper).toBeVisible()
  for (let i = 0; i < opts.monthOffset; i += 1) {
    await popper.locator('button.el-picker-panel__icon-btn.arrow-right').click()
  }
  if (opts.day === undefined) {
    await popper.locator('td.today').click()
  } else {
    await popper.locator('td.available', { hasText: new RegExp(`^${opts.day}$`) }).first().click()
  }
  await popper.getByRole('button', { name: '确定' }).click()
  await expect(popper).toBeHidden()
  return input.inputValue()
}

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
    // 选「今天」→ 面板默认时间 00:00:00 → 已经过去的时刻 → 创建即过期。
    const picked = await pickDate(page, addDialog, { monthOffset: 0 })
    expect(picked, '日期选择器应写入一个时间').toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)
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

  test('选了未来的过期时间后，密钥在到期前仍然可用', async ({ page }) => {
    if (!(await openPage(page))) {
      test.skip(true, '当前登录账号不是管理员，跳过 API Key 管理用例')
    }
    await cleanupLeftovers(page)
    await page.reload({ waitUntil: 'domcontentloaded' })

    const appName = `${APP_PREFIX}-future-${Date.now()}`

    await page.getByRole('button', { name: '新增 API Key' }).click()
    const addDialog = page.getByRole('dialog', { name: '新增 API Key' })
    await expect(addDialog).toBeVisible()
    await addDialog.getByLabel('应用标识').fill(appName)
    // 选「下个月的 15 号」→ 将来 → 到期前必须仍然可用。
    const picked = await pickDate(page, addDialog, { monthOffset: 1, day: 15 })
    await addDialog.getByRole('button', { name: '创建' }).click()

    const apiKey = await readSecretFromDialog(page)
    await page.getByRole('button', { name: '我已复制' }).click()

    // 核心断言：过期时间真的被带上了（不是"永不过期"），且到期前可用。
    const row = page.getByRole('row', { name: new RegExp(appName) })
    await expect(row).toBeVisible()
    await expect(row).not.toContainText('永不过期')
    await expect(row).toContainText(picked.slice(0, 10))
    expect(await statusWithApiKey(page, apiKey, apiPath('/server/version'))).toBe(200)

    await row.getByRole('button', { name: '删除' }).click()
    await messageBoxButton(page, '删除').click()
  })

  test('非法的过期时间被直接拒绝（400 + 契约信封）', async ({ page }) => {
    if (!(await openPage(page))) {
      test.skip(true, '当前登录账号不是管理员，跳过 API Key 管理用例')
    }
    const token = await page.evaluate(() => {
      const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/)
      return m ? decodeURIComponent(m[1]) : ''
    })
    const headers = token ? { 'access-token': token } : {}

    // 前端已经把日期选择器改成只读，**从 UI 发不出非法值** ——
    // 所以这条行为只能直接打 HTTP 验。必须回 400 + `{code,msg,data}` 信封，
    // 而不是裸文本，更不能是 `code:0`（那等于把"本该过期的凭证"变成永久有效）。
    for (const bad of [
      '2026-13-45 99:99:99', // 月份/时分秒越界
      '2026-02-30 10:00:00', // 2 月没有 30 号
      'not-a-date',
      '2026-12-31T23:59:59Z' // ISO 带 T：不是本接口约定的格式
    ]) {
      const resp = await page.request.post(
        apiPath(`/userApiKey/add?app=${APP_PREFIX}-bad&expiresAt=${encodeURIComponent(bad)}`),
        { headers }
      )
      expect(resp.status(), `非法过期时间必须回 400: ${bad}`).toBe(400)
      const body = await resp.json()
      expect(body.code, `${bad} 的响应体必须是契约信封`).toBe(400)
      expect(String(body.msg)).toContain('过期时间格式不正确')
    }
  })
})
