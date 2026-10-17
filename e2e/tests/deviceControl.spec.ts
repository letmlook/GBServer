import { expect, test } from '@playwright/test';

const ONLINE = '34020000001320000001';
const OFFLINE = '34020000001320000002';

test('设备列表独立提供设防和撤防，并展示设备执行结果', async ({ page }) => {
  await page.route('**/device/query/devices*', (route) => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ code: 0, msg: '成功', data: {
      total: 2,
      list: [
        { deviceId: ONLINE, name: '在线设备', onLine: true, streamMode: 'UDP' },
        { deviceId: OFFLINE, name: '离线设备', onLine: false, streamMode: 'UDP' }
      ]
    } })
  }));

  const commands: string[] = [];
  await page.route('**/device/control/guard?*', (route) => {
    const query = new URL(route.request().url()).searchParams;
    expect(query.get('deviceId')).toBe(ONLINE);
    expect(query.get('waitResult')).toBe('true');
    const command = query.get('guardCmd') ?? '';
    commands.push(command);
    const status = commands.length === 1 ? 'confirmed' : commands.length === 2 ? 'rejected' : 'unconfirmed';
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, msg: '成功', data: {
        deviceId: ONLINE, guardCmd: command, sipSent: true,
        deviceResult: { status, sn: commands.length, resultCode: status === 'rejected' ? 'ERROR' : null }
      } })
    });
  });

  await page.goto('/#/device', { waitUntil: 'domcontentloaded' });
  const onlineRow = page.locator('.el-table__body tr').filter({ hasText: ONLINE });
  const offlineRow = page.locator('.el-table__body tr').filter({ hasText: OFFLINE });
  await expect(onlineRow).toBeVisible();
  await expect(onlineRow.getByRole('button', { name: '设防' })).toBeEnabled();
  await expect(onlineRow.getByRole('button', { name: '撤防' })).toBeEnabled();
  await expect(offlineRow.getByRole('button', { name: '设防' })).toBeDisabled();
  await expect(offlineRow.getByRole('button', { name: '撤防' })).toBeDisabled();

  await onlineRow.getByRole('button', { name: '设防' }).click();
  await expect(page.getByText('设备确认已设防')).toBeVisible();
  await onlineRow.getByRole('button', { name: '撤防' }).click();
  await expect(page.getByText('设备拒绝撤防（ERROR）')).toBeVisible();
  await onlineRow.getByRole('button', { name: '设防' }).click();
  await expect(page.getByText('设防指令已发送，设备执行结果未确认')).toBeVisible();
  expect(commands).toEqual(['SetGuard', 'ResetGuard', 'SetGuard']);
});
