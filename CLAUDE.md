# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Backend (default database feature is PostgreSQL)
cargo build
cargo build --release
cargo run
cargo run --release
cargo test
cargo fmt
cargo clippy --all-targets --all-features

# Run focused Rust tests
cargo test <test_name>
cargo test --lib <test_name>
cargo test --test integration_test
cargo test --test jt1078_integration

# Backend with MySQL instead of PostgreSQL
cargo build --release --no-default-features --features mysql
cargo test --no-default-features --features mysql

# Local services used by the default config
docker compose up -d          # PostgreSQL + Redis
docker compose ps
docker compose down           # keeps volumes

# API compatibility smoke test; requires the backend and DB to be running
BASE_URL=http://localhost:18080 node scripts/api-integration-test.js
```

```bash
# Frontend (Vue 2 + Element UI, under web/)
cd web && npm install
cd web && npm run dev          # dev server on :9528, proxies /dev-api to :18080
cd web && npm run build:prod   # production output to web/dist
cd web && npm run lint
cd web && npm run test:unit
cd web && npm run test:ci
```

PowerShell helpers are available on Windows from the repository root: `scripts/build.ps1` builds frontend + backend, `scripts/build-and-run.ps1` builds then runs, and `scripts/run.ps1` runs an existing release binary.

Configuration loads from `config/application.toml` plus environment overrides using the `GBSERVER__SECTION__KEY` form (double underscore separator). The app must be run from the repository root so config files and `web/dist` resolve correctly. The README documents the default admin account (`admin` / `admin`) and first-time SQL import commands if schema auto-init is not sufficient.

## Architecture

This repository is the **GBServer** — a Rust-based GB/T 28181 video platform with the original-style Vue 2 frontend in `web/`. The backend uses Axum/Tower, SQLx, JWT/API-key auth, GB28181 SIP signaling, ZLMediaKit integration, optional Redis caching, platform cascade registration, record scheduling, and JT1078 vehicle terminal support.

### Startup flow (`src/lib.rs` → `run()`)

1. Load config, create a SQLx pool, and ensure required tables. If the core `gb_device` table is missing, startup attempts to initialize the schema from `database/init-postgresql-2.7.4.sql` or `database/init-mysql-2.7.4.sql` via `include_str!`.
2. Create the SIP server when `sip.enabled` is true, wiring it to DB state and WebSocket state.
3. Initialize configured ZLM clients, sync media-server rows into the DB, and start the ZLM health-check loop.
4. Initialize optional Redis, playback/download managers, and shared `AppState`.
5. Spawn background loops for SIP routing, cascade platform registration, record-plan scheduling, and JT1078.
6. Build the Axum router and serve HTTP on `server.port` (default README examples use `18080`).

### Main backend layers

```
handlers/ ──→ db/ ──→ SQLx (PostgreSQL by default, MySQL behind feature flag)
    │
    ├──→ sip/      GB28181 SIP transport, parser/core, device registry, INVITE/catalog/PTZ/SDP logic
    ├──→ zlm/      ZLMediaKit HTTP client, hook receiver, health checker, address building
    ├──→ jt1078/   JT808/JT1078 UDP server, frame parsing, sessions, retransmit tracking
    ├──→ cascade/  upstream platform SIP REGISTER maintenance
    └──→ scheduler/ record-plan background scheduling
```

- `router.rs` is the central route map. Public routes include login, health, metrics, ZLM hooks, and selected frontend/static routes; most `/api/...` routes are wrapped by `auth_middleware`.
- `handlers/` should stay thin: extract Axum params/state, call `db::` or protocol/service modules, and return `WVPResult<T>` or `AppError`. Several `stub.rs` / `device_stub.rs` endpoints intentionally return empty compatibility responses for frontend/API parity.
- `db/` uses one module per table/domain. Functions are free functions over `&db::Pool`; structs typically derive `sqlx::FromRow`. PostgreSQL is the default feature; MySQL-specific SQL is gated with `#[cfg(feature = "mysql")]` where needed.
- `sip/core/` contains low-level SIP message/header/method/status parsing and transaction/dialog primitives. `sip/transport/` owns UDP/TCP networking. `sip/gb28181/` contains application-level device registration, catalog subscription, live/playback/talk INVITE sessions, PTZ, SDP, SSRC, NAT handling, and reconnect behavior.
- `zlm/` wraps ZLMediaKit HTTP APIs and webhook handling. `AppState::get_zlm_client_auto()` selects the least-loaded node, preferring Redis stream counters and falling back to live ZLM API counts.
- `jt1078/` handles vehicle protocol networking and session state. Config supports timeout/retransmit settings and optional hook notification for missing sequence ranges.
- `web/` is a Vue CLI 4 / Vue 2 app. In development, `web/vue.config.js` proxies `/dev-api` and `/static/snap` to the backend at `127.0.0.1:18080`; production assets are served from `web/dist` when `static_dir` is configured.

### Cross-cutting conventions

All normal API responses use `WVPResult<T>` with the standard shape `{ code: 0, msg: "成功", data: ... }`. Handlers generally return `Result<Json<WVPResult<T>>, AppError>` so `?` converts DB/business failures into JSON error responses.

Authentication accepts JWT via `access-token` or `Authorization: Bearer ...`, then falls back to API keys via `X-API-Key` or `apiKey`. Authenticated requests write audit logs asynchronously. Public route exclusions and the protected ZLM proxy are configured in `router.rs` / `auth.rs`.

SDP generation is split by use case (`play_sdp`, `playback_sdp`, `download_sdp`, `talk_sdp`, `broadcast_sdp`). NAT address rewriting is handled in `sip/gb28181/nat_helper.rs` based on configured SDP/stream IPs.
