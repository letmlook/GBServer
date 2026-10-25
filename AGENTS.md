# AGENTS.md - GBServer

Agent instructions for working on this GB28181 video platform server (Rust backend + Vue 3 frontend).

> 📌 **代码冻结状态：已解除（2026-09-19）。** 当日曾短暂冻结，同日经用户要求解除以修复
> 用户管理模块的安全与数据一致性缺陷（commit `bfa65f8`）。可正常提交修复。
> 当前未完成事项见 `docs/OPEN_ISSUES.md`，已实现能力与设计决策见 `docs/STATUS.md`。
>
> **权限约定（重要）**：管理员判定统一走 `src/handlers/authz.rs` —— `authority == "0"`
> 为管理员，内置角色 `id = 1` 兜底。**新增任何管理类端点都必须调用
> `authz::require_admin`**；`/api/role/*`、`/api/userApiKey/*`、`/api/user/{users,all}`
> 曾因只挂 JWT 而让普通用户可自助提权。
>
> **测试状态提醒**：`cargo test` 在 2026-09-19 曾因测试构造器缺字段而**整体编译失败**
> （已于同日修复）。改动 `config` 结构体字段后
> 务必同时检查 `src/test_support.rs` 与 `src/lib.rs` 的测试构造器。

## Project Overview

- **Backend**: Rust with Axum 0.7, SQLx (SQLite default / PostgreSQL / MySQL via cargo features), JWT auth
- **Frontend**: Vue 3 + Element Plus + Vite + TypeScript (in `web/` directory; the old Vue 2 app was deleted from the repo — it survives only in git history)
- **Purpose**: GB28181 protocol video management platform with JT1078 vehicle terminal support

## Build Commands

### Backend (Rust)

```bash
# Development — use DEBUG build for any local verify cycle (4-5x faster, ~2-3 min
# vs ~10-12 min for release). Only build --release when producing a release artifact.
cargo build                # → target/debug/gbserver

# Native run on a dev box (NOT inside the gbserver container). The NO_PROXY
# env vars are REQUIRED: reqwest would otherwise route 127.0.0.1:8080 (ZLM)
# through the system HTTP proxy and the health checker would falsely report
# every ZLM node as offline.
NO_PROXY=localhost,127.0.0.1,::1 no_proxy=localhost,127.0.0.1,::1 \
  ./target/debug/gbserver

# Release (only when packaging / tagging / production image)
cargo build --release      # → target/release/gbserver
# 三个驱动同编在这个二进制里；换库 = 改 config/application.toml 的 [database] url
# （或 GBSERVER__DATABASE__URL 覆盖）+ 重启进程。没有「按方言分别编译」这回事。

# Focused tests
cargo test --test sqlite_compat
```

### 方言冒烟（改过 SQL 就必须跑）

`cargo test` 跑的是 **SQLite**，而 sqlx-postgres 对列类型/宽度是**严格**校验的
（mysql / sqlite 宽松）—— 只有 postgres 会因为"INT4 主键解成 i64"、"VARCHAR 列
绑成整数"这类问题整条语句 500。**sqlite 的单测全绿不代表 postgres 能跑。**

```bash
# 1) 起库
docker compose up -d postgres
docker compose --profile mysql up -d mysql

# 2) 一个二进制起三个实例（端口与已有实例错开）—— 只改 database.url，不改编译
cargo build
GBSERVER__DATABASE__URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver' \
  GBSERVER__SERVER__PORT=18081 \
  GBSERVER__SIP__PORT=25060 GBSERVER__SIP__TCP_PORT=25061 \
  GBSERVER__JT1078__TCP_PORT=60001 GBSERVER__JT1078__UDP_PORT=60002 \
  ./target/debug/gbserver &

# 2') 或者直接跑一键脚本（sqlite / postgres / mysql 三实例全跑一遍并对比）
bash scripts/dialect_smoke_all.sh

# 3) 取 token 跑冒烟
TOKEN=$(curl -s 'http://127.0.0.1:18081/api/user/login?username=admin&password=21232f297a57a5a743894a0e4a801fc3' \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["accessToken"])')
python3 scripts/dialect_smoke.py --base http://127.0.0.1:18081/api --token "$TOKEN"
```

