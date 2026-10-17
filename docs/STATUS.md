# GBServer 当前状态

## 2.0.0 当前验证增量（2026-10-18，`v2.0`）

此节仅记录本轮实际证据；下方 2026-09-19 的规模和真机数据是历史快照。

| 项目 | 本轮结果 | 证据范围 |
|---|---|---|
| Rust 测试 | `cargo test --no-fail-fast --locked`：787 通过、0 失败、3 个文档测试忽略 | SQLite 默认特性；本机自动化，ZLM + Mock 的实际运行证据见下文；真机仍待复验 |
| 版本一致性 | SIP `User-Agent` 统一从 Cargo 包版本生成，覆盖设备侧 SIP 和级联请求；HTTP `/api/server/version`、`/api/system/info` 与 OpenAPI 均返回 `2.0.0` | `cargo fmt --check`、Rust 全量测试与 `cargo build --locked` 通过；本机后端启动后对三个 HTTP 出口实测均为 `2.0.0`；级联 REGISTER/INVITE 的 SIP 报文构造测试通过，真机报文待复验 |
| 前端构建 | `web/npm run build` 通过 | Vue 类型检查与 Vite 打包，不代表浏览器播放通过 |
| M1 播放收流 | 普通点播和级联点播在媒体 ready 后不再二次回连。`on_rtp_server_started` 仅表示端口已打开，不误报媒体到达。TCP-PASSIVE 使用 ZLM `tcp_mode=2`、纯主机 `dst_url` 主动连接，并等真实流注册后才返回成功；失败清理 RTP Server 和 Play 会话 | 回归测试覆盖媒体判定、被动模式等待及取消；本机真实 ZLM + SIP Mock：普通模式无 RTP 时 15.0 秒超时、有 RTP 时 FLV 可读；TCP-PASSIVE 无设备媒体时 15.0 秒失败，ZLM RTP Server 0 残留且 Mock 收到 BYE；使用本机 TCP RTP/PS relay 时约 5.73 秒成功、FLV 头为 `FLV`、停止成功。真机仍待复验 |
| M1 录像重试 | 25 次、800ms 的生产预算不变；新增缺流重试和不可恢复错误次数测试 | `start_record_retry_tests` 4/4；本轮 ZLM + SIP Mock 生成新 MP4，详见下行 |
| M1 云录像验收 | `cloudRecord` 用例只接受本轮新建计划后、所选通道新增的非空 MP4 记录，播放与删除复用其 ID；录制等待中检查后端、ZLM 和托管 SIP Mock | Docker Desktop 真实 ZLM + ffmpeg SIP Mock + 临时 SQLite + Chromium：干净目录同步后 3/3 通过；本轮计划 3、流 `34020000001320000001_34020000001320000001`、记录 2、MP4 125446 字节，播放下载和删除均通过；健康检查单测 2/2。真机未覆盖 |
| M4 浏览器媒体与录像 | 新增可显式启用的 WebRTC 真实媒体用例，分别断言 SDP answer、ICE 连通和视频帧解码；FLV 用例结束时主动停止点播 | 本机真实 ZLM + ffmpeg SIP Mock + 临时 SQLite + Chromium：`E2E_REQUIRE_WEBRTC=1 npx playwright test tests/webrtcMedia.spec.ts` 1/1，ICE=`connected`、解码 3 帧、接收 6159 字节；`playUrls`、`cloudRecord`、`deviceControl` 关键用例 5/5，本轮云录像计划 1、新 MP4 记录 1、177512 字节，播放 Range 与删除通过；`web/npm run build` 通过。浏览器证据仍是 Mock 流，真机与生产拓扑待验收 |
| M4 同源静态部署冒烟 | GBServer 直接托管构建后的 `web/dist`，浏览器业务请求和静态资源经同一后端端口；版本出口保持 `2.0.0` | 基线 `6c4344d`、macOS、本机 debug 后端 + 临时 SQLite + ZLM `master/4b07053`（镜像 `sha256:d1c2ee83…`）；无 Vite，`E2E_BASE_URL=http://127.0.0.1:18080 npx playwright test tests/smoke.spec.ts --project=chromium` 20/20（健康、登录、17 个业务页）；`/login` 返回 200 且引用构建静态资源，`/api/server/version`、`/api/system/info`、OpenAPI 都为 `2.0.0`。这是本机同源拓扑冒烟，尚未验证目标服务器 Docker 部署 |
| C9 收藏删除 | GET、DELETE 两种方法均进入鉴权，未登录返回 401；登录后 HTTP 对两种删除方法验证了收藏表与 `gb_cloud_record.collect` 一致清除。添加收藏也同时设置两套存储；数字 JSON `id` 可用，添加/删除中途失败均回滚 | SQLite 路由鉴权、登录后往返及两项事务失败注入测试通过；PostgreSQL 16、MySQL 8 初始化隔离库分别完成登录、添加、列表、GET 删除、再次添加、DELETE 数字 JSON 删除，最终两套存储均清空；测试库/容器已清理。当前云录像页面无收藏入口，浏览器页面验收不适用 |
| M2 命令应答路由 | `DeviceControl` 和 `DeviceConfig` 响应已映射；设备、CmdType、SN 不匹配或重复应答不完成 pending；回执携带的错误通道 ID 也被拒绝。命令与查询共用进程内唯一 SN，配置查询 pending 覆盖完整 15 秒等待，发送失败和超时后清理 | 定向测试与全量 Rust 测试通过；SQLite 后端 + SIP Mock 的 `ConfigDownload` 即时返回 `source=live`，延迟 11 秒仍在第 15 秒前收到实时回执（实测 11.01 秒）；真机闭环待验证 |
| M2 传输模式结果 | 新增进程内唯一 SN、发送失败清理 pending、传输模式 `waitResult=true` 的设备确认/拒绝/未确认状态；设备列表按状态提示 | 本机真实后端 + Python SIP Mock 的接口运行验证：确认、拒绝、仅 SIP 200 无设备回执、重复回执、5.5 秒迟到回执和新旧 SN 隔离均符合预期；其它控制入口和真机回执待做 |
| M2 设防/撤防结果 | 设防接口支持 `waitResult=true`，只接受 `SetGuard`/`ResetGuard`，返回 `sipSent` 与设备确认/拒绝/未确认状态；设备列表提供独立的设防、撤防动作，不再把在线状态当作已设防状态 | Rust 参数校验测试、前端类型检查与构建通过；真实后端 + SIP Mock HTTP/SIP 往返中，设防的确认、拒绝、无设备回执分别返回 `confirmed`、`rejected`、`unconfirmed`，且与传输模式请求 SN 不同。Chromium Playwright 设备列表用例 1/1 通过，覆盖在线/离线按钮及三种提示；真机与其它控制入口仍待验证 |
| M2 远程录像控制结果 | 远程录像命令等待设备执行回执；只有 `Result=OK` 才更新 StateStore 的录像状态并广播 `record_state`。拒绝或无回执的 HTTP 响应 `recording=null`，保留明确的 `deviceResult` | 真实后端 + SIP Mock HTTP/SIP 往返中，确认、拒绝、无回执分别返回 `confirmed`、`rejected`、`unconfirmed`，`StopRecord` 确认后返回 `recording=false`，SN 与其它命令不同；前端构建与全量 Rust 测试 781/781 通过。真机未验证 |
| C8 直播音频标志 | 点播响应优先以当前 ZLM 流的 ready 音频 track 判定 `hasAudio`，无 track 时回退目录字段，并以 `audioSource` 标明来源 | 音频 track 单测、前端构建及 `cargo test --no-fail-fast --locked`（774 通过、3 文档测试忽略）通过；本轮 ZLM + Mock 点播返回 `audioSource=media` 且 `hasAudio=true`，ZLM 同流 ready 音频 track；真机待复验 |
| M4 多方言运行 | PostgreSQL 与 MySQL 特性分别 `cargo build --no-default-features --features <dialect> --locked` 通过；独立测试库按对应初始化 SQL 建表，后端启动、登录、媒体节点/设备列表查询、收藏 GET 和 DELETE 往返均通过 | PostgreSQL 16 容器的隔离库与 MySQL 8 独立临时容器；各 31 张初始化表，测试后隔离库/容器已清理。证明本轮相关路径可运行，不等于全部业务及目标部署验收 |
| M4 平台通道跨方言更新 | MySQL 自定义名称/地址更新缺少 `UPDATE ... SET`，隔离库测试先复现 1064 错误，修复后通过；PostgreSQL 改用参数绑定；SQLite 共享分支的往返测试通过 | MySQL 8 与 PostgreSQL 16 初始化隔离库分别运行 `mysql_platform_channel`、`postgres_platform_channel` 的忽略测试；两列同时/分别更新及含单引号值通过。默认 SQLite 全量测试包含 `sqlite_custom_fields_roundtrip` |
| M3 管理员诊断 | `on_play`/`on_publish` 各保留最近一条白名单元数据；管理员可查 Hook 和 Redis/StateStore 状态。媒体节点列表和检测响应不返回保存的 ZLM secret；ZLM 代理要求管理员并拒绝原始配置读取，配置下发失败摘要不回显配置值 | 定向诊断及媒体节点测试通过，完整 `cargo test --no-fail-fast --locked` 777 通过、3 文档测试忽略；本机真实 Redis 容器停止时返回 `ping_failed`，恢复后 `connected`；真实 ZLM 容器停止时节点检测返回脱敏错误，恢复后 `reachable=true`。代理 GET 当前返回 405，POST 权限和配置拦截已有回归测试 |

