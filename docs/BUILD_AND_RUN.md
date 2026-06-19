# GBServer 构建与运行指南

> 本文档统一收录 **构建、运行、Docker 部署、端口矩阵**，确保所有路径下端口号一致。

---

## 1. 端口矩阵（Port Matrix）

> ⚠️ **以本表为唯一权威源**。任何代码、配置、文档出现与本表不符的端口，请以本表为准并修复。

### 1.1 核心服务端口

| 端口 | 协议 | 服务 | 用途 | 配置项 |
|-----|------|------|------|-------|
| **18080** | TCP | GBServer | HTTP API + 静态前端 | `server.port` / `GBSERVER__SERVER__PORT` |
| **5060** | UDP | GBServer | GB28181 SIP 信令（UDP） | `sip.port` |
| **5061** | TCP | GBServer | GB28181 SIP 信令（TCP） | `sip.tcp_port` |

### 1.2 数据库 / 缓存

| 端口 | 协议 | 服务 | 用途 |
|-----|------|------|------|
| **5432** | TCP | PostgreSQL | 主数据库（默认 feature） |
| **3306** | TCP | MySQL | 备选数据库（`--features mysql`） |
| **6379** | TCP | Redis | 缓存 / Stream 计数 |

### 1.3 ZLMediaKit（流媒体）

| 端口 | 协议 | 用途 |
|-----|------|------|
| **8080** | TCP | ZLM HTTP API（与 `config/application.toml` 的 `zlm.servers[0].http_port` 对应） |
| **8443** | TCP | ZLM HTTPS |
| **554** | TCP | ZLM RTSP |
| **322** | TCP | ZLM RTSPS |
| **1935** | TCP | ZLM RTMP |
| **8000** | UDP | ZLM WebRTC |
| **9000** | UDP | ZLM SRT |
| **30000–30100** | UDP | ZLM RTP 媒体端口范围（GB28181 流） |

### 1.4 前端开发服务器

| 端口 | 协议 | 用途 |
|-----|------|------|
| **9528** | TCP | Vue dev server（`npm run dev`） |

dev server 通过 `web/vue.config.js` 的 `proxy` 把 `/dev-api` 和 `/static/snap` 反代到 `http://127.0.0.1:18080`。

---

## 2. 目录结构

```
GBServer/
├── Cargo.toml              # Rust 包定义（包名 gbserver，二进制名 gbserver）
├── Cargo.lock
├── Dockerfile              # 多阶段构建（前端 + 后端 → slim 运行时）
├── docker-compose.yml      # 一键拉起 pg + redis + zlm + gbserver
├── .dockerignore
├── config/
│   └── application.toml    # 默认配置（含全部端口号）
├── database/
│   └── init-postgresql-2.7.4.sql
├── web/                    # Vue 2 + Element UI 前端
│   ├── vue.config.js       # dev server 端口 9528 + 反代 18080
│   └── package.json
├── scripts/
│   ├── build.sh / build.ps1          # 仅编译
│   ├── run.sh   / run.ps1            # 仅运行
│   ├── build-and-run.sh / build-and-run.ps1  # 编译+运行
│   ├── run-backend-tests.sh          # 后端集成测试
│   ├── verify-backend-tests.sh       # 测试结果校验
│   ├── init-db-postgres.ps1          # PostgreSQL 初始化（Windows）
│   └── api-integration-test.js       # API 烟雾测试（需服务运行在 18080）
└── docs/
    ├── BUILD_AND_RUN.md     # ← 本文档
    └── OPERATIONS.md
```

---

## 3. 平台构建速查表

