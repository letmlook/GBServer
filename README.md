# WVP GB28181 后端 Rust 版

使用 Rust 完全重写 [wvp-GB28181-pro](https://github.com/648540858/wvp-GB28181-pro) 的后端，**前端** 为 Vue 2 + Element UI，位于本仓库 `web/` 目录。

## 技术栈

- **Web**: Axum 0.7、Tower
- **数据库**: SQLx（默认 **PostgreSQL**；可选 MySQL，与原有 WVP 表结构兼容）
- **鉴权**: JWT（HS256，请求头 `access-token`）
- **配置**: YAML + 环境变量

## 目录说明

- `src/`：Rust 后端源码
- `config/application.yaml`：默认配置
- `web/`：前端（Vue 2 + Element UI），已从原版复制到本目录，构建输出到 `web/dist`

## 运行依赖环境

| 依赖 | 用途 | 版本建议 | 必选 |
|------|------|----------|------|
| **PostgreSQL** 或 **MySQL** | 业务数据（用户、设备、通道、推流、平台等） | PostgreSQL 12+；MySQL 5.7+ / 8.x | ✅ 二选一（默认 PostgreSQL） |
| **Rust** | 编译并运行后端 | 1.70+（stable） | 仅构建时需要；发布后可只保留可执行文件 |
| **Node.js** | 构建前端（`web/dist`） | 14+ / 16+ / 18+（含 npm） | 仅构建时需要；运行时不依赖 |
| **Redis** | 配置中可选，当前后端未使用 | 6.x / 7.x | ❌ 可选（预留） |

- **仅运行已构建好的服务**：需安装并启动 **PostgreSQL** 或 **MySQL**（与编译时选择的数据库一致，默认 PostgreSQL），在 GBServer 根目录执行 `.\target\release\wvp-gb28181-server.exe`（或 `cargo run --release`），并保证 `config/application.yaml` 中 `database.url` 正确、库表已按 WVP 初始化脚本建好。
- **从源码构建**：需安装 **Rust**（后端）和 **Node.js + npm**（前端），再按下方「构建前后端并运行」执行。

**安装参考**（按需选用）：

- **MySQL**：[官网下载](https://dev.mysql.com/downloads/mysql/) 或包管理器（如 Windows 用安装包、Ubuntu `apt install mysql-server`、macOS `brew install mysql`）。
- **PostgreSQL**：[官网下载](https://www.postgresql.org/download/) 或包管理器（如 Ubuntu `apt install postgresql`、macOS `brew install postgresql`）。
- **Rust**：<https://rustup.rs/>，安装后可用 `cargo --version` 验证。
- **Node.js**：<https://nodejs.org/>（建议 LTS），安装后可用 `node -v`、`npm -v` 验证。

### Docker 运行 PostgreSQL + Redis（默认）

项目根目录提供 `docker-compose.yml`，默认一键启动 **PostgreSQL 16** 与 Redis 7，端口与 `config/application.yaml` 一致（PostgreSQL 5432、Redis 6379），无需改配置即可联调。

```bash
# 启动 PostgreSQL + Redis（后台）
docker compose up -d

# 查看状态
docker compose ps

# 停止并删除容器（数据卷保留）
docker compose down
```

- **PostgreSQL**：用户/密码/库在 compose 中为 `postgres` / `postgres` / `wvp`，与默认配置一致。首次使用需导入 WVP 表结构与初始数据：
  ```bash
  docker exec -i wvp-postgres psql -U postgres -d wvp < database/init-postgresql-2.7.4.sql
  ```
- **Redis**：无密码，`redis://127.0.0.1:6379/0`，当前后端未使用，仅预留。
- **MySQL（可选）**：若需同时跑 MySQL，使用 profile 启动：`docker compose --profile mysql up -d`。MySQL 首次使用需导入 `database/init-mysql-2.7.4.sql`（见下方「1. 数据库」）。
- 数据持久化：`postgres_data`、`redis_data`（及可选的 `mysql_data`）卷，`docker compose down` 不会删除；需清空时使用 `docker compose down -v`。

## 快速开始

### 1. 数据库

后端支持 **PostgreSQL**（默认）或 **MySQL**，二选一即可。

- **PostgreSQL**（默认）：使用 `database/init-postgresql-2.7.4.sql`（来源于原 WVP 2.7.4 的 PostgreSQL/金仓版脚本）。先创建数据库与用户，再执行：
  ```bash
  psql -U postgres -d wvp -f database/init-postgresql-2.7.4.sql
  ```
  更多说明见 `database/README.md`。
- **MySQL**：使用 `database/init-mysql-2.7.4.sql`（来源于原 WVP 2.7.4）。执行方式见上方「Docker 运行 MySQL + Redis」或：
  ```bash
  mysql -uroot -p wvp < database/init-mysql-2.7.4.sql
  ```

默认管理员账号：`admin` / 密码 `admin`（MD5：`21232f297a57a5a743894a0e4a801fc3`）。

### 2. 配置

复制并编辑 `config/application.yaml`：

```yaml
server:
  port: 18080

database:
  url: "postgres://用户:密码@127.0.0.1:5432/wvp"   # 使用 MySQL 时改为 mysql://用户:密码@127.0.0.1:3306/wvp

jwt:
  secret: "请改为随机长字符串"
  expiration_minutes: 30

# 前端构建后的静态目录（可选；不配置则仅提供 API）
static_dir: "web/dist"
```

也可通过环境变量覆盖，例如：`WVP__SERVER__PORT=18080`、`WVP__DATABASE__URL=postgres://...`（默认）或 `WVP__DATABASE__URL=mysql://...`。

### 3. 构建前后端并运行

**指定目录说明**：前端构建产物在 `web/dist`，后端可执行文件在 `target/release/`（Release 编译）。配置中 `static_dir: "web/dist"` 指向前端目录，运行需在 **GBServer 根目录** 执行以便正确加载配置与静态资源。

一键构建并运行（PowerShell，在 GBServer 根目录执行）：

```powershell
.\scripts\build-and-run.ps1
```

若已构建过，仅需启动服务可执行：`.\scripts\run.ps1`

或分步执行：

```bash
# 前端（产物 -> web/dist）
cd web
npm install
npm run build:prod

# 后端（产物 -> target/release/）
cd ..
cargo build --release
# 若使用 MySQL，请改为：
# cargo build --release --no-default-features --features mysql

# 运行（必须在 GBServer 根目录，以正确读取 config 与 web/dist）
cargo run --release
```

服务监听 `http://0.0.0.0:18080`。需先启动所选数据库（默认 PostgreSQL 或 MySQL），并确保 `config/application.yaml` 中 `database.url` 与编译时选择的数据库一致。

### 5. 开发时前后端分离

- 前端开发：在 `web` 目录执行 `npm run dev`，代理到 `http://127.0.0.1:18080`（见 `web/vue.config.js`）。
- 后端：在 GBServer 目录 `cargo run`，仅提供 API 即可。

## 已实现接口（与前端兼容）

- **用户**: 登录/登出、userInfo、users 分页、add/delete、changePassword、changePasswordForAdmin、changePushKey
- **设备**: `GET /api/device/query/devices`（分页）、`GET /api/device/query/devices/:deviceId/channels`（分页）
- **流媒体服务器**: list、online/list、one/:id、system/configInfo、system/info、map/config、resource/info
- **推流**: list（分页）、add/update/remove/start、batchRemove、save_to_gb、remove_form_gb（写操作为占位）
- **拉流代理**: list（分页）、ffmpeg_cmd/list、add/update/save/start/stop/delete（写操作为占位）
- **级联平台**: query（分页）、server_config、channel/list、channel/push、add/update/delete、exit/:id
- **实时播放**: play/start、stop、broadcast、broadcast/stop（占位，实际拉流需 ZLM/SIP）
- **区域/分组**: region/tree/list、delete、description、addByCivilCode、queryChildListInBase、base/child/list、update、add、path、tree/query；group/tree/list、add、update、delete、path、tree/query（未实现业务均为占位空数据）
- **角色**: `GET /api/role/all`
- **设备扩展（占位）**: sync_status、devices/:id/delete、sync、transport、guard、subscribe/catalog、subscribe/mobile-position、config/query/BasicParam、channel/one、query/streams、control/record、sub_channels、tree/channel、channel/audio、stream/identification/update、device/update、device/add、query/devices/:id、query/tree/:id
- **流媒体扩展（占位）**: media_server/check、record/check、save、delete、media_info、load、map/model-icon/list
- **级联扩展（占位）**: channel/add、channel/device/add、channel/device/remove、channel/remove、channel/custom/update
- **日志/用户 Key（占位）**: log/list；userApiKey/remark、userApiKeys、enable、disable、reset、delete、add
- **回放/录像（占位）**: playback/start、resume、pause、speed、stop；gb_record/query、download/start、stop、progress；cloud/record/*；record/plan/get、add、update、query、delete、channel/list、link

响应格式与 Java 版一致：`{ "code": 0, "msg": "成功", "data": ... }`，鉴权使用请求头 `access-token`。

### 前后端 API 联调测试

确保后端已启动且数据库可用后，在项目根目录执行：

```bash
node scripts/api-integration-test.js
```

可选环境变量：`BASE_URL=http://localhost:18080`。脚本会先登录获取 token，再逐个请求上述接口并输出 OK/FAIL。

## 后续可扩展

- 设备同步/删除/控制、通道详情、播放/回放与 ZLM 对接、推流/代理启停与 ZLM、录像计划、云录像、告警、日志、JT1078 等可按原 Java 接口逐步实现。
- 国标 SIP 信令、ZLM Hook、Redis 消息等可单独成模块或服务，与当前 HTTP API 协同。

## JT1078 retransmit hook & config

A new JT1078 retransmit detection and notification feature is added.

Config (config/application.yaml):

jt1078:
  timeout_ms: 60000
  retransmit_wait_ms: 200
  retransmit_hook_url: "http://127.0.0.1:18080/api/jt1078/retransmit"

If `retransmit_hook_url` is set, the server will POST JSON reports when missing sequence ranges time out.

## License

与原项目保持一致；Rust 重写部分可视为同一项目的一部分。