本轮尚未接入真实 GB28181 设备；ZLM + Mock 及本机三方言的证据不能替代真机与目标部署拓扑验收。

> 📌 **最近更新：2026-09-19（真实设备接入核验轮，HEAD `8ecec6a`）**
> 本轮把第一台**真实国标设备**（EasyGBD，`34020000001320128497`，TCP）接进平台并做了端到端实测，
> 同时把 §1 的全部计数、§2 的能力矩阵、§6 的验证证据按当前代码/运行态重新实测。
> 逐轮修复过程记录（阶段性快照、修复叙述）已按「只保留最新状态」的要求清理，需要时走 git 历史。
>
> **补充（同日）**：项目转入**独立演进**，全仓清除外部实现（WVP / LiveGBS 等）的命名与引用，
> 并完成响应信封类型、收藏表名等实体改名（详见 §3 首条）。
>
> 本文档与 [`OPEN_ISSUES.md`](OPEN_ISSUES.md) 的分工：
> **本文档 = 已实现什么、为什么这么设计、已验证过什么**；**OPEN_ISSUES = 还没做什么**。

## 1. 规模基线（2026-09-19 实测 @ `8ecec6a`）

| 维度 | 数值 | 复现命令 |
|------|------|----------|
| 后端代码量 | 87,662 行 Rust | `find src -name '*.rs' \| xargs wc -l` |
| 已注册路由 | 430 条唯一 `/api/...` 路径（`router.rs` 425 处 `.route(`） | `grep -oE '"/api/[^"]*"' src/router.rs \| sort -u \| wc -l` |
| Handler 模块 | 31 个（`stub.rs` / `device_stub.rs` **是真实实现**，非兼容 shim，见 §3） | `grep -c 'pub mod' src/handlers/mod.rs` |
| 后端测试 | **750 通过 / 0 失败 / 3 忽略**（15 个测试二进制） | `cargo test --no-fail-fast` |
| 编译 | `cargo check` 0 error；clippy 272 条告警（2026-09-19 早测，本轮未重测） | `cargo check` / `cargo clippy --all-targets` |
| 前端 | 18 个业务视图目录（另有 `404.vue` / `redirect.vue` 两个顶层文件）、17 个 API 模块、13 个通用组件、42 个 SVG 图标、**174** 处 `url:` 字面量 | `find web/src/views -mindepth 1 -maxdepth 1 -type d \| wc -l`；`grep -oE "url[[:space:]]*:[[:space:]]*[\`'\"]" web/src/api/*.ts \| wc -l` |
| e2e | 17 个 spec（59 条 `test()` / 9 条 `test.describe()`）；最近一次运行 **35 通过 / 0 失败 / 0 跳过** | `ls e2e/tests/*.spec.ts \| wc -l`；`e2e/test-results/.last-run.json` |
| 旧 Vue 2 前端 | 已于 2026-09-19 删除，仅存 git 历史 | `git log --oneline -- web-legacy-vue2` |

