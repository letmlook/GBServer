# GBServer 部署与运维手册

> 适用版本：`gbserver` v0.1.0
> 范围：构建、运行、配置、部署分级、监控、灾备与故障排查。
> 本手册负责**部署与运维**；产品当前状态见 [`STATUS.md`](STATUS.md)、
> 未完成事项见 [`OPEN_ISSUES.md`](OPEN_ISSUES.md)、多方言 SQL 注意事项见
> [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md)、`stub.rs` 定位见
> [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md)。
>
> 📌 **最后校订：2026-09-19（真实设备接入核验轮，HEAD `8ecec6a`）** —— 本轮逐条
> 核对了本文的端口、配置项、指标名与附录文件清单，修正了此前遗留的若干错误
> （SIP TCP 端口、ZLM 端口与 `config.ini`、`vue.config.js`、已删除的 `cache.rs`
> /`cascade_service.rs`、若干并不存在的 Prometheus 指标名）。

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
| Rust | 需能编译本仓库（`Cargo.toml` **未**声明 `rust-version`；CI 用 `stable`） | 最新 stable |
| Node.js | **18+**（`web/package.json` 的 `engines.node = ">=18.0.0"`；仅构建前端） | 18 LTS / 20 |
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
( cd web && npm install && npm run build )
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
- 后端：脚本里是 `cargo run`（**没有 file-watcher，改完源码要自己重启**；改前端才有 HMR）

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

`docker-compose.yml` 用 `${GBSERVER_JWT_SECRET:-<内置默认值>}` 语法读取 `.env`。
⚠️ **内置默认值就是仓库里那份演示密钥**（`config/application.toml` 与
`src/security.rs::COMMITTED_DEMO_JWT_SECRET` 相同，会被 `validate_jwt_secret`
判为弱密钥并在启动日志里告警）。**生产必须显式设置 `GBSERVER_JWT_SECRET`。**

---

## 2. 端口矩阵

> ⚠️ 本表已于 2026-09-19 与 `config/application.toml`、`docker/zlm/config.ini`、
> `docker-compose*.yml` 逐项核对。若发现代码/配置与本表不一致，**先核对上述三个源文件
> 再决定改哪一边**，改完同步更新本表 —— 不要无条件按本表去改代码。

### 2.1 核心服务

| 端口 | 协议 | 服务 | 用途 | 配置项 |
|------|------|------|------|--------|
| **18080** | TCP | GBServer | HTTP API + 静态前端 | `server.port` / `GBSERVER__SERVER__PORT` |
| **5060** | UDP **+ TCP** | GBServer | GB28181 SIP 信令 | `sip.port`（UDP）/ `sip.tcp_port`（TCP，**默认等于 `port`**） |
| **9528** | TCP | Vue dev server | 前端开发模式（仅开发） | `web/vite.config.ts` |

> ⚠️ **SIP 的 UDP 与 TCP 默认共用 5060**：国标设备按「同一 IP:端口 + 传输方式」接入，
> `config/application.toml` 里 `port = 5060`、`tcp_port = 5060`（`src/config.rs:236`
> 默认值同样是 5060）。**旧版本文档写的 5061 是错的** —— 照抄会让 TCP 设备注册不上
> （`docker-compose.yml` 里仍留着 `5061:5061` 的发布，但当前没有人监听它）。
> 只有明确要分离两种传输时才把 `tcp_port` 改成别的值，并同步防火墙与设备配置。

### 2.2 数据库 / 缓存

| 端口 | 协议 | 服务 | 用途 |
|------|------|------|------|
| 5432 | TCP | PostgreSQL | 主数据库 |
| 3306 | TCP | MySQL | 备选数据库 |
| 6379 | TCP | Redis | 缓存 / 跨节点 StateStore |

### 2.3 ZLMediaKit

