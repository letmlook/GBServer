# AGENTS.md - GBServer

Agent instructions for working on this GB28181 video platform server (Rust backend + Vue 3 frontend).

## Project Overview

- **Backend**: Rust with Axum 0.7, SQLx (SQLite default / PostgreSQL / MySQL via cargo features), JWT auth
- **Frontend**: Vue 3 + Element Plus + Vite + TypeScript (in `web/` directory; archived Vue 2 app in `web-legacy-vue2/`, reference only)
- **Purpose**: GB28181 protocol video management platform (WVP-PRO compatible) with JT1078 vehicle terminal support

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

# With MySQL instead of SQLite
cargo build --no-default-features --features mysql

# With PostgreSQL instead of SQLite
cargo build --no-default-features --features postgres

# Focused tests
cargo test --test sqlite_compat
```

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
pub async fn handler(...) -> Result<Json<WVPResult<T>>, AppError> {
    let data = db::query(&state.pool, id).await?;
    Ok(Json(WVPResult::success(data)))
}

// Incorrect - don't use unwrap/expect in handlers
let data = db::query(&state.pool, id).await.unwrap();
```

#### Response Format
- Always wrap responses in `WVPResult<T>` (see `src/response.rs`)
- Use `WVPResult::success(data)` for successful responses
- Use `WVPResult::success_empty()` for operations with no return data
- Use `AppError::into_response()` for errors (automatic JSON conversion)

```rust
// Successful response
Ok(Json(WVPResult::success(some_data)))

// Empty success
Ok(Json(WVPResult::success_empty()))

// Error (handled automatically via ?)
Err(AppError::business(ErrorCode::Error400, "invalid input"))
```

#### Database (SQLx)
- Use `sqlx::query_as` for queries returning rows
- Use `sqlx::query_scalar` for aggregate queries
- Use `sqlx::query` for INSERT/UPDATE/DELETE
- Handle SQLite / PostgreSQL / MySQL dialects with `#[cfg(feature = "sqlite")]` / `#[cfg(feature = "postgres")]` / `#[cfg(feature = "mysql")]` blocks in the same function
- Use parameterized queries - never interpolate SQL directly

```rust
// Parameterized query (PostgreSQL)
sqlx::query_as::<_, Device>(
    "SELECT id, device_id, name FROM gb_device WHERE device_id = $1"
)
.bind(device_id)
.fetch_optional(pool)
.await?
```

#### Naming Conventions
- **Functions**: snake_case (`list_devices_paged`, `get_device_by_device_id`)
- **Types**: PascalCase (`AppError`, `WVPResult`, `Device`)
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
use crate::response::WVPResult;
```

#### Handler Pattern
Always extract state and validate input:

```rust
pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<WVPResult<Response>>, AppError> {
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
│   ├── response.rs          # WVPResult
│   ├── auth.rs              # JWT authentication
│   ├── router.rs            # Route definitions (~374 routes)
│   ├── db/                  # Database layer
│   ├── handlers/            # HTTP handlers (incl. stub.rs / device_stub.rs compat shims)
│   ├── sip/                 # GB28181 SIP stack (core/ transport/ gb28181/)
│   ├── zlm/                 # ZLM media server client + hooks
│   ├── jt1078/              # JT1078 vehicle terminal protocol
│   ├── cascade/             # Upstream platform registration
│   ├── scheduler/           # Record plan scheduling
│   ├── ws/ + cluster/ + rpc/ + state_store.rs  # Cluster/WS/state infrastructure
│   └── middleware/          # Audit logging
├── web/                     # Vue 3 frontend (active)
├── web-legacy-vue2/         # Archived Vue 2 frontend (reference only)
├── e2e/                     # Playwright UI tests
├── mock/                    # Python simulators (SIP device / JT1078 terminal / cascade)
├── docs/                    # Deployment guide, WVP parity, stub retirement plan, UI designs
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
3. Return `Result<Json<WVPResult<T>>, AppError>`
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
  `GBSERVER__ZLM__SERVERS__0__HOOK_URL=http://127.0.0.1:18080/api/zlm/hook`
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

- **`config/application.toml` is deployment-specific.** The `[[zlm.servers]]`
  `hook_url` and similar IPs/ports are tuned for one box. Local-dev
  edits to this file (e.g. `host.docker.internal` → `127.0.0.1`) are
  real fixes for THIS box but break docker-compose deployments. Either
  revert before commit, or split into a separate commit with clear
  message.
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
- **`docs/`** — deployment guides, WVP parity status, stub retirement
  plan, UI design specs. Worth skimming before refactoring large areas.
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