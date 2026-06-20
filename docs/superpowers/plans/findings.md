# Findings & Decisions

## Requirements
- 输入：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`（§7 Phase 6） + `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md` + `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`
- 输出：`docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`
- 要求：与既有计划风格一致；可被 subagent 任务级执行；含 File Structure、子任务、关键代码骨架、验收命令、风险与衔接
- 风格：中文；技术名词保持英文；表格化呈现

## Research Findings

### 设计文档 §7 Phase 6 原文要点
1. **Goal**：make JT terminal workflows real rather than route-only.
2. **Tasks (5)**：
   - 6.1 终端注册/鉴权/心跳/TCP/UDP/offline 检测/DB 映射
   - 6.2 命令关联（msg_id + serial + phone）
   - 6.3 实时视频 start/stop/pause/continue/switch
   - 6.4 录像列表/回放 start/控制/stop/下载/上传/列表/删除
   - 6.5 PTZ/文本/电话本/围栏/配置查询/属性/司机/媒体属性
3. **Acceptance**：
   - Simulator + 至少 1 个真实 JT 终端能完成 register / live / playback / record query / selected controls
   - API route 覆盖 WVP-Pro live pause/continue/switch 路径
   - 默认占位坐标/司机数据/摄像头 OSD 不再作为主要生产数据
4. **Estimate**：4-6 周

### 当前代码现状（直接来自仓库）

**`src/jt1078/` 模块**（2032 行）：
- `manager.rs`（371 行）：`Jt1078Manager` 持有 `sessions`/`terminal_addrs`/`seq_counters` 三个 DashMap；`send_command` 走 `send_raw`（**fire-and-forget，不等响应**）；提供 `send_ptz`/`send_live_video`/`send_playback`/`send_wiper`/`send_fill_light`/`send_terminal_control`/`send_text_message`/`send_phone_callback`/`send_vehicle_control`/`send_take_photo`/`send_media_search`/`send_media_upload`/`send_set_phone_book`/`send_query_attributes`/`send_query_location`/`send_connection_control` 等 15+ 命令发送方法
- `command.rs`（353 行）：`build_jt808_frame` + 各类消息体编码（PTZ/live/playback/wiper/fill_light/terminal_control/text_message/phone_callback/vehicle_control/take_photo/media_search/media_upload/set_phone_book/query_attributes/set_params）；**缺 0x8100 终端注册应答编码**、**缺 0x0100 注册请求解析**
- `command_waiter.rs`（400 行）：`JtCommandWaiter` + `JtCmdType` 枚举（20+ variant）+ 13 个单测；**`register`/`unregister`/`try_resolve_by_response` 公共方法缺失**；**0 引用**
- `jt_media_session.rs`（256 行）：`JtMediaSession`（含 phone/channel_id/zlm_stream_id/speed/current_pos_secs/last_activity）+ `JtMediaSessionManager` + `create_live`/`create_playback`/`activate`/`pause`/`resume`/`stop`/`update_position`/`update_speed`/`get`/`remove`/`get_by_type`；**缺 `MediaWaiter` 等待 ZLM 媒体到达**；**0 引用**；**缺 `StreamState` trait 实现**
- `session.rs`（315 行）：`Jt1078Session`（per-connection）+ `parse_jt1078_frame`/`parse_jt1078_structured_frame` 帧解析 + `process_payload` 当前仅识别 `AUTH:<token>`/`HEARTBEAT`；**缺标准 JT/T 808 0x0100/0x8100/0x0102/0x0200/0x0801/0x0001 协议分发**
- `server.rs`（140 行）：`start` 启动 TCP/UDP 监听 + 端口**硬编码 `0.0.0.0:60000`（TCP 与 UDP 同端口）** + spawn accept loop + 调用 `feed_bytes` + `process_payload_for`；**缺 cfg 读端口**、**缺注册协议处理**、**缺命令关联**
- `frame.rs`（145 行）：`parse_jt1078_frame`（legacy length-prefixed）+ `parse_jt1078_structured_frame`（structured seq+ts+xor）
- `mod.rs`（52 行）：`Jt1078Server` 持有 `manager: Arc<RwLock<Option<Arc<Jt1078Manager>>>>`

**`src/handlers/jt1078.rs`**（1503 行）：
- 25+ 路由函数：`terminal_list`/`query`/`add`/`update`/`delete`/`channel_list`/`update`/`add` + `live_start`/`stop` + `playback_start`/`stop`/`control`/`download_url` + `ptz`/`wiper`/`fill_light` + `record_list` + `config_get`/`config_set` + `attribute` + `link_detection` + `position_info` + `text_msg` + `telephone_callback` + `driver_info` + `factory_reset` + `reset` + `connection` + `door` + `media_attribute` + `media_list` + `set_phone_book` + `shooting` + `talk_start`/`stop` + `media_upload_one`
- **关键缺陷**：
  - `live_start`（line 435-503）发 0x9101 后**不 await 0x0001 应答** + 返回 `rtmp://127.0.0.1:1935/live/...` / `rtsp://127.0.0.1:554/...` / `ws://127.0.0.1/live/...` 占位 URL
  - `playback_start`（line 530-590）发 0x9201 后**不 await 0x0001 应答** + 不创建 JtMediaSession
  - `playback_control`（line 616-657）**完全不调 0x9202**（仅返回 build_success）
  - `ptz`/`wiper`/`fill_light`/`text_msg` 等 15+ handler **全部 `build_success("成功")` 占位**
  - `position_info` 返回 `{longitude: 0.0, latitude: 0.0}` 占位
  - `config_get` 返 IP/port 字符串拼装