| 场景 | Linux / macOS | Windows (PowerShell) |
|-----|--------------|----------------------|
| 仅编译前端 + 后端 | `./scripts/build.sh` | `.\scripts\build.ps1` |
| 仅编译后端 | `./scripts/build.sh --skip-frontend` | — |
| 仅编译前端 | `./scripts/build.sh --skip-backend` | — |
| 仅运行（已编译） | `./scripts/run.sh` | `.\scripts\run.ps1` |
| 一键编译 + 运行 | `./scripts/build-and-run.sh` | `.\scripts\build-and-run.ps1` |
| Docker 一键拉起 | `docker compose up -d --build` | `docker compose up -d --build` |
| API 烟雾测试 | `BASE_URL=http://localhost:18080 node scripts/api-integration-test.js` | 同上 |

---

## 4. 本地原生构建（Linux / macOS）

### 4.1 前置依赖

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js（推荐 18.x；Vue 2 + sass-loader 8 在 20+ 上有兼容性问题）
# 推荐用 nvm：nvm install 18 && nvm use 18

# 数据库二选一
docker compose up -d postgres redis    # PostgreSQL（默认）
# 或自建 MySQL：需用 --features mysql 重新编译
```

### 4.2 数据库初始化

启动 postgres 容器时已通过 `docker-entrypoint-initdb.d/init.sql` 自动建表，无需手动执行。

如使用裸 PostgreSQL：

```bash
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql
```

### 4.3 构建

```bash
./scripts/build.sh                       # 前后端都编译
# 或分步
( cd web && npm install && npm run build:prod )
cargo build --release
```

产物：
- 前端：`web/dist/`
- 后端：`target/release/gbserver`

### 4.4 运行

```bash
./scripts/run.sh
# 或直接
./target/release/gbserver
```

监听端口：
- `18080` — HTTP API + 前端
- `5060/udp` — SIP
- `5061` — SIP/TCP

访问 `http://localhost:18080` 即可进入前端（默认账号 `admin / admin`，**首次登录后请立即修改**）。

### 4.5 环境变量覆盖（可选）

所有 `config/application.toml` 字段都可通过 `GBSERVER__SECTION__KEY` 形式覆盖（双下划线分隔）：

```bash
GBSERVER__SERVER__PORT=18080 \
GBSERVER__DATABASE__URL='postgres://postgres:postgrespw@127.0.0.1:5432/gbserver' \
GBSERVER__REDIS__URL='redis://127.0.0.1:6379' \
GBSERVER__JWT__SECRET='your-strong-secret' \
GBSERVER__SIP__PASSWORD='your-sip-password' \
./target/release/gbserver
```

---

## 5. 本地原生构建（Windows）

```powershell
# 安装 Rust：https://rustup.rs
# 安装 Node.js 18.x LTS

.\scripts\build.ps1           # 编译
.\scripts\run.ps1             # 运行
# 或一键
.\scripts\build-and-run.ps1
```

可执行文件：`target\release\gbserver.exe`

---

## 6. Docker 部署（推荐生产）

### 6.1 一键启动

```bash
docker compose up -d --build
```

首次构建约 10–20 分钟（拉镜像 + npm install + cargo 全量编译）。构建产物：

| 服务 | 容器名 | 镜像 | 暴露端口 |
|-----|--------|------|---------|
| gbserver | `gbserver` | 本地构建（多阶段） | 18080, 5060/udp, 5061 |
| postgres | `gbserver-postgres` | `postgres:16` | 5432 |
| redis | `gbserver-redis` | `redis:7-alpine` | 6379 |
| zlm | `gbserver-zlm` | `zlmediakit/zlmediakit:master` | 8080, 8443, 554, 322, 1935, 8000/udp, 9000/udp, 30000-30100/udp |

### 6.2 健康检查

```bash
docker compose ps                  # STATUS 应显示 (healthy)
curl http://localhost:18080/api/health
```

### 6.3 常用操作

```bash
docker compose logs -f gbserver    # 实时日志
docker compose restart gbserver    # 仅重启后端
docker compose down                # 停止（保留 volume）
docker compose down -v             # 停止并清空数据卷
```

### 6.4 通过 `.env` 自定义密钥

在仓库根目录创建 `.env`：

