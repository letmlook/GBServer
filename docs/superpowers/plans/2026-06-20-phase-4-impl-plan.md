# Phase 4 实施方案 — ZLM / Media-Node Production Parity

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 ZLM hook / 媒体节点行为对齐 WVP-Pro：覆盖所有 `on_*` hook、play/publish 鉴权、节点健康检查自动切离线、流状态统一接口、least-load 节点选择。

**Architecture:** 基于现有 `src/zlm/hook.rs` 的 dispatcher 模式扩展；新增 `MediaNode` 抽象管理 secret/RTP port range/keepalive；统一 `StreamStatus` 接口覆盖 GB/push/proxy/SendRtp 四类流；保持三库 cfg（sqlite/postgres/mysql）。

**Tech Stack:** Rust + Axum + SQLx + ZLMediaKit HTTP API + DashMap（in-memory state）+ Redis（可选，节点计数）。

**基线 commit:** `ac1498f`（Phase 3 完成）
**上游设计:** `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 4
**总工作量:** ~80h（约 2 周），拆 6 个子任务。

---

## 全局约束

- **数据库默认 feature = sqlite**：所有 `src/db/*.rs` 新增的表 / 函数必须三态 cfg（`postgres` / `mysql` / `sqlite`）
- **优先复用已有 db 模块**：禁止新增重复表，复用 `gb_media_server` / `gb_stream_push` / `gb_stream_proxy` / `gb_send_rtp`
- **ZLM 节点选择**：least-load 用 Redis 计数器（已有 `cache::media_server_streams`），fallback 顺序：显式 id → least-load → 单节点默认
- **Hook secret 验证**：所有 ZLM 推送的 webhook 必须校验 `secret` 字段（与 `zlm[*].secret` 配置匹配）
- **三库 CI 必跑**：`cargo test --lib` + `cargo test --no-default-features --features postgres --lib` + `cargo test --no-default-features --features mysql --lib` 三路全绿
- **承接 Phase 3 收尾**：3.1 媒体到达、3.4 download 进度已依赖 `on_stream_changed` / `on_rtp_server_started`；Phase 4 在此基础上扩展边界场景

---

## Context

按设计文档 §7 Phase 4 需把 ZLM hook / 媒体节点行为对齐 WVP-Pro：

1. **Hook 覆盖**：WVP-Pro 全部 `on_*` 钩子 + 路由兼容
2. **Play/Publish 鉴权**：secret 验证 + 黑白名单
3. **节点管理**：保存 / 健康检查 / 删除 / 加载 + 自动配置（hook URL + RTP port range）
4. **流状态统一**：GB28181 流 / 推流 / 代理流 / SendRtp 流 共用状态机
5. **节点选择**：least-load（Redis 计数 → 流数最小）

**Acceptance**（设计文档原文）：
- ZLM startup auto-configures hooks
- ZLM stream events resolve pending invite/download/session state
- Media-node offline/online transitions do not leave stale sessions

**当前差距**（代码审计确认，对照 `src/zlm/hook.rs`）：

| # | 现状 | 缺口 |
|---|---|---|
| 1 | 单一 `handle_webhook` dispatcher | 缺 WVP-Pro 路由兼容（`/api/hook/...` 多路径），缺事件类型枚举 |
| 2 | `on_play` / `on_publish` 只记录日志 | 缺 secret 鉴权；任何带 URL 的客户端都能拉流 |
| 3 | `on_server_started` 已自动配置 hooks | 未配置 `rtp.port_range` / `send_rtp.port_range` |
| 4 | `on_server_keepalive` 写 DB | 缺超时检测 → 节点自动切 offline |
| 5 | `gb_stream_push` / `gb_stream_proxy` / `gb_send_rtp` 三表 | 状态字段名不统一（`pushing` / `pulling` / `running`），上层查询需 switch |
| 6 | `select_least_loaded` 用 Redis 计数 | Redis 不可用时 fallback 到顺序选择，缺单元测试覆盖 |
| 7 | `on_rtp_server_timeout` 已实现 | 缺 stale session 清理（device 离线 / 服务重启） |

**预估工作量**：~80h（2 周编码 + 1 周 buffer），6 个子任务，3-5 个 PR。

---

## File Structure

| 路径 | 责任 | 状态 |
|---|---|---|
| `src/zlm/hook.rs` | ZLM webhook dispatcher + 12 个 on_* 处理器 | 改 |
| `src/zlm/auth.rs`（新） | Play/Publish secret 鉴权 + IP 白名单 | 增 |
| `src/zlm/hook_routes.rs`（新） | WVP-Pro 多路径 hook 路由分发 | 增 |
| `src/zlm/media_node.rs`（新） | MediaNode 抽象：health check / keepalive / auto-config | 增 |
| `src/state/stream_status.rs`（新） | 流状态统一接口（GB / push / proxy / SendRtp） | 增 |
| `src/db/media_server.rs` | 节点 CRUD + status + keepalive（已三态 cfg） | 改 |
| `src/lib.rs` | 节点 keepalive 后台 loop + 离线检测 | 改 |
| `src/zlm/client.rs` | ZLM 节点 HTTP API 客户端（已有） | 改（新增 health_check / get_node_config） |
| `tests/integration/zlm_hook_test.rs`（新） | hook 集成测试（mock ZLM） | 增 |

---

## 任务清单

### Task 4.1 — WVP-Pro Hook 路由兼容 + 事件类型枚举（P1，6h）

**目标**：支持 WVP-Pro 的多路径 hook 路由（`/api/hook/...`）+ 用枚举定义所有 ZLM 事件类型，dispatcher 严格匹配。

**Files:**
- Modify: `src/zlm/hook.rs:1-50`（新增 `ZlmHookEvent` 枚举）
- Create: `src/zlm/hook_routes.rs`（多路径路由分发）
- Modify: `src/router.rs`（注册 `/api/hook/*` 多路径）
- Test: `src/zlm/hook.rs` 末尾新增 `tests::test_zlm_hook_event_parse_*`（5-7 个）

**WVP-Pro 路由兼容**（设计文档 §6.1）：
- `/api/hook/on_stream_changed`
- `/api/hook/on_publish`
- `/api/hook/on_play`
- `/api/hook/on_none_reader`
- `/api/hook/on_server_keepalive`
- ...（每个 hook 一个路径，方便前端按需订阅）

**关键代码骨架**：

```rust
// src/zlm/hook.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlmHookEvent {
    ServerStarted,
    ServerKeepalive,
    StreamChanged,
    StreamNotFound,
    StreamNoneReader,
    StreamStarted,
    Publish,
    Play,
    RtpServerStarted,
    RtpServerTimeout,
    SendRtpStopped,
    RecordMp4,
    RecordProgress,
    FlowReport,
    Unknown,
}

impl ZlmHookEvent {
    pub fn from_hook_name(name: &str) -> Self {
        match name {
            "on_server_started" => Self::ServerStarted,
            "on_server_keepalive" => Self::ServerKeepalive,
            "on_stream_changed" => Self::StreamChanged,
            "on_stream_not_found" => Self::StreamNotFound,
            "on_stream_none_reader" => Self::StreamNoneReader,
            "on_stream_started" => Self::StreamStarted,
            "on_publish" => Self::Publish,
            "on_play" => Self::Play,
            "on_rtp_server_started" => Self::RtpServerStarted,
            "on_rtp_server_timeout" => Self::RtpServerTimeout,
            "on_send_rtp_stopped" => Self::SendRtpStopped,
            "on_record_mp4" | "on_record_file" => Self::RecordMp4,
            "on_record_progress" => Self::RecordProgress,
            "on_flow_report" => Self::FlowReport,
            _ => Self::Unknown,
        }
    }

    pub fn default_response(&self) -> serde_json::Value {
        match self {
            // WVP-Pro 兼容：返回 { code: 0, msg: "success" }
            _ => serde_json::json!({"code": 0, "msg": "success"}),
        }
    }
}
```

```rust
// src/zlm/hook_routes.rs
use axum::{routing::post, Router};
use crate::AppState;

pub fn hook_routes() -> Router<AppState> {
    Router::new()
        .route("/api/hook/on_server_started", post(handle_hook_event::<ServerStarted>))
        .route("/api/hook/on_server_keepalive", post(handle_hook_event::<ServerKeepalive>))
        .route("/api/hook/on_stream_changed", post(handle_hook_event::<StreamChanged>))
        .route("/api/hook/on_stream_not_found", post(handle_hook_event::<StreamNotFound>))
        .route("/api/hook/on_stream_none_reader", post(handle_hook_event::<StreamNoneReader>))
        .route("/api/hook/on_publish", post(handle_hook_event::<Publish>))
        .route("/api/hook/on_play", post(handle_hook_event::<Play>))
        .route("/api/hook/on_rtp_server_started", post(handle_hook_event::<RtpServerStarted>))
        .route("/api/hook/on_rtp_server_timeout", post(handle_hook_event::<RtpServerTimeout>))
        .route("/api/hook/on_send_rtp_stopped", post(handle_hook_event::<SendRtpStopped>))
        .route("/api/hook/on_record_mp4", post(handle_hook_event::<RecordMp4>))
        .route("/api/hook/on_flow_report", post(handle_hook_event::<FlowReport>))
        // 默认 `/api/zlm/hook` 仍保留（Phase 0 已注册）
        .route("/api/zlm/hook", post(super::hook::handle_webhook))
}
```

**子任务**：
- [ ] **Step 1**: 添加 `ZlmHookEvent` 枚举 + `from_hook_name` / `default_response` 方法（含 14 个 variant）
- [ ] **Step 2**: 添加单元测试 `test_zlm_hook_event_parse_stream_changed` 等（5-7 个 case）
- [ ] **Step 3**: 运行 `cargo test --lib zlm::hook::tests::test_zlm_hook_event` 确认新测试通过
- [ ] **Step 4**: 创建 `src/zlm/hook_routes.rs`，定义 `hook_routes()` 函数（用泛型 handler 占位）
- [ ] **Step 5**: 在 `src/router.rs` 注册 `hook_routes()`（合并到主 router）
- [ ] **Step 6**: 运行 `cargo build --lib` 确认编译通过
- [ ] **Step 7**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 8**: Commit `feat(phase-4): WVP-Pro hook route compat + ZlmHookEvent enum`

### Task 4.2 — on_play / on_publish Secret 鉴权 + IP 白名单（P0，10h）

**目标**：ZLM 推送的 `on_play` / `on_publish` hook 必须校验 `secret`；客户端 IP 必须在白名单（或设备注册 IP 匹配）。

**Files:**
- Create: `src/zlm/auth.rs`（PlayAuthChecker + PublishAuthChecker）
- Modify: `src/zlm/hook.rs:578-611`（在 on_play / on_publish handler 开头插入鉴权检查）
- Modify: `src/db/media_server.rs`（新增 `get_white_list_cidrs` / `add_white_list_cidr`）
- Modify: `database/init-sqlite-2.7.4.sql` + `init-postgresql-2.7.4.sql` + `init-mysql-2.7.4.sql`（新增 `gb_media_server_white_list` 表）
- Test: `src/zlm/auth.rs` 末尾新增 tests（5 个）

**关键代码骨架**：

```rust
// src/zlm/auth.rs
use std::net::IpAddr;

pub struct HookAuthChecker {
    /// 节点配置 secret（必须匹配）
    expected_secret: String,
    /// 可选白名单 CIDR
    whitelist: Vec<ipnetwork::IpNetwork>,
}

impl HookAuthChecker {
    pub fn new(secret: &str) -> Self {
        Self {
            expected_secret: secret.to_string(),
            whitelist: Vec::new(),
        }
    }

    pub fn with_whitelist(mut self, cidrs: Vec<ipnetwork::IpNetwork>) -> Self {
        self.whitelist = cidrs;
        self
    }

    /// 校验 ZLM 推送的 secret 字段
    pub fn check_secret(&self, provided: &str) -> bool {
        // constant-time 比较，避免 timing attack
        if self.expected_secret.len() != provided.len() {
            return false;
        }
        let mut diff = 0u8;
        for (a, b) in self.expected_secret.bytes().zip(provided.bytes()) {
            diff |= a ^ b;
        }
        diff == 0
    }

    /// 校验客户端 IP
    pub fn check_ip(&self, ip: &IpAddr) -> bool {
        if self.whitelist.is_empty() {
            return true; // 无白名单时放行
        }
        self.whitelist.iter().any(|net| net.contains(*ip))
    }

    pub fn check(&self, secret: &str, ip: &IpAddr) -> AuthResult {
        if !self.check_secret(secret) {
            return AuthResult::UnauthorizedSecret;
        }
        if !self.check_ip(ip) {
            return AuthResult::IpNotWhitelisted;
        }
        AuthResult::Ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    Ok,
    UnauthorizedSecret,
    IpNotWhitelisted,
}
```

```rust
// src/zlm/hook.rs::on_play handler
"on_play" => {
    if let Some(data) = serde_json::from_value::<PlayData>(event.clone()).ok() {
        // Phase 4.2: secret 鉴权 + IP 白名单
        let provided_secret = event.get("secret").and_then(|v| v.as_str()).unwrap_or("");
        let client_ip_str = event.get("ip").and_then(|v| v.as_str()).unwrap_or("0.0.0.0");
        if let Ok(client_ip) = client_ip_str.parse::<std::net::IpAddr>() {
            let auth = HookAuthChecker::new(&data.secret); // 节点 secret 与 on_play 自带 secret 字段一致
            match auth.check(provided_secret, &client_ip) {
                AuthResult::Ok => {
                    tracing::info!("on_play: {}/{}/{} from {} (authorized)",
                        data.app, data.stream, data.schema, client_ip);
                    // ... 继续原逻辑
                }
                AuthResult::UnauthorizedSecret => {
                    tracing::warn!("on_play: secret mismatch from {}", client_ip);
                    return Json(WVPResult::error("Unauthorized: secret mismatch"));
                }
                AuthResult::IpNotWhitelisted => {
                    tracing::warn!("on_play: IP {} not in whitelist", client_ip);
                    return Json(WVPResult::error("Unauthorized: IP not in whitelist"));
                }
            }
        }
    }
}
```

**子任务**：
- [ ] **Step 1**: 在 `Cargo.toml` 添加 `ipnetwork = "0.20"` 依赖
- [ ] **Step 2**: 创建 `src/zlm/auth.rs`，定义 `HookAuthChecker` + `AuthResult`
- [ ] **Step 3**: 添加单元测试 `test_check_secret_match` / `test_check_secret_mismatch` / `test_check_ip_whitelist_match` / `test_check_ip_whitelist_miss` / `test_check_constant_time`（5 个 case 覆盖 constant-time 行为）
- [ ] **Step 4**: 运行 `cargo test --lib zlm::auth::` 确认新测试通过
- [ ] **Step 5**: 在三个 init SQL 文件新增 `gb_media_server_white_list` 表（id, media_server_id, cidr, create_time）
- [ ] **Step 6**: 在 `src/db/media_server.rs` 新增 `get_white_list_cidrs` / `add_white_list_cidr` / `remove_white_list_cidr`（三态 cfg）
- [ ] **Step 7**: 在 `src/zlm/hook.rs::on_play` / `on_publish` handler 开头插入 `HookAuthChecker::check`
- [ ] **Step 8**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 9**: Commit `feat(phase-4): on_play/on_publish secret auth + IP whitelist`

### Task 4.3 — on_server_started 自动配置 RTP Port Range + 完整 Hook（P0，8h）

**目标**：`on_server_started` 触发时不仅配置 hook URL，还自动 set `rtp.port_range` / `send_rtp.port_range` / `hook.enable` / `protocol.enable_*`，与 `gb_media_server` 表配置对齐。

**Files:**
- Modify: `src/zlm/hook.rs:619-680`（扩展 on_server_started 配置项）
- Modify: `src/zlm/client.rs`（新增 `set_rtp_port_range` 辅助方法）
- Test: `src/zlm/hook.rs::tests` 新增 `test_on_server_started_auto_configures_rtp_port_range`

**关键代码骨架**：

```rust
// src/zlm/hook.rs::on_server_started 内的配置循环扩展
if let Some(ref zlm_client) = state.zlm_client {
    // ... 现有 hook URL 配置 ...

    // Phase 4.3: 自动配置 RTP port range（从 gb_media_server.rtp_port_range）
    if let Ok(Some(server_config)) = crate::db::media_server::get_media_server_by_id(
        &state.pool, media_server_id,
    ).await {
        if let Some(ref rtp_range) = server_config.rtp_port_range {
            let (start, end) = parse_port_range(rtp_range)?;
            zlm_client.set_server_config(&secret, "rtp.port_range", &format!("{}-{}", start, end)).await?;
        }
        if let Some(ref srtp_range) = server_config.send_rtp_port_range {
            let (start, end) = parse_port_range(srtp_range)?;
            zlm_client.set_server_config(&secret, "send_rtp.port_range", &format!("{}-{}", start, end)).await?;
        }
        // Phase 4.3: 协议开关（与 ZLM 默认对齐）
        for (key, value) in [
            ("protocol.enable_rtsp", "1"),
            ("protocol.enable_rtmp", "1"),
            ("protocol.enable_hls", "1"),
            ("protocol.enable_http", "1"),
            ("protocol.enable_ws", "1"),
            ("protocol.enable_rtp", "1"),
        ] {
            zlm_client.set_server_config(&secret, key, value).await?;
        }
    }
    tracing::info!("ZLM node {} fully auto-configured", media_server_id);
}

fn parse_port_range(s: &str) -> Result<(u16, u16)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid port range: {}", s));
    }
    Ok((parts[0].parse()?, parts[1].parse()?))
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/zlm/client.rs` 新增 `parse_port_range` 公共函数（可单测）
- [ ] **Step 2**: 添加单元测试 `test_parse_port_range_valid` / `test_parse_port_range_invalid_format` / `test_parse_port_range_non_numeric`
- [ ] **Step 3**: 扩展 `src/zlm/hook.rs::on_server_started` handler：从 `gb_media_server` 表读 `rtp_port_range` / `send_rtp_port_range` / 协议开关，调 `set_server_config` 同步到 ZLM
- [ ] **Step 4**: 添加集成测试 `test_on_server_started_configures_rtp_port_range`（mock ZLM HTTP server 用 `wiremock`）
- [ ] **Step 5**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 6**: Commit `feat(phase-4): on_server_started auto-configures rtp port range + protocol flags`

### Task 4.4 — MediaNode Keepalive 超时切离线（P0，12h）

**目标**：ZLM 节点超过 N 秒（默认 30s）无 keepalive → 自动切 offline；offline 节点的流全部清理由存量媒体会话切换到 remaining online 节点。

**Files:**
- Create: `src/zlm/media_node.rs`（MediaNode trait + HealthChecker）
- Modify: `src/lib.rs`（启动 `media_node_health_loop` 后台任务，每 10s 跑一次）
- Modify: `src/db/media_server.rs`（新增 `list_offline_servers` / `mark_offline_if_expired`）
- Modify: `src/state/stream_status.rs`（新增 `mark_all_streams_offline`）
- Test: `src/zlm/media_node.rs::tests`（5 个）

**关键代码骨架**：

```rust
// src/zlm/media_node.rs
use std::time::Duration;
use chrono::Utc;

pub const DEFAULT_KEEPALIVE_TIMEOUT_SECS: i64 = 30;

pub trait MediaNode: Send + Sync {
    fn id(&self) -> &str;
    fn last_keepalive(&self) -> Option<chrono::DateTime<Utc>>;
    fn is_online(&self) -> bool {
        match self.last_keepalive() {
            Some(t) => (Utc::now() - t).num_seconds() < DEFAULT_KEEPALIVE_TIMEOUT_SECS,
            None => false,
        }
    }
}

/// 后台 loop：每 10s 扫描所有节点，last_keepalive 超过阈值的标 offline
pub async fn health_check_loop(pool: &sqlx::Pool<sqlx::Sqlite>) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        if let Err(e) = run_health_check_once(pool).await {
            tracing::warn!("MediaNode health check failed: {}", e);
        }
    }
}

pub async fn run_health_check_once(pool: &sqlx::Pool<sqlx::Sqlite>) -> anyhow::Result<usize> {
    let offline_threshold = Utc::now() - chrono::Duration::seconds(DEFAULT_KEEPALIVE_TIMEOUT_SECS);
    let affected = crate::db::media_server::mark_offline_if_expired(pool, &offline_threshold.to_rfc3339()).await?;
    if affected > 0 {
        tracing::info!("Marked {} media nodes offline (keepalive timeout)", affected);
    }
    Ok(affected as usize)
}
```

```rust
// src/lib.rs::run() 在后台任务列表新增
tokio::spawn(crate::zlm::media_node::health_check_loop(pool.clone()));
```

**子任务**：
- [ ] **Step 1**: 在 `src/db/media_server.rs` 新增 `mark_offline_if_expired` 函数（三态 cfg SQL：`UPDATE gb_media_server SET status = 0 WHERE status = 1 AND last_keepalive < ?`）
- [ ] **Step 2**: 创建 `src/zlm/media_node.rs`，定义 `MediaNode` trait + `run_health_check_once` 函数
- [ ] **Step 3**: 添加单元测试 `test_is_online_with_recent_keepalive` / `test_is_online_with_old_keepalive` / `test_is_online_no_keepalive` / `test_run_health_check_once_marks_expired`（用 in-memory SQLite）
- [ ] **Step 4**: 在 `src/lib.rs::run()` 启动 `tokio::spawn(media_node::health_check_loop(...))`
- [ ] **Step 5**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 6**: Commit `feat(phase-4): MediaNode keepalive timeout auto-offline + health check loop`

### Task 4.5 — 流状态统一接口（P1，16h）

**目标**：GB28181 流 / 推流 / 代理流 / SendRtp 流 共用 `StreamStatus` 枚举 + `StreamState` trait，简化上层查询逻辑（不需 switch table）。

**Files:**
- Create: `src/state/stream_status.rs`（StreamStatus enum + StreamState trait）
- Modify: `src/db/stream_push.rs`（统一字段命名 + 加 `status` 字段 + 三态 cfg）
- Modify: `src/db/stream_proxy.rs`（同上）
- Modify: `src/db/send_rtp.rs`（同上，如已存在）
- Modify: `database/init-*-2.7.4.sql`（三表统一加 `status` 列，迁移 ALTER TABLE 兼容）
- Modify: `src/handlers/server.rs::list_media_servers`（用统一接口返回）
- Test: `src/state/stream_status.rs::tests`（6 个）

**关键代码骨架**：

```rust
// src/state/stream_status.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    /// 流已注册但未推送
    Ready,
    /// 正在推送/拉取
    Pushing,
    /// 拉取/推流中（GB28181 / SendRtp）
    Active,
    /// 流结束/超时
    Stopped,
    /// 推流失败
    Failed,
}

impl StreamStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pushing | Self::Active)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Pushing => "pushing",
            Self::Active => "active",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

pub trait StreamState: Send + Sync {
    fn stream_id(&self) -> &str;
    fn app(&self) -> &str;
    fn status(&self) -> StreamStatus;
    fn set_status(&mut self, status: StreamStatus);
    fn media_server_id(&self) -> Option<&str>;
    fn device_id(&self) -> Option<&str>;
    fn channel_id(&self) -> Option<&str>;
}
```

**子任务**：
- [ ] **Step 1**: 在三个 init SQL 文件加迁移 `ALTER TABLE gb_stream_push ADD COLUMN status TEXT NOT NULL DEFAULT 'ready'` 等三表（IF NOT EXISTS 防重复）
- [ ] **Step 2**: 创建 `src/state/stream_status.rs`，定义 `StreamStatus` enum + `StreamState` trait
- [ ] **Step 3**: 添加单元测试 `test_stream_status_is_active` / `test_stream_status_as_str` 等（6 个 case）
- [ ] **Step 4**: 让 `StreamPush` / `StreamProxy` / `SendRtpRecord` 实现 `StreamState` trait（impl block）
- [ ] **Step 5**: 在 `src/handlers/server.rs::list_media_servers` 新增 `list_all_streams` handler，调用三表后用 `StreamState` 统一序列化
- [ ] **Step 6**: 添加集成测试 `test_list_all_streams_unified_format`（mock 3 类流各 1 条，验证返回统一 JSON）
- [ ] **Step 7**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 8**: Commit `feat(phase-4): unified StreamStatus + StreamState trait across push/proxy/SendRtp`

### Task 4.6 — Least-Load 节点选择 + 离线节点过滤（P1，10h）

**目标**：`AppState::get_zlm_client_auto` 优先选 Redis 计数最低的 online 节点；过滤 offline 节点；添加降级测试。

**Files:**
- Modify: `src/lib.rs:401-440`（`get_zlm_client_auto` / `select_least_loaded`）
- Modify: `src/state_store.rs:741-750`（`select_least_loaded_server` 加 offline 过滤）
- Test: `src/lib.rs::tests` 新增 `test_get_zlm_client_auto_skips_offline` / `test_get_zlm_client_auto_picks_least_loaded`

**关键代码骨架**：

```rust
// src/lib.rs::select_least_loaded 修改
async fn select_least_loaded(&self) -> Option<(String, Arc<zlm::ZlmClient>)> {
    // Phase 4.6: 优先过滤 online 节点（last_keepalive < 30s）
    if let Some(ref pool) = self.pool {
        let online = match crate::db::media_server::list_online_servers(pool).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to list online media servers: {}", e);
                Vec::new()
            }
        };
        if !online.is_empty() {
            let online_ids: Vec<String> = online.iter().map(|s| s.id.clone()).collect();
            // 调 state_store 选 least-load
            if let Some(id) = self.state_store.select_least_loaded_server_filtered(&online_ids) {
                if let Some(client) = self.zlm_clients.get(&id) {
                    return Some((id, client.clone()));
                }
            }
            // state_store 不可用 / 全部节点都没有流计数 → 取 online 列表第一个
            if let Some(first) = online.first() {
                if let Some(client) = self.zlm_clients.get(&first.id) {
                    return Some((first.id.clone(), client.clone()));
                }
            }
        }
    }
    // 兼容：Redis 不可用时 fallback
    self.zlm_clients.iter().next().map(|(id, c)| (id.clone(), c.clone()))
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/state_store.rs` 新增 `select_least_loaded_server_filtered(&self, allowed_ids: &[String])` 方法（默认实现遍历 `backend`，过滤后选）
- [ ] **Step 2**: 在 `src/lib.rs::select_least_loaded` 加 offline 过滤逻辑
- [ ] **Step 3**: 添加单元测试 `test_select_least_loaded_skips_offline`（mock 3 个 zlm_clients，其中 1 个 offline，验证选剩余 2 个中流数最小的）
- [ ] **Step 4**: 添加单元测试 `test_select_least_loaded_picks_least_loaded`（mock 3 个 client，Redis 计数为 5/3/10，验证选 3 的）
- [ ] **Step 5**: 添加单元测试 `test_select_least_loaded_fallback_when_all_offline`（全 offline → 返回 None）
- [ ] **Step 6**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 7**: Commit `feat(phase-4): least-load selection filters offline nodes + redis fallback`

---

## 验收命令

```bash
# 默认（sqlite）
cargo test --lib

# PostgreSQL
cargo test --no-default-features --features postgres --lib

# MySQL
cargo test --no-default-features --features mysql --lib

# Phase 4 关键单测
cargo test --lib zlm::auth::
cargo test --lib zlm::media_node::
cargo test --lib zlm::hook::tests::test_zlm_hook_event
cargo test --lib state::stream_status::
cargo test --lib lib::tests::test_get_zlm_client_auto
```

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/zlm/hook.rs` | 新增 ZlmHookEvent 枚举 + on_play/on_publish 鉴权 + on_server_started 扩配置 | 8h |
| `src/zlm/auth.rs`（新） | HookAuthChecker + AuthResult | 4h |
| `src/zlm/hook_routes.rs`（新） | WVP-Pro 多路径路由分发 | 4h |
| `src/zlm/media_node.rs`（新） | MediaNode trait + 健康检查 loop | 6h |
| `src/state/stream_status.rs`（新） | StreamStatus + StreamState trait | 6h |
| `src/db/media_server.rs` | mark_offline_if_expired + 白名单 CRUD | 4h |
| `src/db/stream_push.rs` / `stream_proxy.rs` / `send_rtp.rs` | status 字段 + StreamState impl | 6h |
| `database/init-*-2.7.4.sql` | 4 张表加 status 列 / 白名单表 | 2h |
| `src/lib.rs` | 启动 health_check_loop + 优化 select_least_loaded | 4h |
| `src/router.rs` | 注册 hook_routes | 2h |
| `src/handlers/server.rs` | list_all_streams 用统一接口 | 3h |
| `tests/integration/zlm_hook_test.rs`（新） | mock ZLM hook 集成测试 | 4h |
| `docs/OPERATIONS.md` | Phase 4 章节 | 1h |

**总计**：~80h ≈ 2 周编码 + 1 周 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）

- **4.1**：`zlm::hook::tests::test_zlm_hook_event_parse_*` — 5-7 个 case 覆盖 14 个事件
- **4.2**：`zlm::auth::tests::test_check_*` — 5 个 case 覆盖 constant-time / IP 匹配
- **4.3**：`zlm::hook::tests::test_parse_port_range_*` + `test_on_server_started_configures_rtp_port_range`（mock ZLM）
- **4.4**：`zlm::media_node::tests::test_is_online_*` — 4 个 case 覆盖 keepalive 逻辑
- **4.5**：`state::stream_status::tests::test_stream_status_*` — 6 个 case
- **4.6**：`lib::tests::test_get_zlm_client_auto_*` — 3 个 case 覆盖过滤 / 选优 / fallback

### 集成测试（`tests/integration/zlm_hook_test.rs`）

- ZLM `on_play` 推送 → secret mismatch → 401 / secret OK → 200
- ZLM `on_server_started` 推送 → 验证 mock ZLM 收到 12+ 个 `setServerConfig` 调用
- ZLM 节点 30s 无 keepalive → 后台 loop 自动 mark offline
- 3 个节点流数 5/3/10 → `get_zlm_client_auto` 选流数 3 的节点

### 端到端（手测，对应设计文档 Acceptance）

- 真实 ZLM 启动 → POST `/api/hook/on_server_started` → 验证 ZLM 配置被全量覆盖
- 真实 ZLM + 设备推流 → 收到 `on_publish` → 验证 secret 鉴权生效
- 杀掉 ZLM 节点 → 30s 后 GBServer 切 offline → 后续 play 请求自动选其他节点
- 三类流（push/proxy/SendRtp）→ `/api/server/listAllStreams` 统一返回

---

## 衔接说明

### 与 Phase 3 衔接

- **3.1 / 3.4** 用 `on_stream_changed` / `on_rtp_server_started` → Phase 4 在此基础上扩 `on_stream_none_reader` / `on_rtp_server_timeout` 处理
- **3.4 DownloadSession 进度** → 仍依赖 `on_stream_changed`（已有逻辑保留）

### 与 Phase 5 衔接

- **5.x 级联** 用 `on_send_rtp_stopped` → Phase 4 在此 hook 加更细致的事件分发（按 send_rtp_id 路由到具体平台）

### 与 Phase 7 衔接

- **7.x Redis StateStore** → Phase 4 的 `select_least_loaded_server_filtered` 是 StateStore API 的一部分；Phase 7 扩展 Redis backend 时同步实现
- **7.x 跨节点 RPC** → `mark_offline_if_expired` 跨节点时改用 RPC（本期单节点即可）

---

## 风险与缓解

### R1: Hook secret 验证影响现有部署 — **MEDIUM**
- 现有 `on_play` / `on_publish` 不验证 secret；启用后可能误拒
- **缓解**：默认 `expected_secret` 为空时 `check` 直接返回 Ok（兼容旧部署）；通过 config `zlm[*].strict_auth = true` 显式启用严格模式

### R2: Keepalive 超时阈值过短导致频繁切离线 — **MEDIUM**
- 默认 30s 在弱网下可能误判
- **缓解**：config `media_server.keepalive_timeout_secs` 可调（10-120s）；新增 `keepalive_grace_count`（连续 N 次丢失才切 offline）

### R3: 流状态统一破坏旧查询 API — **HIGH ⚠️**
- `StreamPush` / `StreamProxy` / `SendRtp` 三表都有 `pushing` / `pulling` / `running` 字段
- 上层 handler（`/api/server/...`）可能依赖旧字段名
- **缓解**：旧字段保留（`pushing` / `pulling` 仍写原值），`status` 字段为新枚举值（与旧字段并行）；旧 API 不变；新 API 走 `StreamState` trait

### R4: WVP-Pro 多路径路由被前端误用 — **LOW**
- 旧路径 `/api/zlm/hook` 已有 ZLM 配置；新增 `/api/hook/...` 需同步配置到 ZLM
- **缓解**：在 `on_server_started` 期间把 12+ 路径都注册到 ZLM hooks 配置（`hook.on_stream_changed` 等值用 `/api/hook/on_stream_changed` 而非旧路径）

### R5: Least-load 选择在 Redis 不可用时降级到顺序选 — **LOW**
- 顺序选可能选到 offline 节点
- **缓解**：offline 过滤在 Redis 之前；Redis 不可用时遍历 `list_online_servers` 取第一个

---

## 完成判定

- **三库 `cargo test --lib` 全绿**（默认 sqlite + `--features postgres` + `--features mysql` 三路 0 失败）
- **新增 ≥ 25 个单测**覆盖 Phase 4 涉及的所有模块（auth / hook / media_node / stream_status / least-load）
- **集成测试 ≥ 4 个**：mock ZLM hook（auth / on_server_started auto-config / keepalive timeout / least-load）
- **真实 ZLM 部署手测通过**：on_server_started 自动配置全部 hook + rtp port range；on_play secret 鉴权生效
- **主流程代码搜索 `127.0.0.1` 在 hook URL 仍命中**：`src/zlm/hook.rs:649` 的 fallback URL（保留向后兼容）
- **CI workflow 包含三库 job**（已在 Phase 3.7 加 sqlite job；本阶段确认 PG/MySQL 仍工作）
- **`docs/OPERATIONS.md` 新增 Phase 4 章节**：操作步骤可复现