## 2. 已实现能力矩阵

「真机」列 = 该模块**是否已被真实设备（EasyGBD `34020000001320128497`）实测覆盖**；
`—` 表示尚未用真机验证（不等于不可用）。注意 `stub.rs` / `device_stub.rs` 承载的条目
是**真实实现**（见 §3）。

| 模块 | 状态 | 真机 | 说明 |
|------|------|------|------|
| 用户 / 认证 | ✅ 完整 | — | JWT（`access-token` / `Authorization: Bearer`）+ API Key（`X-API-Key` / `apiKey`），审计日志异步落库 |
| API Key | ✅ 完整 | — | add / delete / enable / disable / remark / reset / list（前端**无管理界面**，见 [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) §4） |
| 角色 | ✅ 完整 | — | all / add / delete |
| 设备 CRUD / 接入 | ✅ 完整 | ✅ | 含统计、tree、status、channels；真机：401 摘要鉴权注册、DeviceInfo、Keepalive、目录同步、注销+重注册、传输模式切换 |
| 设备控制 | ✅ 完整 | ⚠️ 部分 | PTZ / 预置位 / 布防 / 录像 / 重启 / 批量；含扫描、巡航、雨刷、光圈、聚焦、拉框缩放。真机仅测了传输模式切换（设备回 `Result=OK`，但**平台未解析 DeviceControl 应答**，见 B14） |
| 设备配置查询 / 更新 | ✅ 完整 | — | 走 SIP 并**等待设备响应**（15s 超时，`handlers/device_control.rs:339`、`device_stub.rs:417`），非 fire-and-forget |
| 通道 | ✅ 完整 | ✅ | 含 civilCode、parent、地图瓦片、行业编码、网络标识；真机目录 1 通道入库（单包，未覆盖分页/乱序） |
| 直播 | ✅ 完整 | ✅ | start / stop / snap / ssrc / share / broadcast / webrtc。真机点播+FLV 实拉成功（见 §6） |
| 回放 | ✅ 完整 | — | start / stop / pause / resume / seek / speed |
| 云端录像 | ✅ 完整 | — | 计划 → 录制 → 落库 → 播放 → 删除全链路 |
| 推流 / 拉流代理 | ✅ 完整 | — | push + proxy + ffmpeg_cmd 启停 |
| 上级平台（级联） | ✅ 完整 | — | REGISTER 保活、目录/通道同步、级联推流、上级点播本级 |
| 媒体节点（ZLM） | ✅ 完整 | — | list / one / save / online / check / load / media_info / record_check；多节点最少负载选择 |
| 录像计划 | ✅ 完整 | — | 分钟 / ISO 星期口径、到点自动拉流录制 |
| 区域 / 业务分组 | ✅ 完整 | — | tree / path / addByCivilCode / sync；前端界面已补 |
| 日志 | ✅ 完整 | — | 列表 + 文件下载（三方言均可用） |
| 报警 | ✅ 完整 | — | list / before / detail / clear / handle / snap / device / batch / delete |
| 对讲 | ✅ 完整 | — | invite / start / stop / ack / bye / status / list；音频上行端到端验证过（假设备） |
| 语音广播 | ✅ 完整 | — | start / stop |
| RTP / PS | ✅ 完整 | — | send / receive + getTestPort |
| 移动位置 | ✅ 完整 | — | 订阅 + history 查询，打通两张位置表 |
| WebRTC | ✅ 可用 | ⚠️ | 直播页与通道播放对话框均已接入；GB28181 流已由用户在当前部署环境验证可正常播放。不同设备及网络拓扑仍建议在交付验收时覆盖。 |
| JT1078 车载终端 | ✅ 路由齐全 | — | 终端 / 通道 / 围栏（圆/多边形/矩形）/ 路线 / 位置 / 多媒体检索 / 录像下载；**GBServer 独有扩展**；真终端未测（D2） |
| 中亿视图（SY） | ✅ 完整 | — | 已实现 13 条 `/api/sy/*`（list / list/ids / list-with-child / list-for-mobile / cont-with-child / box / circle / polygon / address / meeting/list / control/{play,stop,ptz}）；第三方对接所需的另 10 条未实现（A1） |
| 系统 / 监控 | ✅ 完整 | — | `/api/system/info`、`/api/server/system/info`、`/metrics`（12 个指标族）、`/api/health`、`/api/ready` |

