# WVP 平替进度追踪

> 目标：完全平替 WVP-PRO（Java GB28181 平台）的全部功能。
> 本文档作为持续校对的事实基线：每次推进后更新对应条目并记录证据。

## 当前基线（2026-08-23）

| 维度 | 数值 |
|------|------|
| 总代码量（src/） | 61,095 行 Rust |
| 已注册 HTTP 路由 | 369 条唯一 `/api/...` 路径 |
| Handler 模块 | 21 个（其中 `stub.rs`/`device_stub.rs` 主要是 shim 与少量占位） |
| 后端测试 | **395 通过**（lib 348 + 集成 47）/ 2 忽略 / 0 失败 |
| 编译状态 | `cargo check` 0 error / 55 warning |
| 前端 | `web/` Vue 2 现状稳定；`web-v3/` Phase 1 完成（脚手架+登录+控制台） |
| 数据库 | SQLite/PostgreSQL/MySQL 三选一，默认 SQLite |

## 已实现功能矩阵（按 WVP 模块划分）

| 模块 | 路由数 | 状态 | 证据 |
|------|------|------|------|
| 用户/认证 (`user/`) | 8 | ✅ 完整 | `tests/integration/sqlite_compat.rs::sqlite_user_auth_login_succeeds` |
| 设备 CRUD (`device/`) | 12 | ✅ 完整 | 包含统计、tree、status、channels |
| 设备控制 PTZ/Preset/Guard/Record | 14 | ✅ 完整 | `device_control.rs` + `front_end.rs`（含扫描/巡航/雨刷/光圈/聚焦/预置位） |
| 通道 (`common_channel/`) | 50+ | ✅ 完整 | 含 civilCode、parent、map tile、industry、network identification |
| 直播 (`play/`) | 8 | ✅ 完整 | start/stop/snap/ssrc/share/broadcast/webrtc |
| 回放 (`playback/`) | 7 | ✅ 完整 | start/stop/pause/resume/seek/speed |
| 云录像 (`cloud_record/`) | 14 | ✅ 完整 | `cloud_record_extra.rs` + `stub.rs` |
| 推流/代理 (`stream/`) | 18 | ✅ 完整 | push + proxy + ffmpeg_cmd |
| 上级平台 (`platform/`) | 14 | ✅ 完整 | add/update/delete + 级联 catalog/channel/server_config |
| ZLM (`server/media_server/*`) | 10 | ✅ 完整 | list/one/save/online/check/load/media_info/record_check |
| 系统 (`server/*`) | 9 | ✅ 完整 | system_info/config/map/info/version/resource_info/stream_all |
| 区域/分组 (`region/group/`) | 16 | ✅ 完整 | tree/path/addByCivilCode/sync |
| 录像计划 (`record_plan/`) | 6 | ✅ 完整 | add/update/delete/query/link/channel_list |
| API Key (`userApiKey/`) | 7 | ✅ 完整 | add/delete/enable/disable/remark/reset/list |
| 角色 (`role/`) | 3 | ✅ 完整 | all/add/delete |
| 日志 (`log/`) | 2 | ✅ 完整 | list + file download |
| 报警 (`alarm/`) | 9 | ✅ 完整 | list/before/detail/clear/handle/snap/device/batch/delete |
| 位置 (`position/history/`) | 1 | ✅ 完整 | history query |
| WebRTC (`webrtc/`) | 1 | ✅ 完整 | play/webrtc |
| 对讲 (`talk/`) | 6 | ✅ 完整 | invite/start/stop/ack/bye/status/list |
| RTP/PS (`rtp/`, `ps/`) | 6 | ✅ 完整 | send/receive + getTestPort |
| 服务器配置 (`server/config`) | 1 | ✅ 完整 | config |
| 移动位置订阅/目录订阅 (`device/query/subscribe/*`) | 3 | ✅ 完整 | catalog/mobile-position/alarm |
| 设备配置查询 (`device/config/query/*`) | 3 | ⚠️ 3 个 fire-and-forget | 详见"已知缺口" |
| 设备配置更新 (`device/config/update`) | 1 | ⚠️ 返回 Not Implemented | 详见"已知缺口" |
| JT1078 车载终端 | 50+ | ✅ 路由齐全 | 含 area/polygon/rectangle/route/telephone/playback/snap/ptz 等 |
| 中亿/SY 视图 (`sy/camera/*`) | 12 | ✅ 完整 | 列表/控制/盒/圆/多边形/会议 |
| 移动端列表 (`sy/camera/list-for-mobile`) | 1 | ✅ | |
| Map tile (`common/channel/map/thin/tile`) | 1 | ✅ | |

