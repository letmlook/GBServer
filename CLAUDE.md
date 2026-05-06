# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Backend (default: PostgreSQL)
cargo build --release
cargo run
cargo test

# Backend with MySQL
cargo build --release --no-default-features --features mysql

# Frontend
cd web && npm install && npm run build:prod     # production build
cd web && npm run dev                           # dev server (proxies to :18080)

# Integration test (requires DB + Redis)
TEST_DATABASE_URL=postgres://... TEST_REDIS_URL=redis://... cargo test --test integration_test
```

Configuration via `config/application.yaml` or env vars (`WVP__SECTION__KEY`, note double underscore separator). Start database with `docker compose up -d` (PostgreSQL + Redis).

## Architecture

### Startup flow (`src/lib.rs` → `run()`)

1. Create DB pool, init missing tables (auto-runs `database/init-postgresql-2.7.4.sql` or the MySQL variant via `include_str!`)
2. Create SIP server (UDP + optional TCP), wire it with ZLM client and WebSocket state
3. Initialize ZLM clients (one per configured media server), start health checker background loop
4. Spawn 4 background tasks: SIP server event loop, cascade registrar (platform federation), record plan scheduler, JT1078 server
5. Start Axum HTTP server on configured port

### Layered architecture

```
handlers/ ──→ db/ ──→ SQLx (Postgres/MySQL)
    │            │
    ├──→ sip/   (GB28181 SIP signaling — UDP+TCP transport, SIP core parser, GB28181 application layer)
    ├──→ zlm/   (ZLMediaKit HTTP API client, webhook receiver, health checker)
    └──→ jt1078/ (JT808/JT1078 vehicle terminal protocol — UDP server, frame parsing, session management)
```

- **`handlers/`**: Thin HTTP handlers; extract params, call `db::` functions, wrap in `WVPResult<T>`. Stub handlers (`handlers/stub.rs`, `handlers/device_stub.rs`) return empty data — placeholders for API compatibility with the Java frontend.
- **`db/`**: One module per DB table. Functions are free functions taking `&Pool`. Structs derive `sqlx::FromRow`. SQL uses `$1` placeholders (Postgres default); MySQL variants gated behind `#[cfg(feature = "mysql")]`.
- **`sip/core/`**: Low-level SIP — message model, parser (text-based SIP grammar), transaction state machine, dialog tracking, method/status enums.
- **`sip/gb28181/`**: GB28181 application logic — device registry, catalog subscription, invite sessions (live/playback/talk), PTZ control, SDP builder, SSRC management, NAT traversal, stream reconnect.
- **`sip/transport/`**: UDP socket recv/send loop + TCP listener/connection manager. Messages are dispatched to `SipServer` for routing.
- **`zlm/`**: HTTP client for ZLMediaKit APIs (add stream proxy, close streams, get media info), webhook handler for ZLM callbacks (stream change, record status), health checker with DB sync.
- **`cascade/`**: Platform-to-platform SIP registration — loads upstream platforms from DB and maintains periodic SIP REGISTER cycles.

### Request/Response Pattern

All API responses use `WVPResult<T>` (`{ code: 0, msg: "成功", data: ... }`). Handlers return `Result<Json<WVPResult<T>>, AppError>`. `AppError` implements `IntoResponse` — the `?` operator automatically converts `sqlx::Error` and business errors into proper JSON error responses. Error codes mirror the Java WVP conventions (0 = success, 100 = failure, 400 = bad request, 401 = unauthorized, etc.).

### Authentication

JWT (HS256, `access-token` header or `Authorization: Bearer`) and API Key (X-API-Key header or `apiKey` query param). The `auth_middleware` tries JWT first, falls back to API Key lookup in `wvp_user_api_key` table. Audit logs are written asynchronously (`tokio::spawn`) on every authenticated request. Public routes (login, ZLM webhook, health check, metrics) skip auth. The ZLM proxy route (`/zlm/:media_server_id/*path`) requires auth.

### ZLM Node Selection

`AppState::get_zlm_client_auto()` selects the least-loaded ZLM node: tries Redis stream counts first, falls back to querying each ZLM's active stream count via HTTP API. Direct ID or "auto" keyword are supported.

### SIP SDP Handling

SDP is built per session type: `play_sdp()` (live), `playback_sdp()` (history), `download_sdp()` (record download), `talk_sdp()` (voice), `broadcast_sdp()` (broadcast). NAT handling (`nat_helper.rs`) swaps SDP IPs based on `sdp_ip`/`stream_ip` config for NAT traversal.

### Database Schema Init

On startup, if `wvp_device` table is missing, the full SQL init script is executed. Each statement is split by semicolons and run individually (only DDL/DML statements — comment lines are skipped). Additional tables like `position_history` and `audit_log` are ensured via `CREATE TABLE IF NOT EXISTS`.