## 3. 关键设计决策

保留这些结论是为了避免后人重复论证同一个问题。

- **接口文档：代码优先的 OpenAPI（2026-09-19 新增）**。全部 **416** 条路由都由
  handler 上的 `#[utoipa::path]` 注解驱动：`routes!()` 一次注册同时产出 axum 路由与
  OpenAPI path，**结构上不可能「加了路由忘写文档」**。`/swagger-ui` 交互式文档（右上角
  Authorize 填 JWT / API Key 即可直接调用），`/api/openapi.json` 出 OpenAPI 3.1 规范。
  实现与运维见 [`OPENAPI_GUIDE.md`](OPENAPI_GUIDE.md)。
  - 受保护路由（403 条）挂 `auth_middleware` + 审计；公开路由（13 条：登录、health、
    ready、metrics、ZLM hook、RPC、分享播放、缩略图、WebSocket、对讲音频）不挂鉴权。
  - 覆盖门禁：`test_every_documented_path_is_routable` 遍历 spec 每条路径真实发请求并
    断言非 404 —— 文档与实现漂移、迁移丢路由都会在 CI 暴露。
  - 为此把 axum 从 0.7.9 升到 0.8.9（`utoipa-axum` 要求），84 条路由的路径参数
    语法 `:param` → `{param}`；`tower 0.5` / `tower-http 0.6`（http 1.x）。
