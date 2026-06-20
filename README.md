<div align="center">

# GBServer

### GB28181 国标信令 · 流媒体接入 · 级联管理平台（Rust 重写版）

面向 **GB/T 28181-2016** 国标协议、JT1078 车辆终端协议的 **流媒体接入与级联管理平台**。

Rust 全异步后端（Axum + SQLx + Tokio），保留 Vue 2 + Element UI 前端（位于 `web/`），
目标是提供比传统 Java 实现更高的单机并发、更低的资源占用与更现代的工程实践。

[特性](#-核心特性) · [快速开始](#-快速开始) · [文档索引](#-文档索引) · [部署分级](#-部署分级) · [API 概览](#-api-概览)

</div>

---

## 📖 项目简介

GBServer 是面向 **GB/T 28181-2016** 国标协议的流媒体接入与级联管理平台的 Rust 重写实现，
目标是在保持 Java 原版前端 100% 兼容的前提下，提供：

- **更高的单机吞吐**：基于 Tokio + Axum 异步运行时，IO 密集场景下资源占用显著下降。
- **更现代的工程实践**：单一二进制部署、零依赖启动（SQLite 默认）、Cargo feature 切换数据库。
- **数据库三选一**：SQLite（默认） / PostgreSQL / MySQL，编译期通过 `cargo` feature 切换。
- **可观测性**：内置 `/metrics` Prometheus 端点、tracing 结构化日志。
- **协议扩展**：在 GB28181 基础上原生支持 JT1078 车载终端协议、重传检测 Hook。

> 🔍 想了解当前接口实现状态与待补齐项？见 `src/handlers/stub.rs`、`src/handlers/device_stub.rs` 与 `src/router.rs` 的占位标记。

---

## ✨ 核心特性

| 领域 | 能力 |
|------|------|
| **国标信令** | SIP 注册、心跳、目录订阅、Invite/Bye/Info、SDP 协商、SSRC 分配、NAT 地址改写、PTZ 控制 |
| **流媒体** | ZLMediaKit 集成、Hook 接收、节点健康检查、自动选流（最少负载优先 + Redis 计数回退） |
| **推流/代理** | GB28181 推流、拉流代理（FFmpeg 命令）启停控制 |
| **级联平台** | 上级平台 SIP REGISTER 保活、设备/通道同步 |
| **录像计划** | 后台调度器、计划与通道关联、定时录像触发 |
| **JT1078** | 车辆终端 UDP 服务、帧解析、会话状态、序列号重传检测、可选 Webhook 通知 |
| **鉴权** | JWT（HS256，请求头 `access-token`）+ API Key（`X-API-Key` / `apiKey`），审计日志异步落库 |
| **缓存** | 可选 Redis（`cache` / `broker`），运行时按配置自动启用 |
| **可观测** | Prometheus `/metrics`、tracing JSON 日志、健康检查 `/health` |

---

## 🛠️ 技术栈

| 层 | 选型 | 版本 |
|----|------|------|
| Web 框架 | Axum + Tower | 0.7 / 0.4 |
| 异步运行时 | Tokio | 1.x（full） |
| 数据库 | SQLx | 0.7（runtime-tokio） |
| 序列化 | serde / serde_json | 1.x |
| 鉴权 | jsonwebtoken | 9.x |
| 配置 | config + TOML + 环境变量 | 0.14 |
| 日志 | tracing / tracing-subscriber | 0.1 / 0.3 |
| HTTP 客户端 | reqwest（rustls-tls） | 0.11 |
| 缓存 | redis | 0.25 |
| SIP/GB28181 | quick-xml + 自研 SIP 协议栈 | 0.31 |
| 前端 | Vue 2 + Element UI + Vue CLI 4 | 2.6 / 2.15 / 4.4 |

---

## 🏗️ 系统架构

```
                    ┌────────────────────────────────────┐
                    │          前端 (Vue 2 SPA)          │
                    │  web/dist  ←  webpack build        │
                    └──────────────┬─────────────────────┘
                                   │ HTTP / WS
                    ┌──────────────▼─────────────────────┐
                    │         Axum 路由 (router.rs)      │
                    │  /api/* 鉴权中间件   /metrics /health
                    └──┬─────────┬─────────┬─────────┬───┘
                       │         │         │         │
        ┌──────────────▼┐ ┌─────▼─────┐ ┌──▼──────┐ ┌▼────────┐
        │   handlers/   │ │   sip/    │ │  zlm/   │ │ jt1078/ │
        │  业务接口     │ │ GB28181   │ │ ZLM 客户端│ │ 车载终端 │
        └──────┬───────┘ └─────┬─────┘ └────┬────┘ └────┬────┘
               │              │            │            │
        ┌──────▼──────────────▼────────────▼────────────▼───┐
        │              db/  ──  SQLx  ──  Pool              │
        │       (SQLite ⏐ PostgreSQL ⏐ MySQL, feature 切换) │
        └────────────────────────────────────────────────────┘
                                   │
                    ┌──────────────▼─────────────────────┐
                    │  后台循环：SIP / 级联 / 录像计划 / JT │
                    └────────────────────────────────────┘
```

**关键模块**（详见 [`docs/ARCHITECTURE.md`](docs/) 章节 `CLAUDE.md`）：

- `handlers/` — 业务 HTTP 接口，薄层调用 `db/` 与协议模块。
- `sip/core/` + `sip/transport/` + `sip/gb28181/` — SIP 协议栈、UDP/TCP 传输、应用层逻辑。
- `zlm/` — ZLMediaKit HTTP 客户端、Hook 接收、健康检查。
- `cascade/` — 上级平台 SIP REGISTER 保活。
- `scheduler/` — 录像计划后台调度。
- `jt1078/` — JT1078 车辆终端协议与会话。

---

## 📊 部署分级

> 完整说明与硬件建议见 [`docs/DEPLOYMENT_GUIDE.md`](docs/DEPLOYMENT_GUIDE.md)。

| 级别 | 设备数 | 并发流 | 数据库 | Redis | 形态 |
|------|--------|--------|--------|-------|------|
| **L1 演示/开发** | < 50 | < 10 | SQLite | 无 | 单机 |
| **L2 边缘节点** | < 200 | < 20 | SQLite | 无 | 单机 |
| **L3 小规模生产** | < 500 | < 50 | SQLite / PG | 可选 | 单机 |
| **L4 中等生产** | 500 – 2000 | 50 – 200 | PostgreSQL | 可选 | 单机 |
| **L5 大规模生产** | 2000 – 5000 | 200 – 500 | PG + Patroni | 是 | 单机 + 主备 |
| **L6 HA 集群** | > 2000 | > 200 | PG + Patroni | **必选** | 多实例 + SIP LB |
| **L8 MySQL 平迁** | 任意 | 任意 | MySQL | 可选 | 单/多 |

---

## 🚀 快速开始

### 0. 准备环境

| 依赖 | 用途 | 必选 | 安装 |
|------|------|------|------|
| **Rust** 1.70+ | 编译后端 | 构建时 | <https://rustup.rs/> |
| **Node.js** 14+（含 npm） | 构建前端 | 构建时 | <https://nodejs.org/> |
| **SQLite** | 默认数据库 | ✅ 运行时 | **无需安装**，随 Rust crate `rusqlite` 内置 |
| **PostgreSQL** 12+ | 生产数据库 | 二选一 | <https://www.postgresql.org/download/> |
| **MySQL** 5.7+ / 8.x | MySQL 平迁 / 兼容历史部署 | 二选一 | <https://dev.mysql.com/downloads/mysql/> |
| **Redis** 6.x / 7.x | 缓存 | 可选 | 预留接口，当前可关闭 |

### 1. 选择数据库（SQLite 默认开箱即用）

| 后端 | 启动命令 | 适用场景 |
|------|----------|----------|
| **SQLite** ✅ | `cargo run` | 开发 / 演示 / 边缘 / 小规模生产（≤ 500 设备） |
| PostgreSQL | `cargo run --no-default-features --features postgres` | 生产主力 / 多实例 / Patroni 集群 |
| MySQL | `cargo run --no-default-features --features mysql` | MySQL 平迁，schema 与 `database/init-mysql-2.7.4.sql` 完全兼容 |

> 📘 三种后端的对比、迁移路径与限制详见 [`docs/DATABASE_COMPATIBILITY.md`](docs/DATABASE_COMPATIBILITY.md) 与 [`database/README.md`](database/README.md)。

### 2. 初始化数据库

```bash
# SQLite（默认，无需任何操作；首次启动时自动创建 ./data/gbserver.db 并执行 init-sqlite-2.7.4.sql）

# PostgreSQL
createdb gbserver
psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql

# MySQL
mysql -uroot -p -e "CREATE DATABASE gbserver DEFAULT CHARACTER SET utf8mb4;"
mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql
```

> 默认管理员：`admin` / `admin`（密码以 MD5 存储）。

### 3. 配置

复制并按需修改 `config/application.toml`：

```toml
server:
  port: 18080

database:
  url: "sqlite://data/gbserver.db?mode=rwc"   # 或 postgres://… / mysql://…
  sqlite_max_devices: 500                     # SQLite 设备上限

jwt:
  secret: "请改为随机长字符串"
  expiration_minutes: 30

# 前端构建产物目录（可选；不配置则仅提供 API）
static_dir: "web/dist"
```

> 🌐 可通过环境变量覆盖，命名规则：`GBSERVER__SECTION__KEY`（双下划线分隔）。
> 示例：`GBSERVER__SERVER__PORT=18080`、`GBSERVER__DATABASE__URL=postgres://...`。

### 4. 构建并运行

**Windows（PowerShell，在仓库根目录执行）**

```powershell
# 一键：构建前后端 + 启动
.\scripts\build-and-run.ps1

# 仅启动（已构建过）
.\scripts\run.ps1
```

**Linux / macOS（bash）**

```bash
# 1) 构建前端（产物 -> web/dist）
cd web && npm install && npm run build:prod && cd ..

# 2) 构建后端（产物 -> target/release/）
cargo build --release
# 切换 MySQL：cargo build --release --no-default-features --features mysql
# 切换 PG  ：cargo build --release --no-default-features --features postgres

# 3) 启动（必须在仓库根目录，以便正确加载 config 与 web/dist）
cargo run --release
```

> ⚠️ **运行目录**：必须在 GBServer 仓库根目录启动后端，配置路径与 `web/dist` 才能正确解析。

服务默认监听 `http://0.0.0.0:18080`。

### 5. 一键 Docker 启动（仅 PostgreSQL + Redis）

```bash
docker compose up -d          # PostgreSQL 16 + Redis 7
docker compose ps
docker compose down           # 保留数据卷；加 -v 彻底清空
```

MySQL 通过 profile 启动：`docker compose --profile mysql up -d`。

---

## ⚙️ 配置说明

| 配置段 | 关键字段 | 默认值 | 说明 |
|--------|----------|--------|------|
| `server` | `port` | `18080` | HTTP 监听端口 |
| `database` | `url` | `sqlite://data/gbserver.db?mode=rwc` | SQLx 连接串 |
| `database` | `sqlite_max_devices` | `500` | SQLite 设备上限，超出请迁移到 PG |
| `jwt` | `secret` | 占位 | **生产环境必改**为 32+ 位随机字符串 |
| `jwt` | `expiration_minutes` | `30` | Token 有效期 |
| `sip` | `enabled` | `true` | 是否启动 SIP 服务 |
| `sip` | `port` | `5060` | SIP UDP 端口 |
| `sip` | `tcp_port` | `5061` | SIP TCP 端口 |
| `sip` | `sdp_ip` / `stream_ip` | 动态 | SDP/流地址改写源 |
| `jt1078` | `enabled` | `true` | 是否启用 JT1078 服务 |
| `jt1078` | `timeout_ms` | `60000` | 重传检测超时 |
| `jt1078` | `retransmit_wait_ms` | `200` | 重传等待窗口 |
| `jt1078` | `retransmit_hook_url` | — | 缺序上报 webhook（POST JSON） |
| `zlm` | — | — | ZLMediaKit 节点列表，详见 `config/application.toml` 注释 |

> 📌 端口矩阵以 [`docs/BUILD_AND_RUN.md` §1](docs/BUILD_AND_RUN.md) 为唯一权威源。

---

## 📚 文档索引

| 文档 | 用途 |
|------|------|
| [docs/BUILD_AND_RUN.md](docs/BUILD_AND_RUN.md) | 构建、运行、Docker 部署、端口矩阵 |
| [docs/DATABASE_COMPATIBILITY.md](docs/DATABASE_COMPATIBILITY.md) | 三种数据库后端对比与迁移 |
| [docs/DEPLOYMENT_GUIDE.md](docs/DEPLOYMENT_GUIDE.md) | 部署分级、拓扑与硬件建议 |
| [docs/OPERATIONS.md](docs/OPERATIONS.md) | 日常运维、备份、监控、故障排查 |
| [database/README.md](database/README.md) | 初始化脚本说明 |
| [web/README.md](web/README.md) / [README-zh.md](web/README-zh.md) | 前端子项目说明 |

---

## 🌐 API 概览

> 响应格式与 Java 版一致：`{ "code": 0, "msg": "成功", "data": ... }`。
> 鉴权请求头：`access-token`（JWT）或 `X-API-Key` / `apiKey`（API Key）。

| 域 | 主要端点 |
|----|----------|
| **用户** | 登录/登出、userInfo、users 分页、增删改密、changePushKey |
| **设备** | `GET /api/device/query/devices`（分页）、`/devices/:id/channels`（分页） |
| **流媒体服务器** | list、online/list、one/:id、system/configInfo、system/info、map/config、resource/info |
| **推流** | list（分页）、add/update/remove/start、batchRemove、save_to_gb / remove_form_gb（写操作部分为占位） |
| **拉流代理** | list（分页）、ffmpeg_cmd/list、add/update/save/start/stop/delete |
| **级联平台** | query（分页）、server_config、channel/list、channel/push、add/update/delete、exit/:id |
| **实时播放** | play/start、stop、broadcast、broadcast/stop（拉流需 ZLM/SIP） |
| **区域/分组** | region/tree/list、add、update、delete、path、tree/query；group 同上 |
| **角色** | `GET /api/role/all` |
| **回放/录像** | playback/*、gb_record/query、download/*、cloud/record/*、record/plan/* |
| **占位接口** | device/sync_status、device/delete、subscribe/catalog、media_server/check、record/check 等 |

完整列表与占位标记参见 `src/router.rs` 与 `src/handlers/stub.rs`、`device_stub.rs`。

### 接口联调测试

后端启动且数据库就绪后，可通过以下方式联调测试：

- **单元 / 集成测试**：`cargo test`（详见 [测试](#-测试) 章节）。
- **API 端到端冒烟**：使用 `curl` 调通主要路由（登录、用户信息、设备列表、流媒体服务器列表、推流列表、级联平台、区域/分组、回放、云录像等），可参考 [`web/src/api/`](web/src/api/) 中前端已封装的所有端点。
- **Postman / Apifox**：导入 [`docs/OPERATIONS.md`](docs/OPERATIONS.md) 中"接口总览"部分整理的路由。

---

## 👨‍💻 开发指南

### 开发时前后端分离

```bash
# 终端 A：后端（仓库根目录）
cargo run

# 终端 B：前端（web 目录，代理到 18080）
cd web && npm run dev
# 浏览器打开 http://localhost:9528
```

代理规则见 `web/vue.config.js`：`/dev-api` → `http://127.0.0.1:18080`。

### 代码规范

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

### 调试开关

通过环境变量 `RUST_LOG` 控制 tracing 输出，例如：

```bash
RUST_LOG=gbserver=debug,sqlx=warn cargo run
```

---

## 🧪 测试

```bash
# 全部
cargo test

# 聚焦某个测试
cargo test <test_name>
cargo test --lib <test_name>

# 集成测试
cargo test --test integration_test
cargo test --test jt1078_integration

# 三库矩阵（SQLite / PostgreSQL / MySQL）
cargo test --lib                                                  # SQLite（默认）
cargo test --no-default-features --features postgres --lib       # PostgreSQL
cargo test --no-default-features --features mysql --lib           # MySQL
```

CI 矩阵在 [`.github/workflows/db-features.yml`](.github/workflows/db-features.yml) 中对 SQLite / PostgreSQL / MySQL 三个 feature 分别跑回归。

---

## 🗂️ 项目结构

```
GBServer/
├── src/                      # Rust 后端源码
│   ├── lib.rs / main.rs      # 启动入口与 run() 主流程
│   ├── config.rs             # TOML + 环境变量配置加载
│   ├── router.rs             # Axum 路由中心
│   ├── auth.rs               # JWT / API Key 鉴权
│   ├── handlers/             # 业务 HTTP 接口（薄层）
│   ├── db/                   # 按表/域拆分的 SQLx 持久化
│   ├── sip/                  # GB28181 SIP 协议栈
│   ├── zlm/                  # ZLMediaKit 客户端 / Hook
│   ├── jt1078/               # JT1078 车辆终端
│   ├── cascade/              # 上级平台 SIP REGISTER
│   └── scheduler/            # 录像计划后台调度
├── web/                      # 前端（Vue 2 + Element UI）
│   ├── src/api/              # 业务 API 封装（与后端端点一一对应）
│   ├── src/views/            # 业务页面
│   └── dist/                 # 构建产物（运行时由后端 serve）
├── database/                 # 三种数据库的初始化脚本
│   ├── init-sqlite-2.7.4.sql
│   ├── init-postgresql-2.7.4.sql
│   └── init-mysql-2.7.4.sql
├── config/
│   └── application.toml      # 默认配置（含详细注释）
├── docs/                     # 工程文档（部署、测试、运维…）
├── scripts/                  # 构建/运行/测试脚本（PowerShell + bash）
├── tests/                    # 集成测试
├── Dockerfile                # 后端容器镜像
├── docker-compose.yml        # PG + Redis 一键启动
├── docker-compose.sqlite.yml # SQLite + 后端一键启动
├── Cargo.toml                # Rust 工作区与 feature 切换
└── justfile                  # 常用任务快捷方式
```

---

## 🛣️ 路线图

- [x] GB28181 业务接口逐步补齐（Phase 0/1/2/3 — Live/Playback/RecordInfo/Download/Talk-Broadcast）
- [x] SQLite 零依赖默认后端（Phase 1–7）
- [x] JT1078 重传检测 + Webhook
- [x] 启动 warning + 多 DB CI 矩阵
- [ ] 集群化 SIP 负载均衡
- [ ] 完整 WebRTC 播放链路
- [ ] 录像云端转存与对象存储适配
- [ ] 多租户与权限细化

---

## 🤝 贡献指南

欢迎通过 Issue / PR 贡献。提交前请：

1. Fork 仓库并新建特性分支：`git checkout -b feat/your-feature`
2. 通过 `cargo fmt` 与 `cargo clippy --all-targets --all-features`
3. 为新功能/缺陷补充单元测试或集成测试
4. 保持提交粒度小、说明清晰；遵循 Conventional Commits 风格
5. 确保本地 `cargo test` 全绿

> 重大变更前请先开 Issue 讨论，避免重复劳动。

---

## 📜 许可证

本仓库默认遵循 **MIT License**。第三方依赖（ZLMediaKit、Element UI、Vue 等）
各自保留其原始许可证，详见各依赖仓库的 LICENSE 文件。

---

## 🙏 致谢

- 流媒体引擎 [ZLMediaKit](https://github.com/ZLMediaKit/ZLMediaKit)
- 前端模板 [vue-admin-template](https://github.com/PanJiaChen/vue-admin-template) by PanJiaChen
- 所有使用、反馈与贡献者

---

<div align="center">

**[⬆ 回到顶部](#gbserver)**

</div>
