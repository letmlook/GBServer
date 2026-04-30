# WVP 后端平替缺口分析

日期：2026-04-30

本文档记录当前 Rust 后端相对 WVP 后端“直接平替”的剩余差距。结论基于本仓库当前前端 API、`src/router.rs` 已挂载路由、`handlers`/`db` 实现状态、README 中历史说明，以及本轮代码阅读。

## 总体结论

当前项目已经具备 WVP 风格的接口外形，前端大多数请求不会 404；设备、通道、用户、平台、推流、拉流代理、ZLM Hook、云录像、录像计划等也已有不同程度的真实实现。

但距离“直接平替生产 WVP”仍有明显缺口，主要集中在：

- GB28181 SIP 信令链路尚未覆盖完整 WVP 行为。
- ZLM 管理和 Hook 只覆盖常用流状态、录像落库，缺少完整鉴权、配置下发、健康维护。
- 平台级联、JT1078、录像计划调度、日志审计、权限/API Key、地图抽稀等仍偏兼容响应或局部实现。
- 部分接口能返回成功，但没有真正向设备、上级平台或 ZLM 发出完整业务动作。

`cargo test --no-run` 当前可通过，但现有 warning 较多；这只能说明编译通过，不能证明协议级可替换。

## 已较接近可用的部分

| 模块 | 当前状态 | 主要文件 |
| --- | --- | --- |
| 用户登录/用户管理 | JWT 登录、用户 CRUD、密码修改基本可用 | `src/handlers/user.rs`, `src/db/user.rs` |
| 设备/通道基础查询 | 设备列表、通道列表、设备增删改、树查询、部分通道属性落库可用 | `src/handlers/device.rs`, `src/handlers/device_stub.rs`, `src/db/device.rs` |
| 实时播放基础链路 | 已能 open RTP server、发 GB28181 INVITE、生成播放地址；仍需完善 BYE/dialog 管理 | `src/handlers/play.rs`, `src/sip/server.rs` |
| 推流/拉流代理基础管理 | CRUD、启停、ZLM addStreamProxy、Hook 状态同步已具备基础能力 | `src/handlers/stream.rs`, `src/db/stream_push.rs`, `src/db/stream_proxy.rs`, `src/zlm/hook.rs` |
| 云端录像基础查询 | ZLM 录像回调可落库；云录像列表/播放路径部分查询 ZLM | `src/zlm/hook.rs`, `src/handlers/stub.rs`, `src/db/cloud_record.rs` |
| 录像计划 CRUD | 计划和计划项可增删改查 | `src/handlers/stub.rs`, `src/db/record_plan.rs` |
| 告警基础管理 | 告警列表、详情、处理、删除有 DB 实现 | `src/handlers/alarm.rs`, `src/db/alarm.rs` |

## P0：阻碍直接平替的核心缺口

### 1. GB28181 SIP 协议完整性不足

当前已有注册、Catalog、INVITE、PTZ、部分 SUBSCRIBE/NOTIFY 处理，但仍不是完整 WVP 行为。

主要风险：

- `play_stop` 当前调用 `send_talk_bye` 做通用 BYE，缺少清晰的播放会话 dialog/Call-ID 映射。
- 回放 `playback_start` 会发 `send_playback_invite`，但媒体端口、SSRC、设备回放控制、ACK/BYE 和 ZLM RTP 接收链路仍不完整。
- 设备控制接口有些会发 SIP Message，有些只是兼容成功响应。
- Catalog 同步依赖设备在线运行态；重启后的同步任务、任务状态、进度、错误码与 WVP 仍不一致。
- 设备注册、Keepalive、注销、离线、通道离线与 Redis/DB/WebSocket 的一致性还需要系统验证。

建议补齐：

- 建立统一 `InviteSession/Dialog` 映射，实时、回放、下载、语音广播分别维护会话。
- `play_stop`/回放停止使用对应 Call-ID 的 BYE，不再复用 talk BYE。
- 补齐 RecordInfo 查询、回放控制、下载、倍速、拖动的 GB28181 指令链路。
- 增加 GB28181 设备模拟器级集成测试。

### 2. ZLM 管理仍不是完整 WVP 管理面

已实现部分 ZLM client、流列表、RTP Server、StreamProxy、Hook 状态同步、录像回调落库。

剩余缺口：