- **项目自 2026-09-19 起独立演进，不再保留任何外部实现（WVP / LiveGBS 等）的命名与引用**。
  已完成的实体改名（**API 契约不变**，只是内部命名）：
  - 响应信封类型改名为 `ApiResult<T>`（`src/response.rs`，888 处调用点、前端
    `web/src/types/api.ts` 的 `ApiResult<T>`）；JSON 形状仍是 `{code,msg,data}`。
  - 设备配置查询/下发 handler 改为 `config_query_*` / `config_set_*`，DTO 改为
    `ConfigQueryParams`；告警批量删除改为 `alarm_delete_batch`。路由路径未变。
  - 收藏录像表改名为 `gb_record_collect`。**不做旧表兼容迁移**：全仓已无旧表名字面量，
    老部署升级后收藏列表为空（收藏无对外契约承诺）；需要保留数据时手工执行
    `ALTER TABLE <旧表名> RENAME TO gb_record_collect`。
  - Redis 录制态键只认 `gbserver:recording:*`，不再回退读旧命名空间（在途录制态丢失可由
    设备重新上报恢复）。
  - 全仓（代码 / 注释 / 文档 / 测试名）已无任何 WVP / LiveGBS 残留。
- **管理员判定集中在 `src/handlers/authz.rs`**（2026-09-19 新增）：`authority == "0"`
  为管理员、内置角色 `id = 1` 兜底，拒绝返回 `ErrorCode::Error403`。此前 `require_admin`
  内联在 `handlers/user.rs`、硬编码 `role_id == 1`，且只覆盖 4 个端点 —— 导致
  `/api/role/*`、`/api/userApiKey/*`、`/api/user/{users,all}` 任何已登录用户都能调，
  普通用户可自助创建管理员级角色。当前 `authz::require_admin` 有 **17 处调用**
  （`role.rs` 3 / `user.rs` 7 / `stub.rs` 7）。**新增管理类端点必须调用 `authz::require_admin`。**
- **用户列表查询用 `LEFT JOIN` 角色表**：`gb_user` 与 `gb_user_role` 之间没有外键约束，
  历史上删角色不校验引用会留下悬空 `role_id`；用 `INNER JOIN` 时这些用户会被 SQL
  静默丢出列表，而 `count_users` 仍计入 —— 表现为「共 N 条只有 N-k 行、翻页也找不回」。
