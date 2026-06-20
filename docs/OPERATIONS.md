# GBServer Operations Manual

> Last updated: 2026-06-11 — for `gbserver` v0.1.0

This document covers installation, upgrade, runtime, monitoring, and disaster
recovery for the Rust GB28181 server that powers the GB28181 management
surface used by the Vue 2 frontend.

## 1. Quick Start

### 1.1 Prerequisites

| Component | Required | Recommended |
|-----------|----------|-------------|
| OS        | Linux x86_64 / macOS arm64 | Linux x86_64 |
| Rust      | 1.75+ stable | 1.78+ |
| PostgreSQL | 13+ (or MySQL 8.0+) | PostgreSQL 16 |
| Redis     | 6+ (optional, recommended for multi-node) | Redis 7 |
| ZLMediaKit | 2024-01-01 build or newer | Latest release |
| RAM       | 2 GB | 4 GB+ for 500+ devices |

### 1.2 First-time install

```bash
# 1. clone
git clone <your fork>/GBServer.git && cd GBServer

# 2. DB schema (PostgreSQL)
psql -U postgres -d gbserver < database/init-postgresql-2.7.4.sql
# (or MySQL: mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql)

# 3. backend
cp config/application.toml config/application.local.toml   # override secrets
export GBSERVER__JWT__SECRET=$(openssl rand -hex 32)
export GBSERVER__DATABASE__URL=postgres://postgres:postgres@127.0.0.1:5432/gbserver
export GBSERVER__SIP__PASSWORD=$(openssl rand -hex 16)
cargo run --release

# 4. frontend (in a separate shell)
cd web && npm install && npm run dev
```

Backend listens on `:18080`, frontend dev server on `:9528`.
Default admin login: `admin / admin` (rotate on first deploy).

### 1.3 Docker (compose) deploy

```bash
docker compose up -d               # postgres + redis + zlm + backend + frontend
docker compose logs -f gbserver     # tail logs
docker compose exec gbserver bash   # exec into container
```

## 2. Configuration Reference

All settings are loaded from `config/application.toml` plus overrides via
environment variables (`GBSERVER__SECTION__KEY=value`).

### 2.1 Critical settings

| Path | Required | Default | Notes |
|------|----------|---------|-------|
| `jwt.secret` | yes | (none) | ≥ 32 chars; **must** be overridden in production |
| `database.url` | yes | `postgres://...` | sqlx URL |
| `sip.password` | yes | `admin123` | GB28181 SIP digest password |
| `sip.device_id` | yes | `34020000002000000001` | 20-digit local GB-ID |
| `zlm[].secret` | yes | `035c73f7-...` | ZLM HTTP API secret |
| `redis.url` | no | (none) | `redis://host:6379/0` enables multi-node |

### 2.2 SIP / GB28181

```yaml
sip:
  enabled: true
  ip: 0.0.0.0
  port: 5060                 # UDP
  tcp_port: 5061
  device_id: "34020000002000000001"
  password: "admin123"        # via GBSERVER__SIP__PASSWORD
  realm: "3402000000"
  keepalive_timeout: 30       # seconds
  register_timeout: 3600
  charset: "UTF-8"
```

### 2.3 ZLMediaKit

```yaml
zlm:
  servers:
    - id: zlm-a
      ip: 127.0.0.1
      http_port: 8080
      https_port: 8443
      secret: "035c73f7-bb6b-4889-a715-d9eb2d1925cc"
  stream_timeout: 30
  hook_enabled: true
  hook_url: "http://127.0.0.1:18080/api/zlm/hook"
```

ZLM must be configured to POST hooks to `/api/zlm/hook`. Configure via
`config.ini`: `[hook] enable=1, root_url=http://gbserver:18080`.

### 2.4 Multi-node ZLM (load balancing)

Set `redis.url` and the server selects the least-loaded ZLM via the
`media_server_select_least_loaded` Redis ZSET. Each `play_start` /
`playback_start` / `send_play_invite` request is routed to the chosen node.

## 3. Platform Cascade (级联)

### 3.1 Upstream registration