> **部署方式**：`docker-compose.yml` 默认让 ZLM 使用 **host 网络**，上表这些端口
> 由 ZLM 直接绑定在宿主机上，**不需要**在 compose 里逐口映射。
> 好处：GB28181 收流端口池（`rtp_proxy.port_range`）与 WebRTC ICE 候选都直接
> 使用宿主地址，不存在"端口没映射到 → INVITE 成功但收不到流"和
> "候选是容器内网 IP → WebRTC 连不上"两类问题。
>
> ⚠️ Docker Desktop（macOS / Windows）不支持 host 网络，本机开发用叠加文件：
> `docker compose -f docker-compose.yml -f docker-compose.mac.yml up -d`
> （ZLM 切回桥接 + 端口映射，并由后端下发 `rtc_extern_ip`）。

| 端口 | 协议 | 用途 | 说明 |
|------|------|------|------|
| 8080 | TCP | ZLM HTTP API | `[http] port=8080`（`docker/zlm/config.ini`），`zlm.servers[0].http_port` 必须一致 |
| 554 | TCP | ZLM RTSP | `[rtsp] port=554` |
| 443 | TCP | ZLM HTTPS | `[http] sslport=443`。**默认未对外映射**（`docker-compose.mac.yml` 里的 `8443:8443` 映射与本配置不匹配，宿主 8443 实测连不上） |
| 0（关闭） | — | ZLM RTSPS / RTMPS | `[rtsp] sslport=0`、`[rtmp] sslport=0`，**安全协议默认关闭**（compose 里仍留着 `322`/`8443` 的历史映射，属无害残留） |
| 1935 | TCP | ZLM RTMP | `[rtmp] port=1935` |
| 8000 | UDP | ZLM WebRTC（单端口 ICE） | `[rtc] port=8000` |
| 9000 | UDP | ZLM SRT | `[srt] port=9000` |
| 30000–30100 | UDP | ZLM RTP 媒体端口范围（GB28181 流） | `[rtp_proxy] port_range=30000-30100`，与 compose/防火墙必须一致 |

**防火墙**：host 网络下这些端口必须由系统防火墙放行（`firewalld` / `ufw` /
安全组），尤其是 `30000-30100/udp`（设备 RTP 推流）与 `8000/udp`（WebRTC）。

**数据卷**：`docker-compose.yml` 为 ZLM 挂了两个命名卷 ——
`zlmrecord`（`/opt/media/bin/www/record`，云录像/下载产物）与
`zlmsnap`（`/opt/media/bin/www/snap`，截图）。**不要**去掉它们：录像默认写在
容器文件系统里，`docker compose up -d --force-recreate zlm` 或升级镜像会把
已录的 MP4 全部删掉，而数据库 `gb_cloud_record` 里的记录仍在 ——
表现为"录像列表里有条目，点开播放 404/500"。

### 2.4 端口冲突排查

| 端口 | 可能冲突方 | 排查命令 |
|------|----------|---------|
| 18080 | 其他 HTTP 服务 | `lsof -i :18080` / `netstat -ano \| findstr :18080` |
| 5060 | 其他 SIP / PBX（UDP 与 TCP 都要看） | `lsof -i :5060` |
| 5432 | 本地 PostgreSQL | `pg_isready -h 127.0.0.1 -p 5432` |
| 6379 | 本地 Redis | `redis-cli -h 127.0.0.1 -p 6379 ping` |
| 8080 | ZLM 与本机其他 HTTP | 改 `zlm.servers[0].http_port` 并同步 `docker-compose.yml` |

修改任何端口必须同步修改：`config/application.toml` / `docker-compose.yml`（+ `docker-compose.mac.yml`）/
`Dockerfile`（`EXPOSE` / `HEALTHCHECK`）/ `web/vite.config.ts`（dev 代理目标）/ 本文件 §2。

---

## 3. 配置参考

配置文件：`config/application.toml`，可由 `GBSERVER__SECTION__KEY` 环境变量覆盖。

### 3.1 关键配置项

