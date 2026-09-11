# AGENTS.md - GBServer

Agent instructions for working on this GB28181 video platform server (Rust backend + Vue 3 frontend).

## Project Overview

- **Backend**: Rust with Axum 0.7, SQLx (SQLite default / PostgreSQL / MySQL via cargo features), JWT auth
- **Frontend**: Vue 3 + Element Plus + Vite + TypeScript (in `web/` directory; archived Vue 2 app in `web-legacy-vue2/`, reference only)
- **Purpose**: GB28181 protocol video management platform (WVP-PRO compatible) with JT1078 vehicle terminal support

## Build Commands

### Backend (Rust)

```bash
# Development (default feature: sqlite — zero-dependency, auto-creates ./data/gbserver.db)
cargo run

# Release build
cargo build --release

# With MySQL instead of SQLite
cargo build --release --no-default-features --features mysql

# With PostgreSQL instead of SQLite
cargo build --release --no-default-features --features postgres

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

# Production build (vue-tsc type check + vite build → web/dist)
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
docker compose up -d

# Import PostgreSQL schema (if not auto-initialized)
docker exec -i gbserver-postgres psql -U postgres -d gbserver < database/init-postgresql-2.7.4.sql

# In another terminal, run frontend dev server
cd web && npm run dev
```

## Important Notes

- Default admin credentials: `admin` / `admin` (MD5: `21232f297a57a5a743894a0e4a801fc3`)
- JWT secret must be changed in production (`config/application.toml`)
- API uses `access-token` header for authentication
- Response format: `{ "code": 0, "msg": "成功", "data": ... }`