For each row in `gb_platform` with `enable=true`, the `CascadeRegistrar`
sends `REGISTER` on startup and maintains keepalive every 60 s. On `401`,
it retries with digest authentication; on 3+ keepalive misses it
transitions to `Offline` and retries every 30 s.

### 3.2 Upstream Catalog / Info / Status queries

`SipServer::handle_message` detects incoming `MESSAGE` from a registered
platform (looked up by GB-ID in `gb_platform`) and routes Catalog,
DeviceInfo, DeviceStatus queries to handlers that respond with our full
local catalog (all devices and channels).

## 4. Upgrade Procedure

### 4.1 Migrating from prior Java implementation

1. Stop the old Java service: `systemctl stop gbserver`
2. Back up DB: `pg_dump gbserver > backup_$(date +%F).sql`
3. Pull new Rust build: `git pull && cargo build --release`
4. Start new binary: `systemctl start gbserver`
5. Verify: `curl http://localhost:18080/api/server/version`
6. If migration needed, see `docs/MIGRATION.md` (not yet written)

### 4.2 In-place Rust upgrade

```bash
# build new binary alongside old
cargo build --release
mv target/release/gbserver target/release/gbserver.new

# atomic swap on next restart
systemctl stop gbserver
mv target/release/gbserver target/release/gbserver.old
mv target/release/gbserver.new target/release/gbserver
systemctl start gbserver

# rollback if needed
systemctl stop gbserver
mv target/release/gbserver.old target/release/gbserver
systemctl start gbserver
```

DB schema migrations are idempotent — the server auto-runs missing tables
on startup (see `init_db_tables` in `src/lib.rs`).

## 5. JT1078 (车载部标)

The `Jt1078Manager` (`src/jt1078/manager.rs`) handles 9101 (实时音视频) and
9102 (历史音视频) media ports for JT/T 1078 terminals. HTTP routes in
`/api/jt1078/*` provide GB28181-compatible control surface (live pause,
record start/stop, region CRUD, route management). Full JT/T 808/1078
protocol state machine lives in `src/jt1078/` — the HTTP layer is a thin
adapter.

## 6. Monitoring

### 6.1 Health endpoints

- `GET /api/health` — JSON status of db/sip/zlm/redis components, returns 503 if any down
- `GET /metrics` — Prometheus text format
- `GET /api/server/config` — sanitized runtime config (passwords masked)

### 6.2 Logs

Default `RUST_LOG=info,gbserver=debug`. Use `tracing-subscriber`
JSON formatter for production log aggregation:

```bash
RUST_LOG=info,gbserver=debug cargo run --release
```

### 6.3 Key metrics to alert on

| Metric | Source | Threshold |
|--------|--------|-----------|
| `gbserver_db_pool_acquired` | sqlx pool | > 80% saturated |
| `gbserver_sip_keepalive_late_seconds` | SipServer | > 90s |
| `gbserver_zlm_stream_count` | ZLM hook | per-server trend |
| `gbserver_request_duration_seconds` | axum middleware | p99 > 2s |

## 7. Disaster Recovery

### 7.1 DB corruption

```bash
# stop backend first
systemctl stop gbserver
# restore from backup
pg_restore --clean --dbname=gbserver /var/backups/gbserver/gbserver_2026-06-10.sql
# restart
systemctl start gbserver
```

### 7.2 Redis loss

Redis is cache layer only — backend falls back to `InMemoryBackend` (per
node) automatically when Redis is unreachable. State will diverge across
nodes until Redis recovers; this is acceptable for invite/stream state.
For multi-node cascade coordination, ensure Redis is replicated
(Sentinel or Cluster).

### 7.3 ZLM crash

Backend detects ZLM down via missing `on_server_started` hook and marks
`gb_media_server.online=false`. Playback requests during outage return
502. Restart ZLM and the hook re-registers; refresh ZLM config:

```bash
curl -X POST http://zlm:8080/index/api/setServerConfig \
  -d 'secret=YOUR_SECRET&hook.enable=1&hook.root_url=http://gbserver:18080'
```

### 7.4 SIP server down

Backend tries to re-bind on next start; check firewall / port 5060:

