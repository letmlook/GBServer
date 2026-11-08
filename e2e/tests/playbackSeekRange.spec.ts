import { test, expect } from '@playwright/test'

const device = '34020000001320000991'
const channel = '34020000001310000991'

test('回放定位范围随录像片段变化，未知范围不可拖动', async ({ page }) => {
  const records = [
    { name: '跨午夜片段', startTime: '2026-10-18T23:59:00Z', endTime: '2026-10-19T00:01:00Z' },
    { name: '长录像片段', startTime: '2026-10-18T10:00:00', endTime: '2026-10-18T12:00:00' },
    { name: '未知时长片段', startTime: 'invalid', endTime: '' }
  ]
  await page.route('**/gb_record/query/**', (route) => route.fulfill({ json: {
    code: 0, msg: '成功', data: { total: 3, list: records }
  } }))
  await page.route(`**/playback/start/${device}/${channel}*`, (route) => route.fulfill({ json: {
    code: 0, msg: '成功', data: { streamId: 'playback-test', playUrl: 'rtsp://127.0.0.1/test' }
  } }))
  await page.route('**/playback/stop/**', (route) => route.fulfill({ json: { code: 0, msg: '成功', data: null } }))
  await page.goto('/#/playback', { waitUntil: 'domcontentloaded' })
  await page.getByPlaceholder('国标设备ID').fill(device)
  await page.getByPlaceholder('国标通道ID').fill(channel)
  await page.getByRole('button', { name: '检索' }).click()
  await page.getByRole('row', { name: /跨午夜片段/ }).click()
  const slider = page.getByRole('slider')
  await expect(slider).toHaveAttribute('aria-valuemax', '120')
  await page.getByRole('row', { name: /长录像片段/ }).click()
  await expect(slider).toHaveAttribute('aria-valuemax', '7200')
  await page.getByRole('row', { name: /未知时长片段/ }).click()
  await expect(slider).toHaveAttribute('aria-disabled', 'true')
})

test('切换片段前停止旧会话，停止失败时保留旧会话', async ({ page }) => {
  const events: string[] = []
  let stopCount = 0
  await page.route('**/gb_record/query/**', (route) => route.fulfill({ json: {
    code: 0, msg: '成功', data: { total: 2, list: [
      { name: '片段甲', startTime: '2026-10-18T10:00:00', endTime: '2026-10-18T10:01:00' },
      { name: '片段乙', startTime: '2026-10-18T11:00:00', endTime: '2026-10-18T11:01:00' }
    ] }
  } }))
  await page.route('**/playback/start/**', (route) => {
    events.push('start')
    return route.fulfill({ json: { code: 0, msg: '成功', data: {
      streamId: `stream-${events.filter((e) => e === 'start').length}`, playUrl: 'rtsp://127.0.0.1/test'
    } } })
  })
  await page.route('**/playback/stop/**', (route) => {
    events.push(new URL(route.request().url()).pathname)
    stopCount++
    return route.fulfill({ json: stopCount === 1
      ? { code: 1, msg: '停止失败', data: null } : { code: 0, msg: '成功', data: null } })
  })
  await page.goto('/#/playback', { waitUntil: 'domcontentloaded' })
  await page.getByPlaceholder('国标设备ID').fill(device)
  await page.getByPlaceholder('国标通道ID').fill(channel)
  await page.getByRole('button', { name: '检索' }).click()
  await page.getByRole('row', { name: /片段甲/ }).click()
  await expect(page.getByRole('button', { name: '暂停' })).toBeVisible()
  await page.getByPlaceholder('国标设备ID').fill('other-device')
  await page.getByPlaceholder('国标通道ID').fill('other-channel')
  await page.getByRole('row', { name: /片段乙/ }).click()
  await expect.poll(() => stopCount).toBe(1)
  await expect(page.locator('.player-header')).toContainText('片段甲')
  expect(events).toEqual(['start', `/dev-api/playback/stop/${device}/${channel}/stream-1`])
  await page.getByRole('row', { name: /片段乙/ }).click()
  await expect.poll(() => events.length).toBe(4)
  expect(events[2]).toBe(`/dev-api/playback/stop/${device}/${channel}/stream-1`)
  expect(events[3]).toBe('start')
  await page.getByRole('button', { name: '停止', exact: true }).click()
  await expect.poll(() => events.length).toBe(5)
  expect(events[4]).toBe('/dev-api/playback/stop/other-device/other-channel/stream-2')
})
