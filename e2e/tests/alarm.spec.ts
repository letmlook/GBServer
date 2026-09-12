/**
 * 报警页（/#/alarm）契约端到端测试。
 *
 * 这一页此前 4 个动作全是坏的，而且坏法各不相同：
 *   1. 单行「清除」→ `GET /api/alarm/clear`：后端只注册 DELETE → 405；
 *      而后端那个 handler 又是**无条件 `DELETE FROM gb_device_alarm`**——
 *      一旦方法对上，点一行会清空全库告警；
 *   2. 「批量清除」→ `POST /api/alarm/batch`：后端只注册 DELETE → 405；
 *   3. 「处理」→ 参数放 query、body 为空，后端要 `Json<AlarmHandleBody>` → 415；
 *      且「处理结果」字段后端 DTO 里根本不存在；
 *   4. 「级别」列读 `alarmLevel`，后端返回的是 `alarmPriority` → 整列空白。
 *
 * 需要环境里有告警数据：SIP 设备 mock 加 `--auto-alarm-secs 5` 会周期上报
 * 报警通知（见 mock/tools/sip-device/sip_device_mock.py）。没有数据时跳过。
 */

import { test, expect, type Page } from '@playwright/test';

async function gotoAlarm(page: Page) {
  await page.goto('/#/alarm', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe('Alarm page (/alarm)', () => {
  test('级别列有值 + 单行清除只删这一条（不整表清空）', async ({ page }) => {
    await gotoAlarm(page);
    const rows = page.locator('.el-table__body tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 10_000 }).catch(() => {});
    const before = await rows.count();
    if (before === 0) {
      test.skip(true, '环境里没有告警数据（SIP mock 需要 --auto-alarm-secs）');
      return;
    }

    // 「级别」列必须显示中文级别，而不是空白
    // 列序：勾选 / 报警时间 / 设备ID / 通道ID / 级别 / 类型 / 描述 / 状态 / 操作
    const levelText = (await rows.first().locator('td').nth(4).innerText()).trim();
    expect(levelText.length).toBeGreaterThan(0);
    expect(['一级', '二级', '三级', '四级']).toContain(levelText.slice(0, 2));

    // 单行「清除」：必须走 DELETE 到 /alarm/delete/<id>，且后端只删 1 条。
    // 不能用"行数减 1"断言：SIP mock 每几秒就上报一条新告警，列表会同时增长。
    const [clearResp] = await Promise.all([
      page.waitForResponse(
        (r) => /\/alarm\/delete\/\d+$/.test(r.url()) && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await rows.first().getByRole('button', { name: '清除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    const clearBody = (await clearResp.json()) as { data?: { deleted?: number } };
    expect(clearBody.data?.deleted, '单行清除只能删 1 条').toBe(1);

    // 关键回归：点一行绝不能清空全库 —— 列表里必须还有其它告警
    await expect.poll(() => rows.count(), { timeout: 10_000 }).toBeGreaterThan(0);
    const totalText = await page.locator('.el-pagination').first().innerText();
    expect(totalText).not.toContain('共 0 条');
  });

  test('处理报警：提交后能读回处理结论', async ({ page }) => {
    await gotoAlarm(page);
    const rows = page.locator('.el-table__body tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 10_000 }).catch(() => {});
    if ((await rows.count()) === 0) {
      test.skip(true, '环境里没有告警数据');
      return;
    }

    const handleBody: unknown[] = [];
    page.on('request', async (r) => {
      if (r.url().includes('/alarm/handle')) handleBody.push(r.postDataJSON());
    });
    await rows.first().getByRole('button', { name: '处理' }).click();
    const prompt = page.locator('.el-message-box');
    await expect(prompt).toBeVisible();
    await prompt.locator('input').fill('已通过 E2E 确认');
    await prompt.getByRole('button', { name: '确定' }).click();
    // 行内状态列会变成「已处理」，用 toast 文案断言更稳（表格里也有同名 tag）
    await expect(page.locator('.el-message').getByText(/已处理/).first()).toBeVisible({ timeout: 10_000 });

    // 处理结论必须随 JSON body 提交（此前放 query → 415）
    expect(handleBody.length).toBeGreaterThan(0);
    expect(JSON.stringify(handleBody[0])).toContain('已通过 E2E 确认');

    // 重新打开「查看」弹窗，应能看到刚提交的处理结论
    await rows.first().getByRole('button', { name: '查看' }).click();
    await expect(page.getByText(/已通过 E2E 确认/).first()).toBeVisible({ timeout: 10_000 });
  });
});