```bash
ss -ulnp | grep 5060
journalctl -u gbserver --since "10 min ago" | grep -i 'sip'
```

## 8. Testing

### 8.1 Local

```bash
cargo test --lib                          # 151 unit tests
cargo test --test device_simulator_test   # 8 SIP message format tests
```

### 8.2 CI

GitHub Actions runs both DB backends on every push; see
`.github/workflows/backend-ci.yml`.

### 8.3 Parity audit

```bash
# Regenerate interface coverage report (requires reference upstream clone at /tmp/reference-java-impl)
node scripts/parity-audit/extract-interface-coverage.js
# Result lives at docs/parity/interface-coverage-phase-0.md
```

## 9. Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `JWT secret validation failed` on boot | Default / weak secret | `export GBSERVER__JWT__SECRET=$(openssl rand -hex 32)` |
| Devices show offline | SIP UDP 5060 blocked | `ufw allow 5060/udp` and check `sip.ip` config |
| Play URL returns 502 | ZLM unreachable | Check `zlm[*].ip` config + network ACLs |
| Cloud records don't appear | ZLM `on_record_mp4` hook not POSTing | Verify `hook_url` matches `/api/zlm/hook` |
| 105 Missing routes in parity audit | Upstream reference implementation changed | Re-run parity audit script, file issue |

---

## Phase 3 — 真实视频/录像闭环

> 基于 `2026-05-30-wvp-java-parity-design.md` §7 Phase 3。本阶段把 Live / Playback / RecordInfo / Download / Talk-Broadcast 五类视频流从"骨架"升级到"生产闭环"。

### 1. Live Play 真实化

**API**：`POST /api/play/start/{device}/{channel}` 现在等 SIP 200 OK **且** ZLM 媒体到达（`on_stream_changed` 触发 `MediaWaiterManager::resolve_by_stream`），超时清理 RTP server + 发 BYE。

- `src/handlers/play.rs::play_start` 改用 `send_play_invite_and_wait_media`（15s 媒体等待）
- `src/zlm/hook.rs::sync_stream_changed` 在 `data.app == "rtp" && data.register` 时调 `sip.media_waiter_manager().resolve_by_stream`
- `play_stop` 移除 talk BYE fallback；只走 live session 的 BYE

**Subject 命名规范**（与 WVP Java 一致）：

| 用途 | SSRC 前缀 | 类型 | Manager |
|---|---|---|---|
| Live Play | 0 | Play | `InviteSessionManager` |
| Playback | 1 | Playback | `PlaybackInviteSession` |
| Download | 2 | Download | `InviteSessionManager` + `DownloadManager` |
| Talk | 3 | Audio | `TalkManager` |
| Broadcast | 4 | Audio | `BroadcastManager` |

### 2. Playback 真实化

**API**：`POST /api/playback/start/{device}/{channel}` 不再回退到 `rtsp://127.0.0.1/live/...` 占位。

- 先开 ZLM RTP server（端口自动分配）
- 调 `send_playback_invite_and_wait`（15s 媒体等待）
- pause/resume 改用 `send_playback_control(Pause/Resume)`，不再发裸 XML

### 3. RecordInfo 多包等待

**API**：`GET /api/playback/{device}/{channel}/record?page=N&count=M`

- 真正等 SIP 多包 RecordInfo 响应（`PendingRequestManager::register_record_info_multi_packet` + `push_record_info_packet`，SumNum 自终结）
- 分页：page 从 1 开始，count 1..200（默认 20）
- 落库复用 `gb_cloud_record`（三态 cfg，禁止新建 `src/db/record.rs`）

### 4. Download 真实化

**API**：`POST /api/playback/{device}/{channel}/download/start` + `GET /download/progress/{streamId}`

- `DownloadSession` 新增 `zlm_stream_id` / `zlm_app` / `current_bytes` / `total_bytes`
- ZLM `on_stream_changed` 检测 `stream.starts_with("download_")` 时调 `download_manager.update_progress_percent` 切到 downloading
- `update_progress` 改用绝对字节数（current_bytes / total_bytes * 100.0）
- download BYE 真清理：关 RTP server + 移除 download session

### 5. Talk / Broadcast 分流