**`#[ignore]` 的真实库专项用例**（sqlite 跑不到，必须显式指定库）：

```bash
export TEST_POSTGRES_URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver'
export TEST_MYSQL_URL='mysql://root:rootpw@127.0.0.1:3306/gbserver'
cargo test --locked \
  --test postgres_jt1078 --test postgres_device_lookup --test postgres_platform_channel \
  --test mysql_catalog_channel --test mysql_platform_channel \
  -- --ignored
```

> 不要用 `cargo test -- --ignored` 一把梭 —— 那会把标记为 `ignore` 的 doc-test
> 伪代码示例也拉出来跑（它们本来就跑不通），**必须逐个 test target 指定**。

**预期"失败"（不是缺陷）**：CSV 导出的 `/log/list?format=csv` 返回 CSV 文本而非
JSON；`proxy/start` 指向不存在的 RTSP 源时真实 ZLM 回 404。

详见 `docs/OPEN_ISSUES.md` 的 B16 / B30。

### Frontend (Vue 3)

```bash
cd web

# Install dependencies
npm install

# Development server (:9528, proxies /dev-api to backend :18080)
npm run dev

# Production build (vue-tsc type check + vite build → web/dist).
# Backend serves web/dist on :18080 directly — there is NO separate dev
# server in production. After this build, restart the backend to pick it up.
npm run build

# Lint
npm run lint
```

```bash
# End-to-end UI tests (Playwright; requires backend :18080 + frontend dev :9528)
cd e2e && npx playwright test
```

### Run Both

```powershell
# PowerShell (Windows)
.\scripts\build-and-run.ps1
```

### Database Setup

```bash
# SQLite (default) — nothing to do; schema auto-initializes on first start

# PostgreSQL
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql

# MySQL
mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql
```

## Code Style Guidelines

### Rust Backend

#### Error Handling
- Use `thiserror` crate with `AppError` enum (see `src/error.rs`)
- Implement `IntoResponse` for custom errors
- Return `Result<T, AppError>` from handlers
- Use `AppError::business(ErrorCode, msg)` for business logic errors
- Use `?` operator for database and config errors (automatic conversion)

```rust
// Correct
pub async fn handler(...) -> Result<Json<ApiResult<T>>, AppError> {
    let data = db::query(&state.pool, id).await?;
    Ok(Json(ApiResult::success(data)))
}

// Incorrect - don't use unwrap/expect in handlers
let data = db::query(&state.pool, id).await.unwrap();
```

#### Response Format
- Always wrap responses in `ApiResult<T>` (see `src/response.rs`)
- Use `ApiResult::success(data)` for successful responses
- Use `ApiResult::success_empty()` for operations with no return data
- Use `AppError::into_response()` for errors (automatic JSON conversion)

```rust
// Successful response
Ok(Json(ApiResult::success(some_data)))

// Empty success
Ok(Json(ApiResult::success_empty()))

// Error (handled automatically via ?)
Err(AppError::business(ErrorCode::Error400, "invalid input"))
```

#### Database (SQLx)
- Use `sqlx::query_as` for queries returning rows
- Use `sqlx::query_scalar` for aggregate queries
- Use `sqlx::query` for INSERT/UPDATE/DELETE
- Handle SQLite / PostgreSQL / MySQL dialects by dispatching at RUNTIME on the
  **pool's own type** (`pool.dialect()` / `pool.sql()`) — e.g.
  `match pool.dialect() { Dialect::Postgres => ..., _ => ... }`.
  Do NOT use `#[cfg(feature = "sqlite"/"postgres"/"mysql")]` (all three features are
  always on together — a `compile_error!` guard enforces it), and do NOT use the
  process-global `dialect::current()` to pick a branch: it is the startup-time
  default, so a test that builds `Pool::Postgres(..)` directly would get the wrong
  branch. Only use the global where there is genuinely no pool yet (e.g. choosing
  which mock pool a test should create).
