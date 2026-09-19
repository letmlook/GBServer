/**
 * GBServer 控制台（/#/dashboard）专项测试 —— 「重点通道」去重回归
 *
 * 背景（2026-09-19 实测）：ZLM `getMediaList` 对**同一路流按协议各返回一行**
 * （rtsp / rtmp / hls / ts / fmp4）。一路
 * `rtp/34020000001320000001_34020000001320000001` 就是 5 行，
 * `/api/device/query/streams` 原样透出，于是控制台「重点通道」此前取前 6 行铺格子
 * → 同一个通道的视频在面板上出现 3~5 个格子（用户报的「相同的通道视频会出现多个」）。
 *
 * 本 spec 用固定桩数据把这个回归锁死：同一通道无论有多少协议行，只允许 1 个格子。
 */

import { test, expect, type Page } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ARTIFACT_DIR = path.resolve(__dirname, '../artifacts');
fs.mkdirSync(ARTIFACT_DIR, { recursive: true });

/** 一路实时流：设备 ID 与通道 ID 相同（国标通道的常见形态） */
const DEVICE_A = '34020000001320000001';
const CHANNEL_A = '34020000001320000001';
/** 一路只有回放流的通道：流名 = 设备ID_通道ID_开始_结束 */
const DEVICE_B = '34020000001320000002';
const CHANNEL_B = '34020000001320000002';
const PLAYBACK_B = `${DEVICE_B}_${CHANNEL_B}_20260101120000_20260101130000`;

/** ZLM 对一路流会返回的协议行 */
const SCHEMAS = ['rtsp', 'rtmp', 'hls', 'ts', 'fmp4'];

function schemaRows(deviceId: string, channelId: string, stream: string) {
  return SCHEMAS.map((schema) => ({
    deviceId,
    channelId,
    mediaServerId: 'zlmediakit-1',
    schema,
    app: 'rtp',
    stream,
    vhost: '__defaultVhost__',
    // 读者数只挂在真正被播放的那个协议行上（实测 rtsp 行有 readers，其余为 0）
    readerCount: schema === 'rtsp' ? 2 : 0,
    totalReaderCount: schema === 'rtsp' ? 2 : 0,
    aliveSecond: 30
  }));
}

const STREAM_FIXTURE = {
  code: 0,
  msg: '成功',
  data: {
    total: 11,
    list: [
      ...schemaRows(DEVICE_A, CHANNEL_A, `${DEVICE_A}_${CHANNEL_A}`),
      ...schemaRows(DEVICE_B, CHANNEL_B, PLAYBACK_B),
      // 推流没有国标标识，不是"通道"，不得进重点通道
      {
        deviceId: '',
        channelId: '',
        mediaServerId: 'zlmediakit-1',
        schema: 'rtmp',
        app: 'push',
        stream: 'push_smoke_test',
        vhost: '__defaultVhost__',
        readerCount: 0
      }
    ]
  }
};

async function gotoDashboard(page: Page) {
  // 路由是 hash 模式：写 '/dashboard' 会被解析成 '/' 再重定向，页面仍在控制台，
  // 但断言的目标是"控制台页面"本身，所以这里显式带 hash 并校验。
  await page.goto('/#/dashboard', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

function panel(page: Page) {
  return page.locator('section.gb-card').filter({ hasText: '重点通道' });
}

function channelTitles(page: Page) {
  return panel(page).locator('.gb-video-cell__overlay-bottom');
}

test.describe('Dashboard 重点通道（/#/dashboard）', () => {
  test('同一通道的多种协议行只渲染一个格子', async ({ page }) => {
    await page.route('**/device/query/streams*', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(STREAM_FIXTURE)
      })
    );

    await gotoDashboard(page);
    expect(page.url()).toContain('#/dashboard');

    // 11 行原始数据 = 2 个通道（A 实时 / B 回放）+ 1 路推流；
    // 面板只允许出现 2 个格子，且不能有重复通道。
    await expect(channelTitles(page)).toHaveCount(2);
    const titles = await channelTitles(page).allInnerTexts();
    expect(new Set(titles).size).toBe(titles.length);
    // 实时流排在回放流前面，且展示的是实时流名（不带时间戳后缀）
    expect(titles[0]).toBe(`${DEVICE_A}_${CHANNEL_A}`);
    expect(titles[1]).toBe(PLAYBACK_B);

    // 面板通常在第一屏之外，单独截面板本身（就是"同一个通道出现多个"的可视证据）
    await panel(page).screenshot({
      path: path.join(ARTIFACT_DIR, 'dashboard-channels-dedupe.png')
    });
  });

  test('system/info 再慢也不能拖住其它面板（谁先回来谁先渲染）', async ({ page }) => {
    // `/api/server/system/info` 服务端要真采 CPU/网络 + 读磁盘，本机单次约 0.8s，
    // 浏览器里常到 1.2~1.5s。此前控制台是 `await Promise.allSettled([5 个接口])`
    // 之后才统一赋值 —— 于是"设备数 / 流列表 / 告警 / 重点通道"这些 0.5s 就回来的
    // 数据也要陪着它等，用户看到的就是"打开控制台要等约 2s 才出数据"。
    //
    // 这里把 system/info **永久挂起**（不返回），断言其它面板照样渲染：
    // 旧实现下这个断言会一直等到超时失败。
    let releaseInfo!: () => void;
    const infoGate = new Promise<void>((resolve) => {
      releaseInfo = resolve;
    });
    await page.route('**/server/system/info*', async (route) => {
      await infoGate;
      await route.continue();
    });
    await page.route('**/device/query/streams*', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(STREAM_FIXTURE)
      })
    );

    await page.goto('/#/dashboard', { waitUntil: 'domcontentloaded' });

    // system/info 仍挂起时，重点通道就必须已经渲染出 2 个通道
    await expect(channelTitles(page)).toHaveCount(2, { timeout: 10_000 });
    // 「活跃通道」卡片右侧的「直播 N」此时只能靠流列表自己算：桩数据 11 行去重后 = 3 路
    await expect(page.getByText('直播 3')).toBeVisible();

    releaseInfo();
  });

  test('没有活跃流时显示空态，而不是留着上一次的旧格子', async ({ page }) => {
    // 旧实现里 `if (liveList.length > 0)` 只在有流时才重建 channels，
    // 流全部停止后面板会一直挂着"LIVE"的旧格子与旧缩略图。
    await page.route('**/device/query/streams*', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ code: 0, msg: '成功', data: { total: 0, list: [] } })
      })
    );

    await gotoDashboard(page);
    await expect(channelTitles(page)).toHaveCount(0);
    await expect(panel(page).getByText(/暂无正在拉流的通道/)).toBeVisible();
  });

  test('真实环境：重点通道里的通道不重复', async ({ page }) => {
    // 不拦截接口 —— 直接看真实 ZLM 流列表渲染出来的面板。
    // 该断言与环境里有多少路流无关：0 路 / 2 路 / 20 路都必须"通道唯一"。
    await gotoDashboard(page);
    const titles = await channelTitles(page).allInnerTexts();
    expect(new Set(titles).size).toBe(titles.length);

    await panel(page).screenshot({
      path: path.join(ARTIFACT_DIR, 'dashboard-channels-real.png')
    });
  });
});