- **`stub.rs` / `device_stub.rs` 是生产实现，不是待退役的兼容层**（2026-09-19 复核更正）：
  这两个文件名为 `stub`，但 64 个 entry **全部是真实实现**（落库 / 下发 SIP / 调 ZLM），
  没有空占位。其中 **48 条是当前 Vue 3 前端正在调用的活跃路径**。它们的自我描述
  「真实实现已迁出、仅挂旧路径」与实际不符 —— 实现从未迁出，函数体就在文件内。
  详见 [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md)。
- **JT1078 是本项目的独有扩展**：国标协议栈之外自行实现的车辆终端协议层；
  协议操作层 12 个端点均为真实下发并等待终端通用应答，已无「已受理」占位响应。
- **状态源统一到 `StateStore`**：`src/cache.rs` 与 `src/sip/gb28181/cascade_service.rs`
  已确认零生产调用并整体删除（后者 751 行）；级联代码现位于 `src/cascade/`、
  `src/sip/gb28181/cascade.rs`、`src/sip/gb28181/cascade_forward.rs`。
  负载均衡回退链为 `StateStore → ZLM 实时计数 → 首个节点`。
- **数据库三方言用一份 SQL 文本**：统一走 `dyn_where::dialect_sql()` 改写占位符，
  避免「改了这里忘了那里」。注意事项见 [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md)。
- **CI 自动触发保持关闭**：按用户要求只保留 `workflow_dispatch` 手动触发
  （`.github/workflows/ci.yml`），GitHub 层面也已 disable。
- **SIP 的 UDP/TCP 共用同一端口**（默认 5060）：国标设备用「同一端口 + 传输方式」
  接入，`sip.tcp_port` 默认等于 `sip.port`。**别再把它写成 5061** —— 那是旧文档
  遗留的错误值，照抄会让 TCP 设备注册不上。

## 4. 真实设备接入核验（2026-09-19，第一台真机）

| 项 | 值 |
|----|----|
| GB-ID | `34020000001320128497` |
| 名称 / 厂商 / 型号 / 固件 | EasyGBD-128497 / easygbd / `34020000001320128497` / V2.0 |
| 传输 | TCP，注册后由平台切到 `TCP-PASSIVE` |
| 地址 | 192.168.3.121:15060 |
| 通道 | 1 个（`34020000001310000001`，真机流为 **H264 1080×1920@26fps + G.711A(PCMA)**） |

**已核验通过**（逐条证据见 §6）：401 摘要注册与续期、注销（expires=0）后重注册、
Keepalive（30s）、注册后自动 DeviceInfo 查询并落库、目录同步、平台→设备延迟探针（200 OK）、
传输模式切换下发（设备回 `Result=OK`）、实时点播 + FLV 实拉 + 通道缩略图落盘、停止点播。

**真机上暴露、已登记为待办的问题**（详见 OPEN_ISSUES）：
B1（设备 200 OK 宣告的端口 ≠ ZLM RTP 端口时 `connectRtpServer` 被 ZLM 拒绝）、
B6（`play_stop` 把 BYE 发给了**已终止的旧会话**）、
B14（设备 `DeviceControl` 应答未被解析）、
C8（通道 `hasAudio` 与实际流不符：流里有 G.711A，接口却报 `hasAudio=false`）。

**真机尚未覆盖**：PTZ / 布防 / 录像 / 重启 / 配置查询、回放与下载、对讲与语音广播、
目录分页（本机 `SumNum=1` 单包）、设备主动 BYE、心跳超时下线、JT1078 真终端。

## 5. 前端 ↔ 后端契约审计结论

2026-09-12 对 **16 个 API 模块 / 130 条契约**做了逐条审计（前端 `web/src/api/` 的调用
形态与本仓库 router / DTO / 响应键名逐字段比对），逐模块结论如下。审计原文已归档至 git 历史。

