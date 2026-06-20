# Findings & Decisions

## Requirements
- 输入：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`（§7 Phase 7） + `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md` + 当前 main 分支代码（基线 `79bfb29` Phase 6.1 完成）
- 输出：`docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`
- 要求：与既有计划风格一致；可被 subagent 任务级执行；含 File Structure、子任务、关键代码骨架、验收命令、风险与衔接
- 风格：中文；技术名词保持英文；表格化呈现

## Research Findings

### 设计文档 §7 Phase 7 原文要点
1. **Goal**：complete production deployment features（Redis 集群 / RPC / WebSocket / 运维 / 边缘 API）
2. **Tasks (5)**：
   - 7.1 Redis-backed `StateStore` for CSEQ/SN/SSRC, device/stream/session state, GPS/alarm, push/proxy, platform SendRtp, WebSocket fanout
   - 7.2 跨节点 RPC for device control / play/stop / stream state / platform play / SendRtp / cloud-record operations
   - 7.3 Add or align `/api/rtp` `/api/ps` log download / system info / health/readiness / metrics behavior
   - 7.4 Fix public/protected routing for `/api/alarm/*` and `/api/ws` or document them as intentionally public
   - 7.5 Record audit logs with real response statuses
3. **Acceptance**：
   - Single-node and two-node Redis-backed deployments pass core protocol smoke tests
   - WebSocket events and stream states remain consistent across nodes
   - Security route exposure matches intended policy
4. **Estimate**：2-4 周

### 当前代码现状（直接来自仓库审计）

**`src/state_store.rs`（1061 行）— 已有完整 StateStore 抽象层**：
- 13 类 State 结构：`DeviceOnlineState` / `StreamState` / `InviteSessionState` / `MediaServerLoad` / `MobilePositionState` / `ActiveRecordingState` / `JTTerminalState` / `PlatformSendRtpState` / ...
- `StateStore::in_memory()` + `StateStore::redis(url)` 双 backend
- `InMemoryBackend`（172 行，`std::sync::RwLock`）+ `RedisBackend`（316 行，`ConnectionManager` + 自动重连 + 1.5s 连接超时）
- `StateBackend` trait + `serde_json` 序列化所有 State
- 30+ setter/getter/deleter 方法
- **仅 7 个文件实际使用**：`src/lib.rs`（构造）/ `src/scheduler/record_plan.rs` / `src/sip/gb28181/{cascade_forward,playback_session,invite_session}.rs` / `src/cache.rs`（deprecated 注释）
- **缺口**：缺 `PendingRequestState` / `SubscriptionState` / `JtCommandWaiterState` / `JtMediaSessionState` / `RecordingState` 完整迁移

**`src/cache.rs`（140 行）— deprecated 但仍存在**：
- 全部函数 `#[deprecated(note = "迁移到 StateStore::xxx")]`
- `set_device_online` / `get_device_online` / `set_stream_info` / `get_stream_info` / `incr_media_server_streams` / `decr_media_server_streams` / `get_media_server_stream_count` / `reset_media_server_streams` / `set_recording_state` / `get_recording_state` / `del_recording_state`
- **仍被 import**：grep `crate::cache::` 在 handlers/play.rs + playback.rs + stream.rs + device_control.rs 等多处

**`src/rpc.rs`（419 行）— 已实现 RpcRouter**：
- `RpcRequest` / `RpcResponse` / `RpcTransport` trait + `LocalRpc`（单节点 tokio mpsc）+ `RpcRouter`（按 method 分发）
- `register_standard_handlers` 注册 device_control / play_stop / stream_state_changed / cascade_sendrtp_start/stop / cloud_record_sync / ws_broadcast
- **缺口**：缺 `RedisRpcTransport`（跨节点 Pub/Sub）+ 节点发现 + `from_node` 过滤

**`src/metrics.rs`（60 行）— 仅 5 个指标**：
- `jt1078_missing_retransmit_total` / `jt1078_active_sessions` / `sip_devices_online` / `sip_invites_active` / `streams_active`
- 全部 AtomicU64 / AtomicUsize；缺 cluster / RPC / WS / Redis / DB / audit 指标
- **缺口**：缺 Prometheus HELP/TYPE 注释（部分字段缺 HELP）

**`src/handlers/websocket.rs`（98 行）— 单节点 + 无 JWT**：
- `WsState { tx_map: Arc<RwLock<HashMap<client_id, mpsc::Sender>>>` }`
- `broadcast(event, data)` 仅本节点 mpsc 派发
- `ws_handler` 直接 upgrade，**无 JWT 校验**
- **缺口**：JWT 校验 + cluster fanout + 客户端订阅过滤 + 终端事件

**`src/handlers/rtp_control.rs`（157 行）— `/api/rtp/*` `/api/ps/*` 已实现**：
- `rtp_receive_open` / `close` / `send_start` / `send_stop` / `ps_*` / `ps_get_test_port`
- 走 `zlm.open_rtp_server` / `zlm.close_rtp_server` / `zlm.send_rtp_info`
- **注意点**：`/api/ps/send/stop` 是占位（注释明示"implicit on stream teardown"）；可优化但非阻塞

**`src/db/audit_log.rs`（272 行）— 完整三态 cfg**：
- `ensure_table`（postgres/mysql/sqlite）+ `insert` + `list_paged`（含 username / action / start_time / end_time 过滤 + 分页）
- **仅 2 处调用**：`src/auth.rs:102`（login）+ `src/auth.rs:144`（logout）
- **缺口**：handler middleware 自动写 + `/api/log/list` 替换 stub + request_body 字段

**`src/router.rs`（40376 行）— alarm/ws 路由在主体外**：
- Line 956：`/api/ws` 独立追加（未走 `auth_middleware`）
- Line 960-966：`/api/alarm/list` + 7 个 `/api/alarm/*` 独立追加（**未走 `auth_middleware`**）— 设计文档 §4 known issue
- Line 932：`/api/rpc` 已注册
- Line 934：`/metrics` 已注册
- Line 230-237：`/api/rtp/*` `/api/ps/*` 早期注册（被 891-899 覆盖）
- **缺口**：alarm/ws 合并回 `api_protected`；audit middleware 装载

**`src/auth.rs`（约 200 行关键段）— 用户密码明文**：
- 登录成功仅调用 `db::audit_log::insert`（line 102/144）
- **缺口**：password 字段仍是明文（Phase 6 衔接明示）；Argon2 哈希未实现

### 与 Phase 1-6 衔接点

| Phase | 可复用资产 | 7.x 用法 |
|---|---|---|
| Phase 1 | `PendingRequestManager`（key: device_id+sn） | 7.1 扩展 `PendingRequestState` 走 StateStore |
| Phase 1 | `InviteSessionStore`（INVITE 会话） | 7.1 扩展 `InviteSessionState`；Redis 让多节点 SIP 共享 |
| Phase 2 | `SubscriptionLifecycle`（`subscription_state` HashMap） | 7.1 迁 StateStore + 7.2 跨节点续期 |
| Phase 3 | `MediaWaiterManager`（oneshot 等待 ZLM 媒体） | 7.1 `InviteSessionState.zlm_stream_id` 已含 |
| Phase 3 | RecordInfo 多包聚合 | 7.2 跨节点分发 + `cloud_record_sync` |
| Phase 4 | `StateStore`（已实现双 backend） | 7.1 扩展新 State + Repository trait |
| Phase 4 | `select_least_loaded_server_filtered` | 7.5 metrics 加 `gb_media_server_load` |
| Phase 4 | `mark_offline_if_expired` | 7.2 跨节点时由 RPC 同步 |
| Phase 5 | `CascadeRegistrar` / `SendRtpManager` | 7.2 跨节点 SendRtp 用 RedisRpcTransport |
| Phase 5 | 5.5a/b MobilePosition / Alarm 上行 | 7.3 `ws_hub.broadcast_event("jt_position")` |
| Phase 5 | `close_by_stream` 用 `state_store.del_cascade_sendrtp` | 7.1 已就位；Redis 多节点共享 |
| Phase 6 | `JtCommandWaiter` / `JtMediaSessionManager` | 7.1 终端注册表/等待/session 走 StateStore |
| Phase 6 | 终端鉴权码 `auth_code` 明文 | 7.6 哈希（与 user password 一起） |
| Phase 6 | 终端位置/告警 WS 推送未实现 | 7.3 `jt_event` channel |

### Phase 7 子任务映射

| 设计文档 | 当前完成度 | Phase 7 子任务 |
|---|---|---|
| 7.1 Redis-backed StateStore for CSEQ/SN/SSRC + device/stream/session + GPS/alarm + push/proxy + SendRtp + WS fanout | StateStore 1061 行已实现双 backend；仅 7 文件使用；`cache.rs` deprecated 仍 140 行 | 7.1 StateStore 全面接入 + 7 个新 State + `StreamStateRepository` trait + `cache.rs` 全面替换 |
| 7.2 跨节点 RPC（device control / play/stop / stream state / SendRtp / cloud-record） | `RpcRouter` 419 行 + `LocalRpc` + 7 standard handlers 已实现；缺 Redis transport + 节点发现 | 7.2 RedisRpcTransport（Pub/Sub + Stream inbox）+ ClusterRegistry（Redis SET 心跳）+ 节点发现 |
| 7.3 `/api/rtp` `/api/ps` log download / system info / health/readiness / metrics | `rtp_control.rs` 157 行已实现 RTP/PS 真实转发；缺 system/info/stats/version/online-users；`/api/log/list` 是 stub | 7.3 WebSocket cluster fanout + JWT 鉴权 + 终端事件（jt_position / jt_alarm）+ system 端点 + log_audit 真实实现 |
| 7.4 Fix public/protected routing for `/api/alarm/*` `/api/ws` | router.rs:956-966 alarm/ws 独立追加未走 auth；audit_log DB 已有但仅 2 处调用 | 7.4 alarm/ws 合并回 api_protected + audit middleware 自动写所有 handler + status_code 准确 |
| 7.5 审计日志带真实 response statuses | audit_log 表已三态 cfg；`list_paged` 已实现 | 7.5 metrics 扩展 25+ 指标（cluster/RPC/WS/Redis/audit）+ `/api/health` 拆分为 liveness + `/api/ready` readiness |
| 隐含：密码哈希 + 在线用户 + 系统监控 | password 明文 + 缺 `/api/system/*` + 缺 `/metrics` Prometheus HELP/TYPE | 7.6 Argon2 hash + `/api/system/{info,stats,version,online-users}` + 删除 cache.rs + 三库 CI + 文档 |

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Phase 7 拆 6 子任务（5 功能 + 1 横切） | phase-4/5/6 已证明 5-6 子任务粒度最稳 |
| StateStore 优先于 cache.rs：`StreamStateRepository` trait + StateStoreRepository impl | 设计文档 §6.6 强制要求 + 抽象复用 + 避免双重 API |
| RedisRpcTransport 用 Pub/Sub + Stream inbox 双模式 | Pub/Sub 实时 + Stream 至少一次补可靠性；故障时降级 |
| WebSocket JWT 在 upgrade 前校验（不进 auth_middleware） | upgrade 协议特殊；fallback query `?token=` 兼容 |
| audit middleware 用 `tokio::spawn` 异步写 + bypass `/metrics` `/health` `/ready` | 不阻塞响应 + 避免自身递归 |
| password 哈希 Argon2 + 兼容旧明文（一次迁移期 verify 失败时按明文匹配） | 设计文档 §6 安全 + 平滑迁移 |
| ClusterRegistry 用 Redis SET + ZSET 心跳（10s 心跳 / 60s 过期） | 比 gossip 简单；Redis HA 生产可用 |
| 单节点模式 `single_node_mode = true` 跳过 cluster 检查 | Redis 故障时降级运行 |
| 双节点 cluster / ws_cluster 集成测试标 `#[ignore]` 仅本地跑 | CI 无 Redis 依赖；本地 + Redis-CI job 启用 |
| `/api/health` 拆 liveness + `/api/ready` readiness | Kubernetes 标配；liveness 不查 DB 避免重启循环 |
| metrics 25+ 指标加 Prometheus HELP/TYPE | Prometheus relabel + 文档化字段含义 |
| 沿用 phase-4/5/6 的三库 cfg + tests/integration/ 模式 | 与既有 CI 矩阵一致 |
| Phase 7.1 先标 deprecated；Phase 7.6 才删除 cache.rs | 平滑迁移 + 三库 CI 验证 |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| StateStore 已实现 1061 行但仅 7 个文件用 | 7.1 全面接入 + 7 个新 State + Repository trait 抽象 |
| `cache.rs` 已 deprecated 但仍 140 行 | 7.1 全部 `#[deprecated]` + 调用方替换；7.6 删除整个文件 |
| `RpcRouter` 仅 `LocalRpc`，无 Redis transport | 7.2 加 `RedisRpcTransport` + 节点发现 + `from_node` 过滤 |
| `mark_offline_if_expired`（Phase 4）单节点执行 | 7.2 跨节点时通过 RPC 同步 |
| `WsState` 单节点 mpsc + 无 JWT | 7.3 cluster fanout + JWT upgrade 前校验 |
| `/api/alarm/*` 在 router.rs:960-966 独立追加 | 7.4 合并回 `api_protected` 走 auth_middleware |
| `/api/ws` router.rs:956 独立追加无 JWT | 7.4 JWT 在 upgrade 前校验 |
| `auth.rs` 仅 2 处 `db::audit_log::insert` | 7.4 middleware 自动写所有 handler |
| `/api/log/list` 是 stub | 7.4 改用 `audit_log` DB 查询 |
| `metrics.rs` 60 行 5 个指标 | 7.5 扩展到 25+ 指标含 cluster/RPC/WS |
| `/api/health` 简单 SQL ping | 7.5 拆 liveness + readiness（DB+Redis+cluster 检查） |
| 缺 `/ready` `/api/system/info` | 7.5/7.6 新增 |
| 用户 password 明文 | 7.6 Argon2 哈希 + 兼容旧明文迁移 |
| Phase 6 终端事件 WS 未实现 | 7.3 加 `jt_event` channel |
| 缺 `/api/system/online-users` | 7.6 新增（写 `gb_online_user` 表） |
| `state/mod.rs` 20 行无 trait 抽象 | 7.1 加 `StreamStateRepository` trait |

## Final Deliverable
- `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`（~570 行）
- 6 子任务：7.1 StateStore 全面接入 / 7.2 跨节点 RPC + 集群节点发现 / 7.3 WebSocket cluster + JWT + 终端事件 / 7.4 安全路由 + 审计日志 + 日志管理 / 7.5 Metrics + Health + Readiness / 7.6 鉴权码哈希 + 系统端点 + 横切
- 估时 ~128h（3 周编码 + 1 周 buffer）

## Resources
- 设计文档：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`
- 上游基线：WVP-Pro 2.7.4 / commit b760458
- 既有计划：
  - `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`（6.x JT1078）
  - `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`（5.x 级联）
  - `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md`（3.x 视频/录像）
- 关键代码（Phase 7 范畴）：
  - `src/state_store.rs`（1061 行）+ `src/cache.rs`（140 行）— 状态抽象层
  - `src/rpc.rs`（419 行）— RpcRouter + LocalRpc
  - `src/handlers/{websocket,rtp_control,metrics}.rs` — WS / RTP/PS / Metrics
  - `src/db/audit_log.rs`（272 行）— 审计日志 DB
  - `src/metrics.rs`（60 行）— Prometheus 输出
  - `src/auth.rs` — 用户鉴权
  - `src/router.rs`（40376 行，956-966 行 alarm/ws 独立追加）
- 数据库：`database/init-{sqlite,postgresql,mysql}-2.7.4.sql`
- 鉴权：JWT（jsonwebtoken） + Argon2（待引入）
- Redis：`redis = "0.25"` + `connection-manager`

## Visual/Browser Findings
- （无图像/浏览器内容）

---
*2-Action Rule：每 2 次视图/搜索后立即更新；本任务以代码阅读为主，关键发现已落到上方表格*