| 路径 | 必选 | 默认 | 说明 |
|------|------|------|------|
| `jwt.secret` | ✅ | 仓库内**演示密钥**（与 `src/security.rs::COMMITTED_DEMO_JWT_SECRET` 相同） | ≥ 32 字符且不能等于该演示值；**生产必须覆盖**（`GBSERVER__JWT__SECRET`），否则启动日志出现 `JWT secret validation failed` |
| `jwt.expiration_minutes` | — | 720 | 普通会话 token 有效期（分钟），需 ≥ 一个工作时段 |
| `jwt.remember_expiration_minutes` | — | 10080 | 勾「7 天免登录」时的有效期，**必须与前端 cookie 的 7 天对齐** |
| `database.url` | ✅ | `sqlite://data/gbserver.db?mode=rwc` | sqlx URL |
| `database.sqlite_max_devices` | — | 500 | SQLite 设备上限；PG/MySQL 忽略 |
| `sip.password` | ✅ | `admin123` | GB28181 SIP digest 密码 |
| `sip.device_id` | ✅ | `34020000002000000001` | 20 位本级 GB-ID |
| `sip.port` / `sip.tcp_port` | — | 5060 / **5060** | UDP / TCP 监听端口，默认同端口（见 §2.1） |
| `sip.username` | — | `gbserver-001` | 级联注册用的 SIP 账号；留空时降级为 `device_id` |
| `zlm[].secret` | ✅ | 代码兜底 `035c73f7-…`，仓库实际配的是另一串 | 必须与 **ZLM `config.ini` 的 `[api] secret`** 一致（当前仓库两者已对齐） |
| `zlm[].hook_url` | ✅ | 仓库默认 `http://host.docker.internal:18080/api/zlm/hook` | 必须是 **ZLM 能访问到的平台地址**（本机裸跑改成 `127.0.0.1`） |
| `redis.url` | — | 仓库默认**已启用** `redis://127.0.0.1:6379` | 设置后启用 Redis StateStore；单实例可注释掉走 InMemory |
| `server.static_dir` | — | `web/dist` | 前端产物目录 |

### 3.2 SIP / GB28181

```toml
[sip]
enabled = true
ip = "0.0.0.0"
port = 5060                 # UDP
tcp_port = 5060             # TCP；默认与 port 相同（国标"同端口+传输方式"接入）
tcp_enabled = true
device_id = "34020000002000000001"
username = "gbserver-001"   # 级联注册账号；留空降级为 device_id
password = "admin123"       # 通过 GBSERVER__SIP__PASSWORD 覆盖
realm = "3402000000"
keepalive_timeout = 30      # 秒
register_timeout = 3600
charset = "UTF-8"
# sdp_ip / stream_ip 用于 NAT 穿透（公网部署时设为公网 IP）
```

### 3.3 ZLMediaKit

> 下列 `[[zlm.servers]]` 是**示例**；仓库实际值见 `config/application.toml`。

```toml
[[zlm.servers]]
id = "zlmediakit-1"
ip = "192.168.3.88"          # ZLM 对后端可达的地址
http_port = 8080             # 与 docker/zlm/config.ini 的 [http] port 一致
# https_port 缺省 None（本仓库未用；config.ini 的 [http] sslport=443 默认未对外映射）
secret = "<与 docker/zlm/config.ini [api] secret 相同>"
# rtc_extern_ip = "127.0.0.1"   # 桥接/容器部署必填；host 网络留空

[zlm]
stream_timeout = 10          # 秒（config/application.toml 实测值；代码默认同）
hook_enabled = true
hook_url = "http://host.docker.internal:18080/api/zlm/hook"
```

`[[zlm.servers]]` 的两个网络相关字段（host 网络部署时按需）：

| 字段 | 作用 | host 网络 | 桥接 / 容器 |
|------|------|-----------|-------------|
| `hook_url` | 平台接收 ZLM 事件回调的地址，**必须是 ZLM 能访问到的地址** | `http://127.0.0.1:18080/api/zlm/hook`（ZLM 在 host 命名空间） | `http://host.docker.internal:18080/api/zlm/hook`（需给 ZLM 加 `extra_hosts`） |
| `rtc_extern_ip` | 下发 ZLM 的 `rtc.externIP`，即浏览器看到的 ICE 候选地址 | 留空（ZLM 直接通告宿主网卡） | **必填**（本机调试 `127.0.0.1`，服务器填公网 IP / 域名） |

