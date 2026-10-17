# 2.0.0 管理员诊断 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为管理员提供有界脱敏的 ZLM Hook 与 Redis 诊断接口。

**Architecture:** 已认证 Hook 写入白名单内存快照；受保护 handler 只读快照。Redis 诊断直接向已配置的 Redis 连接发送限时 PING，并独立报告 StateStore 后端。

**Tech Stack:** Rust、Axum、Tokio、Redis、utoipa、SQLx。

**Spec:** `docs/superpowers/specs/2026-10-17-v2-admin-diagnostics-design.md`

## Global Constraints

- 两个端点为 `GET /api/test/hook/list`、`GET /api/test/redis`，均调用 `authz::require_admin` 并返回 `ApiResult<T>`。
- 每类 Hook 最多 1 项，字符串不超过 128 字符；绝不保存或输出原始载荷与 secret。
- Redis PING 超时 1 秒；状态只取 `not_configured | connected | unavailable`。
- AppState 增字段时同时更新 `src/test_support.rs` 和 `src/lib.rs` 测试构造器。

## Review Focus

- 拒绝的 Hook 不应被诊断记录；任务 1 测试覆盖。
- 载荷中含 secret/token/URL 时响应不能泄露；任务 1、2 测试覆盖。
- Redis 已配置但 StateStore 回退内存不能报 connected；任务 2 测试覆盖。
- 未登录和普通用户不可读；任务 2 路由测试覆盖。
- Redis 卡住时诊断须在 1 秒预算后返回；任务 2 超时测试覆盖。

---

### Task 1: 有界 Hook 摘要

**Files:** Create `src/zlm/hook_diagnostics.rs`；Modify `src/zlm/mod.rs`、`src/zlm/hook.rs`、`src/lib.rs`、`src/test_support.rs`；Test `src/zlm/hook_diagnostics.rs`。

**Interfaces:** `HookDiagnostics::record(kind: HookKind, summary: HookSummary)`；`HookDiagnostics::list() -> Vec<HookSummary>`。字段限 spec 白名单，固定 HookKind 枚举；`AppState.hook_diagnostics: Arc<HookDiagnostics>`。

- [ ] **Step 1: 写失败测试。** 插入两次同类事件只返回最新项；128 字符截断；含 secret 的模拟 Hook 只留白名单；鉴权失败不记录。
- [ ] **Step 2: 验证失败。** `cargo test --lib hook_diagnostics`，确认缺类型或断言失败。
- [ ] **Step 3: 最小实现。** 只在已验证 ZLM 来源的分支调用 record；其他分支先补统一来源验证或不记录。不把原始 JSON 传入诊断模块。更新所有 AppState 构造器。
- [ ] **Step 4: 验证通过。** `cargo test --lib hook_diagnostics` 与现有 ZLM Hook 测试。
- [ ] **Step 5: 提交。** `git add src/zlm src/lib.rs src/test_support.rs && git commit -m "feat(zlm): retain bounded hook diagnostics"`

### Task 2: 受保护诊断 API

**Files:** Create `src/handlers/diagnostics.rs`；Modify `src/handlers/mod.rs`、`src/openapi/routes_system.rs`、`src/openapi/registry_protected_routes.rs`（如生成/注册机制要求）；Test handler/路由测试模块。

**Interfaces:** `hook_list(State<AppState>, HeaderMap) -> Result<Json<ApiResult<_>>, AppError>`；`redis_status(State<AppState>, HeaderMap) -> Result<Json<ApiResult<_>>, AppError>`。Redis 返回 `backend`、`status`、可选 `latencyMs` 和固定 `reason`。

- [ ] **Step 1: 写失败测试。** 三种身份访问、OpenAPI 与真实路由一致、未配置/正常/断连/超时、secret 不出响应。
- [ ] **Step 2: 验证失败。** `cargo test --lib diagnostics` 与 `cargo test test_every_documented_path_is_routable`，确认入口缺失。
- [ ] **Step 3: 最小实现。** handler 首行鉴权；克隆 Redis connection 后以 `tokio::time::timeout(Duration::from_secs(1), PING)` 检查；固定枚举原因，拒绝输出原始错误。注册受保护路由。
- [ ] **Step 4: 验证通过。** 运行上述定向测试、`cargo fmt --check`、`cargo test --lib`。
- [ ] **Step 5: 提交。** `git add src/handlers src/openapi && git commit -m "feat(ops): add admin diagnostics endpoints"`

### Task 3: 故障演练与状态文档

**Files:** Modify `docs/STATUS.md`、`docs/OPEN_ISSUES.md`。

**Interfaces:** 验收记录包含管理员/普通用户/未登录三种结果，Redis 断连与 ZLM Hook 样本，及脱敏检查。

- [ ] **Step 1: 准备可复现命令。** 在本地隔离测试环境运行后端、ZLM、Redis 并记录版本、端口与日志路径。
- [ ] **Step 2: 执行故障注入。** 发送有效和无效 Hook，断开 Redis；记录 HTTP 状态、业务码、延迟与响应字段，不复制 secret。
- [ ] **Step 3: 运行最终门禁。** `cargo test --no-fail-fast`、`cargo fmt --check`；结果按静态、自动化、真实运行三类记入文档。
- [ ] **Step 4: 提交。** `git add docs && git commit -m "docs: record diagnostics verification"`