- Use parameterized queries - never interpolate SQL directly
- **⚠️ sqlx-postgres 严格校验列类型，sqlx-mysql / sqlx-sqlite 宽松** —— 见
  「Gotchas」里的「postgres 只在运行期炸」一条。写任何跨方言 SQL 前先读那条。

```rust
// Parameterized query (PostgreSQL)
sqlx::query_as::<_, Device>(
    "SELECT id, device_id, name FROM gb_device WHERE device_id = $1"
)
.bind(device_id)
.fetch_optional(pool)
.await?
```

#### 跨方言类型/宽度：先在 SQL 侧统一，别指望解码端兼容

三种方言的**整数列宽度和列类型本身就不一样**，而 sqlx-postgres 对类型是**严格**
校验的（mysql / sqlite 宽松），所以这类问题**只在 postgres 上炸**：

| 差异 | postgres | mysql / sqlite |
|---|---|---|
| 主键 | `serial` → **INT4** | `bigint auto_increment` / `INTEGER` → 64 位 |
| `gb_device_channel.parent_id` | VARCHAR | 同为 VARCHAR（但代码曾把它绑成 i64） |
| 布尔列 | `bool` | `tinyint(1)` / INTEGER |

已知的两类坑与对应写法：

1. **INT4 列按 `i64` 解** → `decoding column "id": mismatched types; Rust type
   i64 (as SQL type INT8) is not compatible with SQL type INT4`。
   用 `dyn_where::bigint_col("c.id")` 把列渲染成 `c.id::bigint`（postgres 侧），
   其它方言原样返回。
2. **VARCHAR 列绑成整数** → postgres `COALESCE types bigint and character
   varying cannot be matched`。这是**解析期**失败：即使请求根本没传该字段
   （绑的是 NULL，类型照样是 int8），整条语句也会 500。
   `parent_id` 已改回 `Option<String>`（与列类型、前端发的
   `parentId?: string`、代码里既有的 `parent_id = '0'` 查询一致）。

占位符（`?` vs `$1`）与布尔列（`bool` vs `tinyint(1)`）是同一类问题的同一种
处理方式：`src/dyn_where.rs` 里的 `dialect_sql` / `BindValue::Bool` /
`bigint_col`。**新增跨方言 SQL 时沿用这三件套，别自己抄一份。**

3. **`serial` 序列与表内容要对齐。** postgres 的 `serial` 主键对应一个独立序列，
   初始化 SQL 若用**显式 id** 播种子行（admin 用户/角色都是 id=1），序列会停在
   初始值（`is_called=false`），**第一次**插数据就撞主键
   （`duplicate key value violates unique constraint "gb_user_pkey"`）。
   后端每次启动会调 `db::seq_sync::sync_serial_sequences` 把序列推到 `MAX(id)`；
   **新增带显式 id 的种子 INSERT 时不用自己操心，但手工往库里插数据时要注意**
   （插完记得 `setval` 或重启后端）。
   找序列用 `pg_depend` 的自动依赖，**不要**用 `pg_get_serial_sequence(table,'id')`
   —— 它在列不存在时是抛错而不是返回 NULL。

> **`cargo test`（默认 sqlite）永远发现不了这一类问题。** 改完涉及 SQL 的代码，
> 必须对**真实 pg / mysql 实例**跑一遍 `scripts/dialect_smoke.py`（见「Build
> Commands」里的方言冒烟）——它历史上一次性抓到过 23 处只在 pg 上炸的缺陷。
> 详见 `docs/OPEN_ISSUES.md` 的 B16。

