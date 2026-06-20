# Findings & Decisions — Phase 6 / Phase 7 实施计划

## Requirements
- 输入：
  - `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6 / Phase 7
  - `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md`
  - `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md`
  - `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`
- 输出：
  - `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`
  - `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`
- 要求：与既有计划风格一致；可被 subagent 任务级执行；含 File Structure、子任务、关键代码骨架、验收命令、风险与衔接
- 风格：中文；技术名词保持英文；表格化呈现

## Research Findings

### 设计文档 §7 Phase 6 原文要点（JT808/JT1078）
1. **Goal**：make JT terminal workflows real rather than route-only.
2. **Tasks (5)**：
   - 6.1 terminal registration/auth、heartbeat、TCP/UDP transport、offline detection、DB mapping
   - 6.2 command correlation by msg_id + serial + phone
   - 6.3 live/start、live/stop、pause/continue/switch → real terminal command + ZLM media arrival
   - 6.4 record list、playback start/control/stop、download、media upload/list/delete
   - 6.5 PTZ、text、phonebook、fence/route、config query/set、attribute、driver、media attribute
3. **Acceptance**：simulator + ≥ 1 真实 JT 终端能注册、live start/stop、playback/control、record 查询、selected controls
4. **API route coverage**：含 WVP-Pro live pause/continue/switch 路径
5. **No default placeholder**：默认占位 coordinates、driver data、media attributes 不应作为生产数据
6. **Estimate**：4-6 周

### 设计文档 §7 Phase 7 原文要点（Redis / RPC / WebSocket / Operations）
1. **Goal**：complete production deployment features.
2. **Tasks (5)**：
   - 7.1 Redis-backed `StateStore` 覆盖 CSEQ/SN/SSRC、device/stream/session、GPS/alarm、push/proxy、platform SendRtp、WebSocket fanout
   - 7.2 cross-node RPC：device_control / play_stop / stream_state / platform play / SendRtp / cloud-record
   - 7.3 `/api/rtp` `/api/ps`、log download、system info、health/readiness、metrics
   - 7.4 public/protected routing for `/api/alarm/*` 与 `/api/ws`
   - 7.5 audit logs with real response statuses
3. **Acceptance**：单节点 + 两节点 Redis 部署通过核心协议 smoke；WebSocket 与流状态跨节点一致
4. **Estimate**：2-4 周

### 当前 JT1078 代码现状（直接来自仓库）
- `src/jt1078/mod.rs`（52 行）：模块声明 + `Jt1078Server` 容器（manager: `Arc<RwLock<Option<Arc<Jt1078Manager>>>>`）；注释「`Phase 6.3: JT1078 media session management`」已包含 jt_media_session
- `src/jt1078/manager.rs`（371 行）：`Jt1078Manager` 完整，sessions/terminal_addrs/seq_counters HashMap，22 个 `send_*` 命令方法（PTZ/live/playback/wiper/text/phonebook/config/...），`cleanup_loop` + retransmit hook，2 个测试
- `src/jt1078/server.rs`（140 行）：start() 绑定 **硬编码 `0.0.0.0:60000` TCP + UDP 同端口冲突**；每连接 spawn tokio task 喂 manager.feed_bytes → process_payload_for 返回 FrameKind；**FrameKind.AuthSuccess/Heartbeat/Data 没有真正派发到 handlers**
- `src/jt1078/session.rs`（315 行）：`Jt1078Session` 简单 token 认证 + last_heartbeat + 简单 length-prefixed reassembly + structured frame reorder；`process_payload` 返回 `FrameKind::{AuthSuccess, AuthFailure, Heartbeat, Data(Vec<u8>)}`
- `src/jt1078/frame.rs`（145 行）：`parse_jt1078_frame` + `parse_jt1078_structured_frame` 解析
- `src/jt1078/command.rs`（353 行）：21 个 `build_*` 编码函数（JT808 frame + 各消息体）
- `src/jt1078/command_waiter.rs`（400 行）：`JtCommandWaiter` 完整（phone+msg_id+serial → oneshot，timeout cleanup，parse_response 通用应答/location），**4 个测试** — **handlers 完全没挂载**
- `src/jt1078/jt_media_session.rs`（256 行）：`JtMediaSessionManager` 完整（create_live/activate/pause/resume/stop/update_position/update_speed），**3 个测试** — **handlers 完全没挂载**
- `src/handlers/jt1078.rs`（1503 行 35 个 handlers）：terminal_list/query/add/update/delete、channel_list/update/add、live_start/stop、playback_start/stop/control、ptz、wiper、fill_light、record_list、config_get/set、attribute、link_detection、position_info、text_msg、telephone_callback、driver_info、factory_reset、reset、connection、door、media_attribute、media_list、set_phone_book、shooting、talk_start/stop、media_upload_one
  - **live_start** 真发 9101 命令 + 调 ZLM openRtpServer，返回 127.0.0.1 URL；但**不调 JtCommandWaiter.wait_for_ack**，**不等 ZLM 媒体到达 hook**，**不调 JtMediaSessionManager.create_live/activate**
  - **live_stop** 真发 9102 close + ZLM closeRtpServer；但**不调 JtMediaSessionManager.stop**
  - **playback_start/control** 复用 send_playback / send_playback_control；但**不回放 start 多包等待**，**没有 pause/continue/switch 路径**
  - **record_list** 调 send_media_search；但**不等多包终端响应**
  - **attribute / media_attribute / config_get / config_set** 直接返回 `serde_json::json!({"code":0, "msg":"success", "data": default_data})` —— **占位数据**
  - **driver_info / factory_reset / door / text_msg** 走 send_command 但**无对应响应解析**
  - **talk_start/stop** 完全没实现 talk session（0x8900）
- `src/handlers/jt1078_extra.rs`（250 行）：confirmation_alarm、terminal_log_list 等
- `src/db/jt1078.rs`：`gb_jt_terminal` 表（phone_number/plate_no/status），4 套 SQL（postgres/mysql/sqlite 都已 cfg），增删改查分页

### 当前 Redis/StateStore/RPC 代码现状
- `src/state_store.rs`（1062 行）：
  - `DeviceOnlineState` / `StreamState` / `InviteSessionState` / `MediaServerLoad` / `MobilePositionState` / `ActiveRecordingState` / `CascadeSendRtpState` 7 个数据模型
  - `StateBackend` trait：18 个方法（device_online_*、stream_*、invite_*、media_server_*、position_*、cascade_sendrtp_*、active_recording_*）
  - `InMemoryBackend`：std RwLock + block_in_place 桥接
  - `RedisBackend`：ConnectionManager + 1.5s 超时；所有方法 grace no-op
  - `StateStore` 统一门面 + broadcast::Sender<StateEvent>
  - **缺**：`CSEQ/SN/SSRC` 计数器 / `alarm_pub_sub` / `gps_history` / `websocket_fanout` / `push_message` / `proxy_message`
  - **缺**：RedisBackend **连接失败告警后端**（只 tracing::warn）
  - **缺**：Redis pub/sub channel（用于 WebSocket 跨节点）
- `src/rpc.rs`（419 行）：
  - `RpcTransport` trait + `LocalRpc` 实现（broadcast::Sender）+ `RpcRouter`（HashMap handlers）
  - `register_standard_handlers`：device_control / play_stop / cloud_record_sync —— **全是 stub handler（payload echo 返回 ok）**
  - `RpcRouter::spawn_listener` + 处理循环 + 测试覆盖 5 case
  - **缺**：`RedisRpc` 实现（仅 docstring 提及）
  - **缺**：device_control / play_stop / cloud_record_sync 真实处理逻辑（应调用 SipServer.send_device_control 等）
- `src/handlers/rtp_control.rs`（157 行 D2 已完成）：`/api/rtp/{receive,send}/*` + `/api/ps/{receive,send}/*` + `/api/ps/getTestPort`，纯 ZLM 透传
- `src/handlers/websocket.rs`：in-memory `WsState.tx_map`，单节点 broadcast
- `src/handlers/rtp_control.rs` 中 `rtp_send_stop` 仅返回 success，**不清理 SendRtp session**
- `src/router.rs`：
  - `/api/rtp/*` 与 `/api/ps/*` 都**挂在 api_public**（line 230-237）
  - `/api/alarm/*` 部分在 `api_public`（parity_extras::alarm_clear/snap line 882-883）+ 部分在 `api_protected`（alarm::alarm_list 等 line 960-966）—— **不一致**
  - `/api/ws` 在 line 956，挂在顶层（绕过 auth）
  - `/api/health` + `/metrics` 在 api_public

### Phase 6 子任务映射

| 设计文档 | 当前完成度 | Phase 6 子任务 |
|---|---|---|
| 6.1 注册/auth/heartbeat/TCP/UDP/DB | server.rs 硬编码 60000 + 简单 token；Jt1078Session 简单认证；TCP/UDP 都同端口冲突 | 6.1 真实注册/认证/独立 TCP/UDP 端口配置 + 离线检测 + DB 映射 |
| 6.2 command correlation by msg_id + serial + phone | `JtCommandWaiter` 完整未挂载；handlers 一律 fire-and-forget | 6.2 把 JtCommandWaiter 挂入所有 send_command 路径，handlers 改为 await ack |
| 6.3 live start/stop + 媒体到达 | live_start 真发 9101 + openRtpServer，但**不等 ack 不等 hook**；**不挂 JtMediaSessionManager** | 6.3 完整直播链路：等 ack → 等 ZLM 媒体到达 → 激活会话 → URL 才返回 |
| 6.3 playback pause/continue/switch + 多包 | send_playback_control 已实现；**没有 pause/continue/switch 路径** | 6.3b playback 多包 + 4 种 control |
| 6.4 record list 多包 + download + media upload | record_list 调 send_media_search 但不等响应 | 6.4 record list 多包等待 + 落库 |
| 6.5 PTZ/text/phonebook/fence/route/config/attribute/driver/media attr | send_* 方法都实现，handlers 走通但**attribute/media_attribute/config_* 返回默认数据** | 6.5 attribute/media_attribute/config_* 真实查询 + fence/route/talk 补全 |
| 6.6 横切 | 无 | 6.6 三库 cfg + JT 模拟器端到端测试 + 文档 |

### Phase 7 子任务映射

| 设计文档 | 当前完成度 | Phase 7 子任务 |
|---|---|---|
| 7.1 Redis StateStore 全覆盖 | 7 个 state 已实现（device/stream/invite/media_server/position/cascade_sendrtp/active_recording）；**缺 CSEQ/SN/SSRC 计数 + alarm pub/sub + gps_history + websocket_fanout + push/proxy message** | 7.1 扩展 StateStore：CSEQ/SN/SSRC 原子计数 + alarm pub/sub + gps_history + push/proxy message |
| 7.2 跨节点 RPC | LocalRpc 完整；RpcRouter + 3 个 stub handler；**缺 RedisRpc** | 7.2 实现 RedisRpc + 把 stub handler 替换为真实 handler（调 SipServer / CascadeRegistrar / cloud_record） |
| 7.3 `/api/rtp` `/api/ps` / log download / system info / health / metrics | `/api/rtp/*` `/api/ps/*` 纯 ZLM 透传（D2）；**无 log download / system info / readiness**；health 单点 ZLM 探测 | 7.3 RTP/PS 协议语义（rtp_send_stop 清理 SendRtp session）+ log download + system info + health+readiness |
| 7.4 public/protected 路由 | `/api/rtp/*` `/api/ps/*` 在 public；`/api/alarm/*` 跨 public/protected；`/api/ws` 在顶层 | 7.4 统一路由策略：把 alarm 全移到 protected + 文档 `/api/rtp` `/api/ps` public 策略 |
| 7.5 audit logs real response | auth.rs 已 `db::audit_log::insert` 但**fire-and-forget + 未记录 response status** | 7.5 audit_log 增 status_code + response_payload 字段 + 同步写 |
| 7.6 WebSocket 跨节点 | ws_handler 内存 tx_map；**单节点** | 7.6 WebSocket 跨节点 fanout：Redis pub/sub channel 推送 |
| 7.7 横切 | 无 | 7.7 三库 cfg + 两节点部署 smoke + 文档 |

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Phase 6 不重写 server.rs 的硬编码 60000；改为读取 `Jt1078Config.tcp_port` / `udp_port` 分离 | 真实部署需要 TCP/UDP 独立端口 |
| Phase 6 不引入新的 service 层；直接挂 `JtCommandWaiter` / `JtMediaSessionManager` 到 handlers | 单点改动 |
| Phase 6 live_start 复用 Phase 3.1 的"等 ZLM 媒体到达 hook"模板 | 保持架构一致 |
| Phase 6 record_list 复用 Phase 3.3 的 `accumulate_record_info` 模式（多包 + 超时） | 保持架构一致 |
| Phase 7 新增 `RedisRpc` 走 Redis pub/sub，复用 RpcRouter；不破坏 LocalRpc | 单机 + 集群双模式 |
| Phase 7 WebSocket 跨节点：StateStore 增 `ws_fanout` channel + ws_handler 在订阅 broadcast 同时也订阅 StateStore 的 ws_fanout 事件 | 复用 StateStore 抽象 |
| Phase 7 audit_log 字段增 status_code + response_payload + duration_ms；fire-and-forget 改为 tokio::spawn 异步写 | 不阻塞 handler |
| Phase 7 路由策略不重写 router.rs 整体；只在 api_public / api_protected 边界精确化 | 最小改动 |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| JT1078 TCP/UDP 都绑 0.0.0.0:60000 端口冲突 | 6.1 拆为独立 TCP/UDP 端口（来自 config） |
| `JtCommandWaiter` 已实现但 handlers 未使用 | 6.2 改所有 send_* 路径：先 register() → 拿到 rx → 发命令 → 等 rx → 解析 |
| live_start 不等 ZLM 媒体到达 | 6.3 复用 Phase 3.1 media_arrival_waiter |
| `JtMediaSessionManager` 已实现但 handlers 未使用 | 6.3/6.4 显式 create_live/activate/stop/update_position |
| `attribute` / `media_attribute` / `config_*` 返回默认 JSON | 6.5 改用 JtCommandWaiter 真实查询 + 等响应解析 |
| `LocalRpc` 3 个标准 handler 是 stub | 7.2 改为真实调用 SipServer / CascadeRegistrar / cloud_record |
| `RedisRpc` 未实现 | 7.2 新建 RedisRpc（pub/sub） |
| WebSocket 仅单节点内存 fanout | 7.6 走 Redis pub/sub 跨节点 |
| `/api/alarm/*` 跨 public/protected 边界 | 7.4 全部移 protected |
| `/api/ws` 在 auth 之外 | 7.4 文档化为「public by design」（WVP-Pro 标准） |
| audit_log 不记录 response status | 7.5 字段扩展 + 异步写 |
| 缺 log download / system info / readiness | 7.3 新增 3 个端点 |

## Final Deliverables
- `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`（~600 行）
- `docs/superpowers/plans/2026-06-20-phase-7-impl-plan.md`（~500 行）

## Resources
- 设计文档：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`
- 上游基线：WVP-Pro 2.7.4 / commit b760458
- 既有计划：
  - `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md`
  - `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md`
  - `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`
- JT1078 关键代码：
  - `src/jt1078/{mod,server,manager,session,frame,command}.rs` — 协议栈
  - `src/jt1078/command_waiter.rs` — 命令关联（未挂载）
  - `src/jt1078/jt_media_session.rs` — 媒体会话（未挂载）
  - `src/handlers/jt1078.rs` — 1503 行 35 handlers
  - `src/db/jt1078.rs` — `gb_jt_terminal` 表
- Redis/RPC 关键代码：
  - `src/state_store.rs` — 1062 行 StateBackend/StateStore
  - `src/rpc.rs` — 419 行 LocalRpc/RpcRouter/handlers
  - `src/handlers/rtp_control.rs` — D2 RTP/PS 路由
  - `src/handlers/websocket.rs` — 单节点 WS
  - `src/auth.rs` — auth_middleware + audit_log
- 数据库：`database/init-{sqlite,postgresql,mysql}-2.7.4.sql`

## Visual/Browser Findings
- （无图像/浏览器内容）

---
*2-Action Rule：每 2 次视图/搜索后立即更新；本任务以代码阅读为主，关键发现已落到上方表格*