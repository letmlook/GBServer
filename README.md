<div align="center">

# GBServer

### GB28181 国标信令 · 流媒体接入 · 级联管理平台

面向 **GB/T 28181-2016** 国标协议、**JT1078** 车辆终端协议的流媒体接入与级联管理平台。
Rust 全异步后端（Axum + SQLx + Tokio），前端 Vue 3 + Element Plus + Vite（[`web/`](web/README.md)）。

[快速开始](#-快速开始) · [核心特性](#-核心特性) · [配置](#-配置) · [测试](#-测试与核验) · [文档索引](#-文档索引)

</div>

---

## 📌 当前状态（2026-09-19）

- **可正常开发与提交**：代码冻结已于 2026-09-19 解除；未完成事项集中在 [`docs/OPEN_ISSUES.md`](docs/OPEN_ISSUES.md)。
- **第一台真实国标设备已接入并核验主链路**（EasyGBD，TCP）：401 摘要注册与续期、Keepalive、
  目录同步、注册后自动 DeviceInfo 落库、实时点播 + FLV 实拉成功（H264 1080×1920 + G.711A）。
- **规模基线**：87,662 行 Rust（141 个文件）· 31 个 handler 模块 · **429 条唯一 `/api/...` 路由**
  · `cargo test --no-fail-fast` **746 通过 / 0 失败 / 3 忽略** · 前端 18 个业务视图。
- **已实现什么 / 为什么这么设计** → [`docs/STATUS.md`](docs/STATUS.md)；**还没做什么** → [`docs/OPEN_ISSUES.md`](docs/OPEN_ISSUES.md)。

---

## ✨ 核心特性

| 领域 | 能力 |
|------|------|
| **国标信令** | SIP 注册（401 摘要鉴权）、心跳、目录订阅、INVITE/BYE/INFO、SDP 协商、SSRC 分配、NAT 地址改写、PTZ |
| **实时流** | ZLMediaKit 集成（Hook、节点健康检查、多节点最少负载选择）、点播/停止/抓图/分享、WebRTC |
| **录像回放** | 历史回放与暂停/续播/拖动/倍速、云端录像（计划 → 录制 → 落库 → 播放 → 删除）、录像下载 |
| **推流 / 拉流** | GB28181 推流、拉流代理（FFmpeg）启停、`ffmpeg_cmd` 生成 |
| **级联平台** | 上级平台 SIP REGISTER 保活、目录/通道同步、级联推流、上级点播本级 |
| **语音** | 对讲（音频上行已端到端验证）、语音广播 |
| **JT1078** | 车辆终端 UDP 服务、帧解析、会话状态、序列号重传检测 + 可选 Webhook（**本项目独有扩展**） |
| **设备管理** | 设备/通道 CRUD 与统计、DeviceInfo/DeviceStatus、配置查询与更新（等设备应答）、区域与业务分组、报警、移动位置 |
| **鉴权** | JWT（`access-token` / `Authorization: Bearer`）+ API Key（`X-API-Key` / `apiKey`）；管理类端点统一走 `authz::require_admin`；审计日志异步落库 |
| **可观测** | Prometheus `/metrics`、`/api/health` + `/api/ready`、tracing 结构化日志、系统信息面板 |

> 尚未实现的端点（中亿视图 SY 定制 10 条、`/api/test/*` 诊断端点、`/api/v1/*` 外部集成协议）
> 已在 [`docs/OPEN_ISSUES.md`](docs/OPEN_ISSUES.md) §A 登记。

---

## 🏗️ 架构

```
前端 Vue 3 SPA (web/dist, 由后端 static_dir 提供)
        │ HTTP / WS
Axum 路由 router.rs ── /api/* 鉴权中间件 · /metrics · /health
        │
 handlers/ ──► db/ ──► SQLx（SQLite 默认 / PostgreSQL / MySQL，cargo feature 切换）
        ├──► sip/      GB28181 SIP 栈（core 解析 · transport UDP/TCP · gb28181 应用层）
        ├──► zlm/      ZLMediaKit HTTP 客户端 · Hook 接收 · 健康检查
        ├──► jt1078/   车辆终端协议与会话
        ├──► cascade/  上级平台注册保活
        ├──► scheduler/ 录像计划后台调度
        └──► ws/ · cluster/ · rpc.rs · state_store.rs   集群 / 跨节点状态
```

- `handlers/` 保持薄层：取参数 → 调 `db::` 或协议模块 → 返回 `ApiResult<T>` / `AppError`。
- `stub.rs` / `device_stub.rs` **名字叫 stub，实为真实生产实现**（64 个 entry 全部落库/下发 SIP/调 ZLM，其中 48 条是前端活跃调用），当前不建议清退，见 [`docs/STUB_COMPAT_PLAN.md`](docs/STUB_COMPAT_PLAN.md)。
- 跨节点部署：`StateStore`（Redis）+ `[rpc] peer_endpoints` + `[cluster]`，负载回退链 `StateStore → ZLM 实时计数 → 首个节点`。

---

## 🚀 快速开始

### 0. 环境依赖

| 依赖 | 用途 | 必需性 |
|------|------|--------|
| **Rust**（stable） | 编译后端 | 构建时 |
| **Node.js 18+**（含 npm） | 编译前端 | 构建时 |
| **SQLite** | 默认数据库 | ✅ 零安装，首次启动自动建库 |
| **PostgreSQL 12+** / **MySQL 5.7+** | 生产数据库 | 二选一（cargo feature） |
| **Redis** | 缓存与跨节点状态 | 可选（单机可关，多实例必配） |
| **ZLMediaKit** | 流媒体引擎 | 播放/录像必需 |

### 1. 选择数据库

```bash
cargo run                                                # SQLite（默认，开箱即用，≤500 设备）
cargo run --no-default-features --features postgres       # PostgreSQL（生产主力 / 多实例）
cargo run --no-default-features --features mysql          # MySQL（平迁 / 兼容历史部署）
```

SQLite 无需任何初始化操作。PG / MySQL 需先建库并导入 schema：

```bash
createdb gbserver && psql -U postgres -d gbserver -f database/init-postgresql-2.7.4.sql
mysql -uroot -p -e "CREATE DATABASE gbserver DEFAULT CHARACTER SET utf8mb4;" \
  && mysql -uroot -p gbserver < database/init-mysql-2.7.4.sql
```

> 默认管理员：`admin` / `admin`（MD5 存储，**上线前必须改**）。

### 2. 构建并运行

**Linux / macOS**

```bash
cd web && npm install && npm run build && cd ..   # 前端产物 → web/dist
cargo build                                        # 本地开发/验证用 debug（比 release 快 4–5 倍）
./target/debug/gbserver                            # 必须在仓库根目录启动
```

```bash
# 容器内外的 ZLM 都在 127.0.0.1 时，务必绕过系统代理，否则健康检查会误判节点离线：
NO_PROXY=localhost,127.0.0.1,::1 no_proxy=localhost,127.0.0.1,::1 ./target/debug/gbserver

cargo build --release                              # 仅在打 tag / 出生产镜像时使用
```

**Windows（PowerShell，仓库根目录）**

```powershell
.\scripts\build-and-run.ps1     # 构建前后端并启动
.\scripts\run.ps1               # 仅启动已构建的二进制
```

服务默认监听 `http://0.0.0.0:18080`；前端静态资源由后端直接提供（无独立生产前端服务）。

### 3. 一键 Docker（PostgreSQL + Redis + ZLMediaKit）

```bash
docker compose up -d                                   # Linux 服务器：ZLM 用 host 网络（推荐）
docker compose -f docker-compose.yml -f docker-compose.mac.yml up -d   # macOS / Windows（Docker Desktop 不支持 host 网络）
docker compose --profile mysql up -d                   # MySQL 变体
docker compose down                                    # 保留数据卷；加 -v 彻底清空
```

为什么 ZLM 默认用 host 网络：GB28181 收流端口池（默认 `30000-30100/udp`）是 ZLM **启动时**建立的，
桥接模式必须逐口发布，一旦不一致就是「INVITE 200 OK 却永远等不到媒体」；WebRTC 也需要浏览器能直连
ZLM 通告的 ICE 候选。改 ZLM 配置请编辑 `docker/zlm/config.ini` 后 `docker compose restart zlm`；
`rtc.externIP` 无需手改——在 `config/application.toml` 的 `[[zlm.servers]]` 配 `rtc_extern_ip` 即可，
后端会在节点上线时下发并回读校验。

---

## ⚙️ 配置

配置来源：`config/application.toml` + 环境变量覆盖（`GBSERVER__SECTION__KEY`，双下划线分隔，
如 `GBSERVER__SERVER__PORT=18080`）。完整字段与注释见该文件本身。

| 配置段 | 关键字段 | 默认值 | 说明 |
|--------|----------|--------|------|
| — | `static_dir` | `web/dist` | 前端产物目录；不配置则仅提供 API |
| `server` | `port` | `18080` | HTTP 监听端口 |
| `database` | `url` | `sqlite://data/gbserver.db?mode=rwc` | SQLx 连接串 |
| `database` | `sqlite_max_devices` | `500` | SQLite 设备上限，超出请迁 PG |
| `jwt` | `secret` | 占位 | **生产必改**为 256-bit 随机串 |
| `jwt` | `expiration_minutes` / `remember_expiration_minutes` | `720` / `10080` | 普通会话 / 「7 天免登录」（须与前端 cookie 对齐） |
| `sip` | `port` / `tcp_port` | `5060` / `5060` | UDP 与 TCP **共用同一端口**；别改成 5061，否则 TCP 设备注册不上 |
| `sip` | `sdp_ip` / `stream_ip` | 空 | 公网部署时填公网 IP（NAT 穿透） |
| `sip.heartbeat` | `latency_probe_interval_secs` | `15` | 设备列表「延迟」列的探针间隔，`0` 关闭 |
| `zlm` | `servers[]` | — | 媒体节点列表：`ip` 必须**真实可达**（别写 127.0.0.1） |
| `zlm.servers[]` | `hook_url` | — | 必须填 **ZLM 能访问到后端**的地址（容器部署用 `host.docker.internal`） |
| `zlm.servers[]` | `rtp_port_range` / `rtc_extern_ip` | `30000-30100` / 空 | 须与 compose 端口映射一致；桥接容器部署必须填宿主可达 IP |
| `cluster` / `rpc` | `enabled` / `peer_endpoints` | `false` / `[]` | 多实例 HA 时启用（依赖 Redis） |
| `jt1078` | `enabled` / `timeout_ms` / `retransmit_hook_url` | `true` / `60000` / — | 车载终端服务与缺序上报 |

> 端口矩阵与三种数据库的选型、迁移路径、备份灾备以 [`docs/DEPLOYMENT_GUIDE.md`](docs/DEPLOYMENT_GUIDE.md) 为唯一权威源。

---

## 🧪 测试与核验

```bash
cargo test                                    # 全量（SQLite 默认 feature，完全自包含）
cargo test --test sqlite_compat               # 数据库层主测试套件（内存 SQLite）
cargo test --test device_simulator_test       # SIP 报文格式与心跳场景
cargo test --test jt1078_e2e_test             # JT1078 命令/应答生命周期
cargo test --no-default-features --features postgres --lib   # 方言矩阵：PostgreSQL
cargo test --no-default-features --features mysql --lib      # 方言矩阵：MySQL
cargo fmt && cargo clippy --all-targets --all-features
```

前端与端到端：

```bash
cd web && npm run build      # vue-tsc 类型检查 + 生产构建（≈17s）
cd web && npm run lint
cd e2e && npm install && npx playwright install chromium && npx playwright test   # 需后端 :18080 + 前端 dev :9528 + ZLM
```

- 默认 SQLite feature 下的测试**不连接** Redis / PG / MySQL / ZLM，CI 无需任何 service 容器。
- `.github/workflows/ci.yml`（门禁：`cargo check --all-targets` + 测试、三 feature 编译、前端构建）当前按用户要求**只保留手动 `workflow_dispatch` 触发**。
- 本地等效命令（`just`）：`just feature-check`、`just clippy`、`just fmt`。
- 已有实测证据的能力清单见 [`docs/STATUS.md`](docs/STATUS.md) §6（真机闭环 / 核心闭环 / 三方言 / e2e）。

---

## 📚 文档索引

| 文档 | 用途 |
|------|------|
| [docs/STATUS.md](docs/STATUS.md) | **当前状态**：规模基线、能力矩阵、关键设计决策、真机核验与证据 |
| [docs/OPEN_ISSUES.md](docs/OPEN_ISSUES.md) | **未完成 / 未验证事项**（唯一待办表） |
| [docs/DEPLOYMENT_GUIDE.md](docs/DEPLOYMENT_GUIDE.md) | 构建、运行、部署分级、配置、监控、灾备、升级、FAQ（唯一对外部署文档） |
| [docs/DB_DIALECT_NOTES.md](docs/DB_DIALECT_NOTES.md) | 写多方言 SQL（SQLite / MySQL / PostgreSQL）的注意事项 |
| [docs/STUB_COMPAT_PLAN.md](docs/STUB_COMPAT_PLAN.md) | `stub.rs` / `device_stub.rs` 的真实定位与清退评估 |
| [AGENTS.md](AGENTS.md) / [CLAUDE.md](CLAUDE.md) | 供 AI/新同学读的仓库约定、易踩坑与敏感改动提示 |
| [database/README.md](database/README.md) | 三方言初始化脚本说明 |
| [web/README.md](web/README.md) · [e2e/README.md](e2e/README.md) · [mock/README.md](mock/README.md) | 前端 · 端到端测试 · 模拟资源（SIP 设备 / JT1078 终端 / 级联平台） |

---

## 🌐 API 概览

统一响应格式（对应后端 `ApiResult<T>`）：`{ "code": 0, "msg": "成功", "data": ... }`；
鉴权请求头 `access-token`（JWT）或 `X-API-Key` / `apiKey`。完整路由见 `src/router.rs`。

| 域 | 主要端点 |
|----|----------|
| **用户 / 角色 / API Key** | 登录登出、userInfo、用户分页与增删改密、`role/*`、`userApiKey/*` |
| **设备** | `GET /api/device/query/devices`（分页）、`/devices/:id/channels`（分页）、统计、tree、status |
| **通道 / 区域 / 分组** | 通道查询与编辑；`region/*`、`group/*`（tree、path、增删改） |
| **实时 / 回放** | `play/start|stop`、broadcast、抓图与分享；`playback/*`、`gb_record/query`、`download/*` |
| **云端录像** | `cloud/record/*`（计划、列表、播放、下载、ZIP 打包、收藏）、`record/plan/*` |
| **流媒体节点** | list、online/list、one、check、load、media_info、system/configInfo、system/info |
| **推流 / 拉流代理** | `push/*`、`streamProxy/*`（含 `ffmpeg_cmd`） |
| **级联平台** | `platform/query`、server_config、channel/list、channel/push、增删改、exit |
| **JT1078** | 终端 / 通道 / 围栏 / 路线 / 位置 / 多媒体检索 / 录像下载 |
| **系统** | `/api/health`、`/api/ready`、`/metrics`、`/api/server/system/info`（控制台单端点） |

---

## 📜 许可证

本仓库遵循 **MIT License**。第三方依赖（ZLMediaKit、Vue、Element Plus 等）各自保留原始许可证。

## 🙏 致谢

[ZLMediaKit](https://github.com/ZLMediaKit/ZLMediaKit) · [vue-admin-template](https://github.com/PanJiaChen/vue-admin-template) · 所有使用与贡献者
