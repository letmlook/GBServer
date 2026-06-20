# GBServer 部署与运维手册

> 适用版本：`gbserver` v0.1.0
> 范围：构建、运行、配置、部署分级、监控、灾备与故障排查。
> 本手册是 `docs/` 目录下唯一对外文档；`README.md` 之外的产品信息均以本文件为权威源。

---

## 目录

1. [构建与运行](#1-构建与运行)
2. [端口矩阵](#2-端口矩阵)
3. [配置参考](#3-配置参考)
4. [数据库后端选型](#4-数据库后端选型)
5. [部署分级](#5-部署分级)
6. [部署模板](#6-部署模板)
7. [监控与日志](#7-监控与日志)
8. [灾备与故障恢复](#8-灾备与故障恢复)
9. [升级与回滚](#9-升级与回滚)
10. [常见问题](#10-常见问题)

---

## 1. 构建与运行

### 1.1 前置依赖

| 依赖 | 必选 | 推荐 |
|------|------|------|
| OS | — | Linux x86_64 |
| Rust | 1.75+ | 1.78+ |
| Node.js | 14+（仅构建前端） | 18 LTS |
| PostgreSQL / MySQL / SQLite | 数据库三选一 | PostgreSQL 16 / SQLite（零依赖） |
| Redis | 多实例必选 | Redis 7 |
| ZLMediaKit | 视频流媒体 | 2024-01-01+ |

### 1.2 数据库三选一

| 后端 | 编译命令 | 适用 |
|------|---------|------|
| **SQLite（默认）** | `cargo build --release` | 开发 / 演示 / 边缘 / ≤ 500 设备 |
| PostgreSQL | `cargo build --release --no-default-features --features postgres` | 生产主力 / 多实例 / Patroni |
| MySQL | `cargo build --release --no-default-features --features mysql` | MySQL 平迁 / 兼容历史部署 |

### 1.3 初始化数据库

```bash
# SQLite：首次启动自动建表（执行 init-sqlite-2.7.4.sql）
cargo run --release

# PostgreSQL
createdb gbserver
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql

# MySQL
mysql -uroot -p -e "CREATE DATABASE gbserver DEFAULT CHARACTER SET utf8mb4;"
mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql
```

默认管理员：`admin` / `admin`（MD5: `21232f297a57a5a743894a0e4a801fc3`），**生产环境必须立即修改**。

### 1.4 平台构建与运行速查

| 场景 | Linux / macOS | Windows (PowerShell) |
|------|---------------|----------------------|
| 仅编译前端 + 后端 | `./scripts/build.sh` | `.\scripts\build.ps1` |
| 仅编译后端 | `./scripts/build.sh --skip-frontend` | — |
| 仅编译前端 | `./scripts/build.sh --skip-backend` | — |
| 仅运行（已编译） | `./scripts/run.sh` | `.\scripts\run.ps1` |
| 一键编译 + 运行 | `./scripts/build-and-run.sh` | `.\scripts\build-and-run.ps1` |
| Docker 一键拉起 | `docker compose up -d --build` | 同左 |
| 初始化 PG schema（Docker 容器） | `bash <(curl ...)` 或手动 | `.\scripts\init-db-postgres.ps1` |

### 1.5 Linux / macOS 原生构建

```bash
# 1) 编译
./scripts/build.sh                       # 前后端都编译
# 或分步
( cd web && npm install && npm run build:prod )
cargo build --release

# 2) 运行
./scripts/run.sh
# 或直接
./target/release/gbserver
```

产物：
- 前端：`web/dist/`
- 后端：`target/release/gbserver`

### 1.6 Windows 原生构建

```powershell
.\scripts\build.ps1           # 编译
.\scripts\run.ps1             # 运行
.\scripts\build-and-run.ps1   # 一键
```

可执行文件：`target\release\gbserver.exe`

### 1.7 开发模式（前后端热更新）

```bash
./scripts/build-and-run.sh        # Linux/macOS
.\scripts\build-and-run.ps1       # Windows
```

- 前端 dev server：`http://localhost:9528`，HMR 热更新，`/dev-api` 反代到后端 `18080`
- 后端：`cargo run`，源码修改后自动重启

### 1.8 环境变量覆盖

所有 `config/application.toml` 字段都可通过 `GBSERVER__SECTION__KEY` 形式覆盖（双下划线分隔）：

```bash
GBSERVER__SERVER__PORT=18080 \
GBSERVER__DATABASE__URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver' \
GBSERVER__REDIS__URL='redis://127.0.0.1:6379' \
GBSERVER__JWT__SECRET='your-strong-secret' \
GBSERVER__SIP__PASSWORD='your-sip-password' \
./target/release/gbserver
```

### 1.9 Docker 部署

```bash
docker compose up -d --build       # 一键启动
docker compose ps                  # 查看状态
docker compose logs -f gbserver    # 实时日志
docker compose restart gbserver    # 仅重启后端
docker compose down                # 停止（保留 volume）
docker compose down -v             # 停止并清空数据卷
```

首次构建约 10–20 分钟（拉镜像 + npm install + cargo 全量编译）。

通过 `.env` 自定义密钥：

```env
GBSERVER_JWT_SECRET=<your-strong-jwt-secret>
GBSERVER_SIP_PASSWORD=<your-sip-password>
```

`docker-compose.yml` 已支持 `${GBSERVER_JWT_SECRET:-default}` 语法，会自动读取 `.env`。

---

## 2. 端口矩阵

> ⚠️ **以本表为唯一权威源**。任何代码、配置出现与本表不符的端口，请以本表为准并修复。

### 2.1 核心服务

| 端口 | 协议 | 服务 | 用途 | 配置项 |
|------|------|------|------|--------|
| **18080** | TCP | GBServer | HTTP API + 静态前端 | `server.port` / `GBSERVER__SERVER__PORT` |
| **5060** | UDP | GBServer | GB28181 SIP 信令 | `sip.port` |
| **5061** | TCP | GBServer | GB28181 SIP 信令（TCP） | `sip.tcp_port` |
| **9528** | TCP | Vue dev server | 前端开发模式 | `web/vue.config.js` |

### 2.2 数据库 / 缓存

| 端口 | 协议 | 服务 | 用途 |
|------|------|------|------|
| 5432 | TCP | PostgreSQL | 主数据库 |
| 3306 | TCP | MySQL | 备选数据库 |
| 6379 | TCP | Redis | 缓存 / 跨节点 StateStore |

### 2.3 ZLMediaKit

| 端口 | 协议 | 用途 |
|------|------|------|
| 8080 | TCP | ZLM HTTP API（`zlm.servers[0].http_port`） |
| 8443 | TCP | ZLM HTTPS |
| 554 | TCP | ZLM RTSP |
| 322 | TCP | ZLM RTSPS |
| 1935 | TCP | ZLM RTMP |
| 8000 | UDP | ZLM WebRTC |
| 9000 | UDP | ZLM SRT |
| 30000–30100 | UDP | ZLM RTP 媒体端口范围（GB28181 流） |

### 2.4 端口冲突排查

| 端口 | 可能冲突方 | 排查命令 |
|------|----------|---------|
| 18080 | 其他 HTTP 服务 | `lsof -i :18080` / `netstat -ano \| findstr :18080` |
| 5060 | 其他 SIP / PBX | `lsof -i :5060` |
| 5061 | 其他 SIP/TCP | 同上 |
| 5432 | 本地 PostgreSQL | `pg_isready -h 127.0.0.1 -p 5432` |
| 6379 | 本地 Redis | `redis-cli -h 127.0.0.1 -p 6379 ping` |
| 8080 | ZLM 与本机其他 HTTP | 改 `zlm.servers[0].http_port` 并同步 `docker-compose.yml` |

修改任何端口必须同步修改：`config/application.toml` / `docker-compose.yml` / `Dockerfile`（`EXPOSE` / `HEALTHCHECK`）/ `web/vue.config.js` / 本文件 §2。

---

## 3. 配置参考

配置文件：`config/application.toml`，可由 `GBSERVER__SECTION__KEY` 环境变量覆盖。

### 3.1 关键配置项

| 路径 | 必选 | 默认 | 说明 |
|------|------|------|------|
| `jwt.secret` | ✅ | 占位 | ≥ 32 字符；**生产环境必须覆盖** |
| `database.url` | ✅ | `sqlite://data/gbserver.db?mode=rwc` | sqlx URL |
| `database.sqlite_max_devices` | — | 500 | SQLite 设备上限；PG/MySQL 忽略 |
| `sip.password` | ✅ | `admin123` | GB28181 SIP digest 密码 |
| `sip.device_id` | ✅ | `34020000002000000001` | 20 位本级 GB-ID |
| `zlm[].secret` | ✅ | 占位 | ZLM HTTP API 密钥（与 ZLM `config.ini` 一致） |
| `redis.url` | — | — | 设置后启用 Redis StateStore |

### 3.2 SIP / GB28181

```toml
[sip]
enabled = true
ip = "0.0.0.0"
port = 5060                 # UDP
tcp_port = 5061
device_id = "34020000002000000001"
password = "admin123"       # 通过 GBSERVER__SIP__PASSWORD 覆盖
realm = "3402000000"
keepalive_timeout = 30      # 秒
register_timeout = 3600
charset = "UTF-8"
```

### 3.3 ZLMediaKit

```toml
[[zlm.servers]]
id = "zlm-1"
ip = "127.0.0.1"
http_port = 8080
https_port = 8443
secret = "035c73f7-bb6b-4889-a715-d9eb2d1925cc"

[zlm]
stream_timeout = 30
hook_enabled = true
hook_url = "http://127.0.0.1:18080/api/zlm/hook"
```

ZLM 必须配置 hook 回调到 `/api/zlm/hook`，配置项：`[hook] enable=1, root_url=http://gbserver:18080`。

多节点 ZLM：设置 `redis.url` 后，`play_start` / `playback_start` / `send_play_invite` 等请求按 Redis ZSET 最小连接数选路。

### 3.4 集群模式（多实例 HA）

```toml
[cluster]
enabled = false                       # 多节点 HA 部署改为 true
single_node_mode = true                # 单节点默认
node_id = ""                           # 留空 = pid 哈希
addr = "http://127.0.0.1:18080"
role = "primary"
heartbeat_interval_secs = 10
heartbeat_ttl_secs = 60

[audit]
enabled = true
retention_days = 90
```

无 Redis 配置时自动降级为单节点模式（`single_node_mode = true`）。

---

## 4. 数据库后端选型

### 4.1 三种后端对比

| 维度 | SQLite | PostgreSQL | MySQL |
|------|--------|------------|-------|
| 零依赖启动 | ✅ | ❌ | ❌ |
| 适用规模 | ≤ 500 设备 | 500 ~ 5000+ | 同 PG（生态习惯） |
| 集群 / HA | ❌（LiteFS / rqlite 不推荐） | ✅ Patroni / Citus | ✅ MGR / PXC / TiDB |
| 高频位置 / 告警写入 | 一般 | 强（TimescaleDB 可加时序扩展） | 强 |
| 备份 | 文件拷贝 / `VACUUM INTO` | `pg_dump` / WAL-G | `mysqldump` / xtrabackup |
| Cargo feature | `default = ["sqlite"]` | `--features postgres` | `--features mysql` |

### 4.2 SQLite 适用判断

| 当前规模 | 建议 |
|---------|------|
| < 200 设备 | SQLite 完全足够 |
| 200 ~ 500 设备 | 关注写并发；位置 / 心跳高频时考虑缓存或批写 |
| > 500 设备 | 迁移到 PostgreSQL；用 `sqlite3 .dump` 导出后导入 PG |

**运行时保护**：`config/application.toml` 中 `database.sqlite_max_devices`（默认 500）。
- 新增设备：当前 `gb_device` 总数 < 上限 → 允许；否则 SIP REGISTER 返回 **503**
- 更新已有设备（重注册）：始终允许
- PG/MySQL 后端：完全忽略此字段

### 4.3 SQLite 部署特征

- 单写者锁 + 并发读：WAL + `busy_timeout=5000ms`
- 备份：每日 `sqlite3 /var/lib/gbserver/gbserver.db ".backup /backup/gbserver-$(date +%F).db"`

### 4.4 决策流程

```
MySQL 平迁？ ─── 是 ──→ MySQL（任意级别）
        ↓ 否
设备 < 500 且无 HA 要求？ ─── 是 ──→ SQLite
        ↓ 否
PostgreSQL（推荐生产主力）
        ├─ 设备 > 5000 或写 > 1000 QPS？ ──→ DB 集群
        └─ 否则 ──→ PG 单实例 + 备份
```

---

## 5. 部署分级

| 级别 | 设备数 | 并发流 | 应用形态 | 数据库 | Redis | ZLM |
|------|--------|--------|----------|--------|-------|-----|
| **L1 演示 / 开发** | < 50 | < 10 | 单机 | SQLite | 无 | 1 |
| **L2 边缘节点** | < 200 | < 20 | 单机 | SQLite | 无 | 1 |
| **L3 小规模生产** | < 500 | < 50 | 单机 | SQLite / PG | 可选 | 1–2 |
| **L4 中等生产** | 500 ~ 2000 | 50 ~ 200 | 单机 | PG | 可选 | 2–3 |
| **L5 大规模生产** | 2000 ~ 5000 | 200 ~ 500 | 单机 + 主备 | **PG + Patroni** | 是 | 3+ |
| **L6 HA 集群** | > 2000 | > 200 | **多实例 + SIP LB** | **PG + Patroni** | **必选** | 3+ |
| **L7 超大规模** | > 5000 | > 500 | 多实例 + LB | **PG + Citus / TimescaleDB** | 是 | 5+ |
| **L8 MySQL 平迁** | 任意 | 任意 | 单 / 多 | MySQL | 可选 | 任意 |

### 5.1 单机部署（L1 ~ L5）

- 无 Redis：`StateStore::in_memory()` 自动启用，所有状态在进程内 DashMap
- 故障即停机，重启后设备需重新注册
- 适用：开发、演示、边缘节点、小规模生产

### 5.2 多实例 + Redis（HA，L6）

```
       ┌──────────────┐
       │ SIP LB /     │  (OpenSIPS / Kamailio)
       │ UDP Proxy    │
       └──────┬───────┘
              │
   ┌──────────┴──────────┐
   ▼                     ▼
GBServer-A           GBServer-B
   │      └── StateStore (Redis) ──┘
   ▼
PostgreSQL (单实例 / Patroni)
```

**多实例已支持**：
- ✅ 媒体服务器多节点（`[[zlm.servers]]` 配置）
- ✅ ZLM 负载均衡（Redis ZSET 最小连接数）
- ✅ 级联 SendRtp 跨节点（`cascade_forward.rs` StateStore 同步）

**当前 HA 缺口**：
- ❌ SIP 负载均衡（需 OpenSIPS / Kamailio）— HA 最大缺口
- ❌ PendingRequest 跨节点路由
- ❌ SubscriptionLifecycle 分布式续期
- ❌ JT1078 UDP 跨节点
- 建议给 ZLM hook 加 `[[zlm.hook_allowlist]]` IP 校验

### 5.3 Redis 故障行为

Redis 不可达时后端自动回退 InMemoryBackend（每节点独立）。跨节点状态会**发散**直至 Redis 恢复：对 invite / stream 状态可接受，对 sendrtp / recording 不推荐。

---

## 6. 部署模板

### 6.1 L1 演示 / 开发（SQLite + 单机）

```toml
[database]
url = "sqlite://data/gbserver.db?mode=rwc"

[server]
port = 18080

[sip]
enabled = false
```

```bash
cargo run --release
```

### 6.2 L3 小规模生产（SQLite + 单机）

```toml
[database]
url = "sqlite:///var/lib/gbserver/gbserver.db?mode=rwc"

[server]
port = 18080

[sip]
enabled = true
ip = "0.0.0.0"
port = 5060

[[zlm.servers]]
id = "zlm-1"
ip = "127.0.0.1"
http_port = 8080
secret = "your-zlm-secret"
enabled = true
```

### 6.3 L4 中等生产（PostgreSQL + 单机）

```toml
[database]
url = "postgres://gbserver:***@10.0.1.10:5432/gbserver"

[server]
port = 18080

[sip]
enabled = true
ip = "10.0.1.20"
port = 5060

[redis]
url = "redis://10.0.1.11:6379/0"   # 可选，开启后可观测

[[zlm.servers]]
id = "zlm-1"
ip = "10.0.1.30"
http_port = 8080
secret = "***"
enabled = true

[[zlm.servers]]
id = "zlm-2"
ip = "10.0.1.31"
http_port = 8080
secret = "***"
enabled = true
```

### 6.4 L6 HA 集群（PG + Patroni + Redis + 多实例 + SIP LB）

```
外部负载均衡层：
  - OpenSIPS / Kamailio：SIP UDP/TCP LB
  - HAProxy / Nginx：HTTP API LB

应用层（无状态）：
  - GBServer-A (10.0.1.20) ← SIP LB → UDP/5060
  - GBServer-B (10.0.1.21) ← SIP LB → UDP/5060

数据层：
  - Redis Sentinel / Cluster（10.0.1.11~13）
  - PostgreSQL Patroni（10.0.1.15 primary + 10.0.1.16/17 replica）
  - etcd cluster (10.0.1.5~7)

媒体层：
  - ZLM-1 (10.0.1.30)
  - ZLM-2 (10.0.1.31)
  - ZLM-3 (10.0.1.32)
```

GBServer 启动参数：

```bash
GBSERVER__DATABASE__URL=postgres://gbserver:***@10.0.1.15:5432/gbserver \
GBSERVER__REDIS__URL=redis://10.0.1.11:6379/0 \
GBSERVER__SIP__IP=10.0.1.20 \
GBSERVER__RPC__NODE_ID=node-a \
GBSERVER__RPC__PEER_ENDPOINTS=["http://10.0.1.21:18080"] \
./gbserver
```

### 6.5 数据库集群方案

| 后端 | 方案 | 规模 | 特性 |
|------|------|------|------|
| PostgreSQL | 单实例 + WAL-G | < 3000 设备 | RPO ~5min |
| PostgreSQL | **Patroni + etcd** | 3000 ~ 10000 | 自动故障切换，RPO < 1s |
| PostgreSQL | **Patroni + Citus** | > 10000 | 水平分片 |
| PostgreSQL | **TimescaleDB** | 高频位置 / 告警 | 时间分区 + 压缩 |
| MySQL | 单实例 + xtrabackup | < 3000 | 传统方案 |
| MySQL | **MySQL Group Replication** | 3000 ~ 8000 | 单主多写，Paxos |
| MySQL | **PXC / Percona XtraDB** | 同上 | 同步复制 |
| MySQL | **TiDB** | > 10000 | 水平扩展，HTAP |

---

## 7. 监控与日志

### 7.1 健康端点

| 端点 | 用途 |
|------|------|
| `GET /api/health` | JSON 状态（db / sip / zlm / redis），任一异常返 503 |
| `GET /api/ready` | 200 仅当 DB + cluster + Redis 正常；单节点自动跳过 cluster |
| `GET /metrics` | Prometheus 文本格式（14+ 指标） |
| `GET /api/server/config` | 脱敏运行时配置（密码遮蔽） |
| `GET /api/system/info` | 版本 + 启动时间 + 特性开关 |
| `GET /api/system/stats` | 设备 / 通道 / 流 / 会话 / JT 终端 / cluster 统计 |

> 区分：`/api/health` 永返 200（不查 DB/Redis，避免 k8s 误重启）；`/api/ready` 才做依赖检查。

### 7.2 日志

默认 `RUST_LOG=info,gbserver=debug`。生产可使用 `tracing-subscriber` JSON formatter：

```bash
RUST_LOG=info,gbserver=debug cargo run --release
```

### 7.3 关键监控指标

| 指标 | 来源 | 告警阈值 |
|------|------|---------|
| `gbserver_db_pool_acquired` | sqlx pool | > 80% 饱和 |
| `gbserver_sip_keepalive_late_seconds` | SipServer | > 90s |
| `gbserver_zlm_stream_count` | ZLM hook | 按节点趋势 |
| `gbserver_request_duration_seconds` | axum 中间件 | p99 > 2s |
| `gbserver_redis_state_keys` | StateStore | 趋势 |
| `gb_cluster_nodes_active` | ClusterRegistry | 跌至 0 |

### 7.4 抓包与诊断

- SIP 抓包：`sngrep` / `Homer`
- ZLM 探活：30s 健康检查循环（`health_checker.run_health_check_loop`）
- 数据库慢查询：PG `pg_stat_statements` / MySQL `slow_query_log`

---

## 8. 灾备与故障恢复

### 8.1 数据库故障

```bash
# 停止后端
systemctl stop gbserver
# 恢复
pg_restore --clean --dbname=gbserver /var/backups/gbserver/gbserver_2026-06-10.sql
# 启动
systemctl start gbserver
```

### 8.2 Redis 故障

Redis 仅作为缓存层，丢失时后端自动回退 InMemoryBackend（每节点独立）。状态在节点间发散至恢复为止；invite / stream 可接受，sendrtp / recording 不推荐。多节点级联请用 Sentinel / Cluster 复制。

### 8.3 ZLM 故障

后端通过 `on_server_started` hook 检测 ZLM 离线，置 `gb_media_server.online=false`。故障期间播放请求返 502。

```bash
# 重启 ZLM 后重新推送 hook 配置
curl -X POST http://zlm:8080/index/api/setServerConfig \
  -d 'secret=YOUR_SECRET&hook.enable=1&hook.root_url=http://gbserver:18080'
```

### 8.4 SIP 服务故障

```bash
ss -ulnp | grep 5060                    # 确认 UDP 5060 监听
journalctl -u gbserver --since "10 min ago" | grep -i 'sip'
# 检查防火墙：ufw allow 5060/udp
```

后端下次启动会重绑；设备端 SIP 服务器地址需填写宿主机或映射后 IP（非 localhost）。

---

## 9. 升级与回滚

### 9.1 原地升级

```bash
# 1) 编译新二进制
cargo build --release
mv target/release/gbserver target/release/gbserver.new

# 2) 原子切换
systemctl stop gbserver
mv target/release/gbserver target/release/gbserver.old
mv target/release/gbserver.new target/release/gbserver
systemctl start gbserver

# 3) 回滚（如需）
systemctl stop gbserver
mv target/release/gbserver.old target/release/gbserver
systemctl start gbserver
```

DB schema 由 `init_db_tables` 在启动时自动执行缺失迁移（幂等）。

### 9.2 从 Java 旧版迁移

1. 停 Java 服务：`systemctl stop gbserver`
2. 备份 DB：`pg_dump gbserver > backup_$(date +%F).sql`
3. 拉取新版：`git pull && cargo build --release`
4. 启动：`systemctl start gbserver`
5. 验证：`curl http://localhost:18080/api/server/version`

### 9.3 数据库迁移

- SQLite → PG：`sqlite3 .dump` 导出后导入 PG
- 配置回滚：通过 `GBSERVER__SECTION__KEY` 环境变量覆盖，无需改文件

### 9.4 集群滚动升级

L6 集群可在 SIP LB 后逐节点重启（前提：SipServer 已支持 dialog 持久化）。

---

## 10. 常见问题

| 现象 | 可能原因 | 处理 |
|------|---------|------|
| 启动报 `JWT secret validation failed` | 默认 / 弱密钥 | `export GBSERVER__JWT__SECRET=$(openssl rand -hex 32)` |
| 设备显示离线 | SIP UDP 5060 被防火墙拦截 | `ufw allow 5060/udp`，检查 `sip.ip` |
| 播放 URL 返 502 | ZLM 不可达 | 检查 `zlm[*].ip` 与网络 ACL |
| 云录像不出现 | ZLM `on_record_mp4` hook 未 POST | 确认 `hook_url` 为 `/api/zlm/hook` |
| `docker compose up -d` 报 `failed to read dockerfile` | 仓库根目录缺 `Dockerfile` / `.dockerignore` | 确认文件存在 |
| ZLM 鉴权失败（hook 收不到） | ZLM secret 与配置不一致 | `docker-compose.yml` 中 `zlm.ZLM_HTTP_SECRET` 与 `config/application.toml` 的 `zlm.servers[0].secret` 完全一致 |
| 前端 9528 代理到 18080 失败 | 后端未启动 | `curl http://localhost:18080/api/health` 验证 |
| SIP 设备注册不上 | 防火墙 / 设备 SIP 地址 / 密码不一致 | 1) 防火墙放通 UDP 5060 / TCP 5061；2) 设备 SIP 地址填宿主机 IP；3) `sip.password` 与设备端一致（默认 `admin123`，生产请改） |
| MySQL 替换 PostgreSQL | feature + URL 不一致 | `cargo build --release --no-default-features --features mysql` 并改 `database.url` 为 `mysql://...` |

---

## 附录 A：项目结构

```
GBServer/
├── Cargo.toml              # Rust 包定义（包名 gbserver）
├── Cargo.lock
├── Dockerfile              # 多阶段构建（前端 + 后端 → slim 运行时）
├── docker-compose.yml      # 一键拉起 pg + redis + zlm + gbserver
├── docker-compose.sqlite.yml
├── LICENSE                 # MIT License
├── README.md
├── config/
│   └── application.toml    # 默认配置
├── database/
│   ├── init-sqlite-2.7.4.sql
│   ├── init-postgresql-2.7.4.sql
│   └── init-mysql-2.7.4.sql
├── docs/
│   └── DEPLOYMENT_GUIDE.md # ← 本文档
├── scripts/                # 构建 / 运行脚本（PowerShell + bash）
├── src/                    # Rust 后端
│   ├── lib.rs / main.rs
│   ├── config.rs / auth.rs / router.rs / metrics.rs / security.rs / cache.rs
│   ├── handlers/           # HTTP 业务接口
│   ├── db/                 # SQLx 持久化（按表拆分）
│   ├── sip/                # GB28181 SIP 协议栈
│   ├── zlm/                # ZLMediaKit 客户端 / Hook
│   ├── jt1078/             # JT/T 808+1078 车辆部标
│   ├── cascade/            # 上级平台 SIP REGISTER
│   ├── scheduler/          # 录像计划调度
│   ├── cluster/            # 集群节点发现
│   ├── state/              # 跨节点状态仓储
│   ├── ws/                 # WebSocket
│   └── middleware/         # audit 等
├── tests/                  # 集成测试
├── web/                    # Vue 2 + Element UI 前端
│   ├── vue.config.js       # dev server :9528 + 反代 :18080
│   └── src/{api,views}/    # 业务 API 封装 + 页面
└── justfile
```

## 附录 B：相关源码索引

| 模块 | 关键文件 |
|------|---------|
| 启动入口 / 主流程 | `src/lib.rs::run()`、`src/main.rs` |
| 配置加载 | `src/config.rs` |
| 路由中心 | `src/router.rs` |
| 鉴权（JWT / API Key） | `src/auth.rs`、`src/ws/jwt.rs` |
| 设备 / 通道 | `src/handlers/{device,device_query,device_control,common_channel}.rs`、`src/db/{device,common_channel}.rs` |
| SIP / GB28181 | `src/sip/{core,transport,gb28181}/`、`src/sip/server.rs` |
| ZLMediaKit | `src/zlm/{client,hook,hook_routes,media_node,health_checker}.rs` |
| JT1078 | `src/jt1078/{manager,server,command,command_waiter,session,jt_media_session,response_parser}.rs` |
| 跨实例状态 | `src/state_store.rs`、`src/state/{repository,stream_status}.rs`、`src/cluster/registry.rs`、`src/rpc.rs` |
| 录像 | `src/handlers/{playback,cloud_record_extra}.rs`、`src/db/cloud_record.rs`、`src/scheduler/record_plan.rs` |
| 推流 / 拉流代理 | `src/handlers/{stream,play}.rs`、`src/db/{stream_push,stream_proxy}.rs` |
| 级联 | `src/cascade/register.rs`、`src/sip/gb28181/{cascade_service,cascade_forward}.rs` |
| 监控 / 健康 / 审计 | `src/handlers/{health,metrics,system}.rs`、`src/middleware/audit.rs`、`src/metrics.rs` |
| WebSocket | `src/ws/{hub,jwt}.rs`、`src/handlers/websocket.rs` |

---

**最后更新：与 `config/application.toml`、`docker-compose.yml`、`Dockerfile` 同步。**
任何端口 / 配置变更请同步更新本文件。
