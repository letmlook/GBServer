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
 * 用例自行创建录像计划，只允许在没有在线通道时跳过。
 */

import { test, expect, type APIRequestContext, type Page } from '@playwright/test';

async function gotoCloudRecord(page: Page) {
  await page.goto('/#/cloudRecord', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

/**
 * 保证本次计划产生一段非空 MP4。
 *
 * 建一个覆盖当前时刻的录像计划
 * （后端在 add/link 时会立即唤醒调度器）→ 关联一个通道 → 设备推流、ZLM 录制 →
 * 删计划（停止录制）→ `on_record_mp4` 钩子落库。整条链路都是真实后端 + 真实 ZLM。
 *
 * 返回 null 表示环境不具备条件（没有在线通道），调用方 skip。
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

async function expectApiSuccess(response: Awaited<ReturnType<APIRequestContext['get']>>, action: string) {
  expect(response.ok(), `${action}: HTTP ${response.status()}`).toBeTruthy();
  const body = (await response.json()) as { code: number; msg?: string };
  expect(body.code, `${action}: ${body.msg ?? ''}`).toBe(0);
}

async function ensureRecording(page: Page, request: APIRequestContext): Promise<number | null> {
  const a = await api(page, request);
  // 只认**平台侧真实录制产物**：`app == 'record_info'` 的行是设备通过
  // RecordInfo 上报的设备侧录像（名字形如「录像片段1」），本机没有对应文件、
  // 也无法播放；用它做断言会得到与契约无关的失败。
  const listRecords = async () => {
    const l = await a.get('/cloud/record/list?page=1&count=1000');
    expect(l.ok(), '录像列表请求失败').toBeTruthy();
    const b = (await l.json()) as { code: number; data?: { list?: Array<{ id: number; app?: string; size?: number; fileName?: string; filePath?: string }> } };
    expect(b.code, '录像列表业务失败').toBe(0);
    return b.data?.list ?? [];
  };
  const oldIds = new Set((await listRecords()).map((r) => r.id));
  const newRecord = async (channelGbId: string) => (await listRecords()).find((r) =>
    !oldIds.has(r.id) && r.app !== 'record_info' && r.fileName?.toLowerCase().endsWith('.mp4') &&
    (r.size ?? 0) > 0 && !!r.filePath && r.stream?.includes(channelGbId));

  // 找一个可用通道
  const channels = await a.get('/common/channel/list?page=1&count=5&online=true');
  expect(channels.ok(), '在线通道查询失败').toBeTruthy();
  const chBody = (await channels.json()) as { code: number; data?: { list?: Array<{ id: number; channelId: string }> } };
  expect(chBody.code, '在线通道查询业务失败').toBe(0);
  const channel = chBody.data?.list?.[0];
  const channelId = channel?.id;
  if (!channelId || !channel?.channelId) return null;

  const now = new Date();
  const minutes = now.getHours() * 60 + now.getMinutes();
  const start = Math.max(0, minutes - 30);
  const stop = Math.min(1440, start + 180);
  const weekday = now.getDay() === 0 ? 7 : now.getDay(); // ISO：周一=1
  const planName = `e2e-cloud-${Date.now()}`;
  const add = await a.post('/record/plan/add', {
    name: planName,
    planItemList: [{ start, stop, weekDay: weekday }]
  });
  await expectApiSuccess(add, '创建录像计划');
  const q = await a.get('/record/plan/query?page=1&count=50');
  const plans = (await q.json()) as { data?: { list?: Array<{ id: number; name: string }> } };
  const planId = plans.data?.list?.find((p) => p.name === planName)?.id;
  expect(planId, '找不到本次创建的计划').toBeDefined();
  try {
    const link = await a.post('/record/plan/link', { planId, channelIds: [channelId] });
    await expectApiSuccess(link, '关联录像通道');
    // INVITE、首帧与 ZLM 切片需要真实录制窗口；落库通常发生在停录后。
    await new Promise((r) => setTimeout(r, 20_000));
  } finally {
    const stopResponse = await a.delete(`/record/plan/delete?planId=${planId}`);
    await expectApiSuccess(stopResponse, '停止录像计划');
  }
  for (let i = 0; i < 30; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    const record = await newRecord(channel.channelId);
    if (record) return record.id;
  }
  throw new Error(`计划 ${planId}、通道 ${channelId} 未产生新的非空 MP4 数据库记录`);
}

test.describe.serial('Cloud record page (/cloudRecord)', () => {
  // 首个用例可能要**真的录一段**（建计划→拉流→录制→落库），
  // 单这一条链路就可能 30s+，所以给整个 spec 放宽超时。
  test.describe.configure({ timeout: 150_000 });
  let recordedId: number | null = null;

  const recordRow = (page: Page, id: number) => page.locator('.el-table__body tbody tr').filter({
    has: page.locator('td').nth(1).getByText(String(id), { exact: true })
  });

  test('列表展示真实录像，开始时间是可读时间而不是毫秒', async ({ page, request }) => {
    // 先进入应用以读取登录 cookie，然后创建本次录制。
    await gotoCloudRecord(page);
    recordedId = await ensureRecording(page, request);
    if (!recordedId) test.skip(true, '没有在线通道，无法开始录制');
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const row = recordRow(page, recordedId!);
    await expect(row).toBeVisible({ timeout: 10_000 });

    // 列序：勾选/ID/App/Stream/开始/结束/设备·通道/大小/操作
    // 「开始」必须是 yyyy-MM-dd HH:mm:ss，不能是 13 位毫秒
    const startText = (await row.locator('td').nth(4).innerText()).trim();
    expect(startText).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);

    // 设备/通道列必须解析出国标编号（来自流名）
    const devText = (await row.locator('td').nth(6).innerText()).trim();
    expect(devText).toMatch(/3402\d+/);
  });

  test('播放：后端返回的地址真的能取到文件（含 Range）', async ({ page, request }) => {
    // 后续动作复用首个用例产生的记录，避免再次录制或选中历史文件。
    await gotoCloudRecord(page);
    if (!recordedId) test.skip(true, '前置用例没有在线通道');
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const row = recordRow(page, recordedId!);
    await expect(row).toBeVisible({ timeout: 10_000 });

    // 拦截 /play/path 的响应，拿到后端拼出的真实地址
    const [resp] = await Promise.all([
      page.waitForResponse((r) => r.url().includes('/cloud/record/play/path'), { timeout: 10_000 }),
      row.getByRole('button', { name: '播放' }).click()
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
    const rowId = String(recordedId);
    const a = await api(page, request);
    const proxied = await a.get(`/cloud/record/download/${rowId}`);
    // 后端代理端点（容器化部署下本机没有文件，走的是从 ZLM 拉取的代理）
    // 只要求 2xx 且确实拿到字节
    expect(proxied.status()).toBeGreaterThanOrEqual(200);
    expect(proxied.status()).toBeLessThan(300);
    expect((await proxied.body()).length).toBeGreaterThan(0);
  });

  test('删除：走 DELETE + body ids，并真的从列表移除', async ({ page, request }) => {
    // 删除的对象仅为首个用例创建的记录。
    await gotoCloudRecord(page);
    if (!recordedId) test.skip(true, '前置用例没有在线通道');
    // 用 reload 而不是再 goto 同一个 URL：hash 路由下重复 goto 可能被当成
    // 同文档导航，页面不会重新 mount/拉数据，列表仍是空态
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
    const row = recordRow(page, recordedId!);
    await expect(row).toBeVisible({ timeout: 10_000 });

    const rowId = String(recordedId);
    const [delResp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes('/cloud/record/delete') && r.request().method() === 'DELETE',
        { timeout: 10_000 }
      ),
      (async () => {
        await row.getByRole('button', { name: '删除' }).click();
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

    await expect(row).toHaveCount(0, { timeout: 10_000 });
  });
});
