# Task Plan: 定制 Phase 7 实施计划

<!--
  WHAT: 本任务的路线图；磁盘工作记忆。
  WHY: 50+ 工具调用后目标会丢；这里保留"主线 + 决策"。
  WHEN: 任何动作前先创建；每个阶段后更新。
-->

## Goal
基于 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 7 与当前 main 分支代码现状（基线 `79bfb29` Phase 6.1 已完成），输出一份可执行的 Phase 7 实施计划 markdown，落到 `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`。

## Current Phase
完成（已交付 phase-7 实施计划）

## Phases

### Phase 1: 阅读设计文档与已有阶段计划
- [x] 阅读 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 7（5 任务 + Acceptance + 2-4 周估算）
- [x] 阅读 `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md` 摸清规划风格 + 衔接说明
- [x] 阅读 `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md` / `phase-4-impl-plan.md` 找约束/风险/验收
- [x] 阅读 `docs/superpowers/plans/progress.md` / `findings.md` 找 Phase 5/6 落地范式
- **Status:** complete

### Phase 2: 审计 Phase 7 关键模块现状
- [x] 审计 `src/state_store.rs`（1061 行）+ `src/cache.rs`（140 行）+ `src/rpc.rs`（419 行）
- [x] 审计 `src/metrics.rs`（60 行）+ `src/handlers/websocket.rs`（98 行）+ `src/handlers/rtp_control.rs`（157 行）
- [x] 审计 `src/db/audit_log.rs`（272 行）+ `src/auth.rs` 审计点
- [x] 审计 `src/router.rs`（40376 行，956-966 行 alarm/ws 独立追加）
- [x] 提炼关键发现：StateStore 双 backend 已实现但仅 7 个文件用；cache.rs 已 deprecated 但仍 140 行；/api/alarm/ws 路由未走 auth；metrics 仅 5 指标
- **Status:** complete

### Phase 3: 编写 phase-7 实施计划文档
- [x] 在 `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md` 输出最终计划
- [x] 6 个子任务：7.1 StateStore 全面接入 / 7.2 跨节点 RPC + 集群节点发现 / 7.3 WebSocket cluster + JWT + 终端事件 / 7.4 安全路由 + 审计日志 + 日志管理 / 7.5 Metrics + Health + Readiness / 7.6 鉴权码哈希 + 系统端点 + 横切
- [x] 关键代码骨架：StreamStateRepository trait / StateStoreRepository / RedisRpcTransport / ClusterRegistry / WsHub / audit_middleware / Argon2 hash
- [x] 风险 R1-R8 + 衔接说明（与 Phase 1-6 全部衔接点）+ 完成判定对齐 phase-5/6 风格
- **Status:** complete

## Key Questions
1. StateStore 抽象层如何设计才能既兼容 InMemory 又兼容 Redis 且不抽象泄漏？
2. RedisRpcTransport 用 Pub/Sub 还是 Stream？是否需要 at-least-once 语义？
3. WebSocket JWT 校验放在 upgrade 前还是后？前端兼容性如何保证？
4. audit middleware 异步写入如何保证不丢？DB 故障时如何降级？
5. Argon2 默认参数 vs 自定义参数？旧明文密码如何一次性迁移？
6. 双节点集成测试在 CI 环境如何跑（Redis 依赖）？

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Phase 7 范围以设计文档 §7 Phase 7 为准（5 任务 + 1 横切 = 6 子任务） | phase-4/5/6 已证明 5-6 子任务粒度最稳 |
| StateStore 扩展 + `StreamStateRepository` trait 抽象 | 设计文档 §6.6 强制要求 + 避免重复声明 |
| RedisRpcTransport 用 Pub/Sub + Redis Stream inbox 双模式 | Pub/Sub 实时 + Stream 至少一次补可靠性 |
| WebSocket JWT 在 upgrade 前校验（不能走 auth_middleware） | upgrade 协议特殊 |
| audit middleware 用 `tokio::spawn` 异步写 | 不阻塞 API 响应 |
| password 哈希用 Argon2 + 兼容旧明文（一次迁移期） | 设计文档 §6 安全考量 + 平滑迁移 |
| cluster 节点发现用 Redis SET + ZSET 心跳 | 比 gossip 简单，生产环境 Redis HA 足够 |
| 6 个集成测试中 cluster / ws_cluster 标 `#[ignore]` 仅本地跑 | CI 无 Redis 依赖 |
| 沿用 phase-4/5/6 的三库 cfg + tests/integration/ 模式 | 与既有 CI 矩阵一致 |
| Phase 7.1 先标 deprecated；Phase 7.6 才删除 cache.rs | 平滑迁移 + 三库 CI 验证 |
| metrics 扩展到 25+ 指标 + Prometheus HELP/TYPE | 设计文档 §10 系统监控 + Prometheus 接入 |
| `/api/health` 拆分为 liveness + `/api/ready` readiness | Kubernetes 标配 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| 暂无 | 1 | — |

## Notes
- 计划最终落点：`docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`（~570 行）
- 写作风格对齐 phase-4 / phase-5 / phase-6 计划（任务编号 + File + 关键代码骨架 + 验收）
- 严格遵循设计文档 §7 Phase 7 原文 Acceptance：单节点 + 双节点 Redis 部署都过；WS 事件跨节点一致；安全路由匹配预期策略
- 基线 commit: `79bfb29`（Phase 6.1 完成）；Phase 7 从此处开始
- Phase 7 与 Phase 1-6 全部衔接（PendingRequest / InviteSession / SubscriptionLifecycle / SendRtp / JtCommandWaiter / auth_code 明文 / 终端事件 WS）
