# GBServer 开发调试环境搭建 — 设计文档

> 日期：2026-06-20
> 范围：仅启动、调试、观察；不改代码
> 数据库：SQLite（零依赖，默认 feature）
> 运行模式：dev server（`cargo run` + `npm run dev`）
> 浏览器交互：API 脚本 + UI 操作指南（手工）

## 1. 目标

把 GBServer 前后端完整地跑起来，串联起 **ZLM 流媒体 / Rust 后端 / Vue 前端 / 浏览器 UI** 四层，
并提供可重复使用的 API 验证脚本和 UI 逐步操作指南，使后续能"像人一样"控制系统进行功能验证。

不在本次范围内：
- 任何源代码修改（`src/`、`web/src/`、Cargo.toml 除外）
- 任何 schema/迁移修改
- 任何生产部署形态的优化

## 2. 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│  scripts/dev-up.sh（新增编排脚本）                            │
│                                                             │
│  1) check toolchain (cargo / node / docker / curl)          │
│  2) start ZLM via docker (port 8080/8443/554)               │
│  3) start backend: cargo run (port 18080 + SIP 5060/5061)   │
│  4) start frontend: cd web && npm run dev (port 9528)       │
│  5) health check + print ready summary                      │
└─────────────────────────────────────────────────────────────┘
        │
        │  (服务持续在后台运行)
        ▼
