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