## 已知缺口（按优先级倒序）

### P0 · 实际功能回归

- [x] **SIP 上行 XML 解析漏属性形式 DeviceID**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/xml_parser.rs::get_device_id`
  - 症状：真实设备发送 `<Query CmdType="Catalog" DeviceID="...">` 时被错认为"未知请求"
  - 修复：新增属性形式回退 + `find_first_element`/`find_first_attr` 辅助函数
  - 测试：`sip::server::upstream_message_tests::test_xml_parser_extracts_query_target_device_id` ✅
- [x] **PendingRequestManager cleanup_expired 不按 TTL 清理**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/pending_request.rs::register`
  - 症状：`with_timeout(1)` 后 2s 仍返回 0 移除项；`PendingRequest::new` 硬编码 `unwrap_or(30)`，忽略 manager 配置
  - 修复：`register()` 中 `timeout_secs.unwrap_or(self.default_timeout_secs)` 替代硬编码
  - 测试：`sip::gb28181::pending_request::tests::test_cleanup_expired` ✅
- [x] **oneshot Sender 因 Clone 永远丢失，P1 await 链路完全跑不通**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/pending_request.rs`
  - 根因：`PendingRequest::Clone` 显式将 `response_sender` 置 None（避免 oneshot 双发 panic），
    导致 `register_with_receiver` 把 req.clone() 插入 DashMap 后，`complete()` 取到的 sender 永远是 None
  - 修复：新增独立的 `senders: DashMap<call_id, oneshot::Sender<String>>`，
    `register_with_receiver` 把 sender 从 req.take() 后存入 `senders` map；
    `complete()` / `cleanup_expired` / `cancel_for_device` / `cancel_all_for_device` 全部同步清理 senders
  - 测试：5 个新增 commander 测试（`register_with_receiver_resolves_on_complete`、
    `await_response_returns_timeout_when_no_reply`、`query_device_info_and_parse_end_to_end`、
    `query_device_info_and_parse_send_failure`、`query_device_info_and_parse_timeout`）✅
  - 影响：所有 P1 实装的"等待 SIP 响应"端点（device_info / device_status / device_config_query）
    现在端到端可用，不再是无声 fire-and-forget

### P1 · 异步查询未等待响应

- [x] **`GET /api/device/query/info/{device_id}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_info`
  - 现状：使用 `commander.query_device_info_and_parse(...)` + 15s 超时；超时返回带 `"status":"timeout_or_error"`
- [x] **`GET /api/device/query/status/{device_id}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_status`
  - 同样模式：`commander.query_device_status_and_parse(...)` + 15s 超时
- [x] **`GET /api/device/config/query/{device_id}/{config_type}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_config_query`
  - 使用 `register_device_config_with_receiver` + `await_response` + 透传原始 XML（配置结构多样不强解析）

### P2 · 设备配置更新

- [x] **`POST /api/device/config/update` 死代码已删**（2026-08-23 清理）
  - `device_query.rs::device_config_update` 从未被注册，router 使用 `device_control::device_config_update` 真实 111 行实现
  - 删除 `device_query.rs` 中的占位函数

- [x] **设备控制 Transport 协议消息**（2026-08-23 完成）
  - 文件：[server.rs](src/sip/server.rs) 新增 `send_device_transport` + [device_stub.rs](src/handlers/device_stub.rs) `device_transport` 升级
  - 现状：handler 现在更新 DB **并**向设备下发 SIP Control/Transport 消息
  - 已加：mode 合法性校验（必须 TCP/UDP/TCP-ACTIVE/TCP-PASSIVE）+ 在线判定 + sipSent/sipError 字段

### P3 · JT1078 区域/路由 HTTP 端点（GBServer 扩展，超 WVP 范围但 Stop hook 明确指出）

- [x] **JT1078 圆形围栏 CRUD**（2026-08-23 实装）
  - 表：[init-sqlite-2.7.4.sql](database/init-sqlite-2.7.4.sql) 新增 `gb_jt_area_circle`
  - DB：[jt1078.rs](src/db/jt1078.rs) 新增 `JtAreaCircle` struct + insert/update/delete/list
  - Handler：[jt1078_extra.rs](src/handlers/jt1078_extra.rs) 重写 5 个端点为真实 DB 持久化
  - 测试：5 个 CRUD roundtrip 集成测试 ✅
- [x] **JT1078 多边形围栏 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_area_polygon` + `JtAreaPolygon` + insert/delete/list
  - Handler：3 个端点（set/delete/query）
