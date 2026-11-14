import { expect, test } from '@playwright/test'

test('直播停止失败保留会话并允许重试', async ({ page }) => {
  const device = '34020000001320000991'
  const channel = '34020000001310000991'
  let stops = 0
  await page.route('**/sy/camera/list-with-child*', (route) => route.fulfill({ json: {
    code: 0, msg: '成功', data: { list: [{ device_id: device, channel_id: channel, name: '测试通道', device_name: '测试设备', is_device: false, status: 'ON', online: true }] }
  } }))
  await page.route(`**/play/start/${device}/${channel}`, (route) => route.fulfill({ json: {
    code: 0, msg: '成功', data: { app: 'rtp', stream: 'test', playUrl: 'rtsp://127.0.0.1/test' }
  } }))
  await page.route('**/play/webrtc', (route) => route.fulfill({ json: { code: 500, msg: '无媒体', data: null } }))
  await page.route(`**/play/stop/${device}/${channel}`, (route) => {
    stops++
    return route.fulfill({ json: stops === 1 ? { code: 500, msg: '模拟停止失败', data: null } : { code: 0, msg: '成功', data: { callId: 'test', mediaCleanup: { rtp: 'failed', streams: 'accepted' } } } })
  })
  await page.goto('/#/live', { waitUntil: 'domcontentloaded' })
  await page.getByText('测试通道', { exact: true }).click()
  const stop = page.locator('.video-cell__footer').first().getByRole('button', { name: '停止' })
  await expect(stop).toBeVisible()
  await stop.click()
  await expect(page.getByText(/停止流失败/)).toBeVisible()
  await expect(page.locator('.player-bar .el-tag')).not.toHaveText('空闲')
  await expect(page.getByText('已停止', { exact: true })).toHaveCount(0)
  await expect(stop).toBeVisible()
  await stop.click()
  await expect(page.getByText('停止命令已发送，但媒体清理未确认')).toBeVisible()
  expect(stops).toBe(2)
})
