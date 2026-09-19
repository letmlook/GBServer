# GBServer 当前状态

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
| 后端测试 | **746 通过 / 0 失败 / 3 忽略**（15 个测试二进制） | `cargo test --no-fail-fast` |
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
| WebRTC | ⚠️ 部分 | ⚠️ | 入口已接（直播页 + 通道播放对话框）；真机流曾建立 rtc 播放会话，但**GB28181 流解不出帧**（见 OPEN_ISSUES B8） |
| JT1078 车载终端 | ✅ 路由齐全 | — | 终端 / 通道 / 围栏（圆/多边形/矩形）/ 路线 / 位置 / 多媒体检索 / 录像下载；**GBServer 独有扩展**；真终端未测（D2） |
| 中亿视图（SY） | ✅ 完整 | — | 已实现 13 条 `/api/sy/*`（list / list/ids / list-with-child / list-for-mobile / cont-with-child / box / circle / polygon / address / meeting/list / control/{play,stop,ptz}）；第三方对接所需的另 10 条未实现（A1） |
| 系统 / 监控 | ✅ 完整 | — | `/api/system/info`、`/api/server/system/info`、`/metrics`（12 个指标族）、`/api/health`、`/api/ready` |

## 3. 关键设计决策

保留这些结论是为了避免后人重复论证同一个问题。

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
- **测试基线（2026-09-19 实测）**：`cargo test --no-fail-fast` **746 通过 / 0 失败 / 3 忽略**。
- **e2e 体系**：Playwright 17 个 spec / 59 条 `test()`，覆盖设备/通道/直播/回放/云录像/
  推流/代理/平台/区域/媒体节点/终端/日志/报警/控制台等页面；最近一次运行
  **35 通过 / 0 失败**（`e2e/test-results/.last-run.json` 为 `status: passed`）。

> ⚠️ e2e 最近一次运行是**筛选后的 35 条**，不是 59 条全量；**三方言冒烟与全量 e2e
> 在本轮未重跑**。上表中非真机部分为历史验证结论。

## 7. 相关文档

| 文档 | 用途 |
|------|------|
| [`OPEN_ISSUES.md`](OPEN_ISSUES.md) | **未完成 / 未验证事项**（唯一待办表） |
| [`DEPLOYMENT_GUIDE.md`](DEPLOYMENT_GUIDE.md) | 构建、运行、部署分级、配置、监控、灾备、升级 |
| [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md) | 写多方言 SQL 的注意事项 |
| [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) | `stub.rs` / `device_stub.rs` 的真实定位与清退评估（当前不建议清退） |
| [`../web/README.md`](../web/README.md) | 前端说明 |
| [`../e2e/README.md`](../e2e/README.md) | 端到端测试说明 |
| [`../mock/README.md`](../mock/README.md) | 模拟测试资源 |
