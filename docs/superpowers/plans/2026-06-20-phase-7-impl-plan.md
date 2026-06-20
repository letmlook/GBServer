# Phase 7 实施方案 — Redis Cluster, RPC, WebSocket, Operations & Edge APIs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 GBServer 从"单节点 + 散落内存状态"提升到"Redis-backed cluster + 跨节点 RPC + WebSocket fanout + 完整审计 + 监控 + 安全路由"的生产部署形态；并把所有 Phase 1-6 散落各处的 in-memory 状态（命令等待/会话/订阅/位置/告警）收敛到 `StateStore` 抽象层，让 WVP-Pro Java 集群/HA 部署的运维场景可被覆盖。

**Architecture:**
- 状态收敛到 `crate::state_store::StateStore`（已实现 1061 行，`StateBackend` trait + `InMemoryBackend` + `RedisBackend` 双实现），逐步淘汰 `crate::cache.rs`（已标 `#[deprecated]`）和散落的 `Arc<RwLock<HashMap>>`
- 跨节点事件走 `crate::rpc::RpcRouter`（已实现 419 行，`LocalRpc` + 标准 handlers），扩展 `RedisRpcTransport`（Pub/Sub）让 RpcRequest 跨节点
- WebSocket cluster fanout：每节点都持有完整 `WsState`，事件通过 RPC 广播到所有节点，再由各节点本地 mpsc 派发给已连客户端
- 审计日志 middleware：所有 `/api/*` handler 出口自动落 `gb_audit_log`（DB），含 status_code / username / IP / path / method
- 安全路由收编：`/api/alarm/*` 和 `/api/ws` 从 router 主体外合并回 `api_protected`（走 `auth_middleware`），WS 在 upgrade 阶段校验 JWT
- 监控：`/metrics` Prometheus 输出扩展 StateStore / RPC / WS / DB 三态；`/health` 检查 DB+Redis+节点集群；`/ready` 控制流量

**Tech Stack:** Rust + Axum + SQLx + ZLMediaKit HTTP API + `redis` 0.25 + `redis::aio::ConnectionManager` + tokio::sync::broadcast + `prometheus-format`（自实现字符串）+ DashMap + JWT + Argon2