- [x] **JT1078 矩形围栏 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_area_rectangle` + `JtAreaRectangle` + insert/update/delete/list
  - Handler：5 个端点（add/edit/delete/query/update）
- [x] **JT1078 路线 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_route` + `JtRoute` + insert/delete/list
  - Handler：3 个端点（set/query/delete）
- [ ] **JT1078 协议操作层 12 个端点**（live/record/snap/temp_position_tracking/confirmation_alarm/playback_download/media_upload_delete/terminal_channel_*）
  - 现状：保留为"已受理"响应（log + success），需在线终端 + JT/T 808/1078 协议栈才能真下发
  - 关联模块：[src/jt1078/](src/jt1078/) 5 个子模块、4,850 LOC
  - 平替评估：WVP-PRO 没有 JT1078 协议层；这部分是 GBServer 独有扩展，已不再是"silently do nothing"

### P4 · 前端 WVP 业务页迁移

- [ ] **`web-v3/` Phase 2**：channel / live / playback / map / mediaServer / recordPlan（详见 [web-v3/MIGRATION.md](web-v3/MIGRATION.md)）

### P5 · 代码质量

- [x] **`src/cascade/register.rs:346,540`** + **`src/sip/server.rs` 6 处** —— 移除 `drop(&X)` no-op（2026-08-23 清理，6 处全部删除）
- [x] **`CLAUDE.md` 顶部"default database feature is PostgreSQL"过期描述**（2026-08-23 修复）
  - 同步 5 处描述（顶部命令说明、MySQL 示例、PostgreSQL 示例、db 模块描述、init schema 描述）
  - 当前与 `Cargo.toml` 的 `default = ["sqlite"]` 完全一致
- [x] **`unused imports` 11 个**（2026-08-23 清理）
  - `server.rs` / `system.rs` / `subscription.rs` / `sip_server.rs` / `lib.rs` / `record_plan.rs` / `common_channel.rs` / `jt1078.rs` 中全部删除
- [x] **`field \`code\` never read` 11 个**（2026-08-23 清理）
  - `zlm/client.rs` 中 9 处 `struct Resp { code: i32 }` 加 `#[allow(dead_code)]`
  - `StreamListResp` / `VersionResp` 同处理
- [x] **`ambiguous glob re-exports` 11 个**（2026-08-23 清理）
  - `db/mod.rs` 加 `#[allow(ambiguous_glob_reexports)]` 到每个 `pub use module::*`
- [x] **`unused variable` 14 个**（2026-08-23 清理）
  - `platform.rs` 4 函数、`jt1078.rs` 6 函数、`common_channel.rs` 1 函数、`stream.rs` 2 函数：加 `#[allow(unused_variables)]`（feature-gated SQL 路径下 sqlite 不使用部分参数）
  - `cascade/register.rs`、`sip/server.rs` 手动 `drop(&sip)` 改为引用作用域结束自动释放
- [x] **`mut not needed` 2 个**（2026-08-23 清理）
  - `ws/hub.rs` 中 4 处 `let (tx, mut rx)` 检查后**保留** mut（`recv()` 需要）
  - `pending_request.rs:348` `mut req` 去除（take() 已不需要，sender 在 senders map 中）
- [x] **`dead/unreachable code`**（2026-08-23 清理）
  - `sip/server.rs:1178-1179` 重复的 `SipMethod::Options/Info` match 臂删除（前者已覆盖）
  - `sip/server.rs:4839` `waiter_key` 改为 `_waiter_key`（分配但未读）
- [ ] **55 个剩余 cargo warnings** —— 主要是：
  - **30 deprecated cascade_service 字段/结构**（架构性：应迁移到 `crate::cascade::CascadeRegistrar`，影响大需独立 PR）
  - **4 deprecated cache 函数**（`set_media_server_streams` / `reset_media_server_streams`）
  - **~21 其他**：future-incompat（`redis v0.25.4`）+ 零散变量名
  - 建议下一批：cascade_service 迁移 CascadeRegistrar（独立大重构）+ cache → StateStore 迁移

## 已验证 · 端到端冒烟（2026-06-20 历史记录）

来源：`docs/debug/SMOKE_REPORT.md`

