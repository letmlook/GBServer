/**
 * 前端设备控制（云台/聚焦光圈/预置位/巡航/扫描/辅助开关/雨刷）契约端到端测试。
 *
 * 这一组接口此前的报文形态与 WVP / GB/T 28181-2022 **都不一致**：
 *   * 聚焦/光圈发的是 2016 风格的独立元素 `<FICmd>IrisOpen</FICmd>`；
 *   * 预置位发 `<PresetCmd>CallPreset</PresetCmd><PresetIndex>7</PresetIndex>`；
 *   * 巡航/扫描/辅助/雨刷发的是**属性式 XML**（`<CruiseCmd id="1" preset="5" action="add" />`）
 *     —— 国标与 WVP 里这些全都是 8 字节 `PTZCmd` 的二进制指令码
 *     （WVP `SIPCommander.frontEndCmdString`），只认 PTZCmd 的设备会把旧写法整条忽略。
 *   * 另外 `/fi/focus` 的命令取值 WVP 是 `near`/`far`/`stop`、`/fi/iris` 是 `in`/`out`/`stop`，
 *     旧实现只认 on/off/open/close，WVP 前端发 `near` 会被直接拒掉。
 *
 * 本 spec 通过页内请求逐个下发，断言：合法取值全部 `code:0`、非法取值明确报错。
 */
import { test, expect, type Page } from '@playwright/test';

const DEVICE = '34020000001320000001';
const CHANNEL = '34020000001320000001';

async function authHeaders(page: Page): Promise<Record<string, string>> {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : '';
  });
  return token ? { 'access-token': token } : {};
}

async function post(page: Page, path: string): Promise<number> {
  const headers = await authHeaders(page);
  const resp = await page.request.get(`/dev-api/front-end/${path}`, { headers });
  expect(resp.status(), path).toBe(200);
  const body = (await resp.json()) as { code: number; msg?: string };
  return body.code;
}

test.describe('Front-end control (/api/front-end/*)', () => {
  test.beforeEach(async ({ page }) => {
    // cookie 需在同源文档里才读得到
    await page.goto('/#/live', { waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
  });

  test('云台与聚焦/光圈：WVP 的取值全部接受，拼错明确报错', async ({ page }) => {
    // 云台方向 + 变倍
    for (const cmd of ['up', 'down', 'left', 'right', 'zoom_in', 'zoom_out', 'stop']) {
      expect(await post(page, `ptz/${DEVICE}/${CHANNEL}?command=${cmd}&speed=50`), cmd).toBe(0);
    }
    expect(await post(page, `ptz/${DEVICE}/${CHANNEL}?command=nope&speed=50`)).toBe(1);

    // 聚焦：WVP 的 near/far/stop（此前 near/far 会被拒）
    for (const cmd of ['near', 'far', 'stop', 'focus_in', 'focus_out']) {
      expect(await post(page, `fi/focus/${DEVICE}/${CHANNEL}?command=${cmd}&speed=30`), cmd).toBe(0);
    }
    expect(await post(page, `fi/focus/${DEVICE}/${CHANNEL}?command=nope`)).toBe(1);

    // 光圈：WVP 的 in/out/stop（此前 in/out 会被拒）
    for (const cmd of ['in', 'out', 'stop', 'open', 'close']) {
      expect(await post(page, `fi/iris/${DEVICE}/${CHANNEL}?command=${cmd}&speed=20`), cmd).toBe(0);
    }
    expect(await post(page, `fi/iris/${DEVICE}/${CHANNEL}?command=nope`)).toBe(1);
  });

  test('预置位 / 巡航 / 扫描 / 辅助开关 / 雨刷全部可下发', async ({ page }) => {
    for (const action of ['add', 'call', 'delete']) {
      expect(
        await post(page, `preset/${action}/${DEVICE}/${CHANNEL}?presetId=5`),
        action
      ).toBe(0);
    }
    expect(await post(page, `cruise/point/add/${DEVICE}/${CHANNEL}?cruiseId=1&presetId=5`)).toBe(0);
    expect(await post(page, `cruise/point/delete/${DEVICE}/${CHANNEL}?cruiseId=1&presetId=5`)).toBe(0);
    expect(await post(page, `cruise/speed/${DEVICE}/${CHANNEL}?cruiseId=1&presetId=5&speed=3`)).toBe(0);
    expect(await post(page, `cruise/time/${DEVICE}/${CHANNEL}?cruiseId=1&presetId=5&time=7`)).toBe(0);
    expect(await post(page, `cruise/start/${DEVICE}/${CHANNEL}?cruiseId=1`)).toBe(0);
    expect(await post(page, `cruise/stop/${DEVICE}/${CHANNEL}?cruiseId=1`)).toBe(0);

    expect(await post(page, `scan/start/${DEVICE}/${CHANNEL}?scanId=2`)).toBe(0);
    expect(await post(page, `scan/set/left/${DEVICE}/${CHANNEL}?scanId=2`)).toBe(0);
    expect(await post(page, `scan/set/right/${DEVICE}/${CHANNEL}?scanId=2`)).toBe(0);
    expect(await post(page, `scan/set/speed/${DEVICE}/${CHANNEL}?scanId=2&speed=9`)).toBe(0);
    expect(await post(page, `scan/stop/${DEVICE}/${CHANNEL}?scanId=2`)).toBe(0);

    expect(await post(page, `auxiliary/${DEVICE}/${CHANNEL}?command=on&switchId=3`)).toBe(0);
    expect(await post(page, `auxiliary/${DEVICE}/${CHANNEL}?command=off&switchId=3`)).toBe(0);
    expect(await post(page, `wiper/${DEVICE}/${CHANNEL}?command=on`)).toBe(0);
    expect(await post(page, `wiper/${DEVICE}/${CHANNEL}?command=off`)).toBe(0);
  });
});