#### Naming Conventions
- **Functions**: snake_case (`list_devices_paged`, `get_device_by_device_id`)
- **Types**: PascalCase (`AppError`, `ApiResult`, `Device`)
- **Modules**: snake_case (`db`, `handlers`, `sip`)
- **Variables**: snake_case
- **Constants**: SCREAMING_SNAKE_CASE

#### Imports
Group imports by crate:

```rust
use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;

use crate::db::{self, Device};
use crate::error::{AppError, ErrorCode};
use crate::response::ApiResult;
```

#### Handler Pattern
Always extract state and validate input:

```rust
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<ApiResult<Response>>, AppError> {
    let value = params.value.ok_or_else(|| 
        AppError::business(ErrorCode::Error400, "缺少参数")
    )?;
    // ... implementation
}
```

#### Configuration
- Use `config` crate with TOML (`config/application.toml`) + environment variables
- Environment variables use `GBSERVER__SECTION__KEY` format
- Load config in `main.rs` and pass to `run()`

#### Logging
- Use `tracing` crate
- Set level via `RUST_LOG` environment variable
- Default: `info,gbserver=debug`

```rust
tracing::info!("Starting server on port {}", port);
tracing::debug!("Query result: {:?}", result);
```

### Frontend (Vue 3)

- Vite 5 + TypeScript, vue-tsc for type checking (`npm run build` runs it)
- Element Plus with on-demand auto-import (unplugin-vue-components)
- Pinia stores in `web/src/store/modules/`, typed API modules in `web/src/api/`
- axios wrapper `web/src/utils/request.ts` adds the `access-token` header and treats `code !== 0` as failure
- Follow existing patterns in `web/src/`

## Project Structure

```
GBServer/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # AppState, run() function, background task wiring
│   ├── config.rs            # Configuration loading
│   ├── error.rs             # AppError, ErrorCode
│   ├── response.rs          # ApiResult
│   ├── auth.rs              # JWT authentication
│   ├── router.rs            # Route definitions (425 处 .route(，429 条唯一 /api/ 路径)
│   ├── db/                  # Database layer
│   ├── handlers/            # HTTP handlers (stub.rs / device_stub.rs 是真实实现，见 docs/STUB_COMPAT_PLAN.md)
│   ├── sip/                 # GB28181 SIP stack (core/ transport/ gb28181/)
│   ├── zlm/                 # ZLM media server client + hooks
│   ├── jt1078/              # JT1078 vehicle terminal protocol
│   ├── cascade/             # Upstream platform registration
│   ├── scheduler/           # Record plan scheduling
│   ├── ws/ + cluster/ + rpc/ + state_store.rs  # Cluster/WS/state infrastructure
│   └── middleware/          # Audit logging
├── web/                     # Vue 3 frontend (active)
├── e2e/                     # Playwright UI tests
├── mock/                    # Python simulators (SIP device / JT1078 terminal / cascade)
├── docs/                    # 当前状态 / 待办 / 部署指南 / 方言注意事项 / stub 模块定位评估
├── config/
│   └── application.toml     # Default configuration
├── database/
│   ├── init-sqlite-2.7.4.sql
│   ├── init-postgresql-2.7.4.sql
│   └── init-mysql-2.7.4.sql
└── Cargo.toml
```

## Common Patterns

### Adding a New Handler

1. Create function in appropriate `handlers/*.rs` file
2. Add route in `router.rs`
3. Return `Result<Json<ApiResult<T>>, AppError>`
4. Use `State(state): State<AppState>` to access app state
5. Use `Query(params): Query<Params>` for query parameters

### Adding a New Database Function

1. Add function in appropriate `db/*.rs` file
2. Return `Result<T, sqlx::Error>` or `sqlx::Result<T>`
3. Handle both MySQL and PostgreSQL syntax differences
4. Use `sqlx::FromRow` derive for struct row mapping

### Running the Application