- `/api/server/media_server/check` 能读取部分配置，但没有完整校验 secret、Hook、RTP 端口范围、RecordAssist。
- `/api/server/media_server/save/delete/load` 与 WVP 的自动配置、Hook 设置、节点启停健康检测仍不完整。
- `on_server_started` 只记录日志，没有重置节点状态、流计数、重新配置 Hook。
- `on_stream_not_found` 的自动拉流逻辑是简化推测，不等价于 WVP 按设备/通道查找并触发播放。
- 多 ZLM 节点负载、跨节点流状态、Redis 失效恢复还缺少完整策略。

建议补齐：

- 实现 ZLM Hook 全集处理：`on_server_started`、`on_server_keepalive`、`on_stream_changed`、`on_publish`、`on_record_mp4`、`on_rtp_server_timeout` 等。
- 保存媒体节点时同步配置 ZLM Hook、RTP、录像参数。
- 增加节点健康状态字段和定时探活。

### 3. 平台级联不完整

`/api/platform/*` 有 DB 和频道共享相关实现，但真正的级联 SIP 行为仍需补齐。

缺口：

- 向上级平台 REGISTER/UNREGISTER、Keepalive、Catalog NOTIFY、状态订阅、通道共享实时更新不足。
- 上级点播下级通道时的 INVITE 转发、RTP 推送、SSRC/SendRtp 管理不完整。
- `src/handlers/cascade.rs` 是完全占位模块，且当前没有挂到路由；不要把它误判为可用级联实现。

建议补齐：

- 明确 `platform.rs` 为唯一平台级联入口，删除或迁移 `cascade.rs`。
- 补齐上级平台注册状态机和 Catalog/DeviceInfo/DeviceStatus 响应。
- 实现上级点播、停止、回放级联。

### 4. JT1078 只是接口兼容，协议未真实实现

`src/handlers/jt1078.rs` 的终端/通道 CRUD 有部分 DB 实现；大量控制接口仅返回“指令已发送”。

明显缺口：

- 没有完整 JT/T 808、JT/T 1078 协议栈、鉴权、心跳、位置、终端参数、媒体属性、录像检索。
- `live_start` 只打开 ZLM RTP server，没有向终端发 9101/实时音视频传输请求。
- PTZ、文本下发、电话回拨、抓拍、控制门、电话本、参数设置等多数没有真实下发。
- 回放、下载、媒体列表返回简化数据或空列表。

建议补齐：

- 单独建设 JT808/JT1078 TCP/UDP 服务、消息编解码、终端会话管理。
- 将 HTTP 控制接口改为调用真实 JT 指令通道。
- 建立终端模拟器测试。

## P1：功能可见但业务深度不足

### 5. 录像计划只有 CRUD，没有调度执行

已实现：

- `/api/record/plan/get/add/update/query/delete/channel/list/link`
- 计划和计划项落库。

缺口：

- 没有后台调度器按计划调用 ZLM 开始/停止录制。
- 通道关联计划后，不会自动处理在线流、离线恢复、跨节点录制。
- 计划变更不会广播任务更新。

建议补齐：

- 增加 `RecordPlanScheduler`。
- 基于 stream online/offline Hook 启停 ZLM MP4 录制。
- 增加计划执行日志和失败重试。

### 6. 国标录像查询/下载仍偏简化

当前 `/api/gb_record/*` 会尝试从 ZLM MP4 文件或下载任务取数据，但不等价于 WVP 的设备侧录像检索。

缺口：

- 缺少 GB28181 `RecordInfo` 查询完整链路和响应聚合。
- 下载逻辑基于 ZLM download helper，未覆盖设备历史回放 RTP 接收、转 MP4、进度和停止。
- 时间过滤、录像类型、分页、错误码与 WVP 不完全一致。

### 7. 通用通道和地图功能有局部占位

已实现：

- 通用通道列表、编辑、重置、区域/分组绑定、地图列表、地图层级保存等。

缺口：

- 地图抽稀 `map/thin/*` 只是固定进度/空成功，没有真实抽稀任务。
- 部分前端控制依赖底层设备控制，离线或未注册时只能返回失败。
- 通道异常修复、父级/行政区规则与 WVP 仍需比对。

### 8. 日志和审计不完整

`/api/log/list` 当前可以避免页面失败，但不是完整 WVP 操作日志/日志文件管理。

缺口：

- 缺少操作审计表、请求记录、用户行为记录。
- 实时日志、历史日志、日志文件下载/筛选与 WVP 仍不完整。

