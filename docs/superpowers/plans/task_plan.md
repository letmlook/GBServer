# Task Plan: 定制 Phase 5 实施计划

<!--
  WHAT: 本任务的路线图；磁盘工作记忆。
  WHY: 50+ 工具调用后目标会丢；这里保留"主线 + 决策"。
  WHEN: 任何动作前先创建；每个阶段后更新。
-->

## Goal
基于 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 5、phase-0/3/4 实施计划与当前 main 分支代码现状，输出一份可执行的 Phase 5 实施计划 markdown，落到 `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`。

## Current Phase
完成（已交付 phase-5 实施计划）

## Phases

### Phase 1: 阅读设计文档与已有阶段计划
- [x] 阅读 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 5
- [x] 阅读 `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md` 摸清规划风格
- [x] 阅读 `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md` 摸清约束/衔接
- [x] 阅读 `docs/superpowers/plans/2026-06-01-dev-plan.md` 找到 5.1-5.3 原始拆分
- [x] 阅读 `docs/superpowers/plans/2026-06-10-comprehensive-impl-plan.md` 看 B 段已完成情况
- **Status:** complete

### Phase 2: 梳理 phase-4 落地情况与 phase-5 范围
- [x] 实地审计 cascade 代码：`src/cascade/{mod,register}.rs`、`src/sip/gb28181/{cascade,cascade_service,cascade_forward}.rs`、`src/handlers/platform.rs`、`src/sip/server.rs` 中的 cascade / platform 入口
- [x] 提炼 phase-5 范围：5 项设计文档子任务 + 当前代码已有能力 + 缺口
- [x] 与 phase-4 衔接点核对（on_send_rtp_stopped 路由、least-load 跨平台选择）
- **Status:** complete

### Phase 3: 编写 phase-5 实施计划文档
- [ ] 在 `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md` 输出最终计划
- [ ] 包含 Context / 当前差距 / 子任务 / File Structure / 验收 / 风险 / 衔接
- [ ] 风格对齐 phase-4（任务拆解 + 关键代码骨架 + 验收命令 + 完成判定）
- **Status:** in_progress

## Key Questions
1. Phase 5 与 Phase 4 的 SendRtp 清理逻辑如何切分？
2. `CascadeRegistrar`（src/cascade/）与 `CascadeService`（src/sip/gb28181/cascade_service.rs）是否合并？
3. 上级 INVITE → 本级 INVITE → ZLM SendRtp 整链路是否需新写 PlayService 级联分支？
4. 跨平台订阅转发（Catalog/MobilePosition/Alarm）作为单独子任务还是并入 5.5？

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Phase 5 范围以设计文档 §7 为准（5 个子任务），不再二次拆分 5.1/5.2/5.3 | phase-4 已证明 5-6 子任务粒度最稳 |
| `CascadeRegistrar`（C3 已实现完整状态机）作为主干，`CascadeService` 仅作为兼容层标记 deprecated | 避免两套状态机混用；具体废弃方式放 Phase 5 实施中再决定 |
| 5.3 上级 INVITE → SendRtp 整链路作为 P0 任务，单独评审 | 设计文档 Acceptance 第 2 条「Java WVP-Pro 能注册本级 + 点播」是 P0 |
| 5.5 跨平台订阅转发拆出 5.5a（Catalog/Position）和 5.5b（Alarm） | Alarm 在前端有独立页面，单独验证更可控 |
| 沿用 phase-4 的三库 cfg + tests/integration/sqlite_compat.rs 模式 | 与既有 CI 矩阵一致 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| 暂无 | 1 | — |

## Notes
- 计划最终落点：`docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`
- 写作风格对齐 phase-3 / phase-4 计划（任务编号 + File + 关键代码骨架 + 验收）
- 避免引入"占位 URL / 假成功响应"，遵循设计文档 §6.1「每个协议请求需要完整生命周期」