`rtc_extern_ip` 也可以在「媒体节点」页保存，后端会在保存与节点上线时下发并回读校验。

**Hook 配置由平台下发，不是手写 `config.ini`。** 后端在节点上线
（`src/zlm/hook.rs`）时下发 `hook.enable`、`hook.admin_params=secret=<zlm secret>`
以及**逐事件**的 `hook.on_*` URL（形如 `…/api/zlm/hook?secret=…`）。
`docker/zlm/config.ini` 里的 `[hook] enable=0` 是只读基线；**该文件没有
`root_url` 这个键**，ZLM 也不认（旧版本文档里的 `root_url=` 写法是错的）。
改完 `config.ini` 记得 `docker compose restart zlm`，但日常不需要手改。

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

> ⚠️ 注意仓库**默认配置里 `[redis] url` 是打开的**（`redis://127.0.0.1:6379`），
> 因此"单机不带 Redis"需要你把该段注释掉，否则启动会尝试连 Redis
> （连不上会回退 InMemory 并打印告警，功能不受影响）。

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
| `GET /api/health` | **存活探针**：只要进程活着就固定返回 200，返回 `{status:"alive"}`；**不查 DB/Redis**（避免 k8s 在 DB 抖动时误杀 Pod） |
| `GET /api/ready` | **就绪探针**：DB 可连（+ 集群模式下至少一个节点在册 / 单节点模式跳过）才 200，否则 503；也检查 Redis（配了才查） |
| `GET /metrics` | Prometheus 文本格式（**12 个指标族**，见 §7.3） |
| `GET /api/server/config` | 脱敏运行时配置（密码遮蔽） |
| `GET /api/server/system/info` | **控制台数据总入口**：CPU/内存/网络/负载/磁盘、服务健康、协议接入配置、`host_ip` 等（`src/handlers/server.rs`） |
| `GET /api/server/system/configInfo` | 控制台配置信息 |
| `GET /api/system/info` | 版本 + 启动时间 + 特性开关 |
| `GET /api/system/stats` | 设备 / 通道 / 流 / 会话 / JT 终端 / cluster 统计 |
| `GET /api/system/version`、`/api/system/online-users` | 版本号 / 在线用户 |

> 区分：`/api/health` 是 liveness，**无条件 200**；`/api/ready` 是 readiness，才做 DB/Redis 依赖检查。实现见 `src/handlers/health.rs`。
> ⚠️ `/api/system/info` 与 `/api/server/system/info` **是两个不同端点**：
> 前者是轻量版本/特性信息，后者是控制台唯一数据源（字段多、每次要真采 CPU/网络/磁盘，
> 单次约 0.26s 起步）。给控制台加面板 = 改后者 + 前端 `web/src/api/log.ts` 的 `SystemInfo`。

### 7.2 日志

默认 `RUST_LOG=info,gbserver=debug`：

```bash
RUST_LOG=info,gbserver=debug cargo run --release
```

`tracing-subscriber` 当前**只启用 `env-filter`**（`Cargo.toml`），`src/main.rs` 用的是
`fmt::layer()` —— 想要 JSON 输出需要先加 `json` feature 并改 main.rs，不要以为已经支持。

### 7.3 关键监控指标

`GET /metrics` 由 `src/metrics.rs::gather()` 手工拼装，当前**共 12 个指标族**
（没有 sqlx pool、没有 HTTP 请求耗时、没有 ZLM hook 指标 —— 下面列表就是全部）：

