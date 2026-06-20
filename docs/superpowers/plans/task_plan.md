# Task Plan: 定制 Phase 6 实施计划

<!--
  WHAT: 本任务的路线图；磁盘工作记忆。
  WHY: 50+ 工具调用后目标会丢；这里保留"主线 + 决策"。
  WHEN: 任何动作前先创建；每个阶段后更新。
-->

## Goal
基于 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6 与当前 main 分支代码现状，输出一份可执行的 Phase 6 实施计划 markdown，落到 `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`。

## Current Phase
完成（已交付 phase-6 实施计划）

## Phases

### Phase 1: 阅读设计文档与已有阶段计划
- [x] 阅读 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6
- [x] 阅读 `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md` 摸清规划风格
- [x] 阅读 `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md` 摸清约束/衔接
- [x] 阅读 `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md` 找 live/playback/record 真实链路模板
- [x] 阅读 `docs/superpowers/plans/progress.md` / `findings.md` 找 Phase 5 落地范式
- **Status:** complete

### Phase 2: 审计 JT808/JT1078 当前代码现状
- [x] 实地审计 `src/jt1078/{mod,manager,session,server,command,command_waiter,jt_media_session}.rs`
- [x] 实地审计 `src/handlers/jt1078.rs`（1503 行）+ `src/db/jt1078.rs`（476 行）
- [x] 提炼关键发现：`JtCommandWaiter` 已存在但 0 引用；`JtMediaSessionManager` 已存在但 0 引用；25+ handler 返 `build_success` 占位
- [x] 与 phase-3/4/5 衔接点核对（Phase 3 RecordInfo 多包 / Phase 4 StreamState trait / Phase 5 CascadeRegistrar 模式）
- **Status:** complete

### Phase 3: 编写 phase-6 实施计划文档
- [x] 在 `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md` 输出最终计划
- [x] 6 个子任务：6.1 标准 JT/T 808 注册 / 6.2 JtCommandWaiter 接入 / 6.3 实时视频+回放真实链路 / 6.4 录像检索+下载+上传真实链路 / 6.5 终端参数+位置+OSD 真实链路 / 6.6 横切+三库+文档
- [x] 关键代码骨架：build_register_response 0x8100 / response_parser 5 类解析 / send_command_and_wait / MediaWaiter / 移除 127.0.0.1 占位
- [x] 风险 R1-R7 + 衔接说明 + 完成判定对齐 phase-4/5 风格
- **Status:** complete

## Key Questions
1. JT 终端命令等待（phone+msg_id+serial）如何与 SIP PendingRequestManager（device_id+sn）模式对齐？
2. JtMediaSessionManager 是否需要复用 Phase 3 MediaWaiterManager？
3. 实时视频 RTP 端口分配与 ZLM openRtpServer 如何串通？
4. 移除 `127.0.0.1/live/...` 占位 URL 的影响范围？
5. 多包媒体检索 0x0800/0x0801 协议复杂度如何控制？

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Phase 6 范围以设计文档 §7 为准（5 个子任务 + 1 横切） | phase-4/5 已证明 5-6 子任务粒度最稳 |
| `JtCommandWaiter` 接入方式：在 `Jt1078Manager` 持有 `Arc<JtCommandWaiter>` + `Arc<JtMediaSessionManager>` + `Arc<Pool>` 三件套 | 避免散落在各处 |
| 6.2 拆 15+ `send_*_and_wait` 方法 | 6.3-6.5 全部 handler 改造都依赖 |
| 6.3 拆 3 个子步骤：创 session → 等 0x0001 → 等 ZLM 媒体 | R1 风险分解 |
| 6.4 多包媒体检索简化为 0x8802 + 0x0801 单包聚合 | R3 风险控制；多包 start+middle+end 放 6.4-followup |
| 6.5 位置从 DB 读（`gb_jt_terminal.last_lat`/`last_lng`），0x8201 兜底 | 避免占位 `{longitude: 0.0, latitude: 0.0}` |
| 移除 `127.0.0.1/live/...` 占位 URL | 设计文档 §6.1 禁项 |
| 沿用 phase-4/5 的三库 cfg + tests/integration/ 模式 | 与既有 CI 矩阵一致 |
| 终端鉴权码走 DB `auth_code` 替代 env-var 临时 token | 设计文档 §7 Phase 6.1 + 安全考量 |
| TCP/UDP 端口从 `Jt1078Config` 读，可配置 | 避免与 SIP 端口冲突 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| 暂无 | 1 | — |

## Notes
- 计划最终落点：`docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`（~700 行）
- 写作风格对齐 phase-4 / phase-5 计划（任务编号 + File + 关键代码骨架 + 验收）
- 避免引入"占位 URL / 假成功响应"，遵循设计文档 §6.1「每个协议请求需要完整生命周期」
- Phase 5 基线 commit: `62b8768`；Phase 6 从此处开始