```env
GBSERVER_JWT_SECRET=<your-strong-jwt-secret>
GBSERVER_SIP_PASSWORD=<your-sip-password>
```

`docker-compose.yml` 已支持 `${GBSERVER_JWT_SECRET:-default}` 语法，会自动读取 `.env`。

---

## 7. 开发模式（前后端热更新）

```bash
./scripts/build-and-run.sh        # Linux/macOS
# 或
.\scripts\build-and-run.ps1       # Windows
```

该模式下：

- 前端：Vue dev server `http://localhost:9528`，HMR 热更新，`/dev-api` 自动反代到后端 `18080`
- 后端：`cargo run`，源码修改后自动重启

---

## 8. 测试

```bash
# 单元测试 + 集成测试
cargo test

# 后端集成测试脚本（需要 pg + redis 运行）
./scripts/run-backend-tests.sh
./scripts/verify-backend-tests.sh

# API 烟雾测试（需要 gbserver 启动在 18080）
docker compose up -d postgres redis
cargo run --release &
sleep 5
BASE_URL=http://localhost:18080 node scripts/api-integration-test.js
```

---

## 9. 端口冲突排查清单

如果启动时出现端口占用，按本表逐项排查：

| 端口 | 可能冲突方 | 排查命令 |
|-----|----------|---------|
| 18080 | 其他 HTTP 服务、Jenkins 等 | `lsof -i :18080` / `netstat -ano \| findstr :18080` |
| 5060 | 其他 SIP 服务、PBX | `lsof -i :5060` / `netstat -an \| findstr :5060` |
| 5061 | 其他 SIP/TCP | 同上 |
| 5432 | 本地 PostgreSQL | `pg_isready -h 127.0.0.1 -p 5432` |
| 6379 | 本地 Redis | `redis-cli -h 127.0.0.1 -p 6379 ping` |
| 8080 | ZLM 与本机其他 HTTP（如 IDEA）冲突 | 改 `zlm.servers[0].http_port` 并同步 `docker-compose.yml` |
| 554 | 极少见（系统 RTSP） | `lsof -i :554` |

如需修改任何端口，**必须同步修改**：

1. `config/application.toml`
2. `docker-compose.yml`（含 `GBSERVER__SERVER__PORT` 等 env）
3. `Dockerfile`（`EXPOSE` 与 `HEALTHCHECK`）
4. `docs/OPERATIONS.md` 中相关章节
5. `web/vue.config.js`（如修改后端 18080）
6. `docs/BUILD_AND_RUN.md`（本文件 §1 端口矩阵）

---

## 10. 常见问题

**Q1：`docker compose up -d` 报 "failed to read dockerfile"**
A：检查仓库根目录是否存在 `Dockerfile` 与 `.dockerignore`。

**Q2：ZLM 鉴权失败（hook 收不到）**
A：确认 `docker-compose.yml` 中 `zlm.ZLM_HTTP_SECRET` 与 `config/application.toml` 的 `zlm.servers[0].secret` 完全一致。

**Q3：前端 9528 端口代理到后端 18080 失败**
A：确认后端已启动并监听 18080。可临时用 `curl http://localhost:18080/api/health` 验证。

**Q4：SIP 设备注册不上**
A：
1. 防火墙放通 UDP 5060 / TCP 5061
2. 设备的 SIP 服务器地址填写宿主机或映射后的 IP（非 localhost）
3. `config/application.toml` 中 `sip.password` 与设备端配置一致（默认 `admin123`，生产请修改）

**Q5：MySQL 想替换默认 PostgreSQL**
A：
```bash
docker compose down
cargo build --release --no-default-features --features mysql
# 同时把 config/application.toml 的 database.url 改为 mysql://... 并改 docker-compose 用 mysql 镜像
```

---

**最后更新：与仓库 `config/application.toml`、`docker-compose.yml`、`Dockerfile` 同步。**
任何端口变更请同步更新本文件 §1 端口矩阵。