# GBServer 当前状态

> 🔒 **代码功能已冻结（2026-09-19）。** 本文件描述**当前**状态；历史过程记录（逐轮修复叙述、
> 阶段性快照）已按「只保留最新状态」的要求清理，需要时走 git 历史。
>
> 本文档与 [`OPEN_ISSUES.md`](OPEN_ISSUES.md) 的分工：
> **本文档 = 已实现什么、为什么这么设计、已验证过什么**；**OPEN_ISSUES = 还没做什么**。

## 1. 规模基线（2026-09-19 实测）

| 维度 | 数值 | 复现命令 |
|------|------|----------|
| 后端代码量 | 87,246 行 Rust | `find src -name '*.rs' \| xargs wc -l` |
| 已注册路由 | 429 条唯一 `/api/...` 路径（`router.rs` 424 处 `.route(`） | `grep -oE '"/api/[^"]*"' src/router.rs \| sort -u \| wc -l` |
| Handler 模块 | 30 个（`stub.rs` / `device_stub.rs` **是真实实现**，非兼容 shim，见 §3） | `grep -c 'pub mod' src/handlers/mod.rs` |
| 后端测试 | **744 通过 / 0 失败 / 3 忽略** | `cargo test --no-fail-fast` |
| 编译 | `cargo check` 0 error；clippy 272 条告警（未清零） | `cargo check` / `cargo clippy --all-targets` |
| 前端 | 18 个业务视图目录、17 个类型化 API 模块、13 个通用组件、42 个 SVG 图标 | `ls web/src/views` |
| 旧 Vue 2 前端 | 已于 2026-09-19 删除，仅存 git 历史 | `git log --oneline -- web-legacy-vue2` |

## 2. 已实现能力矩阵

按业务模块划分。注意 `stub.rs` / `device_stub.rs` 承载的条目**不是 WVP 兼容 shim，
而是真实实现**（见 §3）。

| 模块 | 状态 | 说明 |
|------|------|------|
| 用户 / 认证 | ✅ 完整 | JWT（`access-token` / `Authorization: Bearer`）+ API Key（`X-API-Key` / `apiKey`），审计日志异步落库 |
| API Key | ✅ 完整 | add / delete / enable / disable / remark / reset / list |
| 角色 | ✅ 完整 | all / add / delete |
| 设备 CRUD | ✅ 完整 | 含统计、tree、status、channels |
| 设备控制 | ✅ 完整 | PTZ / 预置位 / 布防 / 录像 / 重启 / 批量；含扫描、巡航、雨刷、光圈、聚焦、拉框缩放 |
| 设备配置查询 / 更新 | ✅ 完整 | 走 SIP 并**等待设备响应**（15s 超时），非 fire-and-forget |
| 通道 | ✅ 完整 | 含 civilCode、parent、地图瓦片、行业编码、网络标识 |
| 直播 | ✅ 完整 | start / stop / snap / ssrc / share / broadcast / webrtc |
| 回放 | ✅ 完整 | start / stop / pause / resume / seek / speed |
| 云端录像 | ✅ 完整 | 计划 → 录制 → 落库 → 播放 → 删除全链路 |
| 推流 / 拉流代理 | ✅ 完整 | push + proxy + ffmpeg_cmd 启停 |
| 上级平台（级联） | ✅ 完整 | REGISTER 保活、目录/通道同步、级联推流、上级点播本级 |
| 媒体节点（ZLM） | ✅ 完整 | list / one / save / online / check / load / media_info / record_check；多节点最少负载选择 |
| 录像计划 | ✅ 完整 | 分钟 / ISO 星期口径、到点自动拉流录制 |
| 区域 / 业务分组 | ✅ 完整 | tree / path / addByCivilCode / sync；前端界面已补 |
| 日志 | ✅ 完整 | 列表 + 文件下载（三方言均可用） |
| 报警 | ✅ 完整 | list / before / detail / clear / handle / snap / device / batch / delete |
| 对讲 | ✅ 完整 | invite / start / stop / ack / bye / status / list；音频上行端到端验证过 |
| 语音广播 | ✅ 完整 | start / stop |
| RTP / PS | ✅ 完整 | send / receive + getTestPort |
| 移动位置 | ✅ 完整 | 订阅 + history 查询，打通两张位置表 |
| WebRTC | ⚠️ 部分 | 入口已接（直播页 + 通道播放对话框），**GB28181 流解不出帧**（见 OPEN_ISSUES B8） |
| JT1078 车载终端 | ✅ 路由齐全 | 终端 / 通道 / 围栏（圆/多边形/矩形）/ 路线 / 位置 / 多媒体检索 / 录像下载；**GBServer 独有扩展，超出 WVP 范围** |
| 中亿视图（SY） | ✅ 完整 | 列表 / 控制 / 盒 / 圆 / 多边形 / 会议 |
| 系统 / 监控 | ✅ 完整 | `/api/system/info`、`/metrics`（Prometheus）、`/api/health`、`/api/ready` |

