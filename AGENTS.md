# AGENTS.md - WVP GB28181 Server

Agent instructions for working on this GB28181 video platform server (Rust backend + Vue 2 frontend).

## Project Overview

- **Backend**: Rust with Axum 0.7, SQLx (PostgreSQL/MySQL), JWT auth
- **Frontend**: Vue 2 + Element UI (in `web/` directory)
- **Purpose**: GB28181 protocol video management platform

## Build Commands

### Backend (Rust)

```bash
# Development
cargo run

# Release build
cargo build --release

# With MySQL instead of PostgreSQL
cargo build --release --no-default-features --features mysql
```

### Frontend (Vue 2)

```bash
cd web

# Install dependencies
npm install

# Development server
npm run dev

# Production build
npm run build:prod

# Lint
npm run lint

# Unit tests
npm run test:unit
```

### Run Both

```powershell
# PowerShell (Windows)
.\scripts\build-and-run.ps1
```

### Database Setup

```bash
# PostgreSQL (default)
psql -U postgres -d wvp -f database/init-postgresql-2.7.4.sql

# MySQL
mysql -uroot -p wvp < database/init-mysql-2.7.4.sql
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
- Handle both PostgreSQL and MySQL with `#[cfg(feature = "postgres")]` / `#[cfg(feature = "mysql")]`
- Use parameterized queries - never interpolate SQL directly

```rust
// Parameterized query (PostgreSQL)
sqlx::query_as::<_, Device>(
    "SELECT id, device_id, name FROM wvp_device WHERE device_id = $1"
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
- Use `config` crate with YAML + environment variables
- Environment variables use `WVP__SECTION__KEY` format
- Load config in `main.rs` and pass to `run()`

#### Logging
- Use `tracing` crate
- Set level via `RUST_LOG` environment variable
- Default: `info,wvp_gb28181_server=debug`

```rust
tracing::info!("Starting server on port {}", port);
tracing::debug!("Query result: {:?}", result);
```

### Frontend (Vue 2)

- Uses Vue CLI 4.4.4
- ESLint for linting (`eslint --ext .js,.vue src`)
- Element UI for components
- Follow existing patterns in `web/src/`

## Project Structure

```
/home/letmlook/GBServer/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # AppState, run() function
│   ├── config.rs            # Configuration loading
│   ├── error.rs             # AppError, ErrorCode
│   ├── response.rs          # WVPResult
│   ├── auth.rs              # JWT authentication
│   ├── router.rs            # Route definitions
│   ├── db/                  # Database layer
│   │   ├── mod.rs           # Pool creation
│   │   ├── device.rs        # Device/Channel queries
│   │   ├── user.rs          # User queries
│   │   └── ...              # Other DB modules
│   ├── handlers/            # HTTP handlers
│   │   ├── user.rs          # User endpoints
│   │   ├── device.rs        # Device endpoints
│   │   └── ...              # Other handlers
│   ├── sip/                 # GB28181 SIP implementation
│   └── zlm/                 # ZLM media server client
├── web/                     # Vue 2 frontend
├── config/
│   └── application.yaml     # Default configuration
├── database/
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
# Start database (Docker)
docker compose up -d

# Import database schema
docker exec -i wvp-postgres psql -U postgres -d wvp < database/init-postgresql-2.7.4.sql

# Run backend
cargo run

# In another terminal, run frontend dev server
cd web && npm run dev
```

## Important Notes

- Default admin credentials: `admin` / `admin` (MD5: `21232f297a57a5a743894a0e4a801fc3`)
- JWT secret must be changed in production (`config/application.yaml`)
- API uses `access-token` header for authentication
- Response format: `{ "code": 0, "msg": "成功", "data": ... }`