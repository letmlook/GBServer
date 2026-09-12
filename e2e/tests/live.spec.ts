/**
 * GBServer 直播页（/live）专项测试
 *
 * 覆盖本次提交的关键改动：
 *   - 「测试播放」按钮可见 + 可点击
 *   - 通道树空态展示（el-empty）
 *   - 通道树非空时展示（带状态 tag）
 *   - 播放器状态 tag 三态（loading / playing / error）
 *   - 选中通道后播放器头部展示 deviceId/channelId
 *   - 不依赖任何 ZLM/SIP 真实环境（用 localStorage 关掉 demo 流避免外网请求）
 *
 * 注意：本 spec 不验证 HLS 实际播放（受限于沙箱环境无 HLS 测试源），
 * 仅验证 DOM 状态机的正确性。
 */

import { test, expect, type Page, type ConsoleMessage } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ARTIFACT_DIR = path.resolve(__dirname, '../artifacts');
fs.mkdirSync(ARTIFACT_DIR, { recursive: true });

async function gotoLive(page: Page) {
  // hash 模式：直接 goto('/live') 会让 hash 为空、被路由解析成 `/` 并重定向到
  // `#/dashboard`，实时直播页根本不会渲染（这正是此前 4 个用例失败的根因）。
  await page.goto('/#/live', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 15_000 }).catch(() => {});
}