**API**：`POST /api/broadcast/start/{device}/{channel}` + `POST /api/broadcast/stop/{device}/{channel}`

- 新增 `src/sip/gb28181/broadcast.rs`（独立 `BroadcastManager`，Subject SSRC 前缀 4）
- `broadcast_start` 改用 `send_broadcast_invite`（端口 `0` 让 ZLM 自动分配，避免与 talk 9100/9101 冲突 — 缓解 R4）
- `broadcast_stop` 改用 `send_broadcast_bye`，与 talk BYE 互不影响
- `TalkManager` 不动

### 6. 验证清单

```bash
# 默认（sqlite）跑 lib 测试
cargo test --lib

# PostgreSQL
cargo test --no-default-features --features postgres --lib

# MySQL
cargo test --no-default-features --features mysql --lib

# 关键单测
cargo test --lib handlers::playback::download_manager_tests
cargo test --lib sip::gb28181::broadcast
cargo test --lib sip::gb28181::pending_request::tests::test_register_record_info_multi_packet
```

### 7. 已知风险（详见 plan §"风险与缓解"）

- **R1** 媒体到达超时（默认 15s，可由 `play_start` query 参数 `mediaTimeoutMs` 覆盖）
- **R4** talk / broadcast 端口冲突（已用 ZLM 自动分配缓解）
- **R5** `on_stream_changed` 误命中其它 stream（仅 `data.stream == expected_stream_id` 时 resolve）

### 8. 主流程代码搜索占位 URL 应为 0 命中

```bash
# 排除测试 fixture 后应该为 0
grep -rn "rtsp://127.0.0.1/live" src/ | grep -v tests/ | wc -l
```

---

## Phase 5 — 平台级联（Platform Cascade）生产闭环

> 基于 `2026-05-30-wvp-java-parity-design.md` §7 Phase 5。本阶段把 GBServer 升级为可作为 WVP-Pro Java 或标准 GB 上级平台的"下级平台"。

### 1. 范围

| 子任务 | 状态 | 关键文件 |
|---|---|---|
| **5.1** CascadeRegistrar 串联 | ✅ | `src/cascade/register.rs`、`src/sip/gb28181/cascade_service.rs` |
| **5.2** 上级 RecordInfo 查询响应 | ✅ | `src/sip/server.rs::handle_record_info_for_platform`、`build_upstream_record_info_response` |
| **5.3** 上级 INVITE → SendRtp 整链路 | ✅ | `src/sip/server.rs::register_cascade_invite`、`parse_cascade_invite_sdp` |
| **5.4** `on_send_rtp_stopped` 路由 | ✅ | `src/sip/gb28181/cascade_forward.rs::close_by_stream`、`src/zlm/hook.rs` |
| **5.5a** MobilePosition 上行转发 | ✅ | `src/sip/server.rs::forward_mobile_position_to_all` |
| **5.5b** Alarm 上行转发 | ✅ | `src/sip/server.rs::forward_alarm_to_all` |
| **5.6** 横切 + 三库 + 文档 | ✅ | `scripts/phase5-test-matrix.sh` |

### 2. 关键 API

#### 2.1 预登记级联 SendRtp 会话

```rust
// 5.3: 解析 SDP → 预登记 session
let cascade_call_id = sip_server.register_cascade_invite(
    platform_id,      // 上级 GB ID
    channel_id,       // 共享通道 ID
    sdp,              // 上级 INVITE SDP body
)?;
```

`register_cascade_invite` 内部：
1. 解析 SDP 提取 `(upstream_host, upstream_port, upstream_ssrc)`
2. 调 `send_rtp_manager.handle_upstream_invite` 预登记
3. 返回 `cascade_call_id`

后续设备 INVITE 200 OK 触发 `send_rtp_manager.get_by_channel(channel_id)` 循环，自动 `zlm.start_send_rtp(...)` 推向上级。

#### 2.2 上级 / MobilePosition / Alarm 上行转发