**`src/db/jt1078.rs`**（476 行）：
- `JtTerminal` 结构 + `JtChannel` 结构
- 已三态 cfg：`list_terminals_paged`/`count_terminals`/`get_terminal_by_phone`/`get_terminal_by_id`/`get_channel_by_id`/`get_online_terminals`/`count_online_terminals`/`insert_channel`/`update_channel`/`list_channels_by_terminal`/`insert_terminal`/`update_terminal`
- **缺**：`get_auth_code_by_phone` / `update_last_position` / `get_last_position` / `update_attribute` / `insert_media_item` / `list_media_items_by_terminal`
- **缺字段**：`JtTerminal` 缺 `auth_code` / `last_lng` / `last_lat` / `last_position_time` / `attribute` JSON

### 与 Phase 1-5 衔接点

| Phase | 可复用资产 | 6.x 用法 |
|---|---|---|
| Phase 1 | `PendingRequestManager`（key: device_id+sn） | 6.2 `JtCommandWaiter` 复用模式（key 改为 phone+msg_id+serial） |
| Phase 1 | `InviteSessionStore`（INVITE 会话） | 6.3 `JtMediaSessionManager` 复用模式（key 改为 phone+channel_id） |
| Phase 3 | `MediaWaiterManager`（ZLM 媒体等待） | 6.3 实时视频媒体等待复用 oneshot 模式 |
| Phase 3 | RecordInfo 多包聚合 `accumulate_*` 模板 | 6.4 0x0801 媒体检索多包聚合 |
| Phase 4 | `StreamState` trait + `StreamStatus` 枚举 | 6.3 `JtMediaSession` 实现 `StreamState` |
| Phase 4 | ZLM `on_stream_changed` hook 路由 | 6.3 路由到 `JtMediaSessionManager.activate_and_resolve` |
| Phase 5 | `CascadeRegistrar` 注册状态机 | 6.1 终端注册鉴权模式参考（DB 配置 + 状态机） |
| Phase 5 | `SendRtpManager` cascade 转发 | 6.3 终端视频若需级联转发 |

### Phase 6 子任务映射

