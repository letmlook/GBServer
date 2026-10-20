import { test, expect } from '@playwright/test'

const DEVICE = '34020000001320000991'
const CHANNEL = '34020000001310000991'

test('通道播放停止失败不能显示已停止，重试成功后关闭不重复发送', async ({ page }) => {
  let stopCalls = 0
  await page.route('**/common/channel/list*', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      code: 0, msg: '成功', data: { total: 1, list: [{
        id: 991, deviceId: DEVICE, channelId: CHANNEL,
        name: '停止结果回归通道', status: 'ON'
      }] }
    })
  }))
  await page.route(`**/play/start/${DEVICE}/${CHANNEL}`, (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ code: 0, msg: '成功', data: {
      app: 'rtp', stream: 'stop-result-test',
      playUrl: 'rtsp://127.0.0.1:554/rtp/stop-result-test'
    } })
  }))
  await page.route(`**/play/stop/${DEVICE}/${CHANNEL}`, (route) => {
    stopCalls += 1
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(stopCalls === 1 || stopCalls === 4
        ? { code: 500, msg: '模拟 BYE 发送失败', data: null }
        : { code: 0, msg: '成功', data: null })
    })
  })

  await page.goto('/#/channel', { waitUntil: 'domcontentloaded' })
  const row = page.getByRole('row', { name: /停止结果回归通道/ })
  await expect(row).toBeVisible()
  await row.getByRole('button', { name: '播放' }).click()
  const dialog = page.getByRole('dialog', { name: /停止结果回归通道/ })
  await expect(dialog).toBeVisible()
  const stopButton = dialog.getByRole('button', { name: '停止' })

  const rejected = page.waitForResponse((response) => response.url().includes(`/play/stop/${DEVICE}/${CHANNEL}`))
  await stopButton.click()
  await rejected
  await page.waitForTimeout(150)
  expect(stopCalls).toBe(1)
  await expect(page.getByText('已停止', { exact: true })).toHaveCount(0)
  await expect(dialog.locator('.play-dialog__bar .el-tag')).not.toContainText('空闲')

  const accepted = page.waitForResponse((response) => response.url().includes(`/play/stop/${DEVICE}/${CHANNEL}`))
  await stopButton.click()
  await accepted
  await expect.poll(() => stopCalls).toBe(2)
  await expect(page.getByText('已停止', { exact: true })).toBeVisible()

  await dialog.getByRole('button', { name: '关闭此对话框' }).click()
  await page.waitForTimeout(500)
  expect(stopCalls).toBe(2)

  await row.getByRole('button', { name: '播放' }).click()
  await expect(dialog).toBeVisible()
  await dialog.getByRole('button', { name: '关闭此对话框' }).click()
  await expect.poll(() => stopCalls).toBe(3)
  await page.waitForTimeout(500)
  expect(stopCalls).toBe(3)

  await row.getByRole('button', { name: '播放' }).click()
  await expect(dialog).toBeVisible()
  await dialog.getByRole('button', { name: '关闭此对话框' }).click()
  await expect.poll(() => stopCalls).toBe(4)
  await expect(page.getByText('停止命令未确认，设备可能仍在推流')).toBeVisible()
  await page.goto('/#/dashboard', { waitUntil: 'domcontentloaded' })
  expect(stopCalls).toBe(4)
})

test('起流请求返回前关闭弹窗，迟到的成功响应仍停止该流', async ({ page }) => {
  let releaseStart: () => void = () => {}
  const delayedStart = new Promise<void>((resolve) => { releaseStart = resolve })
  let stopCalls = 0
  await page.route('**/common/channel/list*', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ code: 0, msg: '成功', data: { total: 1, list: [{
      id: 991, deviceId: DEVICE, channelId: CHANNEL,
      name: '迟到起流回归通道', status: 'ON'
    }] } })
  }))
  await page.route(`**/play/start/${DEVICE}/${CHANNEL}`, async (route) => {
    await delayedStart
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, msg: '成功', data: {
        app: 'rtp', stream: 'late-start-test',
        playUrl: 'rtsp://127.0.0.1:554/rtp/late-start-test'
      } })
    })
  })
  await page.route(`**/play/stop/${DEVICE}/${CHANNEL}`, (route) => {
    stopCalls += 1
    return route.fulfill({
      status: 200, contentType: 'application/json',
      body: JSON.stringify({ code: 0, msg: '成功', data: null })
    })
  })

  await page.goto('/#/channel', { waitUntil: 'domcontentloaded' })
  const row = page.getByRole('row', { name: /迟到起流回归通道/ })
  await expect(row).toBeVisible()
  const requested = page.waitForRequest((request) => request.url().includes(`/play/start/${DEVICE}/${CHANNEL}`))
  await row.getByRole('button', { name: '播放' }).click()
  await requested
  const dialog = page.getByRole('dialog', { name: /迟到起流回归通道/ })
  await dialog.getByRole('button', { name: '关闭此对话框' }).click()
  releaseStart()
  await expect.poll(() => stopCalls, { timeout: 5000 }).toBe(1)
})

/**
 * 关闭**先于**起流响应到达时，那条流同样必须被停掉。
 *
 * 关闭弹窗时 `playData` 还是空的（起流请求没回来），关闭逻辑发不出停止命令；
 * 随后迟到的起流响应才把流真正建起来。若不补这一刀，后端会一直留着
 * SIP 会话和 ZLM 收流端口，没人看也一直占着。
 *
 * 用例刻意等弹窗**完全消失**后再放行起流响应：关闭事件此时已被组件观察到，
 * 覆盖的是"关闭在前、起流在后"这一顺序（另一条用例覆盖相反顺序）。
 */
test('关闭先于起流响应时，迟到的流也必须被停掉', async ({ page }) => {
  let releaseStart: () => void = () => {}
  const delayedStart = new Promise<void>((resolve) => { releaseStart = resolve })
  let stopCalls = 0
  await page.route('**/common/channel/list*', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ code: 0, msg: '成功', data: { total: 1, list: [{
      id: 991, deviceId: DEVICE, channelId: CHANNEL,
      name: '先关后起流回归通道', status: 'ON'
    }] } })
  }))
  await page.route(`**/play/start/${DEVICE}/${CHANNEL}`, async (route) => {
    await delayedStart
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, msg: '成功', data: {
        app: 'rtp', stream: 'close-first-test',
        playUrl: 'rtsp://127.0.0.1:554/rtp/close-first-test'
      } })
    })
  })
  await page.route(`**/play/stop/${DEVICE}/${CHANNEL}`, (route) => {
    stopCalls += 1
    return route.fulfill({
      status: 200, contentType: 'application/json',
      body: JSON.stringify({ code: 0, msg: '成功', data: null })
    })
  })

  await page.goto('/#/channel', { waitUntil: 'domcontentloaded' })
  const row = page.getByRole('row', { name: /先关后起流回归通道/ })
  await expect(row).toBeVisible()
  const requested = page.waitForRequest((request) => request.url().includes(`/play/start/${DEVICE}/${CHANNEL}`))
  await row.getByRole('button', { name: '播放' }).click()
  await requested
  const dialog = page.getByRole('dialog', { name: /先关后起流回归通道/ })
  await dialog.getByRole('button', { name: '关闭此对话框' }).click()
  // 弹窗彻底消失 => 关闭已被组件观察到，此时关闭逻辑因起流未完成而发不出停止命令。
  await expect(dialog).toBeHidden({ timeout: 10_000 })
  expect(stopCalls).toBe(0)

  releaseStart()
  await expect.poll(() => stopCalls, { timeout: 5000 }).toBe(1)
})