```bash
# SQLite default: just run the backend (schema auto-initializes)
cargo run

# Or with PostgreSQL/Redis/ZLM via Docker Compose
# First set GBSERVER_SIP_ADVERTISE_IP in .env to the host IPv4 reachable by devices.
# SIP binds 0.0.0.0 in the container but advertises this host IP in Via/Contact/SDP.
#
# ZLM 默认用 **host 网络**（Linux 服务器：收流端口池 / WebRTC 候选 / RTSP-RTMP
# 都直接落在宿主，不需要逐口映射）：
docker compose up -d
#
# Docker Desktop（macOS / Windows）**不支持 host 网络**，本机开发必须叠加
# 桥接文件（把 ZLM 切回桥接 + 端口映射，并给后端下发 rtc_extern_ip）：
docker compose -f docker-compose.yml -f docker-compose.mac.yml up -d

# Import PostgreSQL schema (if not auto-initialized)
docker exec -i gbserver-postgres psql -U postgres -d gbserver < database/init-postgresql-2.7.4.sql

# In another terminal, run frontend dev server
cd web && npm run dev
```

## Local Dev Environment (this repo's box, Linux)

The dev machine runs behind a corporate HTTP proxy and has IPv6 broken.
These are NOT app bugs — they are environmental facts future agents must
work around, not try to fix.

- **HTTP/HTTPS proxy**: `192.168.3.88:7892` (HTTP). Set in `~/.bashrc` as
  `http_proxy` / `https_proxy` / `all_proxy`.
- **IPv6 is unreliable** on this host. Many public DNS records return
  only AAAA, and `dns/docker` fail. Workarounds used here:
  - `dnf config-manager addrepo --from-repofile=...` for Docker repo (the
    official Docker repo doesn't have a Fedora 44 path yet — point
    `$releasever` at 43 instead by editing the repo file)
  - `dnf.conf` already has `ip_resolve=4` (added during this session)
  - `curl --noproxy '*'` or `curl -4` to bypass the proxy for loopback
    tests; the system proxy otherwise intercepts 127.0.0.1 calls and
    returns 502 Bad Gateway
- **Docker daemon HTTP proxy**: configured via
  `/etc/systemd/system/docker.service.d/http-proxy.conf` so
  `docker pull` reaches Docker Hub (otherwise IPv6-only DNS fails).
  Restart docker after editing: `sudo systemctl daemon-reload && sudo systemctl restart docker`.
- **`docker compose` plugin** is in `docker-compose-plugin` rpm and lives
  at `/usr/libexec/docker/cli-plugins/docker-compose`. Legacy
  `docker-compose` binary is NOT installed; only the v2 plugin works.
- **host.docker.internal is NOT resolvable** on this Linux box (that's
  a Docker Desktop-ism). For native backend runs, set
  `GBSERVER_ZLM_PRIMARY_HOOK_URL=http://127.0.0.1:18080/api/zlm/hook`
  — do NOT commit `host.docker.internal` into the repo.

## Dashboard Layout (after recent redesign)

`web/src/views/dashboard/index.vue` is a 4 + 3 grid of real-time panels:

Row 1 (charts, `gb-grid--4col`):
- CPU 使用率, 内存使用率, 网络流量 (Mbps), 系统负载 (1/5/15min)

Row 2 (resource & config, `gb-grid--3col`):
- 磁盘使用率 — horizontal bars, one row per mount from `disk_all[]`
- 服务健康 — 6 icon rows (ZLM / Postgres / Redis / GB28181 / JT1078 / 录制计划)
- 协议接入 — top "本机 IP" banner (`host_ip`) + GB28181 + JT1078 config dl

Auto-refreshes every 2s via `setInterval(loadAll, 2000)`; cleared on
unmount. `loadAll` uses `Promise.allSettled` and feeds an `apiOk` ref
that drives the health panel's green/yellow/red pills.

`/api/server/system/info` (`src/handlers/server.rs`) is the single
endpoint backing all panels. It returns ring buffers (cpu[]/mem[]/net[]
/load[]/disk_history[]), scalar percentages, disk_all[] (unfiltered
`df` output), sip_config (plain-text password — admins need it to
configure devices), jt1078_config, and host_ip (cached 5 min via
UDP-socket trick). Adding new dashboard panels almost always means
extending this one response.