```rust
// 5.5a: 本级设备位置上报 → 广播所有 Active 级联平台
let count = sip_server.forward_mobile_position_to_all(
    device_id, latitude, longitude, speed, direction, time,
).await?;

// 5.5b: 本级设备告警 → 广播所有 Active 级联平台
let count = sip_server.forward_alarm_to_all(
    device_id, alarm_priority, alarm_method, alarm_time, description,
).await?;
```

查询 `db_platform::get_all_online_platforms`（`status=1 AND enable=1`），对每个平台调 `send_platform_message`。

#### 2.3 ZLM SendRtp 异常断开清理

```rust
// 5.4: ZLM 推 on_send_rtp_stopped → close_by_stream
send_rtp_manager.close_by_stream(&data.stream)  // 精确 / 前缀匹配
```

匹配规则：
- 精确等于 `cascade_call_id`
- 前缀匹配（容忍 ZLM 追加的 `.ts` / `.h264` 后缀）
- 关闭后同步从 StateStore 删除

### 3. 验收

#### 3.1 三库测试矩阵

```bash
bash scripts/phase5-test-matrix.sh
# 预期：sqlite 268 passed / postgres 261 passed / mysql 261 passed
```

#### 3.2 Phase 5 关键单测汇总（19 个新增）

```bash
cargo test --lib phase5_   # 19 个 phase5_ 前缀单测
```

| 模块 | 测试名 | 数量 |
|---|---|---|
| `cascade::register::c3_tests` | `phase5_build_digest_response_*` | 3 |
| `sip::gb28181::cascade_forward::tests` | `phase5_close_by_stream_*` | 4 |
| `sip::server::upstream_message_tests` | `phase5_parse_cascade_invite_sdp_*` | 6 |
| `sip::server::upstream_message_tests` | `phase5_register_cascade_invite_*` | 2 |
| `sip::server::upstream_message_tests` | `phase5_forward_mobile_position_*` | 2 |
| `sip::server::upstream_message_tests` | `phase5_forward_alarm_*` | 2 |
| `sip::server::upstream_message_tests` | `phase5_build_upstream_record_info_response_*` | 3 |

#### 3.3 CascadeService 已 deprecated

```bash
# 生产路径不再使用（grep 验证）
grep -rn "CascadeService::" src/ --include="*.rs" | grep -v "cascade_service.rs\|tests"
# 预期：无结果
```

`#[deprecated(since = "0.5.0")]` 警告会引导后续阶段删除。

### 4. 真实部署手测（与设计文档 Acceptance 对应）

| 步骤 | 预期 | 备注 |
|---|---|---|
| 真实 WVP-Pro Java 启动 | 配置本级为下级平台 | 需 `gb_platform.enable=true` |
| WVP-Pro 注册本级 | 收到 200 OK | 401 鉴权 → digest 重试 → 200 |
| WVP-Pro 查询目录 | 收到本级设备列表 | Catalog / Info / Status / **RecordInfo**（待 5.2） |
| WVP-Pro 点播本级通道 | 拉流成功 | `register_cascade_invite` + 设备 INVITE → ZLM SendRtp |
| WVP-Pro 停止 | 本级 SendRtp 关闭 | `on_send_rtp_stopped` → `close_by_stream` |
| WVP-Pro 订阅告警/位置 | 收到上报 | `forward_*_to_all` 路径 |

### 5. 风险与衔接

- **R1** 上级 INVITE 4 模块串通 — 已通过 `register_cascade_invite` 入口收敛，单元测试覆盖 SDP 解析 6 个 case
- **R2** CascadeService deprecated — 与 CascadeRegistrar 并存，`#[allow(deprecated)]` 抑制警告
- **R3** `close_by_stream` 误关非 cascade 流 — 通过 `SendRtpManager` 内部查找，作用域隔离

### 衔接

- **Phase 3.1** Live 媒体等待 → 5.3 上级 INVITE 复用同一条 ZLM SendRtp 路径
- **Phase 3.3** RecordInfo 多包等待 → 5.2 上级 RecordInfo 直接复用（待实施）
- **Phase 4.5** StreamStatus 统一接口 → 5.4 `close_by_stream` 复用 Stopped 状态
- **Phase 7** Redis StateStore → 5.4 已通过 `store.remove_cascade_sendrtp` 同步（E1 已实现）
