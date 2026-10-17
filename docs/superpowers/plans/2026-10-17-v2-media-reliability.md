# 2.0.0 媒体与录像可靠性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除已收流后的无效 RTP 回连，并以本轮新 MP4 作为录像成功证据。

**Architecture:** 普通播放以 ZLM 媒体就绪为判据；TCP-PASSIVE 维持主动连接并精确回收。调度器的重试条件用定向测试固定；Playwright 等待新录像文件及记录，并监测依赖进程。

**Tech Stack:** Rust、Tokio、ZLMediaKit、Playwright、TypeScript。

**Spec:** `docs/superpowers/specs/2026-09-19-media-recording-reliability-design.md`

## Global Constraints

- 不修改现有 `/api/sy/*`，不实现 `/api/v1/*`。
- 只重试“流未出现”；维持生产预算 25 次、800ms。
- 成功证据须为本轮新 MP4 及数据库记录；接口 200 不足以通过。
- Mock 已有 `connection_lost` 与 `error_received`，先核对现状，不重复改写。

## Review Focus

- 媒体已就绪而 SDP 端口不同时不中断播放；任务 1 测试覆盖。
- TCP-PASSIVE 连接失败无 RTP Server 和 Play 会话残留；任务 1 测试覆盖。
- 不可恢复 startRecord 错误只尝试一次；任务 2 测试覆盖。
- 历史 MP4 不可满足本轮录制断言；任务 3 测试覆盖。
- 后端、ZLM、Mock 退出必须报环境错误而非跳过；任务 3 测试覆盖。

---

### Task 1: 播放收流与清理

**Files:** Modify `src/handlers/play.rs`、`src/sip/server.rs`；Test 同文件测试模块。

**Interfaces:** 普通路径在 `send_play_invite_and_wait_media` 成功后只记录端口差异；TCP-PASSIVE 保留 `connect_rtp_server`，失败调用 `close_rtp_server` 与按 `StreamType::Play` 定位的会话清理。

- [ ] **Step 1: 写失败测试。** SDP 20002、ZLM 30066 且媒体 ready 时，断言无 connect 调用；TCP-PASSIVE connect 失败时断言 RTP 与 Play 会话均释放。
- [ ] **Step 2: 验证失败。** 运行 `cargo test --lib play` 和 SIP 播放定向测试，保留触发当前问题的输出。
- [ ] **Step 3: 最小实现。** 删除媒体 ready 后普通路径的二次 connect；日志记录端口差异，TCP-PASSIVE 错误携传输模式与 ZLM 摘要。
- [ ] **Step 4: 验证通过。** 运行同一批定向测试；在真实 ZLM + Mock 环境证明 SDP 端口不一致仍可持续播放与停止。
- [ ] **Step 5: 提交。** `git add src/handlers/play.rs src/sip/server.rs && git commit -m "fix(play): skip reconnect after media ready"`

### Task 2: 录像启动重试边界

**Files:** Modify `src/scheduler/record_plan.rs`；Test 同文件测试模块。

**Interfaces:** 保持 `RecordPlanScheduler::start_record_with_retry` 的对外行为，抽出可注入单次 `start_record` 结果的私有 helper，便于断言尝试次数。

- [ ] **Step 1: 写失败测试。** `[stream-missing, success]` 两次；25 次 missing 后失败；`already recording` 幂等成功；连接拒绝只一次。
- [ ] **Step 2: 验证失败。** `cargo test --lib start_record_retry`，记录尚无可注入边界或错误分类不符。
- [ ] **Step 3: 最小实现。** 仅调整测试接缝与必要错误分类，维持 25 次、800ms，不扩展重试范围。
- [ ] **Step 4: 验证通过。** `cargo test --lib start_record_retry` 与 `cargo test --lib scheduler::record_plan`。
- [ ] **Step 5: 提交。** `git add src/scheduler/record_plan.rs && git commit -m "test(record): cover bounded start retries"`

### Task 3: 新 MP4 的 E2E 证据

**Files:** Modify `e2e/tests/cloudRecord.spec.ts`、`e2e/global-setup.ts`；Create `e2e/test-helpers/runtime-health.ts`；Modify `e2e/README.md`。

**Interfaces:** `ensureRecording` 返回本轮 `planId`、`streamId`、`recordId` 与文件路径；初始录像 ID 集合在创建计划前采集。健康 helper 在轮询期间检查已启动后端、ZLM、Mock 的存活。

- [ ] **Step 1: 写失败测试。** 历史记录和 `app=record_info` 不能通过；依赖进程中途退出应带 PID/日志路径失败；有效新 MP4 + DB 记录通过。
- [ ] **Step 2: 验证失败。** `cd e2e && npx playwright test tests/cloudRecord.spec.ts`，保留当前结果与环境信息。
- [ ] **Step 3: 最小实现。** 唯一计划名，等待录制开始并保持有效窗口后停止；轮询新 MP4 文件及对应 DB 记录，超时输出计划、流、Hook 和文件诊断。只清理本轮创建的计划和进程。
- [ ] **Step 4: 验证通过。** 在 ZLM + Mock 环境运行同一 spec；明确标注无真实设备时只能算 Mock 证据。
- [ ] **Step 5: 提交。** `git add e2e && git commit -m "test(e2e): require current cloud recording artifact"`
