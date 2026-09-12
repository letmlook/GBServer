/**
 * 云端录像页（/#/cloudRecord）契约端到端测试。
 *
 * 这一页此前 5 个动作全是坏的，而且都是"接口回成功/报错但功能为零"：
 *   1. 删除：前端 `GET /cloud/record/delete?id=`，后端只注册 DELETE 且从
 *      **body** 读 `{ids:[...]}` → 405 + 参数进不去；
 *   2. 播放：`play/path` 传 `id`，后端只认 `recordId` → 永远返回空路径；
 *      而且列表只扫 ZLM 的 `app=record&stream=record`，**设备录像一条都查不到**；
 *   3. 时间筛选：前端发 `toISOString()`（带毫秒和 Z），后端解析失败当成 0
 *      → 选了结束时间列表必然空白；
 *   4. `deviceId/channelId` 后端 DTO 里没有 → 静默丢弃；
 *   5. 打包下载：传的是组合串 id，后端按 i64 解析 → "missing ids"；
 *      且容器化部署下按本机路径找文件，必然"全部不可用"。
 *
 * 需要环境里已经有一段真实录像（录像计划拉流录制即可，见 docs/WVP_PARITY.md
 * 第二十八轮的运行配方）。没有数据时跳过。
 */

import { test, expect, type APIRequestContext, type Page } from '@playwright/test';

