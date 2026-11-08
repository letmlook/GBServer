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