| 设计文档 | 当前完成度 | Phase 6 子任务 |
|---|---|---|
| 6.1 终端注册/鉴权/心跳/offline | `Jt1078Manager` 80% 完整；TCP/UDP 监听 + 简单 AUTH 鉴权；**缺标准 JT/T 808 0x0100 注册协议 + DB 鉴权码** | 6.1 标准注册 + auth_code + 端口配置化 |
| 6.2 命令关联 | `JtCommandWaiter` 已存在但 0 引用；所有 `send_*` fire-and-forget | 6.2 JtCommandWaiter 接入 + 15+ `send_*_and_wait` |
| 6.3 live/playback/control | `live_start` 返 `127.0.0.1/live/...` 占位；`JtMediaSessionManager` 已存在但 0 引用；ZLM 钩子未接线 | 6.3 live/playback/control 真实链路 + JtMediaSession 接入 + ZLM 钩子路由 |
| 6.4 record/upload/attribute | `record_list` 仅查 ZLM/DB，缺 0x8802；`media_upload_one` 缺 0x8803 + 上传进度 | 6.4 record/upload/attribute 真实链路 + 0x0801 多包聚合 |
| 6.5 params/position/attribute | `config_get` 拼字符串；`position_info` `{0.0, 0.0}` 占位；缺 0x8104/0x0107/0x8107/0x8201 | 6.5 params/position/attribute 真实链路 + DB 落库 |

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| `Jt1078Manager` 持有 `Arc<JtCommandWaiter>` + `Arc<JtMediaSessionManager>` + `Arc<Pool>` 三件套 | 单一入口管理命令等待 / 媒体会话 / DB 鉴权 |
| 6.2 拆 15+ `send_*_and_wait` 方法 | 6.3-6.5 全部 handler 改造都依赖；命名对齐已有 `send_*` |
| 6.3 拆 3 个子步骤：创 session → 等 0x0001 → 等 ZLM 媒体 | R1 风险分解；每步单独单测 |
| 6.4 多包媒体检索简化为 0x8802 + 0x0801 单包聚合 | R3 风险控制；多包 start+middle+end 放 6.4-followup |
| 6.5 位置从 DB 读（`gb_jt_terminal.last_lat`/`last_lng`），0x8201 兜底 | 避免占位 `{longitude: 0.0, latitude: 0.0}`；提升性能 |
| 移除 `127.0.0.1/live/...` 占位 URL | 设计文档 §6.1 禁项 |
| 终端鉴权码走 DB `auth_code` 替代 env-var 临时 token | 设计文档 §7 Phase 6.1 + 安全考量 |
| TCP/UDP 端口从 `Jt1078Config` 读，可配置 | 避免与 SIP 端口冲突；可部署到多网卡 |
| `JtMediaSession` 实现 `StreamState` trait | 与 Phase 4 推流/代理/SendRtp 状态统一 |
| 沿用 phase-4/5 的三库 cfg + tests/integration/ 模式 | 与既有 CI 矩阵一致 |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| `JtCommandWaiter` 已存在 400 行但 0 引用 | 6.2 需新增 `register`/`unregister`/`try_resolve_by_response` 公共方法，并在 manager.rs 接线 |
| `JtMediaSessionManager` 已存在 256 行但 0 引用 | 6.3 需新增 `MediaWaiter` + `wait_for_media` + `activate_and_resolve` + `StreamState` impl |
| TCP/UDP 端口硬编码 `0.0.0.0:60000` | 6.1 从 `Jt1078Config` 读 + 拆分 tcp_port/udp_port |
| `JtTerminal` 缺 `auth_code` 字段 | 6.1 三态 cfg 迁移 + DB 新增字段 |
| `live_start` 返 `127.0.0.1/live/...` 占位 | 6.3 等 ZLM 媒体到达后返真实 RTMP/RTSP |
| `ptz`/`wiper`/`fill_light` 等 15+ handler 返 `build_success("成功")` | 6.2 + 6.5 全部改用 `send_*_and_wait` |
| `position_info` 返 `{longitude: 0.0, latitude: 0.0}` | 6.5 DB 读 + 0x8201 兜底；失败时返 404 |
| ZLM `on_stream_changed` 钩子未路由到 JtMediaSession | 6.3 新增路由逻辑（仿 `data.stream.starts_with("jt1078_")`） |

## Final Deliverable
- `docs/superpowers/plans/2026-06-20-phase-6-impl-plan.md`（~700 行）
- 5 子任务 + 1 横切：6.1 标准注册 / 6.2 JtCommandWaiter 接入 / 6.3 live+playback+control / 6.4 record+upload+attribute / 6.5 params+position+attribute / 6.6 横切+三库+文档
- 估时 ~150h（4 周编码 + 1 周 buffer）

## Resources
- 设计文档：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`
- 上游基线：WVP-Pro 2.7.4 / commit b760458
- 既有计划：
  - `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md`（4.x ZLM）
  - `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`（5.x 级联）
  - `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md`（3.x 视频/录像）
- 关键代码：
  - `src/jt1078/{mod, manager, session, server, command, frame, command_waiter, jt_media_session}.rs` — 协议栈
  - `src/handlers/jt1078.rs` — `/api/jt1078/*` 路由
  - `src/db/jt1078.rs` — `gb_jt_terminal` / `gb_jt_terminal_channel` 表
  - `src/zlm/hook.rs` — ZLM 钩子（路由到 JtMediaSession）
  - `src/state/stream_status.rs` — `StreamState` trait
- 数据库：`database/init-{sqlite,postgresql,mysql}-2.7.4.sql`
- 标准规范：JT/T 808-2011 / JT/T 1078-2016 道路运输车辆卫星定位系统终端通讯协议

## Visual/Browser Findings
- （无图像/浏览器内容）

---
*2-Action Rule：每 2 次视图/搜索后立即更新；本任务以代码阅读为主，关键发现已落到上方表格*