| 指标 | 类型 | 含义 / 建议阈值 |
|------|------|-----------------|
| `jt1078_missing_retransmit_total` | counter | JT1078 丢包重传次数，持续上涨说明链路差 |
| `jt1078_active_sessions` | gauge | 活跃车载终端会话数 |
| `sip_devices_online` | gauge | 在线国标设备数；骤降即告警 |
| `sip_invites_active` | gauge | 活跃 INVITE 会话数（点播/回放） |
| `streams_active` | gauge | 活跃媒体流数 |
| `gb_cluster_nodes_active` | gauge | 集群节点数；跌至 0 告警 |
| `gb_rpc_messages_total` | counter | 跨节点 RPC 消息总数 |
| `gb_ws_clients_connected` | gauge | WebSocket 连接数 |
| `gb_audit_log_writes_total` | counter | 审计日志写入总数 |
| `gb_audit_log_writes_failed` | counter | 审计日志写入失败数；> 0 需排查 |
| `gb_redis_state_keys` | gauge | Redis 状态键数（`-1` = 未启用 Redis） |
| `gb_build_info` | gauge | `{version="…"} 1`，用于确认在跑哪个版本 |

### 7.4 抓包与诊断

- SIP 抓包：`sngrep` / `Homer`
- ZLM 探活：健康检查循环 `zlm/health_checker.rs::run_health_check_loop`，
  **间隔 `[[zlm.servers]] check_interval_secs`（仓库默认 10s）**、
  单次超时 `timeout_secs`（默认 30s）；连续丢失次数超阈值才判离线
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

**判离线的是周期性健康检查，不是 hook。** `on_server_started` 是 ZLM 启动时**上报自己在线**
（`src/zlm/hook.rs` 置 `online: true`）；真正把节点标离线的是健康检查循环
（`zlm/media_node.rs`，按**连续丢失次数**超阈值置离线）。DB 列名是
`gb_media_server.status`（1=在线 / 0=离线），**没有 `online` 列**。

故障期间播放类接口**不会返回 HTTP 502**，而是 **HTTP 200 + 业务错误码**
（如 `{"code":<非0>,"msg":"Media Server error: …"}`）；显式返回 HTTP 502 的只有
`handlers/server.rs`（节点检测）与 `handlers/device_query.rs` 的少数路径。

```bash
# 重启 ZLM 后：平台会在下一轮健康检查（默认 10s）自动重新下发 hook 配置，
# 通常不需要手工干预。若确实要手工确认，看节点是否恢复在线：
curl -s 'http://127.0.0.1:18080/api/server/media_server/check?id=zlmediakit-1' \
  -H "access-token: $TOKEN"        # → {"code":0,"data":{"reachable":true,"httpPort":8080}}
# 或直接看 ZLM 是否已带 hook 配置（注意 ZLM 收到 setServerConfig 会重写 config.ini！）
curl -s 'http://127.0.0.1:8080/index/api/getServerConfig?secret=YOUR_SECRET' | head -c 400
```

> ⚠️ 不要按旧版本文档手写 `hook.root_url=…` —— ZLM 没有这个键，平台下发的是
> `hook.enable` + `hook.admin_params=secret=…` + 逐事件 `hook.on_*` URL（见 §3.3）。
> 另：`setServerConfig` 会让 ZLM **重写整份 `config.ini`**（注释会丢），
> 因此该文件是只读挂载，运行期配置一律由平台下发。

### 8.4 SIP 服务故障

```bash
ss -ulnp | grep 5060                    # 确认 UDP 5060 监听
ss -tlnp | grep 5060                    # 确认 TCP 5060 监听（默认与 UDP 同端口）
journalctl -u gbserver --since "10 min ago" | grep -i 'sip'
# 检查防火墙：ufw allow 5060/udp && ufw allow 5060/tcp
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
5. 验证：`curl http://localhost:18080/api/health`（存活）；`curl http://localhost:18080/api/ready`（就绪，依赖正常才 200）

### 9.3 数据库迁移

- SQLite → PG：`sqlite3 .dump` 导出后导入 PG
- 配置回滚：通过 `GBSERVER__SECTION__KEY` 环境变量覆盖，无需改文件

### 9.4 集群滚动升级

L6 集群可在 SIP LB 后逐节点重启 —— **但当前不具备真正的无损前提**：
仓库里**没有 dialog 持久化**（invite 会话只存在进程内），且 §5.2 已列出
"PendingRequest 跨节点路由"与"SubscriptionLifecycle 分布式续期"两项未实现。
因此滚动重启期间**该节点上的点播/回放会话会中断**，设备需要重新 INVITE；
升级窗口请安排在低峰期。