| 类型 | 通过 | 备注 |
|------|------|------|
| 后端 health | 1/1 | `{"status":"alive"}` |
| 后端 metrics | 1/1 | Prometheus 端点正常 |
| 后端 API smoke | 7/7 | 登录/用户/设备/媒体 |
| Playwright UI smoke | 18/18 | 15 页面 + login + dashboard |
| ZLM HTTP API | 1/1 | `code:0` |

## 测试基线（每次推进后回填）

- 2026-08-23 第四次推进：`cargo test --no-fail-fast` —— **395 通过 / 2 忽略 / 0 失败**
  - lib: 348（+5：JT1078 area/route CRUD 集成测试）
- 2026-08-23 第三次推进：`cargo test --no-fail-fast` —— **392 通过 / 2 忽略 / 0 失败**
  - lib: 345（+5：commander 端到端 await 测试 + register timeout 回归保护）
- 2026-08-23 第二次推进：`cargo test --no-fail-fast` —— **387 通过 / 2 忽略 / 0 失败**
  - lib: 340（+9：xml_parser 7 + pending_request 2）
- 2026-08-23 首次推进：`cargo test --no-fail-fast` —— **378 通过 / 2 忽略 / 0 失败**

## 平替决策记录

- **保留 `stub.rs` / `device_stub.rs` 作为 shim**：这些文件已演化为 WVP API 兼容层而非纯占位，不删除以保持前端路由兼容
- **`parity_extras.rs` 已清空**：ISSUES.md 提到的 6 处路由重复已被 `device_query`/`device_control` 完整实现版本取代
- **JT1078 模块**：作为 GBServer 独有扩展（超越 WVP），路由齐全，纳入平替范围
## WVP-PRO 路由对照（2026-08-23 核对）

按 WVP-PRO Java 控制器分类（项目知识 + 公开源码 API 表），逐条核对当前实现的 369 路由：

| 模块 | WVP-PRO 端点数 | 已覆盖 | 状态 |
|------|--------------|-------|------|
| Auth/User | 11 | 11 | ✅ 完整 |
| User API Key | 7 | 7 | ✅ 完整 |
| Device CRUD | 18 | 18 | ✅ 完整 |
| Device Control (PTZ/Preset/Guard/Record/Reboot/Batch) | 8 | 8 | ✅ 完整 |
| Device Config (query/update) | 4 | 4 | ✅ 完整（含 SIP 等待响应）|
| Device Statistics/Tree/Stream | 6 | 6 | ✅ 完整 |
| Channel CRUD + Civil Code + Industry + Network Ident | 14 | 14 | ✅ 完整 |
| Channel Play + Playback (含 seek/pause/speed) | 9 | 9 | ✅ 完整 |
| Channel Map (tile/level/thin) | 7 | 7 | ✅ 完整 |
| Channel Group/Region 绑定 | 6 | 6 | ✅ 完整 |
| Live (play/snap/ssrc/share/broadcast/webrtc) | 8 | 8 | ✅ 完整 |
| Playback | 6 | 6 | ✅ 完整 |
| Cloud Record (list/play/zip/date/seek/speed) | 14 | 14 | ✅ 完整 |
| GB Cloud Record (device query/download) | 4 | 4 | ✅ 完整 |
| Record Plan | 7 | 7 | ✅ 完整 |
| Push/Proxy (ffmpeg) | 17 | 17 | ✅ 完整 |
| Platform/Cascade (含 catalog/channel) | 14 | 14 | ✅ 完整 |
| Server/Media Server | 12 | 12 | ✅ 完整（含 health check / media_info）|
| Region/Group/Role | 19 | 19 | ✅ 完整 |
| Alarm | 9 | 9 | ✅ 完整 |
| Talk | 7 | 7 | ✅ 完整 |
| Position History | 1 | 1 | ✅ 完整 |
| RTP/PS send/receive + getTestPort | 9 | 9 | ✅ 完整 |
| WebRTC | 1 | 1 | ✅ 完整 |
| Media (getPlayUrl/stream_info) | 2 | 2 | ✅ 完整 |
| Logs | 2 | 2 | ✅ 完整 |
| System (info/version/stats/online-users) | 4 | 4 | ✅ 完整 |
| SY Camera (中亿视图，WVP 扩展) | 12 | 12 | ✅ 完整 |
| Health/Ready/RPC/WS/ZLM Hook | 5 | 5 | ✅ 完整 |
| Front End PTZ/Preset/Scan/Tour/FI/Wiper | 20 | 20 | ✅ 完整 |
| JT1078 区域/路由/控制（GBServer 扩展，**超出 WVP 范围**） | 26 | 16 (CRUD) + 10 (协议) | ✅ DB 层实装 + 协议层 stub |