| 模块 | 条目 | 结论 |
|------|------|------|
| `channel.ts` | 8 | ✅ 已修 |
| `alarm.ts` | 10 | ✅ 已修 |
| `live.ts` | 6 | ✅ 已修 |
| `user.ts` | 6 | ✅ 已修（`query` 搜索 / 权限缺口 / 悬空角色，见 OPEN_ISSUES C5–C7） |
| `cloudRecord.ts` | 14 | ✅ 已修（连带修掉 5 个审计未覆盖的深层缺陷） |
| `device.ts` | 6 | ✅ 已修 |
| `jtDevice.ts` | 13 | ✅ 已修（连带修掉 TEXT 列声明成 `Option<i32>` 的 500） |
| `log.ts` | 7 | ✅ 已修（postgres `serial` vs `i64` 的 500） |
| `mediaServer.ts` | 7 | ✅ 已修（连带修掉心跳时间戳格式导致的「节点全部误判离线」） |
| `platform.ts` | 11 | ✅ 已修（`serverGBDomain` 曾错填成 `sip.realm`） |
| `playback.ts` | 3 | ✅ 已修 |
| `region.ts` | 8 | ✅ 已修（并补上整个模块缺失的前端界面） |
| `streamProxy.ts` | 10 | ✅ 已修 |
| `streamPush.ts` | 12 | ✅ 已修 |
| `syCamera.ts` | 6 | ✅ 已修 |
| `talk.ts` | 1 | ✅ 已修 |

## 6. 已验证过什么（证据类型）

说明这些能力**有实测证据**，不是「路由挂载即认为可用」：

- **真机闭环（2026-09-19，第一台真实国标设备）**：
  - 注册链路：`REGISTER → 401 Challenge → 200`（expires 3600）、注销（expires=0）后 25s 内重注册；
  - 注册后**自动 DeviceInfo 查询**并落库（name/manufacturer/model/firmware 均有值）；
  - 目录同步：`SumNum=1`、1 包、1 通道入库；
  - 延迟探针：每 15s 一条 MESSAGE，设备回 200 OK 结算 RTT；
  - 实时点播：`INVITE → 100 → 200 OK → ACK`，ZLM 出流后**FLV 实拉 3,291,903 字节 / 6s**，
    `ffprobe` 识别为 **H264 1080×1920 + pcm_alaw**；缩略图落盘
    `data/snapshots/34020000001320128497_34020000001310000001.jpg`（98,327 字节）；
  - 停止点播：ZLM 资源与流列表均清空。
- **核心闭环（模拟设备 + 真实 ZLM）**：历史回放 + 回放控制、对讲（假设备收到 RTP 包）、
  语音广播、设备控制 5 项、JT1078 终端/通道、级联平台、云录像全链路。
- **三方言运行时验证**：PostgreSQL 与 MySQL 都真机跑过全量冒烟，抓到并修掉
  数十处只在非 SQLite 上炸的缺陷（占位符语法、列类型、`INSERT ... RETURNING`、
  `CAST` 等）。
- **真实 ZLM 集成**：hook 全链路、流列表、录制文件落盘/删除、多节点负载均衡。
- **测试基线（2026-09-19 实测）**：`cargo test --no-fail-fast` **750 通过 / 0 失败 / 3 忽略**。
- **e2e 体系**：Playwright 17 个 spec / 59 条 `test()`，覆盖设备/通道/直播/回放/云录像/
  推流/代理/平台/区域/媒体节点/终端/日志/报警/控制台等页面；最近一次运行
  **35 通过 / 0 失败**（`e2e/test-results/.last-run.json` 为 `status: passed`）。

> ⚠️ e2e 最近一次运行是**筛选后的 35 条**，不是 59 条全量；**三方言冒烟与全量 e2e
> 在本轮未重跑**。上表中非真机部分为历史验证结论。

## 7. 相关文档

| 文档 | 用途 |
|------|------|
| [`ROADMAP_2.0.md`](ROADMAP_2.0.md) | 2.0.0 迭代顺序、范围与验收条件 |
| [`OPEN_ISSUES.md`](OPEN_ISSUES.md) | **未完成 / 未验证事项**（唯一待办表） |
| [`DEPLOYMENT_GUIDE.md`](DEPLOYMENT_GUIDE.md) | 构建、运行、部署分级、配置、监控、灾备、升级 |
| [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md) | 写多方言 SQL 的注意事项 |
| [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) | `stub.rs` / `device_stub.rs` 的真实定位与清退评估（当前不建议清退） |
| [`../web/README.md`](../web/README.md) | 前端说明 |
| [`../e2e/README.md`](../e2e/README.md) | 端到端测试说明 |
| [`../mock/README.md`](../mock/README.md) | 模拟测试资源 |