### 9. 用户 API Key 管理没有接入鉴权

已实现：

- API Key 的新增、列表、启用、禁用、重置、备注、删除基本落库。

缺口：

- `auth_middleware` 只校验 JWT `access-token`/Bearer token。
- API Key 没有作为独立认证方式接入。
- 缺少过期时间、权限范围、接口访问审计和限流。

## P2：兼容性和生产化缺口

### 10. 接口响应结构和字段兼容仍需逐接口核对

虽然接口外形大多兼容，但 WVP 前端和第三方调用方通常依赖字段细节。

需要核对：

- 分页字段 `total/list`、`page/count`、空数据格式。
- 时间格式：字符串、秒、毫秒混用。
- `deviceId`/`channelId`/`gbId`/`gbDeviceId` 字段别名。
- 错误码和错误消息。

### 11. 多数据库兼容需要实测

代码中大量 `#[cfg(feature = "postgres")]` / `mysql` 分支已存在，但需要真实测试。

风险：

- SQL 方言、布尔值、`ON CONFLICT`/`ON DUPLICATE KEY`、自增 ID、保留字 `self`。
- 自动建表脚本拆分执行可能忽略失败。

### 12. 配置、部署、运维与 WVP 差异

缺口：

- WVP 常见配置项没有全部映射到 `config/application.yaml`。
- 没有完整迁移工具、数据兼容说明、Java WVP 数据库升级兼容策略。
- 运行时没有完善的健康检查、指标、任务状态页。

## 已挂载但仍需重点复核的接口清单

### 设备扩展

- `/api/device/query/sync_status`
- `/api/device/query/devices/:device_id/sync`
- `/api/device/query/transport/:device_id/:stream_mode`
- `/api/device/config/query/:device_id/BasicParam`
- `/api/device/query/streams`
- `/api/device/control/record`
- `/api/device/query/sub_channels/:device_id/:parent_channel_id/channels`
- `/api/device/query/tree/channel/:device_id`
- `/api/device/query/channel/audio`
- `/api/device/query/channel/stream/identification/update/`

### 媒体服务

- `/api/server/media_server/check`
- `/api/server/media_server/record/check`
- `/api/server/media_server/save`
- `/api/server/media_server/delete`
- `/api/server/media_server/media_info`
- `/api/server/media_server/load`
- `/api/server/map/model-icon/list`

### 回放/录像

- `/api/playback/start/:device_id/:channel_id`
- `/api/playback/resume/:stream_id`
- `/api/playback/pause/:stream_id`
- `/api/playback/speed/:stream_id/:speed`
- `/api/playback/seek/:stream_id/:seek_time`
- `/api/gb_record/query/:device_id/:channel_id`
- `/api/gb_record/download/start/:device_id/:channel_id`
- `/api/gb_record/download/progress/:device_id/:channel_id/:stream_id`
- `/api/cloud/record/*`

### JT1078

- `/api/jt1078/live/start`
- `/api/jt1078/playback/start/`
- `/api/jt1078/ptz`
- `/api/jt1078/config/get`
- `/api/jt1078/config/set`
- `/api/jt1078/attribute`
- `/api/jt1078/media/list`
- `/api/jt1078/talk/start`
- 其他控制类接口目前多数是兼容响应。

### 地图/通用通道

- `/api/common/channel/map/thin/*`
- `/api/common/channel/front-end/*`
- `/api/common/channel/playback/*`

## 建议实施顺序

1. 完成 GB28181 会话模型：实时播放、停止、回放、下载、录像查询全链路。
2. 完成 ZLM 节点管理和 Hook 全量处理，保证流状态和 DB 一致。
3. 完成平台级联 SIP 注册、Catalog、点播、回放。
4. 完成录像计划调度器和计划执行状态。
5. 根据是否必须支持车载业务，决定是否建设完整 JT1078 协议栈；否则在产品说明中标注为非平替范围。
6. 接入 API Key 鉴权、操作日志、权限控制。
7. 做 WVP 真实接口契约测试：用同一套前端和脚本分别打 Java WVP 与 Rust 后端，对比字段、状态码和行为。

## 备注

README 中“已实现接口”章节有历史滞后描述：部分曾标注为占位的推流、拉流代理、云录像、录像计划已经有真实实现；也有部分看似成功的接口仍只是兼容响应。后续应以本文档和代码现状为准，并在每轮补齐后同步更新本文档。
