import { test, expect } from '@playwright/test'

test('管理员可查看待清理回放并重试，失败时保留记录', async ({ page }) => {
  let attempts = 0
  let present = true
  await page.route('**/user/userInfo', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { id: 1, username: 'admin', role: { id: 1, name: 'admin', authority: '0' } }
  } }))
  await page.route('**/playback/recovery?*', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { total: present ? 1 : 0, offset: 0, limit: 20, items: present ? [
      { streamId: 'recover-stream', deviceId: 'device-1', channelId: 'channel-1', callId: 'call-1', status: 'cleanup_pending', deviceResult: 'unconfirmed' }
    ] : [] }
  } }))
  await page.route('**/playback/recovery/recover-stream/retry', route => {
    attempts++
    if (attempts === 1) return route.fulfill({ json: { code: -1, msg: 'BYE 发送失败', data: null } })
    present = false
    return route.fulfill({ json: { code: 0, msg: '成功', data: { status: 'local_cleanup_complete', deviceResult: 'unconfirmed' } } })
  })
  await page.goto('/#/operations/systemInfo', { waitUntil: 'domcontentloaded' })
  const card = page.getByTestId('playback-recovery')
  await expect(card).toContainText('recover-stream')
  await card.getByRole('button', { name: '重试清理' }).click()
  await expect(card).toContainText('recover-stream')
  await card.getByRole('button', { name: '重试清理' }).click()
  await expect(card).not.toContainText('recover-stream')
  await expect(page.getByText('本地清理已完成，设备执行结果未确认')).toBeVisible()
  expect(attempts).toBe(2)
})

test('普通用户不显示管理员回放恢复入口', async ({ page }) => {
  let recoveryRequests = 0
  await page.route('**/user/userInfo', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { id: 2, username: 'viewer', role: { id: 2, name: 'viewer', authority: '9' } }
  } }))
  await page.route('**/playback/recovery?*', route => { recoveryRequests++; return route.abort() })
  const userInfo = page.waitForResponse(response => response.url().includes('/user/userInfo'))
  await page.goto('/#/operations/systemInfo', { waitUntil: 'domcontentloaded' })
  await userInfo
  await expect(page.locator('.metric-card').first()).toBeVisible()
  await expect(page.getByTestId('playback-recovery')).toHaveCount(0)
  expect(recoveryRequests).toBe(0)
})

test('待清理列表分页和加载失败可见', async ({ page }) => {
  let fail = false
  await page.route('**/user/userInfo', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { id: 1, username: 'admin', role: { id: 1, name: 'admin' } }
  } }))
  await page.route('**/playback/recovery?*', route => {
    if (fail) return route.fulfill({ status: 500, json: { code: 500, msg: '不可用', data: null } })
    const offset = Number(new URL(route.request().url()).searchParams.get('offset'))
    return route.fulfill({ json: { code: 0, msg: '成功', data: {
      total: 21, offset, limit: 20, items: [{ streamId: `stream-${offset}`, deviceId: 'dev', channelId: 'ch', callId: 'call', status: 'cleanup_pending', deviceResult: 'unconfirmed' }]
    } } })
  })
  await page.goto('/#/operations/systemInfo', { waitUntil: 'domcontentloaded' })
  const card = page.getByTestId('playback-recovery')
  await expect(card).toContainText('stream-0')
  await card.locator('.btn-next').click()
  await expect(card).toContainText('stream-20')
  fail = true
  await page.getByRole('button', { name: '刷新' }).click()
  await expect(card).toContainText('待清理会话加载失败，请重试')
})

test('重启后保留的记录显示待恢复且不能发送重试', async ({ page }) => {
  let retries = 0
  await page.route('**/user/userInfo', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { id: 1, username: 'admin', role: { id: 1, name: 'admin', authority: '0' } }
  } }))
  await page.route('**/playback/recovery?*', route => route.fulfill({ json: {
    code: 0, msg: '成功', data: { total: 1, offset: 0, limit: 20, items: [{
      streamId: 'persisted-stream', deviceId: 'dev', channelId: 'ch', callId: 'call', status: 'recovery_required',
      deviceResult: 'unconfirmed', retryable: false, retryReason: '已保存的 SIP 对话尚未恢复，暂不能重试'
    }] }
  } }))
  await page.route('**/playback/recovery/**/retry', route => { retries++; return route.abort() })
  await page.goto('/#/operations/systemInfo', { waitUntil: 'domcontentloaded' })
  const card = page.getByTestId('playback-recovery')
  await expect(card).toContainText('persisted-stream')
  await expect(card).toContainText('待对话恢复')
  await expect(card.getByRole('button', { name: '重试清理' })).toHaveCount(0)
  expect(retries).toBe(0)
})