## Gotchas for Common Edits

- **新增 `<el-table>` 必须写 `table-layout="auto"`**。Element Plus 默认
  `table-layout: fixed`，会把每列压到声明的 `min-width` 附近，于是国标 ID、
  `IPv4:端口`、时间戳这类**不可断行**的内容被折成两行
  （「3402000000131000 / 00001」）。**不要改用在 CSS 里把 `<table>` 设成
  auto**：fixed 模式下 EP 把表头渲染成**另一张独立的 `<table>`**，两张表各自
  按内容算列宽，表头与表体立刻错位；只有走 EP 自己的 auto 模式（表头渲染进同一张
  表）列宽才对齐，`fixed="right"` 的固定列也才会走 EP 的 sticky + 投影逻辑。
  另外长文本列（拼接串 / JSON / URL）要配 `show-overflow-tooltip`：列宽是内容
  驱动的，不封顶的列会把整张表撑到几千像素宽。样式细则与原因见
  `web/src/styles/_element-overrides.scss` 的「表格」段。
- **要让用户选「时刻」就用 `web/src/components/GbTimeSelect/index.vue`，别用
  `el-time-select`**。后者的 `end="24:00"` **选不出 24:00**：它内部用
  `dayjs(current, 'HH:mm').format('HH:mm')` 生成选项，`24:00` 会被归一成次日
  `00:00`，于是列表末尾多出一个**值为 `00:00` 的重复项** —— 用户以为选的是
  「当天结束」，实际拿到的是「当天开始」（录像计划里写 `start='00:00',
  stop='24:00'` 的时段就是这样被悄悄改掉的）。`GbTimeSelect` 自己生成 15 分钟
  档选项，末档是字面量 `24:00`，且只有「结束时间」提供它。
  「日期范围 + 起止时刻」的查询条件用 `web/src/components/GbDateTimeRange`。
- **延迟只有一个来源：国标设备列表的「延迟」列**（`web/src/api/device.ts`
  `queryDeviceLatency`）。它是**平台 ↔ 设备**的 SIP 往返：平台按
  `sip.heartbeat.latency_probe_interval_secs`（默认 15s，0 = 关闭）
  向每台在线设备发一条 MESSAGE 探针，用设备回的 200 OK 结算 RTT。
  采样见 `src/sip/gb28181/latency.rs` + `src/sip/server.rs` 的「设备延迟探针」
  循环，响应结算挂在 `handle_response` 里（用 `lat_` Call-ID 前缀认领）。
  数据只在内存里，不落库。
  - 顶部导航栏那个「延迟」**已删除**（曾显示浏览器↔后端的 HTTP 往返，
    而 `Navbar.vue` 用的 `/api/server/system/info` 服务端有固定约 260ms
    采样 sleep —— CPU 60ms + 网络 2×100ms，见 `src/handlers/server.rs`
    的 `read_cpu_usage_impl` / `read_net_rx_impl` —— 显示出来不是网络延迟）。
    那个请求现在只用来喂「平台信息」弹层，别再把它当成延迟探针。
  - 副作用提示：dashboard 每 2s 打一次同一个 `system/info`，每次都要等那
    ~260ms 的 sleep，接口本身的响应时间被自己抬高了。