---

## 10. 常见问题

| 现象 | 可能原因 | 处理 |
|------|---------|------|
| 启动报 `JWT secret validation failed` | 用的是仓库内置演示密钥 | `export GBSERVER__JWT__SECRET=$(openssl rand -hex 32)`（**不覆盖也会启动，只是告警**） |
| 设备显示离线 | SIP 端口被防火墙拦截 | `ufw allow 5060/udp && ufw allow 5060/tcp`，检查 `sip.ip` |
| 点播接口 `code != 0` 且 `msg` 含 `Media Server error` | ZLM 不可达 | 检查 `zlm[*].ip` / `http_port` 与网络 ACL（注意播放类接口是 **HTTP 200 + 业务错误码**，不是 502） |
| 云录像不出现 | ZLM `on_record_mp4` hook 未 POST | 确认 `hook_url` 是 ZLM 能访问到的 `/api/zlm/hook`，且 `zlm.servers[0].secret` == `docker/zlm/config.ini` 的 `[api] secret` |
| `docker compose up -d` 报 `failed to read dockerfile` | 仓库根目录缺 `Dockerfile` / `.dockerignore` | 确认文件存在 |
| ZLM 鉴权失败（hook 收不到） | ZLM secret 与平台配置不一致 | 以 **`docker/zlm/config.ini` 的 `[api] secret`** 为准，把它同步到 `config/application.toml` 的 `zlm.servers[0].secret`。⚠️ compose 里的 `ZLM_HTTP_SECRET` **当前与二者不一致，且该镜像并不消费它**，不要照它排查 |
| 前端 9528 代理到 18080 失败 | 后端未启动 | `curl http://localhost:18080/api/health` 验证 |
| SIP 设备注册不上 | 防火墙 / 设备 SIP 地址 / 密码不一致 | 1) 防火墙放通 **5060/udp 与 5060/tcp**（`tcp_port` 默认 = `port`，**不是 5061**）；2) 设备 SIP 地址填宿主机 IP；3) `sip.password` 与设备端一致（默认 `admin123`，生产请改） |
| 真机点播成功但 FLV 拉不到字节 | 设备按自己 SDP 端口推流且 ZLM 收不到 | 先看日志有无 `on_publish` / `on_stream_changed register=true`；TCP 设备优先用 `TCP-PASSIVE`（见 `OPEN_ISSUES.md` B1） |
| 本机裸跑后端时 ZLM 调用偶发 `error sending request` | 系统 HTTP 代理把宿主 IP（如 `192.168.3.88`）也代理了 | 给后端进程设 `NO_PROXY=localhost,127.0.0.1,::1,<ZLM 宿主 IP>` |
| MySQL 替换 PostgreSQL | feature + URL 不一致 | `cargo build --release --no-default-features --features mysql` 并改 `database.url` 为 `mysql://...` |

---

## 附录 A：项目结构