┌──────────────────────────┐  ┌────────────────────────────┐
│  scripts/smoke/*.sh      │  │  docs/debug/UI_WALKTHROUGH │
│  (6 个 curl 验证脚本)     │  │  (UI 逐步点击指南)          │
└──────────────────────────┘  └────────────────────────────┘
        │                                │
        ▼                                ▼
   后端 API 直连                  浏览器手工 / Playwright
   (验证业务逻辑)                 (验证前端 UI)
```

### 2.1 服务清单

| 组件 | 端口 | 启动方式 | 配置来源 |
|------|------|----------|----------|
| ZLMediaKit | 8080 (HTTP API) / 8443 (HTTPS) / 554 (RTMP) / 1935 (RTMP publish) | `docker run -d` | 镜像默认 + `MEDIA_SECRET` env 覆盖 |
| GBServer 后端 | 18080 (HTTP) / 5060 (SIP UDP) / 5061 (SIP TCP) | `cargo run`（后台） | `config/application.toml` |
| Vue 前端 dev server | 9528 | `npm run dev`（后台） | `web/vue.config.js` 代理到 :18080 |
| SQLite | — | 文件 `data/gbserver.db` | 自动建表（首次启动） |

### 2.2 关键环境变量

| 变量 | 用途 | 默认值 |
|------|------|--------|
| `ZLM_SECRET` | ZLM HTTP API 密钥，需与 `config/application.toml` 中 `[[zlm.servers]]` 一致 | `S63648HLbxckv7YjpPTXXRTOsAVGo0Ia`（与 toml 保持一致） |
| `RUST_LOG` | 后端日志级别 | `info,gbserver=debug` |
| `GBSERVER__SERVER__PORT` | 后端 HTTP 端口 | `18080` |
| `GBSERVER__DATABASE__URL` | 数据库连接串 | `sqlite://data/gbserver.db?mode=rwc` |

## 3. 文件清单（本次新增/修改）

### 3.1 新增脚本

| 路径 | 作用 |
|------|------|
| `scripts/dev-up.sh` | 编排启动：toolchain 检查 → 拉起 ZLM → 启后端 → 启前端 → 健康检查 |
| `scripts/dev-down.sh` | 停止：杀 cargo/node 进程 + 删 ZLM 容器 |
| `scripts/dev-status.sh` | 查询：三个服务是否在线、端口监听、PID |
| `scripts/dev-logs.sh` | 输出：tail 三个服务的最近 N 行日志 |
| `scripts/smoke/01-login.sh` | 登录 + JWT 验证 + 401/403 用例 |
| `scripts/smoke/02-device.sh` | 设备列表、增改、状态查询 |
| `scripts/smoke/03-channel.sh` | 通道列表、目录订阅 |
| `scripts/smoke/04-stream.sh` | ZLM 健康、播放/录像/对讲 INVITE |
| `scripts/smoke/05-user.sh` | 用户/角色 CRUD、密码修改 |
| `scripts/smoke/06-plan.sh` | 录像计划、级联平台、JT1078、系统概览 |
| `scripts/smoke/_lib.sh` | 公共函数：jq 解析、token 存储、颜色输出 |

### 3.2 新增文档

| 路径 | 作用 |
|------|------|
| `docs/debug/UI_WALKTHROUGH.md` | UI 逐步操作指南（每个菜单的"点击 X → 看到 Y"） |
| `docs/debug/SMOKE_REPORT.md` | 每次 smoke 跑完自动追加报告（时间、结果矩阵） |
| `docs/debug/ISSUES.md` | 调试中观察到的异常（不修，仅记录） |

### 3.3 不修改

- `src/**` 任何 Rust 源文件
- `web/src/**` 任何前端源文件
- `Cargo.toml`、`web/package.json` 依赖
- `config/application.toml` 配置（用 env var 覆盖）
- `database/**` SQL schema

## 4. 启动流程（dev-up.sh 伪代码）

```bash
1. set -euo pipefail
2. PROJECT_DIR=$(git rev-parse --show-toplevel)
3. cd "$PROJECT_DIR"
4. log "Step 1/5: toolchain check"
   require cargo node npm docker curl jq
5. log "Step 2/5: start ZLM (docker)"
   docker rm -f gbserver-zlm 2>/dev/null || true
   docker run -d --name gbserver-zlm \
     -p 8080:8080 -p 8443:8443 -p 554:554 -p 1935:1935 \
     -e MEDIA_SECRET=S63648HLbxckv7YjpPTXXRTOsAVGo0Ia \
     zlmediakit/zlmediakit:latest
   wait_for_url http://127.0.0.1:8080/index/api/version
6. log "Step 3/5: start backend (cargo run)"
   mkdir -p data logs
   cd "$PROJECT_DIR"
   RUST_LOG=info,gbserver=debug \
   nohup cargo run --manifest-path Cargo.toml \
     > logs/backend.log 2>&1 &
   echo $! > .pids/backend.pid
   wait_for_url http://127.0.0.1:18080/health
7. log "Step 4/5: start frontend (npm run dev)"
   cd "$PROJECT_DIR/web"
   [ -d node_modules ] || npm install
   nohup npm run dev > "$PROJECT_DIR/logs/frontend.log" 2>&1 &
   echo $! > ../.pids/frontend.pid
   wait_for_url http://127.0.0.1:9528
8. log "Step 5/5: print summary"
   print "Backend  http://127.0.0.1:18080  (health, /metrics, /api/*)"
   print "Frontend http://127.0.0.1:9528   (login with admin/admin)"
   print "ZLM      http://127.0.0.1:8080   (secret 见 config)"
   print "Smoke    bash scripts/smoke/*.sh"
   print "Logs     tail -f logs/{backend,frontend}.log"
   print "Stop     bash scripts/dev-down.sh"
```

## 5. 调试覆盖矩阵

| 模块 | API 路径 | UI 入口 | smoke 脚本 |
|------|----------|---------|-----------|
| 登录/鉴权 | `POST /api/user/login` | `/login` | `01-login.sh` |
| 设备管理 | `GET/POST /api/device/...` | 设备管理菜单 | `02-device.sh` |
| 通道/目录 | `GET /api/device/query/channels` | 设备详情 → 通道 | `03-channel.sh` |
| 流媒体/ZLM | `GET /zlm/...`、`POST /api/zlm/hook` | 设备 → 播放按钮 | `04-stream.sh` |
| 用户/角色 | `GET /api/user/...` | 系统管理 → 用户 | `05-user.sh` |
| 计划/级联/JT1078/概览 | 多个 | 各菜单 | `06-plan.sh` |

## 6. 错误处理策略

| 失败点 | 处理 |
|--------|------|
| toolchain 缺失 | 立即退出，列出缺失项 |
| ZLM 拉起失败 | 不阻断后续；标注"流媒体相关 API 将不可用" |
| 后端编译失败 | 退出 1，输出 `logs/backend.log` 最后 50 行 |
| 前端依赖安装失败 | 不阻断；标注"前端可能未启动，但后端可 API 验证" |
| 健康检查超时 | 输出已监听端口、最后日志、可能原因 |
| 单个 smoke 失败 | 继续执行后续，最后输出汇总表 |

## 7. 成功标准

| 编号 | 标准 | 验证方式 |
|------|------|----------|
| S1 | `scripts/dev-up.sh` 退出码 0 | 脚本本身 |
| S2 | `curl http://127.0.0.1:18080/health` 返回 200 | `dev-status.sh` |
| S3 | `curl -I http://127.0.0.1:9528` 返回 200 | `dev-status.sh` |
| S4 | `curl http://127.0.0.1:8080/index/api/getServerConfig?secret=...` 返回 code=0 | `04-stream.sh` |
| S5 | `01-login.sh` 拿到 access-token，后续请求 200 | smoke |
| S6 | `02-device.sh` 列出至少 1 个设备 | smoke |
| S7 | `05-user.sh` 用户列表非空（init SQL 默认 admin） | smoke |
| S8 | 浏览器手工 `admin/admin` 登录后能进入主页（截图为证） | UI_WALKTHROUGH.md |
| S9 | `ISSUES.md` 记录所有观察到的异常/警告 | 文档 |

## 8. 风险与限制

| 风险 | 缓解 |
|------|------|
| 首次 `cargo run` 编译耗时 3-10 分钟 | 提前提示；后台编译不阻塞 smoke |
| `npm install` 耗时 1-3 分钟 | 提示用户耐心等待；带 `--prefer-offline` |
| ZLM 镜像拉取需联网 | 失败时跳过，不阻断；后续可重试 |
| SIP `:5060` 占用冲突 | 检查并提示；提供 env var 覆盖 |
| 前端 webpack 编译 ESLint 警告 | 已 `lintOnSave: false`，预期内 |
| 项目 `web/dist` 不存在 | dev server 不依赖 dist；不影响本次 |
| 浏览器实际交互需用户手工 | UI_WALKTHROUGH.md 提供截图位 |

## 9. 后续可选项（本次不做）

- Playwright 自动化脚本（项目 `e2e/` 已有，可复用，但需评估范围）
- release 模式构建（用户已选 dev，不做）
- 修改默认账号密码（用户已选"不改代码"）
- 加固 SIP 注册设备仿真（项目已有 `tests/integration/sip/device_simulator_test.rs`，可单独启动）