- **`config/application.toml` is deployment-specific.** The `[[zlm.servers]]`
  `hook_url` and similar IPs/ports are tuned for one box. Local-dev
  edits to this file (e.g. `host.docker.internal` → `127.0.0.1`) are
  real fixes for THIS box but break docker-compose deployments. Either
  revert before commit, or split into a separate commit with clear
  message.
  - **`[[zlm.servers]]` 的 `ip` 与 `rtc_extern_ip` 写死了一台开发机的 LAN IP。**
    换机器、或那台机器 IP 变了，就必须用**标量**环境变量覆盖（不要用
    `GBSERVER__ZLM__SERVERS__0__*` —— 那会把节点数组解析成 map，启动时报
    `invalid type: map, expected a sequence`）：
    ```bash
    export GBSERVER_ZLM_PRIMARY_IP=127.0.0.1            # 后端 → ZLM HTTP
    export GBSERVER_ZLM_PRIMARY_RTC_EXTERN_IP=<本机LAN IP>  # 浏览器 → ZLM RTC
    ```
    用旧值时的症状**很像 WebRTC 实现坏了**：浏览器拿到的 ICE 候选不可达，
    `iceConnectionState` 永远停在 `checking`、一个字节都收不到。
    排查顺序：先看后端日志里健康检查是否把节点判成 Online，再看
    `getServerConfig` 的 `rtc.externIP` 是不是当前 IP，最后才怀疑实现。
    同样的坑见 `e2e/README.md`。
- **Dashboard chunk size**: `web/dist/static/index-*.js` for the
  dashboard view is currently ~27 kB. If a single PR adds much more,
  consider lazy-loading the view (`defineAsyncComponent`).
- **Vue 3 icons are resolved through `vite-plugin-svg-icons` sprite** at
  `src/icons/svg/`. If you add a new menu icon, drop the .svg there AND
  set `meta.icon` in `web/src/router/index.ts` to the file's basename.
  Element Plus icon names (`video-camera`, `cpu`, etc.) are NOT
  resolved by `<svg-icon>` and will silently render blank.
- **Disk usage on Fedora reports 0** if `read_all_disk_usage()` doesn't
  recognize `/dev/mapper/*`. The fix (already merged) adds `mapper`
  to the recognized device prefixes AND special-cases mapper in
  `disk_family()` to skip VG-folding. Don't undo this when refactoring.
- **SIP password shows plain on dashboard** — this is intentional,
  admins need to copy it into devices / 下级平台. The endpoint is
  behind JWT (`access-token` header) so it's not public.
- **ZLM mirror for dashboard tile layer**: `https://webrd0N.is.autonavi.com/appmaptile?...`
  is the chosen tile source (no API key, China-reachable, GCJ02). Don't
  swap to `tile.openstreetmap.org` (blocked) or `tianditu` (needs token).

## Important Notes

- Default admin credentials: `admin` / `admin` (MD5: `21232f297a57a5a743894a0e4a801fc3`)
- JWT secret must be changed in production (`config/application.toml`)
- API uses `access-token` header for authentication
- Response format: `{ "code": 0, "msg": "成功", "data": ... }`

## Documentation to Read Before Sensitive Edits

- **`CLAUDE.md`** — build commands, architecture overview, cross-cutting
  conventions (sibling of this file, slightly different focus).
- **`docs/`** — 当前状态（`STATUS.md`）、未完成事项（`OPEN_ISSUES.md`）、部署指南
  （`DEPLOYMENT_GUIDE.md`）、多方言 SQL 注意事项（`DB_DIALECT_NOTES.md`）。
  敏感改动前值得先扫一遍。
- **`src/handlers/server.rs::system_info`** — the response shape for
  `/api/server/system/info` is the contract for the entire dashboard.
  Any new field added here almost always needs a matching change in
  `web/src/api/log.ts`'s `SystemInfo` interface AND a panel in
  `web/src/views/dashboard/index.vue`.
- **`src/zlm/health_checker.rs`** — load ZLM nodes from config into DB,
  push hook config, write `media_server.status` / `last_keepalive_time`.
  Edits here affect every health check loop iteration.
- **`src/db/media_server.rs::read_all_disk_usage`** — pre-existing filter
  logic that silently affected Fedora/LVM. If you refactor disk detection,
  preserve both `/dev/sd*|nvme*|vd*|xvd*|hd*` AND `/dev/mapper/*`,
  and the special case in `disk_family()` that prevents VG-folding.