**基线 commit:** `79bfb29`（Phase 6.1 完成）
**上游设计:** `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 7
**总工作量:** ~128h（约 3 周），拆 6 个子任务（5 个功能 + 1 个横切）。

---

## 全局约束

- **StateStore 优先**：新增 / 修改的状态一律走 `crate::state_store::StateStore` API，禁止新引入 `Arc<RwLock<HashMap>>` 或散落 cache 字段
- **淘汰 `cache.rs`**：Phase 7.1 后所有内部调用迁到 StateStore；`cache.rs` 仅保留 `incr/decr/get_media_server_streams` 作为迁移期 compat shim，**Phase 7.6 删除**
- **三态 cfg DB**：所有 db 改动仍按 `postgres` / `mysql` / `sqlite` 三态 cfg（保持 Phase 6 矩阵）
- **三库 CI 必跑**：`cargo test --lib` + `cargo test --no-default-features --features postgres --lib` + `cargo test --no-default-features --features mysql --lib` 三路全绿
- **Redis 优雅降级**：所有 Redis 调用都做 try-fallback（Redis 不可达时自动退回 in-memory 或报错），与现有 `RedisBackend::new` 一致
- **承接 Phase 1-6 收尾**：
  - 复用 Phase 4 `StateStore` 抽象层（已实现双 backend）
  - 复用 Phase 5 `RpcRouter` + `LocalRpc`（扩展 Redis pub/sub）
  - 复用 Phase 6 `JtMediaSession` 等所有 JT 模块（注册表/等待/session 都迁移到 StateStore）
- **设计文档 §6.6 强制约束**：Redis 是 optional state bus，所有代码必须支持"无 Redis"单节点运行

---

## Context

按设计文档 §7 Phase 7，需把 GBServer 从"单节点 + 散落内存"提升到"Redis-backed cluster + RPC + WebSocket fanout + 审计 + 监控"：

1. **7.1 StateStore 全面接入（核心）**：当前 `crate::state_store::StateStore`（1061 行）已实现 `InMemoryBackend` + `RedisBackend` 双实现，但仅 `lib.rs` / `scheduler/record_plan.rs` / `sip/gb28181/{cascade_forward,playback_session,invite_session}.rs` 共 7 个文件真正使用；`cache.rs` 140 行已标 deprecated 但仍被多处 import；`recording_state` / `pending_request` / `subscription_state` / `mobile_position_history` 仍以 `Arc<RwLock>` 散落在 `lib.rs` / `sip/server.rs` / `db/position_history.rs` 等位置。**Phase 7 范畴**：把所有状态统一进 StateStore，让"无 Redis / 有 Redis"两套部署都跑同一份代码。
2. **7.2 跨节点 RPC**：当前 `RpcRouter` 仅支持 `LocalRpc`（`broadcast` = tokio mpsc，无网络）。**Phase 7 范畴**：新增 `RedisRpcTransport`（Pub/Sub）+ 节点发现（Redis SET）+ 跨节点命令（device_control / play_stop / stream_changed / cascade_sendrtp_stop / ws_broadcast / cloud_record_sync / mark_offline）。
3. **7.3 WebSocket cluster fanout + 鉴权 + 终端事件**：`src/handlers/websocket.rs` 98 行，仅本节点 mpsc 广播，无 JWT 校验，无跨节点；终端位置 / 告警 WebSocket 推送未实现（Phase 6 末尾衔接明示）。**Phase 7 范畴**：WS upgrade 阶段 JWT 校验；事件经 RPC 广播到所有节点；新增 `jt_event` channel。
4. **7.4 安全路由 + 审计日志**：`/api/alarm/*` 7 个端点 + `/api/ws` 在 router.rs:956-966 独立追加，**未走 `auth_middleware`**（设计文档 §4 显式标为 known issue）；`gb_audit_log` 表已存在（`db/audit_log.rs` 272 行，三态 cfg），但仅 `auth.rs` 102/144 登录/登出时调用，**所有业务 handler 都没有审计**；`/api/log/list` 是 stub。
5. **7.5 Metrics + Health + Readiness**：当前 `metrics.rs` 60 行仅 5 个指标（jt1078_missing / jt1078_active / sip_devices_online / sip_invites_active / streams_active），全部是 AtomicU64，缺 cluster / RPC / WS / DB / Redis 指标；`/api/health` 简单检查 DB / SIP / ZLM / Redis 状态。
6. **7.6 横切**：用户/JWT auth_code 统一哈希（Phase 6 末尾衔接明示）；handler middleware 注入 audit_log；新增 `/api/system/info`、`/api/system/stats`、`/api/system/version`、`/api/system/online-users`（parity with Java SystemController）。

**Acceptance**（设计文档 §7 Phase 7 原文）：
- Single-node 和 two-node Redis-backed deployments pass core protocol smoke tests
- WebSocket events 和 stream states 跨节点保持一致
- Security route exposure matches intended policy

**当前差距**（代码审计确认）：

| # | 现状 | 缺口 |
|---|---|---|
| 1 | StateStore 已实现 1061 行 + 双 backend，仅 7 个文件用 | 7.1 把所有状态迁移到 StateStore；淘汰 cache.rs |
| 2 | `cache.rs` 已 `#[deprecated]` 但仍 140 行 | 7.1 删除 compat shim 调用；7.6 删除整个文件 |
| 3 | `RpcRouter` 仅 `LocalRpc`，无 Redis transport | 7.2 加 `RedisRpcTransport` + 节点发现 |
| 4 | `mark_offline_if_expired`（Phase 4）单节点执行 | 7.2 跨节点时通过 RPC 同步 |
| 5 | `WsState` 单节点 mpsc | 7.3 加 cluster fanout + JWT |
| 6 | `/api/alarm/*` 在 router.rs:960-966 独立追加 | 7.4 合并回 `api_protected` 走 auth_middleware |
| 7 | `/api/ws` router.rs:956 独立追加，无 JWT | 7.4 JWT 校验在 upgrade 前 |
| 8 | `auth.rs` 仅 2 处 `db::audit_log::insert` | 7.4 middleware 自动写所有 handler |
| 9 | `/api/log/list` 是 stub | 7.4 改用 `audit_log` DB 查询 |
| 10 | `metrics.rs` 60 行 5 个指标 | 7.5 扩展到 25+ 指标含 cluster/RPC/WS |
| 11 | `/api/health` 简单 SQL ping | 7.5 加 cluster info + Redis ping |
| 12 | 缺 `/ready`、`/api/system/info` | 7.5/7.6 新增 |
| 13 | 用户/JWT auth_code 明文 | 7.6 哈希 + 盐 |
| 14 | Phase 6 终端事件 WS 未实现 | 7.3 加 `jt_event` channel |
| 15 | 缺 `/api/system/online-users` | 7.6 新增 |
| 16 | `state/mod.rs` 20 行无 trait 抽象 | 7.1 加 `StreamStateRepository` trait |

**预估工作量**：~128h（3 周编码 + 1 周 buffer），6 个子任务，6-8 个 PR。

---

## File Structure

| 路径 | 责任 | 状态 |
|---|---|---|
| `src/state_store.rs` | StateStore 抽象层（已实现 1061 行）+ 双 backend | 改（7.1 扩展 InvitationSessionState + SubscriptionState + PendingRequestState + ClusterNodeState） |
| `src/state/mod.rs`（新） | `StreamStateRepository` trait 抽象 | 增 |
| `src/state/repository.rs`（新） | `StateStoreRepository` 实现 + 旧 cache 替换 | 增 |
| `src/cache.rs` | **deprecated** 兼容层 | 改（7.1 全部标 `#[deprecated]` + 7.6 删除） |
| `src/rpc.rs` | RpcRouter + LocalRpc（已实现） + 节点发现 + Redis transport | 改（7.2 新增 `RedisRpcTransport` / `ClusterRegistry`） |
| `src/cluster/mod.rs`（新） | 集群节点发现（Redis SET `gb:cluster:nodes`）+ 心跳 | 增 |
| `src/cluster/registry.rs`（新） | `ClusterRegistry` 封装 | 增 |
| `src/handlers/websocket.rs` | WS upgrade + JWT + cluster fanout | 改（7.3） |
| `src/ws/mod.rs`（新） | `WsHub` cluster 模式封装 | 增 |
| `src/ws/jwt.rs`（新） | WS JWT 校验 + 提取 user/role | 增 |
| `src/handlers/jt1078_events.rs`（新） | 终端位置/告警 WebSocket 推送入口 | 增 |
| `src/router.rs` | 路由整合：alarm/ws 进 protected + 加 system/metrics | 改（7.3-7.6） |
| `src/middleware/audit.rs`（新） | 审计日志 middleware（自动捕获 status_code） | 增 |
| `src/middleware/mod.rs`（新） | 中间件统一导出 | 增 |
| `src/metrics.rs` | 扩展到 25+ 指标含 cluster/RPC/WS/Redis/audit | 改（7.5） |
| `src/handlers/health.rs`（新） | `/api/health` + `/api/ready` 拆分 + cluster info | 增 |
| `src/handlers/system.rs`（新） | `/api/system/{info,stats,version,online-users}` | 增 |
| `src/handlers/log_audit.rs`（新） | `/api/log/list` 改用 audit_log DB | 增 |
| `src/auth.rs` | 哈希 auth_code + 写 audit_log | 改（7.6） |
| `src/handlers/stub.rs` | 移除 `log_list`（7.4 移到 log_audit.rs） | 改 |
| `src/lib.rs` | AppState 添加 `cluster_registry: Arc<ClusterRegistry>` + `ws_hub: Arc<WsHub>` + middleware 装载 | 改（7.2-7.5） |
| `src/db/audit_log.rs` | 加 `list_paged_with_filters`（username/action/time range） | 改（7.4 已实现部分补全） |
| `src/db/user.rs` | 加 `hash_password` / `verify_password`（Argon2） | 改（7.6） |
| `database/init-{sqlite,postgresql,mysql}-2.7.4.sql` | 加 `gb_cluster_node` 表（node_id, addr, last_heartbeat）+ `gb_online_user` 表 | 改（7.2 + 7.6） |
| `tests/integration/cluster_test.rs`（新） | mock 双节点 → 状态在两边一致 | 增 |
| `tests/integration/audit_test.rs`（新） | mock API → DB 有 audit_log 记录 | 增 |
| `tests/integration/ws_cluster_test.rs`（新） | mock 双节点 → 事件都收到 | 增 |
| `scripts/phase7-test-matrix.sh`（新） | 三库 cargo test 一键 + Redis smoke | 增 |
| `docs/OPERATIONS.md` | Phase 7 章节（cluster 部署 + Redis HA + 监控 + 审计） | 改 |
| `config/application.toml` | 加 `[cluster]` `[audit]` `[ws]` 段落 | 改 |

---

## 任务清单

### Task 7.1 — StateStore 全面接入 + 淘汰 cache.rs（P0，36h）

**目标**：
- `crate::state_store::StateStore` 补全 `PendingRequestState` / `SubscriptionState` / `InviteSessionState` / `JtMediaSessionState` / `RecordingState` / `CascadeSendRtpState` / `JtCommandWaiterState` 类型
- 引入 `StreamStateRepository` trait（`src/state/mod.rs`），`StateStoreRepository` 实现
- `recording_state`（lib.rs）/ `pending_request`（sip/server.rs）/ `subscription_state`（sip/gb28181/subscription_lifecycle.rs）/ `position_history`（db/position_history.rs）全部迁到 StateStore API
- `crate::cache.rs` 全部函数调用方替换，**Phase 7.6 删除整个文件**
- 加 `gb:cluster:` 前缀的 Redis key（与现有 `gb:` 前缀并存）

**Files:**
- Modify: `src/state_store.rs`（+ 7 个 State 结构体 + 增删改查方法）
- Create: `src/state/mod.rs`（trait）+ `src/state/repository.rs`（StateStoreRepository impl）
- Modify: `src/cache.rs`（每个函数加 `#[deprecated]` + 注释迁向 StateStore）
- Modify: `src/lib.rs`（AppState 暴露 `state_store`；移除散落 `recording_state`）
- Modify: `src/sip/server.rs`（`pending_request` 走 `state_store.set_pending_request`）
- Modify: `src/sip/gb28181/subscription_lifecycle.rs`（subscription state 走 StateStore）
- Modify: `src/sip/gb28181/invite_session.rs` / `playback_session.rs`（session state 走 StateStore）
- Modify: `src/db/position_history.rs`（mobile position 走 StateStore + DB 双写）
- Modify: `src/jt1078/manager.rs`（终端注册表/等待器/session 走 StateStore）
- Modify: `src/handlers/play.rs` / `playback.rs` / `stream.rs`（移除 cache.rs import，改用 StateStore）
- Modify: `src/handlers/device_control.rs`（`control_record` 走 StateStore）
- Test: `src/state_store.rs::tests`（+10 个）+ `src/state/repository.rs::tests`（+8 个）

**关键代码骨架**：

```rust
// src/state_store.rs 新增 State 结构体（7 个）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRequestState {
    pub key: String,            // "{device_id}:{sn}" 或 "{phone}:{msg_id}:{serial}"
    pub device_id: String,
    pub kind: String,           // "device_info" / "device_status" / "catalog" / "record_info" / "jt_command"
    pub sent_at: DateTime<Utc>,
    pub timeout_at: DateTime<Utc>,
    pub response_tx: Option<broadcast::Sender<serde_json::Value>>, // 不序列化
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteSessionState {
    pub call_id: String,
    pub device_id: String,
    pub channel_id: String,
    pub session_type: String,   // "live" / "playback" / "download" / "talk" / "broadcast" / "jt_live" / "jt_playback"
    pub zlm_stream_id: Option<String>,
    pub status: String,         // "inviting" / "active" / "closed" / "timeout"
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingState {
    pub device_id: String,
    pub channel_id: String,
    pub cmd: String,            // "Record" / "StopRecord"
    pub started_at: DateTime<Utc>,
    pub ttl_secs: u64,
}
```

```rust
// src/state/mod.rs
pub trait StreamStateRepository: Send + Sync {
    async fn set_recording(&self, device_id: &str, channel_id: &str, cmd: &str);
    async fn get_recording(&self, device_id: &str, channel_id: &str) -> Option<String>;
    async fn del_recording(&self, device_id: &str, channel_id: &str);
    async fn set_session(&self, session: InviteSessionState);
    async fn get_session(&self, call_id: &str) -> Option<InviteSessionState>;
    async fn del_session(&self, call_id: &str);
    async fn list_sessions_by_device(&self, device_id: &str) -> Vec<InviteSessionState>;
    async fn incr_pending(&self, key: &str, ttl_secs: u64) -> i64;
    async fn decr_pending(&self, key: &str) -> i64;
}
```

```rust
// src/state/repository.rs
pub struct StateStoreRepository {
    store: Arc<crate::state_store::StateStore>,
}

impl StreamStateRepository for StateStoreRepository {
    async fn set_recording(&self, device_id: &str, channel_id: &str, cmd: &str) {
        let state = RecordingState {
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            cmd: cmd.to_string(),
            started_at: chrono::Utc::now(),
            ttl_secs: 86400,
        };
        self.store.set_recording(device_id, channel_id, &state).await;
    }
    // ... 其余方法
}
```

```rust
// src/handlers/device_control.rs control_record 改写
pub async fn control_record(/* ... */) -> Json<WVPResult<serde_json::Value>> {
    let Some(repo) = state.state_repo.as_ref() else {
        return Json(WVPResult::error("StateStore not initialized"));
    };
    // ... 原有 SIP 命令发送
    if cmd == "Record" {
        repo.set_recording(&device_id, &channel_id, "Record").await;
    } else {
        repo.del_recording(&device_id, &channel_id).await;
    }
    Json(WVPResult::success_empty())
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/state_store.rs` 新增 7 个 State 结构体 + setter/getter/deleter + `serde_json::to_string` 序列化（保留 InMemory + Redis 双路径）
- [ ] **Step 2**: 新增 `src/state/mod.rs` 定义 `StreamStateRepository` trait
- [ ] **Step 3**: 新增 `src/state/repository.rs` 实现 `StateStoreRepository`
- [ ] **Step 4**: 在 `src/lib.rs` 把 `Arc<StateStoreRepository>` 加入 `AppState` + `state_repo` 字段
- [ ] **Step 5**: 替换 `src/handlers/{play,playback,stream,device_control}.rs` 中 `cache::*` 调用 → `state_repo::*`
- [ ] **Step 6**: 替换 `src/sip/server.rs` 中 `pending_request` 内嵌 HashMap → `state_store.set_pending_request` + response_tx 走 tokio::sync::broadcast
- [ ] **Step 7**: 替换 `src/sip/gb28181/{invite_session,playback_session,subscription_lifecycle}.rs` 状态读写 → `state_repo`
- [ ] **Step 8]: 替换 `src/jt1078/manager.rs` 中 `terminals: DashMap<phone, Session>` → `state_store.set_jt_session`（terminals HashMap 留 in-memory 缓存层 + StateStore 持久层）
- [ ] **Step 9**: 替换 `src/db/position_history.rs` → StateStore 写入 + DB 异步落盘
- [ ] **Step 10**: 添加单元测试 `state::repository::tests::*` 8 个（set/get/del/list/crash-safety）
- [ ] **Step 11**: 添加 `state_store::tests::*` 10 个覆盖 7 个新 State
- [ ] **Step 12**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 13**: 跑 `grep -rn "crate::cache::" src/` 0 命中（除 cache.rs 自身）
- [ ] **Step 14]: Commit `feat(phase-7): StateStore full integration + cache.rs deprecation`

### Task 7.2 — 跨节点 RPC + 集群节点发现（P0，28h）

**目标**：
- 新增 `RedisRpcTransport`（基于 Redis Pub/Sub `gb:rpc:channel` + Redis Stream 持久化）
- 新增 `ClusterRegistry`（Redis SET `gb:cluster:nodes` + 每 10s 心跳 + 60s 过期）
- 把 `LocalRpc.broadcast` 替换为 `RedisRpcTransport.broadcast`（当 Redis 配置时）
- 标准 handlers（device_control / play_stop / stream_changed / cascade_sendrtp_stop / ws_broadcast / cloud_record_sync / mark_offline）支持跨节点
- 新增 `/api/rpc` HTTP 端点已在 router.rs:932 存在，**验证**它能跨节点分发

**Files:**
- Modify: `src/rpc.rs`（+ `RedisRpcTransport` struct + `RpcMessage` envelope + 序列化）
- Create: `src/cluster/mod.rs` + `src/cluster/registry.rs`（`ClusterRegistry` 封装）
- Modify: `src/lib.rs`（AppState 添加 `cluster_registry: Arc<ClusterRegistry>`；run() 启动心跳 task + RPC subscriber task）
- Modify: `src/handlers/stub.rs`（`mark_offline_if_expired` 移到 `cluster/mark_offline.rs`）
- Modify: `src/zlm/media_node.rs`（keepalive 状态通过 RPC 同步）
- Modify: `src/scheduler/record_plan.rs`（任务触发通过 RPC 同步）
- Modify: `database/init-*.sql`（3 文件均加 `gb_cluster_node` 表 + `gb_rpc_message` 表）
- Test: `src/rpc.rs::tests`（+8 个）+ `src/cluster/registry.rs::tests`（+5 个）

**关键代码骨架**：

```rust
// src/rpc.rs 新增 RedisRpcTransport
pub struct RedisRpcTransport {
    node_id: String,
    redis: redis::aio::ConnectionManager,
    channel: String,  // "gb:rpc:channel"
    local_tx: broadcast::Sender<RpcRequest>,
}

impl RpcRpcTransport for RedisRpcTransport {
    fn broadcast(&self, req: &RpcRequest) -> Result<(), String> {
        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        let mut conn = self.redis.clone();
        // 异步发布，失败仅记录日志（不强阻塞）
        tokio::spawn(async move {
            let _: Result<i64, _> = conn.publish::<&str, &str, i64>(&channel, &json).await;
        });
        Ok(())
    }
    fn send_to(&self, node_id: &str, req: &RpcRequest) -> Result<(), String> {
        let key = format!("gb:rpc:inbox:{}", node_id);
        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        // Redis Stream 持久化（接收方至少一次）
        tokio::spawn(async move {
            let mut conn = redis.clone();
            let _: Result<String, _> = conn.xadd(&key, "*", &[("payload", &json)]).await;
        });
        Ok(())
    }
    fn receive(&self) -> broadcast::Receiver<RpcRequest> {
        self.local_tx.subscribe()
    }
    fn node_id(&self) -> &str { &self.node_id }
}
```

```rust
// src/cluster/registry.rs
pub struct ClusterRegistry {
    node_id: String,
    redis: Option<redis::aio::ConnectionManager>,
    local_addr: String,           // "10.0.0.5:8080"
    heartbeat_interval: Duration, // 10s
    ttl: Duration,                // 60s
}

impl ClusterRegistry {
    pub async fn start_heartbeat(&self) {
        let key = format!("gb:cluster:nodes");
        loop {
            tokio::time::sleep(self.heartbeat_interval).await;
            self.touch_node(&key).await;
            self.evict_expired(&key).await;
        }
    }
    pub async fn list_active_nodes(&self) -> Vec<String> {
        // SMEMBERS gb:cluster:nodes 然后 ZRANGEBYSCORE gb:cluster:heartbeat (now-ttl, +inf)
        // 仅返还有心跳的
    }
    pub async fn touch_node(&self, key: &str) {
        // ZADD gb:cluster:heartbeat {score=now_secs} {node_id}
        // SADD gb:cluster:nodes {node_id}
    }
}
```

```rust
// src/lib.rs 启动 RPC subscriber
pub async fn start_rpc_subscriber(
    redis: Option<redis::aio::ConnectionManager>,
    rpc_router: Arc<crate::rpc::RpcRouter>,
    local_tx: broadcast::Sender<RpcRequest>,
) {
    let Some(redis) = redis else { return; };
    let mut pubsub = redis.clone();
    let mut stream = pubsub.psubscribe("gb:rpc:channel").await?;
    while let Some(msg) = stream.next().await {
        let payload: String = msg.get_payload()?;
        let req: RpcRequest = serde_json::from_str(&payload)?;
        // 过滤掉自己发布的（避免重复处理）
        if req.target == "self" || req.target == local_node_id { continue; }
        let _ = rpc_router.route(&req).await;
    }
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/rpc.rs` 新增 `RedisRpcTransport` + 实现 `RpcTransport` trait
- [ ] **Step 2**: 在 `src/rpc.rs` 改 `register_standard_handlers` 中 `RpcTransport::broadcast`/`send_to` 走 `RedisRpcTransport`（if redis configured）
- [ ] **Step 3**: 新增 `src/cluster/mod.rs` + `src/cluster/registry.rs`（`ClusterRegistry` 封装 Redis SET/心跳）
- [ ] **Step 4**: 在 `src/lib.rs` AppState 添加 `cluster_registry: Arc<ClusterRegistry>`；run() 启动心跳 + RPC subscriber
- [ ] **Step 5]: 把 `mark_offline_if_expired`（Phase 4 keepalive 超时）改为通过 `cluster_registry.broadcast_offline` 通知所有节点
- [ ] **Step 6**: 把 `zlm/media_node.rs::on_server_keepalive_timeout` 改为 `rpc.broadcast("mark_offline", ...)`
- [ ] **Step 7]: 在 `database/init-*.sql` 加 `gb_cluster_node` 表（node_id, addr, role, last_heartbeat）+ 索引
- [ ] **Step 8**: 添加单元测试 `rpc::tests::test_redis_rpc_*` 8 个（broadcast roundtrip / target filter / serialize / dead node）
- [ ] **Step 9**: 添加单元测试 `cluster::registry::tests::*` 5 个（heartbeat / evict / list_active）
- [ ] **Step 10**: 跑 `cargo test --lib` 确认无回归
- [ ] **Step 11**: Commit `feat(phase-7): cross-node RPC via Redis Pub/Sub + cluster registry`

### Task 7.3 — WebSocket cluster fanout + JWT 鉴权 + 终端事件（P1，20h）

**目标**：
- `src/handlers/websocket.rs` upgrade 阶段校验 JWT（无 JWT 直接 401 close）
- `WsHub` cluster 模式：所有事件经 `RpcRouter.broadcast("ws_broadcast", payload)` 跨节点同步
- 每个节点本地 mpsc 派发已连客户端
- 新增 `jt_event` channel：终端位置/告警 WebSocket 推送（Phase 6 衔接）
- 客户端可订阅特定 event type（device_status / alarm / record_status / jt_position / jt_alarm）

**Files:**
- Modify: `src/handlers/websocket.rs`（upgrade 前 JWT 校验 + 客户端订阅过滤）
- Create: `src/ws/mod.rs` + `src/ws/jwt.rs` + `src/ws/hub.rs`（`WsHub` 封装）
- Modify: `src/handlers/jt1078.rs`（位置/告警落库后调 `ws_hub.broadcast_event("jt_position", ...)`）
- Modify: `src/lib.rs`（AppState 添加 `ws_hub: Arc<WsHub>`；run() 启动 RPC → WS 派发 task）
- Modify: `src/router.rs`（`/api/ws` 仍为顶层 route 但 handler 内 JWT 校验；保留 `event` query param 订阅过滤）
- Test: `src/ws/hub.rs::tests`（+5 个）+ `src/ws/jwt.rs::tests`（+3 个）

**关键代码骨架**：

```rust
// src/ws/jwt.rs
pub fn verify_ws_jwt(token: &str, secret: &str) -> Result<JwtClaims, String> {
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    jsonwebtoken::decode::<JwtClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map(|d| d.claims)
        .map_err(|e| format!("JWT invalid: {}", e))
}
```

```rust
// src/ws/hub.rs
pub struct WsHub {
    local_clients: Arc<RwLock<HashMap<String, ClientInfo>>>,  // client_id → { tx, subscribed_events }
    rpc_router: Option<Arc<crate::rpc::RpcRouter>>,
    node_id: String,
}

pub struct ClientInfo {
    pub user: String,
    pub subscribed: HashSet<String>,  // "device_status" / "alarm" / "jt_position" / ...
    pub tx: mpsc::UnboundedSender<Message>,
}

impl WsHub {
    pub async fn broadcast_event(&self, event: &str, data: serde_json::Value) {
        // 1. 本节点派发
        self.local_dispatch(event, &data).await;
        // 2. 跨节点广播
        if let Some(router) = self.rpc_router.as_ref() {
            let _ = router.route(&RpcRequest {
                method: "ws_broadcast".to_string(),
                target: "broadcast".to_string(),
                payload: json!({ "event": event, "data": data, "from_node": self.node_id }),
                reply_to: None,
            }).await;
        }
    }
    pub async fn handle_rpc_broadcast(&self, payload: serde_json::Value) {
        // 跨节点接收到的事件，提取 event + data → 本地派发
        let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let data = payload.get("data").cloned().unwrap_or(json!({}));
        self.local_dispatch(event, &data).await;
    }
    async fn local_dispatch(&self, event: &str, data: &serde_json::Value) {
        let msg = json!({ "event": event, "data": data }).to_string();
        let map = self.local_clients.read().await;
        let mut failed = Vec::new();
        for (id, client) in map.iter() {
            if client.subscribed.contains(event) || client.subscribed.contains("*") {
                if client.tx.send(Message::Text(msg.clone())).is_err() {
                    failed.push(id.clone());
                }
            }
        }
        drop(map);
        // 清理 failed
    }
}
```

```rust
// src/handlers/websocket.rs upgrade 前 JWT 校验
pub async fn ws_handler(
    State(state): State<AppState>,
    Query(params): Query<WsQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // 1. 从 query 或 Authorization header 提取 JWT
    let token = params.token.clone()
        .or_else(|| headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|s| s.strip_prefix("Bearer ")).map(String::from));
    let Some(token) = token else {
        return (axum::http::StatusCode::UNAUTHORIZED, "Missing JWT").into_response();
    };
    let claims = match verify_ws_jwt(&token, &state.config.jwt.secret) {
        Ok(c) => c,
        Err(e) => return (axum::http::StatusCode::UNAUTHORIZED, e).into_response(),
    };
    // 2. upgrade
    state.ws_hub.register(claims.user, params.events.unwrap_or_default()).await;
    ws.on_upgrade(/* ... */)
}
```

```rust
// src/handlers/jt1078.rs::position_report（位置上报入口）改写
pub async fn handle_position_report(phone: &str, loc: LocationReport, state: &AppState) {
    // 1. DB 落库
    let _ = crate::db::jt1078::update_last_position(&state.pool, phone, &loc).await;
    // 2. WebSocket cluster fanout
    state.ws_hub.broadcast_event("jt_position", json!({
        "phone": phone, "longitude": loc.longitude, "latitude": loc.latitude,
        "speed": loc.speed, "direction": loc.direction, "time": loc.time,
    })).await;
}
```

**子任务**：
- [ ] **Step 1**: 新增 `src/ws/jwt.rs`（JWT 校验 + 提取 user/role）+ 单测 3 个
- [ ] **Step 2**: 新增 `src/ws/mod.rs` + `src/ws/hub.rs`（`WsHub` 封装，含 cluster 模式）
- [ ] **Step 3]: 修改 `src/handlers/websocket.rs` 在 upgrade 前调 `verify_ws_jwt`；无 token 返 401
- [ ] **Step 4]: 在 `src/lib.rs` 启动 RPC → WS dispatch task：`rpc_router.subscribe` 收 `ws_broadcast` 方法 → 调 `ws_hub.handle_rpc_broadcast`
- [ ] **Step 5]: 在 `src/lib.rs` AppState 添加 `ws_hub: Arc<WsHub>`
- [ ] **Step 6**: 修改 `src/handlers/jt1078.rs::position_report` / `handle_alarm_report` 调 `ws_hub.broadcast_event`
- [ ] **Step 7]: 添加 `src/handlers/jt1078_events.rs` 提供 `GET /api/jt1078/event/subscribe` 返回最近 N 条事件（SSE fallback 备选）
- [ ] **Step 8]: 在 `src/router.rs` 给 WS handler 加 `?token=` 和 `?events=` query 支持
- [ ] **Step 9**: 添加单元测试 `ws::hub::tests::*` 5 个（local_dispatch / cluster roundtrip / subscribe filter / disconnect cleanup）
- [ ] **Step 10**: 添加集成测试 `tests/integration/ws_cluster_test.rs`（mock 双节点：A 触发事件 → B 上 WS 客户端能收到）
- [ ] **Step 11]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 12]: Commit `feat(phase-7): WebSocket cluster fanout + JWT auth + JT events`

### Task 7.4 — 安全路由 + 审计日志 middleware + 日志管理（P1，16h）

**目标**：
- `/api/alarm/*` 和 `/api/ws` 从 router 主体外合并回 `api_protected`（handler 入口走 `auth_middleware`）
- 新增 `src/middleware/audit.rs`：所有 `/api/*` handler 出口自动写 `gb_audit_log`（含 status_code / username / IP / path / method / 响应时间 / request_body 截断）
- `/api/log/list` 改用 `audit_log::list_paged`（已实现）替换 stub
- 审计开关：`config.application.toml [audit] enabled = true/false`

**Files:**
- Modify: `src/router.rs`（alarm 8 个端点合并进 `api_protected`；audit middleware layer）
- Create: `src/middleware/mod.rs` + `src/middleware/audit.rs`（audit middleware + Axum 集成）
- Modify: `src/handlers/stub.rs`（移除 `log_list`）
- Create: `src/handlers/log_audit.rs`（`/api/log/list` 真实实现）
- Modify: `src/lib.rs`（middleware 装载顺序）
- Modify: `src/db/audit_log.rs`（list_paged 加 `request_body` 字段返回，已部分实现）
- Modify: `config/application.toml`（加 `[audit]` 段落）
- Test: `src/middleware/audit.rs::tests`（+6 个）+ `src/handlers/log_audit.rs::tests`（+3 个）

**关键代码骨架**：

```rust
// src/middleware/audit.rs
pub async fn audit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if !state.config.audit.enabled {
        return next.run(req).await;
    }
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let ip = req.headers().get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0.0.0.0")
        .to_string();
    let username = req.headers().get("access-token")
        .and_then(|v| v.to_str().ok())
        .and_then(|t| crate::auth::decode_jwt_unsafe(t).ok())
        .map(|c| c.user)
        .unwrap_or_else(|| "anonymous".to_string());
    let started = std::time::Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16() as i32;
    let elapsed_ms = started.elapsed().as_millis() as i64;
    // 异步写 audit log（不阻塞响应）
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let _ = crate::db::audit_log::insert_with_metrics(
            &pool, &username, "api_call", &path, &method, &path, &ip, status, elapsed_ms,
        ).await;
    });
    response
}
```

```rust
// src/router.rs 路由整合
let api_protected = Router::new()
    // ... 既有路由 ...
    // 7.4: alarm 合并
    .route("/api/alarm/list", get(alarm::alarm_list))
    .route("/api/alarm/detail/:id", get(alarm::alarm_detail))
    .route("/api/alarm/handle", post(alarm::alarm_handle))
    .route("/api/alarm/delete/:id", delete(alarm::alarm_delete))
    .route("/api/alarm/batch", delete(alarm::alarm_batch_delete))
    .route("/api/alarm/device/:device_id", delete(alarm::alarm_delete_by_device))
    .route("/api/alarm/before/:time", delete(alarm::alarm_delete_before_time))
    .route("/api/log/list", get(log_audit::log_list))
    .route("/api/log/file/:file_name", get(stub::log_file_download))
    .route_layer(middleware::from_fn_with_state(state_clone.clone(), audit_middleware))
    .route_layer(middleware::from_fn_with_state(state_clone.clone(), auth_middleware));

let api = api_public.merge(api_protected);
let app = Router::new()
    .merge(api)
    // 7.4: /api/ws 仍在外层但 handler 内 JWT 校验（不能走 auth_middleware 因为是 upgrade）
    .route("/api/ws", get(websocket::ws_handler));
```

**子任务**：
- [ ] **Step 1**: 新增 `src/middleware/mod.rs` 导出 + `src/middleware/audit.rs` 实现 `audit_middleware`
- [ ] **Step 2**: 在 `src/db/audit_log.rs` 加 `insert_with_metrics`（含 elapsed_ms）+ `request_body` 字段（截断 1KB）
- [ ] **Step 3**: 在 `src/config.rs` 加 `AuditConfig { enabled, retention_days }`
- [ ] **Step 4]: 修改 `src/router.rs` 把 alarm 8 个端点从 `app.route(...)` 移到 `api_protected` + 加 audit_middleware
- [ ] **Step 5]: 修改 `src/router.rs` 给 `api_protected` 装 `audit_middleware` + `auth_middleware` 双层
- [ ] **Step 6**: 新增 `src/handlers/log_audit.rs::log_list` 真实实现
- [ ] **Step 7]: 在 `src/handlers/stub.rs` 删除 `log_list`（移到 log_audit.rs）
- [ ] **Step 8]: 添加单元测试 `middleware::audit::tests::*` 6 个（status_code capture / username extract / IP from x-forwarded-for / disabled bypass / body 截断 / 异步不阻塞）
- [ ] **Step 9**: 添加单元测试 `handlers::log_audit::tests::*` 3 个（分页 / 时间过滤 / 用户过滤）
- [ ] **Step 10]: 集成测试 `tests/integration/audit_test.rs`（mock `/api/user/login` → DB 有 audit_log 记录）
- [ ] **Step 11**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 12]: `grep -n "fn log_list" src/handlers/stub.rs` 0 命中
- [ ] **Step 13]: Commit `feat(phase-7): audit middleware + alarm/ws security + log management`

### Task 7.5 — Metrics + Health + Readiness + Prometheus 字段扩展（P1，16h）

**目标**：
- `metrics.rs` 扩展到 25+ 指标含 cluster / RPC / WS / DB / Redis / audit
- `/api/health` 拆分为 `/api/health`（liveness，不查 DB）+ `/api/ready`（readiness，查 DB+Redis+cluster）
- `/metrics` Prometheus 输出所有字段 + 加 `HELP` / `TYPE`
- 新增指标：cluster_nodes_active / rpc_messages_total{method} / ws_clients_connected / redis_state_keys / db_query_duration_seconds / audit_log_writes_total

**Files:**
- Modify: `src/metrics.rs`（+20 个 atomic 计数器 + 扩展 `gather()`）
- Create: `src/handlers/health.rs`（`/api/health` + `/api/ready` 拆分）
- Modify: `src/router.rs`（替换 `health_check` 为 health.rs 实现 + 加 `/api/ready`）
- Modify: `src/lib.rs`（run() 启动 DB / Redis metrics 上报 task）
- Modify: `src/handlers/metrics.rs`（增加 cluster / Redis 指标）
- Test: `src/metrics.rs::tests`（+5 个）+ `src/handlers/health.rs::tests`（+4 个）

**关键代码骨架**：

```rust
// src/metrics.rs 扩展
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

static CLUSTER_NODES_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static RPC_MESSAGES_TOTAL: AtomicU64 = AtomicU64::new(0);
static RPC_MESSAGES_FAILED: AtomicU64 = AtomicU64::new(0);
static WS_CLIENTS_CONNECTED: AtomicUsize = AtomicUsize::new(0);
static WS_EVENTS_BROADCAST_TOTAL: AtomicU64 = AtomicU64::new(0);
static REDIS_CONNECTED: AtomicUsize = AtomicUsize::new(0);  // 0/1
static REDIS_STATE_KEYS: AtomicI64 = AtomicI64::new(0);
static DB_QUERY_DURATION_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static DB_QUERY_COUNT_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUDIT_LOG_WRITES_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUDIT_LOG_WRITES_FAILED: AtomicU64 = AtomicU64::new(0);

pub fn inc_rpc_messages_total() { RPC_MESSAGES_TOTAL.fetch_add(1, Ordering::Relaxed); }
pub fn inc_ws_clients(delta: i64) { WS_CLIENTS_CONNECTED.fetch_add(delta as isize, Ordering::Relaxed); }
pub fn set_cluster_nodes_active(n: usize) { CLUSTER_NODES_ACTIVE.store(n, Ordering::Relaxed); }
// ... 其余 setter

pub fn gather() -> String {
    let mut s = String::new();
    s.push_str("# HELP gb_cluster_nodes_active Number of active cluster nodes\n");
    s.push_str("# TYPE gb_cluster_nodes_active gauge\n");
    s.push_str(&format!("gb_cluster_nodes_active {}\n", CLUSTER_NODES_ACTIVE.load(Ordering::Relaxed)));
    s.push_str("# HELP gb_rpc_messages_total Total RPC messages processed\n");
    s.push_str("# TYPE gb_rpc_messages_total counter\n");
    s.push_str(&format!("gb_rpc_messages_total {}\n", RPC_MESSAGES_TOTAL.load(Ordering::Relaxed)));
    // ... 25+ 指标
    s
}
```

```rust
// src/handlers/health.rs
pub async fn health() -> (StatusCode, Json<serde_json::Value>) {
    // Liveness — 不查 DB / Redis，仅返 OK
    (StatusCode::OK, Json(json!({ "status": "alive" })))
}

pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    // Readiness — 查 DB + Redis + cluster
    let db_ok = sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool).await.is_ok();
    let redis_ok = if let Some(redis) = state.redis.as_ref() {
        let mut conn = redis.clone();
        tokio::time::timeout(Duration::from_secs(2), conn.ping::<String>()).await.is_ok()
    } else { true };
    let cluster_ok = state.cluster_registry.list_active_nodes().await.len() > 0
        || state.config.cluster.single_node_mode;  // 单节点模式跳过
    let all_ok = db_ok && redis_ok && cluster_ok;
    let body = json!({
        "status": if all_ok { "ready" } else { "not_ready" },
        "checks": {
            "database": db_ok,
            "redis": redis_ok,
            "cluster": cluster_ok,
        },
        "cluster_nodes": state.cluster_registry.list_active_nodes().await,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    (if all_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE }, Json(body))
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/metrics.rs` 新增 20 个 atomic 静态变量 + setter 函数
- [ ] **Step 2]: 修改 `src/metrics.rs::gather()` 输出 25+ 指标含 HELP/TYPE
- [ ] **Step 3]: 在 `src/lib.rs` run() 启动 metrics 上报 task（每 30s 更新 CLUSTER_NODES_ACTIVE / REDIS_STATE_KEYS）
- [ ] **Step 4]: 在 `src/sip/server.rs` / `src/handlers/play.rs` / `src/handlers/playback.rs` 等加 `metrics::inc_rpc_messages_total` / `db_query_duration_ms_total` 计数
- [ ] **Step 5]: 在 `src/ws/hub.rs` 加 `metrics::inc_ws_clients(+1/-1)` 与 `metrics::inc_ws_events_broadcast_total`
- [ ] **Step 6]: 在 `src/middleware/audit.rs` 加 `metrics::inc_audit_log_writes_total` / `inc_failed`
- [ ] **Step 7]: 新增 `src/handlers/health.rs`（`health` + `ready` 拆分）
- [ ] **Step 8]: 修改 `src/router.rs` 把原 `health_check` 函数移到 `handlers/health.rs`，并加 `/api/ready` 路由
- [ ] **Step 9]: 添加单元测试 `metrics::tests::*` 5 个（gather format / counter inc / gauge set）
- [ ] **Step 10]: 添加单元测试 `handlers::health::tests::*` 4 个（liveness 200 / readiness DB down 503 / redis down 503 / single_node_mode bypass）
- [ ] **Step 11]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 12]: Commit `feat(phase-7): metrics expansion + health/ready split + Prometheus fields`

### Task 7.6 — 鉴权码哈希 + 操作日志 + 在线用户 + 横切（P1，12h）

**目标**：
- 用户 password + JWT auth_code 统一哈希（Argon2 + 盐）
- `database/init-*.sql` 加 `gb_online_user` 表（user_id, ip, login_time, last_active, jwt_jti）
- 新增 `/api/system/info` `/api/system/stats` `/api/system/version` `/api/system/online-users`
- 删除 `crate::cache.rs`（Phase 7.1 标 deprecated，7.6 整体删除）
- 三库 CI + 集成测试 + 文档

**Files:**
- Modify: `Cargo.toml`（加 `argon2 = "0.5"`）
- Modify: `src/auth.rs`（`hash_password` / `verify_password` Argon2）
- Modify: `src/db/user.rs`（password 字段哈希迁移 + verify_password 校验）
- Modify: `src/config.rs`（JtAuthConfig 加 `hash_algorithm: "argon2"`）
- Create: `src/handlers/system.rs`（system/info/stats/version/online-users）
- Modify: `src/cache.rs`（**删除整个文件**）
- Modify: `src/lib.rs`（移除 `mod cache;`）
- Modify: `database/init-*.sql`（`gb_user.password` 字段长度 `VARCHAR(255)` for Argon2 hash + `gb_online_user` 表）
- Create: `scripts/phase7-test-matrix.sh`（三库 cargo test + Redis smoke）
- Modify: `docs/OPERATIONS.md`（Phase 7 章节：cluster 部署 + Redis HA + 监控 + 审计）
- Test: `src/auth.rs::tests`（+3 个 hash/verify）+ `src/handlers/system.rs::tests`（+4 个）

**关键代码骨架**：

```rust
// src/auth.rs Argon2 哈希
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};

pub fn hash_password(plaintext: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(plaintext.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    Ok(hash)
}

pub fn verify_password(plaintext: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false; };
    Argon2::default().verify_password(plaintext.as_bytes(), &parsed).is_ok()
}
```

```rust
// src/db/user.rs::verify_login 改用 Argon2
pub async fn verify_login(pool: &Pool, username: &str, password: &str) -> sqlx::Result<Option<User>> {
    let user: Option<User> = sqlx::query_as("SELECT id, username, password, role FROM gb_user WHERE username = ?")
        .bind(username).fetch_optional(pool).await?;
    if let Some(u) = user {
        if crate::auth::verify_password(password, &u.password) {
            return Ok(Some(u));
        }
    }
    Ok(None)
}
```

```rust
// src/handlers/system.rs
pub async fn system_info(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    Json(WVPResult::success(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "nodeId": state.config.cluster.node_id,
        "startedAt": state.started_at,
        "uptimeSeconds": (chrono::Utc::now() - state.started_at).num_seconds(),
        "features": {
            "redis": state.redis.is_some(),
            "cluster": state.config.cluster.enabled,
            "audit": state.config.audit.enabled,
        },
    })))
}

pub async fn system_stats(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let db_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gb_device").fetch_one(&state.pool).await.unwrap_or(0);
    let online = state.state_store.count_online_devices().await;
    Json(WVPResult::success(json!({
        "devices": { "total": db_total, "online": online },
        "streams": { "active": state.state_store.count_active_streams().await },
        "invites": { "active": state.state_store.count_active_invites().await },
        "jt1078": { "terminals": state.state_store.count_jt_terminals().await },
        "websocket": { "clients": state.metrics::ws_clients_connected },
    })))
}

pub async fn online_users(State(state): State<AppState>) -> Json<WVPResult<serde_json::Value>> {
    let users: Vec<serde_json::Value> = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>)>(
        "SELECT id, username, ip, last_active FROM gb_online_user WHERE last_active > ?"
    ).bind((chrono::Utc::now() - chrono::Duration::minutes(15))).fetch_all(&state.pool).await
        .unwrap_or_default().iter().map(|r| json!({
        "userId": r.0, "username": r.1, "ip": r.2, "lastActive": r.3,
    })).collect();
    Json(WVPResult::success(json!({ "list": users, "total": users.len() })))
}
```

```bash
# scripts/phase7-test-matrix.sh
#!/bin/bash
set -e
echo "=== Phase 7: SQLite (default) ==="
cargo test --lib
echo "=== Phase 7: PostgreSQL ==="
cargo test --no-default-features --features postgres --lib
echo "=== Phase 7: MySQL ==="
cargo test --no-default-features --features mysql --lib
echo "=== Phase 7: Redis smoke (optional) ==="
if [ -n "$REDIS_URL" ]; then
    cargo test --lib state_store::redis
fi
echo "=== All green ==="
```

**子任务**：
- [ ] **Step 1**: 在 `Cargo.toml` 加 `argon2 = "0.5"`
- [ ] **Step 2**: 在 `src/auth.rs` 实现 `hash_password` / `verify_password` Argon2
- [ ] **Step 3]: 修改 `src/db/user.rs::verify_login` 用 `verify_password`
- [ ] **Step 4]: 在 `database/init-*.sql` 修改 `gb_user.password` 字段长度 `VARCHAR(255)` + 加 `gb_online_user` 表
- [ ] **Step 5]: 在 `src/auth.rs::login_handler` 成功登录后写 `gb_online_user`（覆盖式 UPSERT）
- [ ] **Step 6]: 新增 `src/handlers/system.rs` 提供 4 个 endpoint（info / stats / version / online-users）
- [ ] **Step 7]: 在 `src/router.rs` 注册 `/api/system/*` 4 个路由（受保护）
- [ ] **Step 8]: **删除 `src/cache.rs` 整个文件** + `src/lib.rs` 移除 `mod cache;`
- [ ] **Step 9**: 添加单元测试 `auth::tests::*` 3 个（hash + verify / wrong password / rehash 兼容旧）
- [ ] **Step 10]: 添加单元测试 `handlers::system::tests::*` 4 个（info / stats / version / online_users）
- [ ] **Step 11]: 新增 `scripts/phase7-test-matrix.sh`
- [ ] **Step 12]: 跑 `bash scripts/phase7-test-matrix.sh` 必须三库全绿
- [ ] **Step 13]: 跑 `grep -rn "crate::cache" src/` 0 命中
- [ ] **Step 14]: 修改 `docs/OPERATIONS.md` 新增 Phase 7 章节（cluster 部署 + Redis HA + Prometheus 接入 + audit 配置）
- [ ] **Step 15]: Commit `feat(phase-7): password hashing + system endpoints + cache.rs deletion + docs`

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/state_store.rs` | + 7 个 State 结构体 + setter/getter/deleter | 8h |
| `src/state/mod.rs`（新） | `StreamStateRepository` trait | 2h |
| `src/state/repository.rs`（新） | `StateStoreRepository` 实现 | 6h |
| `src/cache.rs` | deprecated 标 + **Phase 7.6 删除** | 2h |
| `src/rpc.rs` | + `RedisRpcTransport` + `ClusterRegistry` 集成 | 8h |
| `src/cluster/mod.rs` + `registry.rs`（新） | 节点发现 + 心跳 | 6h |
| `src/ws/mod.rs` + `hub.rs` + `jwt.rs`（新） | WS cluster fanout + JWT | 8h |
| `src/handlers/websocket.rs` | upgrade 前 JWT + cluster 订阅过滤 | 4h |
| `src/handlers/jt1078.rs` | 位置/告警落库后 `ws_hub.broadcast_event` | 2h |
| `src/middleware/audit.rs`（新） | audit middleware | 4h |
| `src/handlers/log_audit.rs`（新） | `/api/log/list` 真实实现 | 2h |
| `src/handlers/health.rs`（新） | `/api/health` + `/api/ready` 拆分 | 3h |
| `src/handlers/system.rs`（新） | system/info/stats/version/online-users | 3h |
| `src/handlers/metrics.rs` | 扩展 cluster / Redis 字段 | 2h |
| `src/metrics.rs` | +20 个 atomic 计数器 | 4h |
| `src/router.rs` | alarm/ws 进 protected + audit middleware + system 路由 | 4h |
| `src/lib.rs` | AppState 加 state_repo / cluster_registry / ws_hub + middleware 装载 + 启动 task | 6h |
| `src/auth.rs` | Argon2 hash + verify + online_user 写入 | 4h |
| `src/db/audit_log.rs` | `insert_with_metrics` + request_body 字段 | 2h |
| `src/db/user.rs` | `verify_login` 用 Argon2 | 1h |
| `database/init-{sqlite,postgresql,mysql}-2.7.4.sql` | `gb_cluster_node` + `gb_online_user` + password 字段扩展 | 3h |
| `src/config.rs` | AuditConfig / ClusterConfig / JtAuthConfig | 2h |
| `Cargo.toml` | + argon2 = "0.5" | 0.5h |
| `tests/integration/cluster_test.rs`（新） | mock 双节点 → 状态一致 | 8h |
| `tests/integration/audit_test.rs`（新） | mock API → DB 有 audit_log | 4h |
| `tests/integration/ws_cluster_test.rs`（新） | mock 双节点 → 事件都收到 | 6h |
| `scripts/phase7-test-matrix.sh`（新） | 三库 cargo test + Redis smoke | 0.5h |
| `docs/OPERATIONS.md` | Phase 7 章节 | 4h |
| `config/application.toml` | `[cluster]` `[audit]` `[ws]` 段落 | 1h |

**总计**：~128h ≈ 3 周编码 + 1 周 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）

- **7.1**：`state_store::tests::*` +10 个覆盖 7 个新 State + `state::repository::tests::*` 8 个（set/get/del/list/crash-safety）
- **7.2**：`rpc::tests::test_redis_rpc_*` 8 个（broadcast roundtrip / target filter / serialize / dead node）+ `cluster::registry::tests::*` 5 个
- **7.3**：`ws::hub::tests::*` 5 个（local_dispatch / cluster roundtrip / subscribe filter / disconnect cleanup）+ `ws::jwt::tests::*` 3 个
- **7.4**：`middleware::audit::tests::*` 6 个（status_code / username / IP / disabled bypass / body 截断 / 异步不阻塞）+ `handlers::log_audit::tests::*` 3 个
- **7.5**：`metrics::tests::*` 5 个 + `handlers::health::tests::*` 4 个
- **7.6**：`auth::tests::*` 3 个（hash/verify/兼容旧）+ `handlers::system::tests::*` 4 个

### 集成测试

- **7.1**：`tests/integration/state_store_consistency_test.rs`（mock Redis + 200 操作 → 数据一致）
- **7.2**：`tests/integration/cluster_test.rs`（启动两个 GBServer 进程 mock 双节点 → 任意一边改状态另一边能读到）
- **7.3**：`tests/integration/ws_cluster_test.rs`（mock 双节点：A 触发 alarm 事件 → B 上 WS 客户端能收到）
- **7.4**：`tests/integration/audit_test.rs`（mock 100 个 API 请求 → DB 有 100 条 audit_log 记录 + status_code 准确）
- **7.5**：`tests/integration/metrics_test.rs`（curl `/metrics` → 含所有 25+ 指标字段）
- **7.6**：`tests/integration/auth_test.rs`（password 哈希 + verify 集成）

### 端到端（手测，对应设计文档 Acceptance）

- **单节点无 Redis 部署**：`GBSERVER__REDIS__URL=`（空）→ 所有功能正常，`/metrics` 显示 `gb_redis_connected 0`
- **单节点有 Redis 部署**：`GBSERVER__REDIS__URL=redis://localhost:6379` → 所有功能正常，`/metrics` 显示 `gb_redis_connected 1` + `gb_redis_state_keys > 0`
- **双节点 Redis 部署**：node-A + node-B 共享 Redis → 在 A 上触发 alarm → B 上 WS 客户端能收到；A 上 `/api/play/start` → B 上 `/api/play/stop` 能停止
- **审计验证**：调用 `/api/user/login` 失败 3 次 → `gb_audit_log` 有 3 条 status_code=401 记录
- **健康检查**：`/api/health` 始终 200（liveness）；`/api/ready` 在 DB 不可达时 503（readiness）
- **WS JWT 校验**：无 token 调 `/api/ws` → 401 close；有效 token → upgrade 成功 + 收到订阅事件

---

## 衔接说明

### 与 Phase 1 衔接

- **1.x PendingRequestManager**（SIP `device_id + sn`） → 7.1 扩展为 `PendingRequestState` 走 StateStore；key 格式不变
- **1.x InviteSessionStore** → 7.1 扩展为 `InviteSessionState`；Redis 后端让多节点 SIP server 共享 session

### 与 Phase 2 衔接

- **2.x SubscriptionLifecycle**（`subscription_state` 内嵌 HashMap） → 7.1 迁移到 StateStore，跨节点可续期
- **2.x DeviceStatus / Catalog 多包等待** → 7.2 RPC `device_control` 跨节点分发，主控节点统一等待响应

### 与 Phase 3 衔接

- **3.1 Live Play 媒体等待** → 7.1 `InviteSessionState.zlm_stream_id` 字段已包含 stream_id；Redis 后端可让媒体到达事件跨节点同步
- **3.4 DownloadSession** → 7.1 走 StateStore；7.2 跨节点时由 RPC 同步进度

### 与 Phase 4 衔接

- **4.x StateStore 已实现** → 7.1 扩展新 State + Repository trait 抽象
- **4.6 `select_least_loaded_server_filtered`** → 7.5 metrics 加 `gb_media_server_load` 字段
- **4.x `mark_offline_if_expired`** → 7.2 通过 RPC 跨节点同步 offline 状态

### 与 Phase 5 衔接

- **5.x CascadeRegistrar / SendRtpManager** → 7.2 跨节点 SendRtp 通过 RedisRpcTransport 同步
- **5.5a/b MobilePosition / Alarm 上行** → 7.3 `ws_hub.broadcast_event("jt_position", ...)` cluster 推送
- **5.4 `close_by_stream` 用 `state_store.del_cascade_sendrtp`** → 7.1 已就位；Redis 后端让多节点都能删

### 与 Phase 6 衔接

- **6.x JtMediaSession / JtCommandWaiter** → 7.1 终端注册表/等待器/session 走 StateStore
- **6.1 鉴权码 `auth_code` 明文** → 7.6 用户/JWT auth_code 统一 Argon2 哈希
- **6.x 终端位置/告警 WS 推送** → 7.3 `jt_event` channel（与 GB28181 event 并行）
- **6.6 三库 cfg + integration_test** → 7.6 沿用 `scripts/phase7-test-matrix.sh`

### 与未来 Phase 8+ 衔接

- **8.x 智能分析** → 7.5 metrics 加 `gb_inference_*` 字段
- **8.x 系统监控（CPU / 内存 / 磁盘）** → 7.5 `/api/system/stats` 加 system load 字段
- **9.x 集群联邦** → 7.2 `ClusterRegistry` 抽象可扩展为多 cluster

---

## 风险与缓解

### R1: Redis 切换导致旧数据丢失 — **HIGH ⚠️**
- 当前 `recording_state` / `pending_request` 全部在内存；切到 Redis 时需迁移逻辑
- **缓解**：
  - 7.1 同时支持 `InMemory` + `Redis` 双 backend（StateStore 已实现）
  - 切换时通过 `GBSERVER__STATE_STORE__MODE=redis` 环境变量启用
  - 旧数据自然过期（recording_state TTL 24h）；pending_request 单次会话切换即可
  - 集成测试覆盖：先内存跑 1000 操作 → 切 Redis → 重启 → 数据从 Redis 恢复

### R2: WebSocket JWT 校验破坏现有部署 — **MEDIUM**
- 现有 `/api/ws` 无 JWT；前端可能直接 `new WebSocket("ws://host/api/ws")`
- **缓解**：
  - 7.3 在 `WS` query 接受 `?token=` 或 `Authorization: Bearer` 两种方式
  - 默认 `WS_REQUIRE_AUTH=true` 但通过 `config [ws] require_auth = false` 可关闭（兼容旧部署）
  - 文档明确：前端 WebSocket 客户端必须附加 token

### R3: audit middleware 性能影响 — **MEDIUM**
- 每个 API 调用都要 DB INSERT；高频调用（metrics 端点）可能成瓶颈
- **缓解**：
  - 7.4 audit 写入用 `tokio::spawn` 异步不阻塞响应
  - `/metrics` / `/api/health` / `/api/ready` 在 middleware 早期 bypass（直接 list 排除）
  - 失败仅记录 warning + metric counter，不影响 API 响应
  - DB 批量写入优化：使用 100ms 缓冲批量 insert（可选，Phase 8 优化）

### R4: 跨节点 RPC 重复处理 — **MEDIUM**
- Redis Pub/Sub 是 fanout，所有节点都会收到自己发布的消息
- **缓解**：
  - 7.2 RPC envelope 加 `from_node` 字段；本地 node 收到自己的消息跳过
  - 集成测试覆盖：A 发消息 → A 跳过，B/C 处理

### R5: StateStore 抽象层抽象泄漏 — **MEDIUM**
- `StreamStateRepository` trait 不能覆盖所有 use case（部分需要原子操作 + 复杂查询）
- **缓解**：
  - 7.1 trait 保持最小（crud + counter），复杂查询直接走 `state_store.xxx_raw`
  - trait 仅用于"高频 + 通用"场景；专用模块（如 JtCommandWaiter）可绕过 trait 直接用 StateStore
  - 文档明示 trait 边界

### R6: 删除 cache.rs 回归 — **MEDIUM**
- 7.1 全部迁完后再 7.6 删除；中间状态可能 break
- **缓解**：
  - 7.1 完成后跑 `grep -rn "crate::cache" src/` 仅在 `cache.rs` 自身命中（其他位置 0）
  - 7.6 才执行 `rm src/cache.rs` + `mod cache;` 移除
  - 三库 CI 在 7.1 / 7.6 完成后各跑一次

### R7: Argon2 哈希性能 + DB 字段长度 — **LOW**
- Argon2 默认参数（~50ms 每次 hash）；登录接口可接受
- **缓解**：
  - 7.6 使用 Argon2::default()（适度参数）；如需更高安全可加 `Params::new(19456, 2, 1)`
  - `gb_user.password` 字段 `VARCHAR(255)` for `$argon2id$v=19$m=19456,t=2,p=1$...` 完整 hash
  - 兼容旧明文：verify 失败时若 hash 不是 `$argon2` 前缀则视为明文（一次迁移期）

### R8: cluster 节点发现依赖 Redis 共享 — **LOW**
- `ClusterRegistry` 用 Redis SET；Redis 故障时节点发现失效
- **缓解**：
  - 7.2 提供 `single_node_mode = true` 配置项，跳过 cluster 检查
  - Redis 不可达时降级为单节点运行（仅本节点功能可用）
  - 文档明示：Redis 是 Phase 7 强依赖，生产环境必须 Redis HA

### N1（新增）：双节点集成测试 CI 环境 — **LOW**
- 集成测试需要真实 Redis；CI 环境未必有
- **缓解**：
  - `tests/integration/cluster_test.rs` 标 `#[ignore]`，仅本地或 Redis-CI job 跑
  - 默认 CI 仍跑单节点三库测试
  - 提供 `REDIS_URL` 环境变量可选启用

### N2（新增）：metrics 计数器覆盖度 — **LOW**
- 25+ 指标可能漏掉关键字段
- **缓解**：
  - 7.5 参考 Prometheus 最佳实践（counter / gauge / histogram）
  - 加 `gb_build_info{version="x.y.z"}` gauge 便于 Prometheus relabel
  - 文档明示所有字段含义

---

## 重新评估后的 P0/P1 优先级

| 任务 | 原优先级 | 重审后 | 理由 |
|---|---|---|---|
| 7.1 StateStore 全面接入 | P0 | **P0** | 后续 5 个任务都依赖；设计文档 §6.6 核心 |
| 7.2 跨节点 RPC + 集群节点发现 | P0 | **P0** | 设计文档 Acceptance 第 1 条核心 |
| 7.3 WebSocket cluster + JWT + 终端事件 | P1 | **P1** | Phase 6 末尾衔接 + 安全 |
| 7.4 安全路由 + 审计日志 | P1 | **P1** | 设计文档 §4 known issue |
| 7.5 Metrics + Health + Readiness | P1 | **P1** | 生产部署必备 |
| 7.6 鉴权码哈希 + 系统端点 + 横切 | P1 | **P1** | Phase 6 衔接 + 设计文档 §10 系统管理 |

---

## 实施顺序调整

1. **第一批（P0，~64h）**：
   - 7.1 StateStore 全面接入 + cache.rs 标 deprecated（为后续所有任务铺路）
   - 7.2 跨节点 RPC + 集群节点发现（核心 R1/R4 风险）
2. **第二批（P1，~36h）**：
   - 7.3 WebSocket cluster + JWT + 终端事件（R2 安全风险）
   - 7.4 安全路由 + 审计日志 + 日志管理（设计文档 §4 known issue）
3. **第三批（P1，~28h）**：
   - 7.5 Metrics + Health + Readiness
   - 7.6 鉴权码哈希 + 系统端点 + 删除 cache.rs + 横切

---

## 完成判定

- **三库 `cargo test --lib` 全绿**（默认 sqlite + `--features postgres` + `--features mysql` 三路 0 失败）
- `scripts/phase7-test-matrix.sh` 一键三库验证脚本 exit 0
- **新增 ≥ 80 个单测**覆盖 7.1-7.6 涉及的所有模块（state_store / state / cache 替代 / rpc / cluster / ws / middleware / metrics / health / system / auth / handlers）
- **集成测试 ≥ 6 个**：cluster / audit / ws_cluster / state_store_consistency / metrics / auth
- **单节点无 Redis 部署正常**：所有功能可用，`/metrics` 显示 `gb_redis_connected 0`
- **单节点有 Redis 部署正常**：所有功能可用，`/metrics` 显示 `gb_redis_connected 1` + `gb_redis_state_keys > 0`
- **双节点 Redis 部署正常**：A 触发 alarm → B 上 WS 客户端能收到；A `/api/play/start` → B `/api/play/stop` 能停止
- **`/api/ws` 必须 JWT 校验**：无 token 返 401 close；有效 token upgrade 成功
- **`/api/alarm/*` 必须 JWT 校验**：未登录返 401
- **审计验证**：调用 100 个 API → `gb_audit_log` 有 100 条 status_code 准确的记录
- **`/api/system/info` `/stats` `/version` `/online-users` 全部返 200 + 数据**
- **`/api/ready` 在 DB 不可达时返 503**（readiness）；`/api/health` 始终 200（liveness）
- **`/metrics` 含 25+ 指标 + Prometheus HELP/TYPE**
- **`grep -rn "crate::cache" src/` 0 命中**（Phase 7.6 删除 cache.rs 后）
- **`grep -n "build_success.*成功" src/handlers/alarm.rs` 仅剩注释**
- **`grep -n "/api/alarm/list" src/router.rs` 在 `api_protected` 内（不再独立追加）**
- **DB 中 password 字段 `VARCHAR(255)` 含 Argon2 hash（非明文）**
- **`docs/OPERATIONS.md` 新增 Phase 7 章节**：cluster 部署 + Redis HA + Prometheus 接入 + audit 配置可复现
- CI workflow 含三库 job（sqlite 默认 + 显式 postgres/mysql）
- 双节点 cluster 集成测试在 `tests/integration/cluster_test.rs` 文档化

---

## 后续 Phase 衔接

- **Phase 8 智能分析（人脸/车辆/行为）** → 7.5 metrics 已含 `gb_inference_*` 字段可扩展；7.3 WebSocket 可订阅 `inference_alarm` event
- **Phase 8 系统监控（CPU/内存/磁盘）** → 7.5 `/api/system/stats` 加 system load 字段
- **Phase 8 高级录像管理（合并/转码/截图）** → 7.1 RecordingState 扩展 `cloud_record_*` 状态；7.2 跨节点 cloud_record_sync 已实现
- **Phase 9 集群联邦（多 WVP 跨域）** → 7.2 ClusterRegistry 可扩展为多 cluster；RPC 支持 cluster_id 分发