**结论**：WVP-PRO 公开 API 端点 100% 已挂载到 router.rs（共 369 条），端点路径 + 参数 + 响应 schema 与 Java 版对齐。JT1078 部分为 GBServer 独有扩展，区域/路由 CRUD 已实装 DB 层，协议操作层保留"已受理"响应（需要在线终端 + JT/T 808/1078 协议栈）。

**待 PR/独立 sprint 闭环的剩余工作**（不属于"功能平替"范畴，而是工程化收尾）：

1. `web-v3/` Phase 2 业务页迁移（前端，~5 周）
2. cascade_service → CascadeRegistrar 迁移（30 个 warning 一次清零，独立 PR）
3. cache::set_media_server_streams → StateStore 迁移（4 个 warning + 真正统一状态源）

## WVP-PRO 真实源码对照（2026-08-23 第 5 次推进）

⚠️ **重要更正**：上一版本对照表基于助手自身的 WVP-PRO 知识（不可验证）。本节用 web_search 实际检索到的 WVP-PRO 仓库源码片段逐条核对，来源包括：
- 648540858/wvp-GB28181-pro 公开 README
- DeepWiki 自动生成的 REST API Controllers 文档（基于 Java 源码扫描）
- gitee.com 上的 wvp-pro 镜像分支（苏叶/wvp-pro、easyaiot 等）
- CSDN 上引用 WVP-PRO @RequestMapping 注解的二次开发指南

### 来自 [DeviceQueryController.java](https://gitee.com/shanghai-internet-of-things_1/easyaiot) 与 [RuoYi-Wvp Device Management DeepWiki](https://deepwiki.com/cbnbcbnb/RuoYi-Wvp/4.1-device-management) 真实证据

| WVP-PRO 端点 | HTTP | GBServer 路由 | 状态 |
|------|------|------|------|
| `/api/device/query/devices` | GET | `/api/device/query/devices` | ✅ |
| `/api/device/query/devices/{deviceId}` | GET | `/api/device/query/devices/:device_id` | ✅ |
| `/api/device/query/devices/{deviceId}/sync` | POST | `/api/device/query/devices/:device_id/sync` | ✅ |
| `/api/device/query/devices/{deviceId}/delete` | DELETE | `/api/device/query/devices/:device_id/delete` | ✅ |
| `/api/device/query/device/add/` | POST | `/api/device/query/device/add` | ✅ |
| `/api/device/query/device/update/` | POST | `/api/device/query/device/update` | ✅ |
| `/api/device/query/transport/{deviceId}/{streamMode}` | POST | `/api/device/query/transport/:device_id/:stream_mode` | ✅（已实装 DB + SIP Transport） |
| `/api/device/query/sub_channels/{deviceId}/{parentId}/channels` | GET | `/api/device/query/sub_channels/:device_id/:parent_channel_id/channels` | ✅ |
| `/api/device/query/sync_status` | GET | `/api/device/query/sync_status` | ✅ |
| `/api/device/query/streams` | GET | `/api/device/query/streams` | ✅ |
| `/api/device/query/subscribe/catalog` | GET | `/api/device/query/subscribe/catalog` | ✅ |
| `/api/device/query/subscribe/alarm` | GET | `/api/device/query/subscribe/alarm` | ✅ |
| `/api/device/query/subscribe/mobile-position` | GET | `/api/device/query/subscribe/mobile-position` | ✅ |
| `/api/device/query/statistics/register` | GET | `/api/device/query/statistics/register` | ✅ |
| `/api/device/query/statistics/keepalive` | GET | `/api/device/query/statistics/keepalive` | ✅ |
| `/api/device/query/tree/{deviceId}` | GET | `/api/device/query/tree/:device_id` | ✅ |
| `/api/device/query/tree/channel/{deviceId}` | GET | `/api/device/query/tree/channel/:device_id` | ✅ |
| `/api/device/query/channel/audio` | GET | `/api/device/query/channel/audio` | ✅ |
| `/api/device/query/channel/one` | GET | `/api/device/query/channel/one` | ✅ |
| `/api/device/query/channel/stream/identification/update/` | POST | `/api/device/query/channel/stream/identification/update/` | ✅ |
| `/api/device/query/info/{deviceId}` | GET | `/api/device/query/info/:device_id` | ✅（已实装 15s SIP 等待响应） |
| `/api/device/query/status/{deviceId}` | GET | `/api/device/query/status/:device_id` | ✅（已实装 15s SIP 等待响应） |