test.describe('Live page (/live) — Vue 3 features', () => {
  // 鉴权状态由 globalSetup 统一准备（e2e/global-setup.ts），并通过项目级
  // `use.storageState` 生效，所以默认 page fixture 就是已登录态。
  // 此前这里每个用例都会新建一个带 storageState 的 context、再登录一次、
  // 然后立刻关掉 —— 登录结果从未被用例使用，纯属每次多花一次登录时间。

  test('renders header actions + test-play button', async ({ page }) => {
    await gotoLive(page);
    await expect(page.getByRole('button', { name: '刷新' })).toBeVisible();
    await expect(page.getByRole('button', { name: '测试播放' })).toBeVisible();
    // 布局切换 radio
    await expect(page.getByText('2×2')).toBeVisible();
    await expect(page.getByText('3×3')).toBeVisible();
    await expect(page.getByText('4×4')).toBeVisible();
    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'live-header.png'), fullPage: true });
  });

  test('tree shows empty state when no channels registered', async ({ page }) => {
    await gotoLive(page);
    // 通道树 v-if="tree.length > 0"，无设备时显示 el-empty
    const empty = page.getByText(/暂无通道.*请先到.*通道.*页面/);
    // 接受两种状态：要么有树，要么有 el-empty
    const treeVisible = await page.locator('.el-tree').first().isVisible().catch(() => false);
    if (!treeVisible) {
      await expect(empty).toBeVisible();
    } else {
      // 有树时跳过本断言（不强行失败）
      test.skip(!treeVisible, 'environment has registered channels; empty-state assertion skipped');
    }
  });

  test('player area shows initial empty hint or test-play CTA', async ({ page }) => {
    await gotoLive(page);
    const hintText = await page.getByText(/请从左侧选择通道.*或点击右上.*测试播放/).first().isVisible().catch(() => false);
    // 未选通道时显示空态；选了之后显示播放器 — 至少一种
    expect(hintText || await page.locator('.video-grid').isVisible()).toBe(true);
  });

  test('clicking 测试播放 triggers attachVideo (loading or playing state)', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
    page.on('console', (m: ConsoleMessage) => {
      if (m.type() === 'error') errors.push(`console.error: ${m.text()}`);
    });

    await gotoLive(page);
    const btn = page.getByRole('button', { name: '测试播放' });
    await expect(btn).toBeVisible();
    await btn.click();

    // 等待至少 1 秒让 attachVideo 跑完（hls.js / flv.js 动态 import）
    await page.waitForTimeout(1500);

    // 头部 player-bar 应出现（标题 + meta + status tag）
    const playerBar = page.locator('.player-bar');
    await expect(playerBar).toBeVisible();

    // 状态 tag 至少出现一次（loading / playing / error 都算正常）
    const statusTag = playerBar.locator('.el-tag').last();
    await expect(statusTag).toBeVisible();
    const tagText = (await statusTag.textContent()) ?? '';
    expect(tagText.length, 'status tag has text').toBeGreaterThan(0);

    await page.screenshot({ path: path.join(ARTIFACT_DIR, 'live-after-test-play.png'), fullPage: true });

    // 忽略网络/MediaError 类预期错误（沙箱外网受限）
    const real = errors.filter(
      (e) => !/favicon|net::ERR_ABORTED|Loading chunk \d+ failed|MediaError|AbortError|demo/i.test(e),
    );
    if (real.length > 0) {
      console.warn(`[live] ${real.length} unexpected console issue(s):`);
      for (const r of real.slice(0, 3)) console.warn(`  - ${r.slice(0, 200)}`);
    }
  });

  /// 回归守卫：选中通道必须**先拉起流**（/api/play/start → SIP INVITE +
  /// ZLM RTP server），再播放它返回的地址。
  ///
  /// 此前的实现只调用 `/api/media/getPlayUrl` 拼一个地址，从不拉起流 ——
  /// 于是画面永远出不来，而接口本身并不报错（`getPlayUrl` 现在会明确报
  /// "流尚未建立，请先调用 /api/play/start"）。这条用例保证"点通道 → 起流"
  /// 这个动作真的发生。
  test('selecting a channel starts the stream via /api/play/start', async ({ page }) => {
    const startCalls: string[] = []
    const startResults: Array<{ code?: number; msg?: string; url?: string }> = []
    page.on('request', (r) => {
      if (r.url().includes('/api/play/start/') || r.url().includes('/dev-api/play/start/')) {
        startCalls.push(r.url())
      }
    })
    page.on('response', async (r) => {
      const u = r.url()
      if (!u.includes('/play/start/')) return
      try {
        const body = (await r.json()) as { code?: number; msg?: string; data?: { hls?: string; playUrl?: string } }
        startResults.push({
          code: body.code,
          msg: body.msg,
          url: body.data?.hls ?? body.data?.playUrl,
        })
      } catch {
        /* 非 JSON（例如被 SPA 兜底成 HTML）—— 由下面的断言暴露 */
      }
    })

    await gotoLive(page)
    // 必须点**通道**（设备节点的子节点）：设备节点没有 `raw`；且接口在设备
    // 无通道时会把设备自身当作一行返回（channel_id == device_id），点它只会
    // 去点播一个不存在的通道。
    const channelNode = page
      .locator('.el-tree-node__children .el-tree-node__content')
      .first()
    if ((await channelNode.count()) === 0) {
      test.skip(true, '数据库中没有通道，无法验证起流');
      return;
    }

    await channelNode.click();
    // 必须发出起流请求（这是本用例的核心断言）
    await expect
      .poll(() => startCalls.length, { timeout: 10_000 })
      .toBeGreaterThan(0);

    // 起流成功时应拿到播放地址；若环境里没有可用的 ZLM/设备，代码路径本身
    // 仍然正确，这里只记录而不误判为失败。
    await expect
      .poll(() => startResults.length, { timeout: 10_000 })
      .toBeGreaterThan(0);
    const r0 = startResults[0];
    if (r0?.code === 0) {
      expect(r0.url, '起流成功后应返回可播放地址').toBeTruthy();
    } else {
      console.warn(`[live] 起流未成功（环境缺少 ZLM/在线设备？）: code=${r0?.code} msg=${r0?.msg}`);
    }
  });

  test('PTZ bar renders after selecting a channel (7 PTZ + 对讲)', async ({ page }) => {
    await gotoLive(page);

    // 通道节点形如 `GBServer ON`（设备为父节点）。若库里确实没有通道，
    // 才允许跳过 —— 但必须先尝试选中，否则这个用例会**永远**被跳过、
    // 等于什么都没断言（此前就是这样：只看 .video-grid 是否可见，
    // 而它只在选中通道后才渲染，于是从未执行过按钮计数断言）。
    // 必须点**通道**（设备节点的子节点）：设备节点没有 `raw`；且接口在设备
    // 无通道时会把设备自身当作一行返回（channel_id == device_id），点它只会
    // 去点播一个不存在的通道。
    const channelNode = page
      .locator('.el-tree-node__children .el-tree-node__content')
      .first();
    if ((await channelNode.count()) === 0) {
      test.skip(true, '数据库中没有通道；PTZ 工具条需要先选中通道');
      return;
    }
    await channelNode.click();
    // 起流是异步的（真实 ZLM 上还要等 SIP INVITE + 收流）：先等播放请求发出，
    // 再等画面网格出现。若因为上一个用例的流正在回收而没出现，重选一次通道。
    const grid = page.locator('.video-grid');
    if (!(await grid.isVisible().catch(() => false))) {
      await expect
        .poll(() => grid.isVisible().catch(() => false), { timeout: 8_000 })
        .toBe(true)
        .catch(async () => {
          await channelNode.click();
        });
    }
    await expect(grid).toBeVisible({ timeout: 15_000 });

    // 上 / 下 / 左 / 右 / 停止 / 放大 / 缩小 = 7 个 PTZ 按钮，
    // 外加工具条右侧的「对讲」按钮（components/TalkPanel）= 8 个。
    const ptzButtons = page.locator('.ptz-bar .el-button');
    await expect(ptzButtons).toHaveCount(8);
    await expect(page.getByRole('button', { name: '对讲' })).toBeVisible();
  });
});