```
GBServer/
├── Cargo.toml              # Rust 包定义（包名 gbserver；sqlite 为默认 feature）
├── Cargo.lock
├── Dockerfile              # 多阶段构建（前端 + 后端 → slim 运行时）
├── docker-compose.yml      # 一键拉起 pg + redis + zlm + gbserver（ZLM 用 host 网络）
├── docker-compose.mac.yml  # macOS/Windows 叠加（ZLM 切回桥接 + 端口映射）
├── docker-compose.sqlite.yml
├── LICENSE                 # MIT License
├── README.md
├── AGENTS.md / CLAUDE.md   # 给自动化 agent 的工程约定
├── config/
│   └── application.toml    # 默认配置
├── database/
│   ├── init-sqlite-2.7.4.sql
│   ├── init-postgresql-2.7.4.sql
│   └── init-mysql-2.7.4.sql
├── docker/
│   └── zlm/config.ini      # ZLM 只读基线配置（hook 由平台运行期下发）
├── docs/
│   ├── STATUS.md           # 当前状态（能力矩阵 / 设计决策 / 真机核验）
│   ├── OPEN_ISSUES.md      # 唯一待办表
│   ├── DEPLOYMENT_GUIDE.md # ← 本文档
│   ├── DB_DIALECT_NOTES.md # 多方言 SQL 注意事项
│   └── STUB_COMPAT_PLAN.md # stub.rs / device_stub.rs 定位评估
├── e2e/                    # Playwright 端到端测试（17 个 spec）
├── mock/                   # Python 模拟器（SIP 设备 / JT1078 终端 / 级联）
├── scripts/                # 构建 / 运行脚本（bash + PowerShell + dialect_smoke.py）
├── src/                    # Rust 后端
│   ├── lib.rs / main.rs
│   ├── config.rs / auth.rs / router.rs / metrics.rs / security.rs / dyn_where.rs
│   ├── state_store.rs / rpc.rs / archive.rs / logging.rs / serde_flex.rs / test_support.rs
│   ├── handlers/           # HTTP 业务接口（含 authz.rs 统一管理员判定）
│   ├── db/                 # SQLx 持久化（按表拆分）
│   ├── sip/                # GB28181 SIP 协议栈（core / transport / gb28181）
│   ├── zlm/                # ZLMediaKit 客户端 / Hook
│   ├── jt1078/             # JT/T 808+1078 车辆部标
│   ├── cascade/            # 上级平台 SIP REGISTER
│   ├── scheduler/          # 录像计划调度
│   ├── cluster/            # 集群节点发现
│   ├── state/              # 跨节点状态仓储
│   ├── ws/                 # WebSocket
│   └── middleware/         # audit 等
├── tests/                  # 集成测试
├── web/                    # Vue 3 + Element Plus + Vite 前端
│   ├── vite.config.ts      # dev server :9528 + /dev-api 反代 :18080
│   └── src/{api,views,components,store}/  # API 封装 / 页面 / 通用组件 / Pinia
└── justfile
```

## 附录 B：相关源码索引

| 模块 | 关键文件 |
|------|---------|
| 启动入口 / 主流程 | `src/lib.rs::run()`、`src/main.rs` |
| 配置加载 | `src/config.rs` |
| 路由中心 | `src/router.rs` |
| 鉴权（JWT / API Key） | `src/auth.rs`、`src/handlers/authz.rs`、`src/ws/jwt.rs` |
| 设备 / 通道 | `src/handlers/{device,device_query,device_control,common_channel}.rs`、`src/db/{device,common_channel}.rs` |
| SIP / GB28181 | `src/sip/{core,transport,gb28181}/`、`src/sip/server.rs` |
| ZLMediaKit | `src/zlm/{client,hook,hook_routes,media_node,health_checker}.rs` |
| JT1078 | `src/jt1078/{manager,server,command,command_waiter,session,jt_media_session,response_parser}.rs` |
| 跨实例状态 | `src/state_store.rs`、`src/state/{repository,stream_status}.rs`、`src/cluster/registry.rs`、`src/rpc.rs` |
| 录像 | `src/handlers/{playback,cloud_record_extra}.rs`、`src/db/cloud_record.rs`、`src/scheduler/record_plan.rs` |
| 推流 / 拉流代理 | `src/handlers/{stream,play}.rs`、`src/db/{stream_push,stream_proxy}.rs` |
| 级联 | `src/cascade/register.rs`、`src/sip/gb28181/{cascade,cascade_forward}.rs` |
| 多方言 SQL | `src/dyn_where.rs`、`src/db/read_smoke.rs` |
| 监控 / 健康 / 审计 | `src/handlers/{health,metrics,system,server}.rs`、`src/middleware/audit.rs`、`src/metrics.rs` |
| WebSocket | `src/ws/{hub,jwt}.rs`、`src/handlers/websocket.rs` |

---

**最后更新：2026-09-19（与 `config/application.toml`、`docker-compose*.yml`、
`docker/zlm/config.ini`、`src/metrics.rs` 逐项核对）。**
任何端口 / 配置 / 指标变更请同步更新本文件。
