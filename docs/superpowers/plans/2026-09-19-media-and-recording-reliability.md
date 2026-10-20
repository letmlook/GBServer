# 媒体与录像可靠性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除已出流点播的无效 RTP 回连，并让录像计划与 `cloudRecord` E2E 以本轮形成的录像产物作为成功证据。

**Architecture:** 普通播放以 `MediaWaiter` 的媒体就绪为权威，不再以 SDP 端口差异触发二次连接；TCP-PASSIVE 保留主动连接和精确清理。录像调度保留有界重试，E2E 在有效录制窗口后停止计划并等待 MP4 落库。

**Tech Stack:** Rust/Axum/SQLx/Tokio、ZLMediaKit Hook、Playwright/TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-19-media-recording-reliability-design.md`

## Global Constraints

- A1 保留待办和既有 `/api/sy/*`；A3 不实现。
- 所有 HTTP 成功响应使用 `ApiResult<T>`；不得恢复粗粒度 SIP BYE 定位。
- 只有“流未出现”可重试，且重试必须有固定预算。
- E2E 不得用固定短 sleep 或 HTTP 200 作为录像成功依据。
- 每项完成后运行定向验证并独立提交。

## Review Focus

- SDP 端口不同但媒体已到达时，不得调用 `connectRtpServer` 或中断播放。
- TCP-PASSIVE 主动连接失败时，RTP Server 与 `StreamType::Play` 会话必须都被清理。
- 不可恢复的 `startRecord` 错误只能尝试一次。
- 历史录像不能被当作本轮 E2E 产物。
- 后端或 Mock 退出必须报环境故障，不能被 skip 掩盖。

---

### Task 1: 修正媒体就绪后的 RTP 回连

**Files:**
- Modify: `src/handlers/play.rs:404-440`
- Modify: `src/sip/server.rs:6877-6920`
- Test: 播放相关 Rust 测试模块

**Interfaces:**
- Consumes: `SipServer::send_play_invite_and_wait_media(...) -> Result<(String, String, Option<SipResponse>), _>`
- Produces: 普通路径只记录 SDP 端口差异；TCP-PASSIVE 路径继续调用 `connect_rtp_server`。

- [ ] **Step 1: 写失败回归测试**

构造“SDP 端口 20002、RTP 端口 30066、媒体已就绪”的请求，断言播放成功且 ZLM 的 `connect_rtp_server` 调用次数为零；构造 TCP-PASSIVE 连接失败，断言调用 `close_rtp_server(stream_id)` 和 `send_session_bye_by_type(..., StreamType::Play)`。

- [ ] **Step 2: 运行失败测试**

Run: `cargo test <播放回归测试名> --lib`

Expected: 当前普通路径仍调用 `connect_rtp_server`，断言失败。

- [ ] **Step 3: 实现最小行为变更**

删除 `play.rs` 及 `SipServer::start_live_stream` 中媒体已就绪后的回连块，替换为结构化 info 日志：

```rust
tracing::info!(
    device_id, channel_id,
    sdp_media_port = device_port,
    zlm_rtp_port = rtp_server.port,
    "SDP port differs after media is ready; keeping active RTP receiver"
);
```

TCP-PASSIVE 的失败分支继续精确关闭 RTP 与 SIP 会话，并将错误文本明确为传输模式/ZLM 兼容性失败。

- [ ] **Step 4: 验证**

Run: `cargo test <播放回归测试名> --lib && cargo test --lib`

Expected: 普通路径没有回连请求；TCP-PASSIVE 失败仍完成清理。

- [ ] **Step 5: 提交**

Run: `git add src/handlers/play.rs src/sip/server.rs && git commit -m "fix(play): avoid RTP reconnect after media ready"`

### Task 2: 为录像启动重试建立可测试边界

**Files:**
- Modify: `src/scheduler/record_plan.rs:26-65,511-540`
- Test: `src/scheduler/record_plan.rs`

**Interfaces:**
- Consumes: `ZlmClient::start_record("1", "__defaultVhost__", app, stream)`
- Produces: 私有 `start_record_with_retry_fn`，接受单次异步尝试并供生产 wrapper 调用。

- [ ] **Step 1: 写失败测试**

测试结果序列 `[stream-not-found, Ok]` 恰好尝试两次；连续 stream-not-found 在 `attempts=3` 时恰好三次并返回最后错误；`[connection-refused]` 只尝试一次。

- [ ] **Step 2: 运行失败测试**

Run: `cargo test start_record_retry --lib`

Expected: 当前 helper 直接依赖 `ZlmClient`，无法注入结果序列和断言尝试次数。

- [ ] **Step 3: 实现最小 helper**

抽取接收 `FnMut() -> Future<Output = anyhow::Result<()>>` 的私有 helper；已有 `start_record_with_retry` 用闭包委托 `zlm.start_record`。保持生产参数 25 次、800ms 与既有“already recording”幂等处理不变。

- [ ] **Step 4: 验证**

Run: `cargo test start_record_retry --lib && cargo test scheduler::record_plan --lib`

Expected: 三种结果的尝试次数都符合断言。

- [ ] **Step 5: 提交**

Run: `git add src/scheduler/record_plan.rs && git commit -m "test(recording): cover bounded start retry"`

### Task 3: 使 cloudRecord E2E 使用本轮录像证据

**Files:**
- Modify: `e2e/tests/cloudRecord.spec.ts:40-115`
- Create/Modify: `e2e/test-helpers/runtime-health.ts`
- Modify: `e2e/global-setup.ts`

**Interfaces:**
- Consumes: `/record/plan/{add,link,delete}`、`/cloud/record/list`、`/health`、`/ready`
- Produces: `ensureRecording(...) -> Promise<{ recordId: string } | null>`

- [ ] **Step 1: 写失败断言**

在创建计划前保存录像 ID 集合；要求 `ensureRecording` 返回的记录不在初始集合中，且 `app !== "record_info"`。

- [ ] **Step 2: 运行失败测试**

Run: `cd e2e && npx playwright test tests/cloudRecord.spec.ts --project=chromium`

Expected: 旧实现可把历史录像当成功，无法满足本轮记录断言。

- [ ] **Step 3: 实现确定性闭环**

创建唯一名称计划并关联在线通道后，轮询流/录制开始，保持最短有效录制窗口，再删除计划；在总超时内轮询新 ID 的 MP4 记录。每次长轮询先读取 health/ready；无在线通道可 skip，运行中服务不健康必须抛测试失败，错误包含计划名、通道、初始/当前记录 ID 和健康摘要。

- [ ] **Step 4: 验证**

Run: `cd e2e && npx playwright test tests/cloudRecord.spec.ts --project=chromium`

Expected: 三条用例只消费本轮形成的 MP4；环境退出显示诊断失败。

- [ ] **Step 5: 提交**

Run: `git add e2e && git commit -m "test(e2e): make cloud recording evidence deterministic"`

### Task 4: 记录 Mock 与服务生命周期

**Files:**
- Modify: `mock/tools/sip-device/sip_device_mock.py:872-881`
- Create: Mock 的最小 Python 回调测试
- Modify: `e2e/README.md` 与启动 helper

**Interfaces:**
- Consumes: asyncio `connection_lost(exc)`、`error_received(exc)`
- Produces: 回调只记录 warning；E2E 仅清理本次记录的 PID/进程组。

- [ ] **Step 1: 写失败测试**

直接调用 `connection_lost(None)`、`connection_lost(OSError("closed"))` 和 `error_received(OSError("udp"))`，断言不抛异常。

- [ ] **Step 2: 实现与验证**

补齐必要的可导入测试入口；启动 helper 记录 PID、日志、退出码，并在长轮询前健康检查。清理仅处理记录的 PID。

Run: `python -m unittest <mock 测试模块>`

Expected: 回调不会使 Mock 崩溃。

- [ ] **Step 3: 提交**

Run: `git add mock e2e && git commit -m "test(e2e): report service lifecycle failures"`

### Task 5: 文档与最终门禁

**Files:**
- Modify: `docs/OPEN_ISSUES.md`
- Modify: `docs/STATUS.md`

- [ ] **Step 1: 更新状态**

回写 B1 新行为、B2/B3 已提交状态、B4 测试编排边界、B5 的确定性证据；A1 保留，A3 范围外。

- [ ] **Step 2: 运行最终验证**

Run: `cargo fmt --check && cargo test --lib && git diff --check`

Run: `cd web && npm run build`

Run: `cd e2e && npx playwright test tests/cloudRecord.spec.ts --project=chromium`

Expected: 所有已具备环境的命令退出码为零；环境未启动则记录阻塞而不宣称通过。

- [ ] **Step 3: 提交**

Run: `git add docs/OPEN_ISSUES.md docs/STATUS.md && git commit -m "docs: record media reliability verification"`