## 3. 关键设计决策

保留这些结论是为了避免后人重复论证同一个问题。

- **`stub.rs` / `device_stub.rs` 是生产实现，不是待退役的兼容层**（2026-09-19 复核更正）：
  这两个文件名为 `stub`，但 65 个 entry **全部是真实实现**（落库 / 下发 SIP / 调 ZLM），
  没有空占位。其中 **49 条是当前 Vue 3 前端正在调用的活跃路径**。它们的自我描述
  「真实实现已迁出、仅挂旧路径」与实际不符 —— 实现从未迁出，函数体就在文件内。
  详见 [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md)。
- **JT1078 纳入平替范围**：WVP-PRO 没有 JT1078 协议层，这是本项目的**独有扩展**；
  协议操作层 12 个端点均为真实下发并等待终端通用应答，已无「已受理」占位响应。
- **状态源统一到 `StateStore`**：`src/cache.rs` 与 `src/sip/gb28181/cascade_service.rs`
  已确认零生产调用并整体删除（后者 751 行）；负载均衡回退链为
  `StateStore → ZLM 实时计数 → 首个节点`。
- **数据库三方言用一份 SQL 文本**：统一走 `dyn_where::dialect_sql()` 改写占位符，
  避免「改了这里忘了那里」。注意事项见 [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md)。
- **CI 自动触发保持关闭**：按用户要求只保留 `workflow_dispatch` 手动触发
  （`.github/workflows/ci.yml`），GitHub 层面也已 disable。

## 4. 前端 ↔ 后端契约审计结论

2026-09-12 对 **16 个 API 模块 / 130 条契约**做了逐条审计（对照 WVP-PRO Java 源码
与本仓库 router / DTO / 响应键名），逐模块结论如下。审计原文已归档至 git 历史。

| 模块 | 条目 | 结论 |
|------|------|------|
| `channel.ts` | 8 | ✅ 已修 |
| `alarm.ts` | 10 | ✅ 已修 |
| `live.ts` | 6 | ✅ 已修 |
| `user.ts` | 6 | ⚠️ 1 条仍在：`UsersQuery` 无 `query` 字段（当前无调用方传，无用户可见影响） |
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

## 5. 已验证过什么（证据类型）

说明这些能力**有实测证据**，不是「路由挂载即认为可用」：

- **核心闭环 9 条**：设备接入、实时点播 → ZLM → FLV 实拉、历史回放 + 回放控制、
  对讲（假设备收到 RTP 包）、语音广播、设备控制 5 项、JT1078 终端/通道、
  级联平台、云录像全链路。
- **三方言运行时验证**：PostgreSQL 与 MySQL 都真机跑过全量冒烟，抓到并修掉
  数十处只在非 SQLite 上炸的缺陷（占位符语法、列类型、`INSERT ... RETURNING`、
  `CAST` 等）。
- **真实 ZLM 集成**：hook 全链路、流列表、录制文件落盘/删除、多节点负载均衡。
- **e2e 体系**：Playwright 16 个 spec，覆盖设备/通道/直播/回放/云录像/推流/代理/
  平台/区域/媒体节点/终端/日志/报警等页面。

> ⚠️ e2e 在 2026-09-19 冻结轮**未重测**（需后端 :18080 + 前端 :9528 + 依赖服务同时在跑）。
> 上表为历史验证结论，不代表冻结时刻的复测结果。

## 6. 相关文档

| 文档 | 用途 |
|------|------|
| [`OPEN_ISSUES.md`](OPEN_ISSUES.md) | **未完成 / 未验证事项**（唯一待办表） |
| [`DEPLOYMENT_GUIDE.md`](DEPLOYMENT_GUIDE.md) | 构建、运行、部署分级、配置、监控、灾备、升级 |
| [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md) | 写多方言 SQL 的注意事项 |
| [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) | `stub.rs` / `device_stub.rs` 的真实定位与清退评估（冻结期内不执行） |
| [`../web/README.md`](../web/README.md) | 前端说明 |
| [`../e2e/README.md`](../e2e/README.md) | 端到端测试说明 |
| [`../mock/README.md`](../mock/README.md) | 模拟测试资源 |