async function gotoCloudRecord(page: Page) {
  await page.goto('/#/cloudRecord', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

/**
 * 保证环境里至少有一段落好的录像。
 *
 * 空列表时不直接 skip，而是**自己造一段**：建一个覆盖当前时刻的录像计划
 * （后端在 add/link 时会立即唤醒调度器）→ 关联一个通道 → 设备推流、ZLM 录制 →
 * 删计划（停止录制）→ `on_record_mp4` 钩子落库。整条链路都是真实后端 + 真实 ZLM。
 *
 * 返回 false 表示环境不具备条件（没有通道/设备不在线），调用方 skip。
 */
/**
 * 前端的 token 存在 cookie `gbserver_token` 里（`utils/auth.ts`），
 * 而 axios 把它放进 `access-token` 头。用 `request` fixture 直接打后端时要手动带上。
 * 另外 dev 环境前端走 `/dev-api`（vite 重写成 `/api`）。
 */
async function api(page: Page, request: APIRequestContext) {
  const token = await page.evaluate(() => {
    const m = document.cookie.match(/(?:^|; )gbserver_token=([^;]+)/)
    return m ? decodeURIComponent(m[1]) : ''
  });
  const headers = token ? { 'access-token': token } : {};
  return {
    get: (url: string) => request.get(`/dev-api${url}`, { headers }),
    post: (url: string, data: unknown) => request.post(`/dev-api${url}`, { data, headers }),
    delete: (url: string) => request.delete(`/dev-api${url}`, { headers })
  };
}

async function ensureRecording(page: Page, request: APIRequestContext): Promise<boolean> {
  const a = await api(page, request);
  // 只认**平台侧真实录制产物**：`app == 'record_info'` 的行是设备通过
  // RecordInfo 上报的设备侧录像（名字形如「录像片段1」），本机没有对应文件、
  // 也无法播放；用它做断言会得到与契约无关的失败。
  const hasRealRecord = async () => {
    const l = await a.get('/cloud/record/list?page=1&count=50');
    if (!l.ok()) return false;
    const b = (await l.json()) as { data?: { list?: Array<{ app?: string }> } };
    return (b.data?.list ?? []).some((r) => r.app !== 'record_info');
  };
  if (await hasRealRecord()) return true;

  // 找一个可用通道
  const channels = await a.get('/common/channel/list?page=1&count=5&online=true');
  const chBody = (await channels.json()) as { data?: { list?: Array<{ id: number }> } };
  const channelId = chBody.data?.list?.[0]?.id;
  if (!channelId) return false;

  const now = new Date();
  const minutes = now.getHours() * 60 + now.getMinutes();
  const start = Math.max(0, minutes - 30);
  const stop = Math.min(1440, start + 180);
  const weekday = now.getDay() === 0 ? 7 : now.getDay(); // ISO：周一=1
  const planName = `e2e-cloud-${Date.now()}`;
  await a.post('/record/plan/add', {
    name: planName,
    planItemList: [{ start, stop, weekDay: weekday }]
  });
  const q = await a.get('/record/plan/query?page=1&count=50');
  const plans = (await q.json()) as { data?: { list?: Array<{ id: number; name: string }> } };
  const planId = plans.data?.list?.find((p) => p.name === planName)?.id;
  if (!planId) return false;
  await a.post('/record/plan/link', { planId, channelIds: [channelId] });

  // 等录制产生文件（设备 INVITE + 收流 + 首帧 + ZLM 切片）
  let ok = false;
  for (let i = 0; i < 25; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    if (await hasRealRecord()) {
      ok = true;
      break;
    }
    // 调度器每 60s 才轮询一次；首轮若因设备刚注册/收流失败而没录上，
    // 重新 link 一次会**立即**唤醒调度器（后端在 link 后 notify），
    // 避免整个用例白等一分钟。
    if (i > 0 && i % 6 === 0) {
      await a.post('/record/plan/link', { planId, channelIds: [channelId] });
    }
  }
  // 停录：ZLM 在文件收尾时会触发 on_record_mp4，把记录落库（含文件大小）
  await a.delete(`/record/plan/delete?planId=${planId}`);
  for (let i = 0; i < 15 && !ok; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    if (await hasRealRecord()) {
      ok = true;
      break;
    }
  }
  await new Promise((r) => setTimeout(r, 1500));
  return ok;
}

test.describe('Cloud record page (/cloudRecord)', () => {
  // 首个用例可能要**真的录一段**（建计划→拉流→录制→落库），
  // 单这一条链路就可能 30s+，所以给整个 spec 放宽超时。
  test.describe.configure({ timeout: 150_000 });

  test('列表展示真实录像，开始时间是可读时间而不是毫秒', async ({ page, request }) => {
    // 先进入应用（cookie 需要在同源文档里才能读到），再准备录像数据
    await gotoCloudRecord(page);
    const ready = await ensureRecording(page, request);
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const rows = page.locator('.el-table__body tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 10_000 }).catch(() => {});
    if (!ready || (await rows.count()) === 0) {
      test.skip(true, '环境里没有可用通道/在线设备，无法生成录像');
      return;
    }

    // 列序：勾选/ID/App/Stream/开始/结束/设备·通道/大小/操作
    // 「开始」必须是 yyyy-MM-dd HH:mm:ss，不能是 13 位毫秒
    const startText = (await rows.first().locator('td').nth(4).innerText()).trim();
    expect(startText).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);

    // 设备/通道列必须解析出国标编号（来自流名）
    const devText = (await rows.first().locator('td').nth(6).innerText()).trim();
    expect(devText).toMatch(/3402\d+/);
  });

  test('播放：后端返回的地址真的能取到文件（含 Range）', async ({ page, request }) => {
    // 先进入应用（cookie 需要在同源文档里才能读到），再准备录像数据
    await gotoCloudRecord(page);
    const ready = await ensureRecording(page, request);
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const rows = page.locator('.el-table__body tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 10_000 }).catch(() => {});
    if (!ready || (await rows.count()) === 0) {
      test.skip(true, '环境里没有云端录像');
      return;
    }

    // 拦截 /play/path 的响应，拿到后端拼出的真实地址
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/cloud/record/play/path'), { timeout: 10_000 }),
      rows.first().getByRole('button', { name: '播放' }).click()
    ]);
    const body = (await resp.json()) as { code: number; data?: { httpPath?: string } };
    expect(body.code).toBe(0);
    const url = body.data?.httpPath ?? '';
    expect(url, '必须返回可播放地址（此前恒为空）').toBeTruthy();

    // 地址要真的能取到字节
    const head = await request.get(url, { headers: { Range: 'bytes=0-1023' } });
    expect(head.status(), `GET ${url}`).toBeGreaterThanOrEqual(200);
    expect(head.status()).toBeLessThan(300);

    // 顺带验证后端代理端点（容器化部署下本机没有文件，走的是代理）
    const rowId = (
      await page.locator('.el-table__body tbody tr').first().locator('td').nth(1).innerText()
    ).trim();
    const a = await api(page, request);
    const proxied = await a.get(`/cloud/record/download/${rowId}`);
    // 后端代理端点（容器化部署下本机没有文件，走的是从 ZLM 拉取的代理）
    // 只要求 2xx 且确实拿到字节
    expect(proxied.status()).toBeGreaterThanOrEqual(200);
    expect(proxied.status()).toBeLessThan(300);
    expect((await proxied.body()).length).toBeGreaterThan(0);
  });

  test('删除：走 DELETE + body ids，并真的从列表移除', async ({ page, request }) => {
    // 先进入应用（cookie 需要在同源文档里才能读到），再准备录像数据
    await gotoCloudRecord(page);
    const ready = await ensureRecording(page, request);
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const rows = page.locator('.el-table__body tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 10_000 }).catch(() => {});
    const before = await rows.count();
    if (!ready || before === 0) {
      test.skip(true, '环境里没有云端录像');
      return;
    }

    const rowId = (await rows.first().locator('td').nth(1).innerText()).trim();
    const [delResp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/cloud/record/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await rows.first().getByRole('button', { name: '删除' }).click();
        await page.getByRole('button', { name: '确定' }).click();
      })()
    ]);
    const body = (await delResp.json()) as {
      code: number;
      data?: { deleted?: string[]; failed?: string[] };
    };
    expect(body.code).toBe(0);
    expect(body.data?.deleted ?? []).toContain(rowId);
    expect(body.data?.failed ?? []).toHaveLength(0);

    await expect(rows).toHaveCount(before - 1, { timeout: 10_000 });
  });
});
