# Task Plan: 定制 Phase 6 / Phase 7 实施计划

<!--
  WHAT: 本任务的路线图；磁盘工作记忆。
  WHY: 50+ 工具调用后目标会丢；这里保留"主线 + 决策"。
  WHEN: 任何动作前先创建；每个阶段后更新。
-->

## Goal
基于 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6 / Phase 7 与当前 main 分支代码现状（phase-5 已收尾），分别产出可执行的 Phase 6 / Phase 7 实施计划 markdown，落到：
- `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`
- `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`

## Current Phase
编写 Phase 6 / Phase 7 实施计划（task #3 in_progress，task #4 pending）

## Phases

### Phase 1: 阅读设计文档与已有阶段计划
- [x] 阅读 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6 / Phase 7
- [x] 阅读 phase-3 / phase-4 / phase-5 实施计划摸风格
- **Status:** complete

### Phase 2: 审计 JT1078 当前代码现状
- [x] 读 `src/jt1078/{mod,manager,server,session,frame,command,command_waiter,jt_media_session}.rs`
- [x] 读 `src/handlers/jt1078.rs`（1503 行 35+ handlers）
- [x] 读 `src/handlers/jt1078_extra.rs`
- [x] 读 `src/db/jt1078.rs`（gb_jt_terminal 表）
- [x] 审计 router 中 `/api/jt1078/*` 路由
- [x] 提炼 Phase 6 缺口
- **Status:** complete

### Phase 3: 审计 Redis/StateStore/RPC 当前代码现状
- [x] 读 `src/state_store.rs`（1062 行，StateBackend trait + InMemory + Redis 实现）
- [x] 读 `src/rpc.rs`（419 行，LocalRpc + RpcRouter + 3 个标准 handler）
- [x] 读 `src/handlers/rtp_control.rs`（D2 已完成 RTP/PS 路由）
- [x] 读 `src/handlers/websocket.rs`（in-memory tx_map 单节点）
- [x] 读 `src/router.rs` 中 `/api/rtp/*` `/api/ps/*` `/api/alarm/*` `/api/ws` 路由策略
- [x] 提炼 Phase 7 缺口
- **Status:** complete

### Phase 4: 编写 Phase 6 实施计划文档
- [ ] 落 `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`
- [ ] 风格对齐 phase-5（Context / 差距 / 子任务 / File Structure / 验收 / 风险）
- **Status:** in_progress

### Phase 5: 编写 Phase 7 实施计划文档
- [ ] 落 `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`
- [ ] 风格对齐 phase-5
- **Status:** pending

## Key Questions
1. JT1078 live_start 是否要等 ZLM 媒体到达 hook 才返回 stream URL（与 Phase 3.1 live 等 hook 对齐）？
2. `JtCommandWaiter` / `JtMediaSessionManager` 已实现但 handlers 未挂载——是改 handlers 还是新建 service 层？
3. JT1078 注册/auth 真实协议 vs 当前「简单 token」如何升级？
4. Phase 7 Redis StateStore 已实现，但很多 domain（CSEQ/SN/SSRC、alarm pub/sub、GPS history）覆盖不完整——是扩展 trait 还是新建专门 Redis service？
5. 跨节点 RPC：RedisRpc 只在 docstring 提及，LocalRpc 是唯一实现——是否在本阶段补 RedisRpc？

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Phase 6 拆 6 子任务：6.1 注册/auth/heartbeat 真实化 / 6.2 直播 start/stop 等终端 ack + ZLM hook / 6.3 回放 start/control 多包 / 6.4 录像列表多包 / 6.5 通用命令 PTZ/text/phonebook/fence/route/config/attribute 等真实化 / 6.6 横切 | 设计文档 Phase 6 5 项任务粒度合理 |
| `JtCommandWaiter` / `JtMediaSessionManager` 直接挂入 handlers（不新建 service 层）| 已实现但未使用，单点改动 |
| Phase 6 不重写 session.rs 的「简单 token」认证；保留为开发模式（兼容旧终端），用「真实 auth」做新分支（向后兼容） | 不破坏既有 JT 终端测试 |
| Phase 7 拆 5 子任务：7.1 Redis StateStore 扩展（CSEQ/SN/SSRC + GPS history） / 7.2 RedisRpc 实现 + 跨节点 RPC / 7.3 WebSocket 多节点 fanout / 7.4 `/api/rtp` `/api/ps` 协议语义 + audit log / 7.5 安全路由策略 + 健康/日志/指标 | 设计文档 Phase 7 5 项任务粒度合理 |
| Phase 7 不重写 LocalRpc；新建 `RedisRpc` 作为第二实现，注册到 RpcRouter 双模式切换 | 最小侵入 |
| Phase 7 WebSocket 跨节点走 Redis Pub/Sub；StateStore.subscribe 不动，新增 `ws_fanout` channel | 复用 Redis 基础设施 |
| Phase 6/7 都继承 phase-5 的「三库 cfg + tests/integration/sqlite_compat.rs 模式」 | 与既有 CI 矩阵一致 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| 暂无 | 1 | — |

## Notes
- 计划最终落点：`docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md` 与 `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`
- 写作风格对齐 phase-3 / phase-4 / phase-5 计划（任务编号 + File + 关键代码骨架 + 验收）
- 避免引入"占位 URL / 假成功响应"，遵循设计文档 §6.1「每个协议请求需要完整生命周期」