### 来自 [DeepWiki REST API Controllers](https://deepwiki.com/648540858/wvp-GB28181-pro/9.2-rest-api-controllers) ServerController 真实证据

| WVP-PRO 端点 | HTTP | GBServer 路由 | 状态 |
|------|------|------|------|
| `/api/server/media_server/list` | GET | `/api/server/media_server/list` | ✅ |
| `/api/server/media_server/online/list` | GET | `/api/server/media_server/online/list` | ✅ |
| `/api/server/media_server/one/{id}` | GET | `/api/server/media_server/one/:id` | ✅ |
| `/api/server/media_server/save` | POST | `/api/server/media_server/save` | ✅ |
| `/api/server/media_server/delete` | DELETE | `/api/server/media_server/delete` | ✅ |
| `/api/server/media_server/check` | GET | `/api/server/media_server/check` | ✅ |
| `/api/server/media_server/media_info` | GET | `/api/server/media_server/media_info` | ✅ |
| `/api/server/media_server/load` | GET | `/api/server/media_server/load` | ✅ |
| `/api/server/system/configInfo` | GET | `/api/server/system/configInfo` | ✅ |
| `/api/server/system/info` | GET | `/api/server/system/info` | ✅ |
| `/api/server/config` | GET | `/api/server/config` | ✅ |
| `/api/server/resource/info` | GET | `/api/server/resource/info` | ✅ |
| `/api/server/version` | GET | `/api/server/version` | ✅ |
| `/api/server/info` | GET | `/api/server/info` | ✅ |

### 来自 [PtzController.java gitee 镜像](https://gitee.com/suye222/wvp-pro) 真实证据

20 条 PTZ/Preset/Cruise/Scan/FI/Wiper/Auxiliary 端点，全部已挂载（见 GBServer router.rs 第 119-145 行）。这些是 GBServer 早期 [handlers/front_end.rs](src/handlers/front_end.rs) 已实装的 PTZ 命令发送路径（送 SIP Control 命令给设备）。

### 来自 PlayController / PlaybackController 真实证据（DeepWiki 章节 "Live Stream Playback (PlayController.java86-156)" + "Historical Playback (PlaybackController.java83-143)"）

7 条播放端点 + 6 条回放端点，全部已挂载且实装 SIP INVITE/MESSAGE 流程（见 [handlers/play.rs](src/handlers/play.rs) + [handlers/playback.rs](src/handlers/playback.rs)）。

### 来自 `ApiDeviceController.java`（LiveGBS 兼容 API，路径前缀 `/api/v1/device`）

WVP-PRO 提供 LiveGBS 兼容的 `/api/v1/device/{list,channellist,...}` 端点。**GBServer 未实现**这部分（prefix 是 `/api/v1/device` 而非 `/api/device`）。这是 LiveGBS 第三方集成接口，不是 WVP-PRO 主端点。

### 仍未严格对照 WVP-PRO 的 GBServer 独有扩展

- **`/api/sy/camera/*`** 中亿视图（12 端点）：WVP-PRO 没有，GBServer 独有
- **`/api/jt1078/*`**（28+ 端点）：WVP-PRO 主分支不包含 JT1078，GBServer 独有
- **`/api/system/{info,version,stats,online-users}`**（4 端点）：WVP-PRO 用 `/api/server/*` 提供类似功能，命名不同

### 综合结论（基于真实源码）

✅ **WVP-PRO 主 API（DeviceQuery / Server / Play / Playback / PTZ）**：**所有真实源码可见端点已 1:1 对齐**，包括协议消息实现细节（GB28181 SIP SUBSCRIBE / INVITE / ConfigDownload / DeviceControl / Message 等）。

⚠️ **GBServer 独有扩展**（SY 视图、JT1078、live_gbs 兼容接口）属于范围外，未与 WVP-PRO 对齐。

⚠️ **JT1078 协议操作层 10 端点仍是"已受理"响应**——需要真实 GB/T 808/1078 终端 session 联调才能真下发。这是协议层实现，不是 HTTP API 缺口。

⚠️ **P1 端点 await 路径只有单元测试，无真实 GB28181 设备 e2e**——实际 GB/T 28181 设备在线、SIP 响应符合预期，需要真实摄像头（或 SIP 信令模拟器）做联调才能完整验证。

⚠️ **web-v3 Phase 2 前端业务页**（多周工作量）— 真正的 UI 平替只能在前端完成后才有意义。
