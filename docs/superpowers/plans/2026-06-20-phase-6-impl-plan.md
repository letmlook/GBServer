# Phase 6 实施方案 — JT808/JT1078 Production Parity

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 GBServer 的 JT808/JT1078 部标终端（车机/行驶记录仪/视频部标）从"路由 + fire-and-forget 命令"提升到"真实终端能注册 → 心跳 → 实时视频 → 录像回放 → 录像检索 → 下载 → 控制"的完整生产链路；让前端 jtDevice 页面（`/api/jt1078/*`）的"播放/回放/检索/控制"按钮产生与 WVP-Pro 等价的结果。

**Architecture:**
- 状态机收敛到 `Jt1078Manager`（`src/jt1078/manager.rs`），新增 `JtCommandWaiter`（已存在但未接入）+ `JtMediaSessionManager`（已存在但未接入）的真实接线
- 命令发送走 `send_command_and_wait(phone, msg_id, body) -> Result<JtResponse, Error>`，**禁止 fire-and-forget**
- 媒体流到达/失败走 ZLM `on_stream_changed` / `on_publish` / `on_rtp_server_timeout` 钩子闭环
- DB 全部三态 cfg（`postgres` / `mysql` / `sqlite`，默认 sqlite）
- 终端注册走"GBServer 注册码鉴权（基于 `JtTerminal.auth_code`）"替换当前 env-var 临时 token

**Tech Stack:** Rust + Axum + SQLx + ZLMediaKit HTTP API + DashMap（终端注册表 / 命令等待 / 媒体会话）+ GB28181 SIP 端口复用 + JT808/JT1078 TCP/UDP 监听 + JT 平台 ABL/位置协议

**基线 commit:** `62b8768`（Phase 5 完成）
**上游设计:** `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 6
**总工作量:** ~150h（约 4 周），拆 5 个子任务 + 1 个横切任务。

---

## 全局约束

- **数据库默认 feature = sqlite**：所有 `src/db/*.rs` 新增/修改的表 / 函数必须三态 cfg（`postgres` / `mysql` / `sqlite`）
- **优先复用已有 db 模块**：`gb_jt_terminal` / `gb_jt_terminal_channel` / `gb_stream_push` / `gb_send_rtp` / `gb_cloud_record` 不新建重复表
- **三库 CI 必跑**：`cargo test --lib` + `cargo test --no-default-features --features postgres --lib` + `cargo test --no-default-features --features mysql --lib` 三路全绿
- **命令关联优先于命令发送**：每个 `send_*` 公共方法必须支持 `send_and_wait(phone, msg_id, body, timeout)`，handler 端默认用 send_and_wait
- **真实协议链路**：不允许"占位 URL / 假成功响应"绕开协议；每个子任务都必须在测试中验证完整 command → response → 状态更新 → ZLM 媒体到达
- **承接 Phase 1-5 收尾**：
  - 复用 Phase 1 `PendingRequestManager` 模式（key 改为 `phone+msg_id+serial`）
  - 复用 Phase 3 `MediaWaiterManager` 模式（ZLM 媒体到达 → 激活 JtMediaSession）
  - 复用 Phase 4 `StreamStatus` 统一接口（JtMediaSession 实现 `StreamState` trait）
  - 复用 Phase 5 `SendRtpManager` 模式（cascade 转发 JT 流时复用）

---

## Context

按设计文档 §7 Phase 6，需把 GBServer JT808/JT1078 能力从"路由存在 + fire-and-forget 命令"提升到"终端能注册 → 真实命令响应 → ZLM 媒体到达"全链路：

1. **6.1 终端注册 / 心跳 / 离线**：当前 `Jt1078Manager` 已有 TCP/UDP 监听 + 简单 `AUTH:<token>` 鉴权（env-var GBSERVER__JT1078__TOKEN）；缺标准 JT/T 808 注册协议（0x0100 消息体 7 字段：province/city/manufacturer/terminal_model/terminal_id/iccid/hardware_version），缺基于 DB `JtTerminal.auth_code` 的鉴权，缺终端注册响应（0x8100 鉴权码回包）
2. **6.2 命令关联（msg_id + serial + phone）**：`JtCommandWaiter` 已有完整代码（`command_waiter.rs` 400 行，13 个单测），**但所有 handler 仍直接调 `send_command` 跳过等待**——即"fire-and-forget"
3. **6.3 实时视频 / 回放 / 控制**：`live_start` / `playback_start` / `playback_control` 仍返回 `127.0.0.1/live/...` 占位 URL（设计文档 §6.1 禁项）；`JtMediaSessionManager` 已存在但**未被任何 handler 实际使用**；ZLM 媒体到达钩子未接线
4. **6.4 录像检索 / 下载 / 上传**：`record_list` 仅查 ZLM MP4 + 兜底查 cloud_record，缺 JT 平台 0x8802 媒体检索命令 + 多包聚合（设计文档 §6.5）；`media_upload_one` 仅调 `media_upload(0x8803)` 缺 `media_attribute`/`media_list` 流程
5. **6.5 PTZ / 文本 / 电话本 / 围栏 / 司机信息 / 摄像头 OSD 等控制**：`ptz` / `wiper` / `fill_light` / `text_msg` / `telephone_callback` / `driver_info` / `media_attribute` 等 handler 已存在但**仍为 `build_success` 占位**，未真正调 JtCommandWaiter 发命令 + 等响应
6. **6.6 终端参数 / 位置 / OSD**：`config_get`/`config_set`/`attribute`/`position_info` 同样占位；缺 0x8104 查询参数 / 0x8103 设置参数 / 0x0102 属性上报 / 0x0200 位置上报的完整解析

**Acceptance**（设计文档原文）：
- Simulator + 至少 1 个真实 JT 终端能完成 register / live video / playback / record query / selected controls
- API route 覆盖 WVP-Pro live pause/continue/switch 路径
- 默认占位坐标 / 司机数据 / 摄像头 OSD 不再作为主要生产数据返回

**当前差距**（代码审计确认）：

| # | 现状 | 缺口 |
|---|---|---|
| 1 | `Jt1078Manager.send_command` 走 `send_raw`（fire-and-forget） | 6.2 需新增 `send_command_and_wait` 包装 `JtCommandWaiter` |
| 2 | 所有 handler 调 `send_*` 后不 await 响应 | 6.2 + 6.3 + 6.5 全部需改为 `send_*_and_wait` |
| 3 | `JtCommandWaiter` 定义完整但 0 引用 | 6.2 需在 `Jt1078Manager` 持有 `Arc<JtCommandWaiter>` 并在 `send_command` 中注册 + 在 `process_response_payload` 中解析 |
| 4 | `JtMediaSessionManager` 0 引用 | 6.3 需在 `Jt1078Manager` 持有 `Arc<JtMediaSessionManager>` 并在 live/playback handler 中 create + 在 ZLM on_stream_changed 中 activate |
| 5 | `Jt1078Session` 当前用 `AUTH:<token>` 简化协议 | 6.1 需实现标准 JT/T 808 0x0100 注册协议（body 7 字段） + 0x8100 鉴权响应 |
| 6 | TCP/UDP 端口硬编码 `0.0.0.0:60000` | 6.1 需从 `Jt1078Config` 读取 + 拆分 `tcp_port` / `udp_port` |
| 7 | DB `JtTerminal` 缺 `auth_code` 字段 | 6.1 需在 `database/init-*.sql` 加 `auth_code VARCHAR(32)` 列 + 三态 cfg 迁移 |
| 8 | `live_start` 返 `127.0.0.1/live/...` 占位 | 6.3 需 ZLM 媒体到达钩子真正触发 → 等 N 秒 → 返回真实 RTMP/RTSP |
| 9 | `playback_start` / `playback_control` 不发命令 | 6.3 需真发 0x9201 + 等 0x9202 应答 + 处理 0x9102 流控 |
| 10 | `record_list` 不发 0x8802 | 6.4 需真发媒体检索 + 多包聚合（仿 Phase 3.3 RecordInfo 等待模板） |
| 11 | `ptz` / `wiper` / `fill_light` / `text_msg` 返 `build_success("成功")` | 6.5 全部需发命令 + 等响应 + 状态回写 |
| 12 | `config_get` 返 IP/port 字符串拼装 | 6.6 需真发 0x8104 + 解析 0x0107 终端参数应答 |
| 13 | `position_info` 返 `{longitude: 0.0, latitude: 0.0}` | 6.6 需从 DB `gb_jt_terminal_channel.last_lat`/`last_lng` 读 + 加 `last_position_time` |

**预估工作量**：~150h（4 周编码 + 1 周 buffer），6 个子任务，6-8 个 PR。

---

## File Structure

| 路径 | 责任 | 状态 |
|---|---|---|
| `src/jt1078/manager.rs` | 终端注册表 + 命令发送 + 命令等待 + 媒体会话入口（单 Arc） | 改（6.2 + 6.3） |
| `src/jt1078/server.rs` | TCP/UDP 监听 + 注册协议解析 | 改（6.1 端口配置 + 0x0100 解析） |
| `src/jt1078/session.rs` | Per-connection session（保留）+ 注册鉴权状态机 | 改（6.1 注册流程） |
| `src/jt1078/command_waiter.rs` | 命令→响应关联（已有 400 行） | 改（6.2 在 manager 接线） |
| `src/jt1078/jt_media_session.rs` | 媒体会话管理（已有 256 行） | 改（6.3 接入 + StreamState trait） |
| `src/jt1078/command.rs` | JT808/JT1078 命令编码（已有 353 行） | 改（6.1 加 0x0100/0x8100/0x0102/0x0104 等编解码） |
| `src/jt1078/response_parser.rs`（新） | 0x0100 注册应答 / 0x0102 属性应答 / 0x0104 查询参数应答 / 0x0200 位置上报 / 0x0801 多包媒体检索应答 解析 | 增 |
| `src/services/jt_service.rs`（新） | JtService 域服务：live_start / playback_start / record_list / config_get 真实逻辑 | 增 |
| `src/handlers/jt1078.rs` | HTTP 路由（1503 行） | 改（6.3-6.6 全部调 JtService） |
| `src/db/jt1078.rs` | `gb_jt_terminal` / `gb_jt_terminal_channel` 表（三态 cfg） | 改（6.1 auth_code + 6.6 last_position） |
| `src/zlm/hook.rs` | `on_stream_changed` / `on_publish` / `on_rtp_server_timeout` | 改（6.3 路由到 JtMediaSessionManager） |
| `src/state/stream_status.rs`（Phase 4） | `StreamState` trait | 改（6.3 JtMediaSession impl） |
| `src/lib.rs` | `run()` 启动 `jt1078::server::start` + JtService 注入 AppState | 改（6.1） |
| `database/init-{sqlite,postgresql,mysql}-2.7.4.sql` | `gb_jt_terminal` 加 `auth_code` / `last_lng` / `last_lat` / `last_position_time` 列 + 索引 | 改（6.1 + 6.6） |
| `tests/integration/jt1078_e2e_test.rs`（新） | mock 终端 TCP → 注册 → 命令 → 媒体到达 | 增 |
| `scripts/phase6-test-matrix.sh`（新） | 三库 `cargo test --lib` 一键验收 | 增 |
| `docs/OPERATIONS.md` | Phase 6 章节 | 改 |

---

## 任务清单

### Task 6.1 — 标准 JT/T 808 终端注册 + 鉴权码 + 端口配置化（P0，20h）

**目标**：
- 实现 JT/T 808 标准 0x0100 终端注册消息解析（body 7 字段：province/city/manufacturer/terminal_model/terminal_id/iccid/hardware_version）
- 终端鉴权码走 DB `gb_jt_terminal.auth_code`（替代 env-var 临时 token）
- 实现 0x8100 终端注册应答（body：应答流水号 + 结果 + 鉴权码）
- TCP/UDP 端口从 `Jt1078Config` 读取（拆分 `tcp_port` / `udp_port`）
- DB `gb_jt_terminal` 加 `auth_code` 列（三态 cfg 迁移）

**Files:**
- Modify: `src/config.rs`（`Jt1078Config` 加 `tcp_port: u16` / `udp_port: u16`）
- Modify: `src/jt1078/server.rs`（从 cfg 读端口 + 拆分 TCP/UDP bind）
- Modify: `src/jt1078/command.rs`（新增 `build_register_response` 0x8100 编码 + `parse_register_request` 0x0100 解析）
- Modify: `src/jt1078/response_parser.rs`（新文件）—— 注册 / 属性 / 参数 / 位置 / 媒体检索 5 类响应解析
- Modify: `src/jt1078/session.rs`（`expected_token` → `expected_auth_code`，查 DB 验证）
- Modify: `src/jt1078/manager.rs`（持有 `Arc<Jt1078Db>` + 鉴权时查 DB）
- Modify: `src/db/jt1078.rs`（新增 `get_terminal_by_phone` 已存在；新增 `get_auth_code`）
- Modify: `database/init-{sqlite,postgresql,mysql}-2.7.4.sql`（3 文件均加 `auth_code` + 索引）
- Test: `src/jt1078/command.rs::tests`（5-6 个）+ `src/jt1078/response_parser.rs::tests`（5-6 个）

**关键代码骨架**：

```rust
// src/jt1078/command.rs（新增 0x8100 应答编码）
pub fn build_register_response(
    phone: &str, register_serial: u16, result: u8, auth_code: &str,
) -> Vec<u8> {
    let body = build_register_response_body(register_serial, result, auth_code);
    build_jt808_frame(0x8100, phone, 0, &body)
}

fn build_register_response_body(serial: u16, result: u8, auth_code: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&serial.to_be_bytes());
    body.push(result);  // 0=成功 1=车辆已被注册 2=数据库中无该车辆 3=终端已被注册 4=数据库中无该终端
    let code_bytes = auth_code.as_bytes();
    body.extend_from_slice(code_bytes);
    body
}
```

```rust
// src/jt1078/response_parser.rs（新文件）
pub struct RegisterRequest {
    pub province: u16,
    pub city: u16,
    pub manufacturer: String,  // 5 bytes
    pub terminal_model: String, // 20 bytes
    pub terminal_id: String,    // 7 bytes
    pub iccid: String,          // 10 bytes (BCD)
    pub hardware_version: String, // variable
}

pub fn parse_register_request(body: &[u8]) -> Result<RegisterRequest, String> {
    if body.len() < 50 {
        return Err(format!("register body too short: {}", body.len()));
    }
    let province = u16::from_be_bytes([body[0], body[1]]);
    let city = u16::from_be_bytes([body[2], body[3]]);
    let manufacturer = std::str::from_utf8(&body[4..9])
        .map_err(|e| e.to_string())?
        .trim_end_matches('\0')
        .to_string();
    let terminal_model = std::str::from_utf8(&body[9..29])
        .map_err(|e| e.to_string())?
        .trim_end_matches('\0')
        .to_string();
    let terminal_id = std::str::from_utf8(&body[29..36])
        .map_err(|e| e.to_string())?
        .trim_end_matches('\0')
        .to_string();
    // ICCID BCD 解码
    let iccid = bcd_to_string(&body[36..46])?;
    let hardware_version = std::str::from_utf8(&body[46..])
        .map(|s| s.trim_end_matches('\0').to_string())
        .unwrap_or_default();
    Ok(RegisterRequest { province, city, manufacturer, terminal_model, terminal_id, iccid, hardware_version })
}

fn bcd_to_string(bcd: &[u8]) -> Result<String, String> {
    let mut s = String::with_capacity(bcd.len() * 2);
    for &b in bcd {
        s.push(((b >> 4) & 0x0F) as char);
        s.push((b & 0x0F) as char);
    }
    Ok(s)
}
```

```rust
// src/jt1078/session.rs::process_payload 改写
pub enum FrameKind {
    AuthSuccess,  // 已注册
    AuthFailure,  // 注册失败
    RegisterRequest(RegisterRequest), // 0x0100 → 需要 0x8100 应答
    Heartbeat,
    Data(Vec<u8>),
    LocationReport(LocationReport), // 0x0200
    AttributeReport(AttributeReport), // 0x0102
    MediaItems(Vec<MediaItem>),     // 0x0801
    /// 通用应答 0x0001
    CommonResponse { serial: u16, msg_id: u16, result: u8 },
}

pub fn process_payload(&mut self, msg_id: u16, serial: u16, payload: &[u8]) -> FrameKind {
    match msg_id {
        0x0100 => {
            // 终端注册
            match parse_register_request(payload) {
                Ok(req) => {
                    // 异步查 DB 验证 + 应答 — 此处仅返回 Request，让 caller 调 DB + 发送 0x8100
                    FrameKind::RegisterRequest(req)
                }
                Err(_) => FrameKind::AuthFailure,
            }
        }
        0x0102 => {
            // 终端属性上报
            match response_parser::parse_attribute_report(payload) {
                Ok(a) => FrameKind::AttributeReport(a),
                Err(_) => FrameKind::Data(payload.to_vec()),
            }
        }
        0x0200 => {
            // 位置上报
            match response_parser::parse_location_report(payload) {
                Ok(l) => FrameKind::LocationReport(l),
                Err(_) => FrameKind::Data(payload.to_vec()),
            }
        }
        0x0801 => {
            // 多包媒体检索应答 — 需重组
            FrameKind::MediaItems(response_parser::parse_media_items_first(payload))
        }
        0x0001 => {
            // 通用应答
            FrameKind::CommonResponse {
                serial: u16::from_be_bytes([payload[0], payload[1]]),
                msg_id: u16::from_be_bytes([payload[2], payload[3]]),
                result: payload[4],
            }
        }
        0x0002 => FrameKind::Heartbeat,
        _ => FrameKind::Data(payload.to_vec()),
    }
}
```

```rust
// src/jt1078/server.rs::start
let tcp_port = cfg.as_ref().and_then(|c| c.tcp_port).unwrap_or(60000);
let udp_port = cfg.as_ref().and_then(|c| c.udp_port).unwrap_or(60000);
let tcp_addr = format!("0.0.0.0:{}", tcp_port);
let udp_addr = format!("0.0.0.0:{}", udp_port);
// TCP/UDP bind 分别用 tcp_addr / udp_addr
```

**子任务**：
- [ ] **Step 1**: 在 `src/config.rs` 给 `Jt1078Config` 加 `tcp_port: Option<u16>` / `udp_port: Option<u16>`
- [ ] **Step 2**: 在 `src/jt1078/command.rs` 加 `build_register_response` + `parse_register_request`（或拆到 response_parser.rs）
- [ ] **Step 3**: 创建 `src/jt1078/response_parser.rs`，定义 `RegisterRequest` / `LocationReport` / `AttributeReport` / `MediaItem` + 各自 `parse_*` 函数
- [ ] **Step 4**: 添加单元测试 `test_parse_register_request_valid` / `test_parse_register_request_short_body` / `test_bcd_to_string_*`（6-8 个 case）
- [ ] **Step 5**: 在 `src/db/jt1078.rs` 加 `get_auth_code_by_phone(pool, phone) -> Result<Option<String>>`（三态 cfg）
- [ ] **Step 6**: 在 `database/init-sqlite-2.7.4.sql` + `init-postgresql-2.7.4.sql` + `init-mysql-2.7.4.sql` 加 `ALTER TABLE gb_jt_terminal ADD COLUMN auth_code VARCHAR(32)`（`IF NOT EXISTS` 防重复）
- [ ] **Step 7**: 修改 `src/jt1078/session.rs::process_payload` 拆分 `FrameKind::RegisterRequest` + `LocationReport` + `AttributeReport` + `MediaItems` + `CommonResponse`
- [ ] **Step 8**: 修改 `src/jt1078/manager.rs`：持有 `Arc<Pool>` 引用，注册时调 `get_auth_code_by_phone` + 拼 0x8100 应答
- [ ] **Step 9**: 修改 `src/jt1078/server.rs` 读 cfg 的 tcp_port/udp_port + 调用 frame parser 而非 `process_payload(token)`
- [ ] **Step 10**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 11**: Commit `feat(phase-6): JT/T 808 standard terminal registration + auth_code from DB + port config`

### Task 6.2 — JtCommandWaiter 全量接入（P0，28h）

**目标**：
- `Jt1078Manager` 持有 `Arc<JtCommandWaiter>`
- 所有 `send_*` 公共方法增加 `*_and_wait` 版本，handler 端默认用 wait 版本
- 收到终端应答时（0x0001 通用应答 + 0x0002 心跳 + 0x0100/0x0102/0x0104/0x0200/0x0801 业务应答）按 `phone+msg_id+serial` 匹配 waiter
- 超时清理
- handler 端改造：`ptz` / `wiper` / `fill_light` / `text_msg` / `telephone_callback` / `driver_info` / `connection` / `door` / `shooting` / `media_upload_one` / `reset` / `factory_reset` / `media_attribute` / `media_list` / `set_phone_book` / `talk_start` / `talk_stop` / `live_start` / `playback_start` / `playback_control` / `record_list` / `config_get` / `config_set` / `attribute` / `position_info` 全部从 `build_success` 占位改为 `send_*_and_wait` 真实调用

**Files:**
- Modify: `src/jt1078/manager.rs`（持有 `Arc<JtCommandWaiter>` + 新增 `send_command_and_wait` + `send_ptz_and_wait` / `send_live_video_and_wait` / `send_playback_and_wait` / `send_playback_control_and_wait` / `send_media_search_and_wait` / `send_set_params_and_wait` / `send_query_location_and_wait` / `send_query_attributes_and_wait` / `send_text_message_and_wait` 等 15+ 方法）
- Modify: `src/jt1078/command_waiter.rs`（新增 `try_resolve_by_response` 公共方法接收 `phone+msg_id+serial` + 返回 `Result<JtResponse>`）
- Modify: `src/jt1078/manager.rs`（在 `process_payload_for` 中识别 0x0001 通用应答 → 调 `command_waiter.try_resolve_by_response`）
- Modify: `src/handlers/jt1078.rs`（15+ handler 改用 `*_and_wait`）
- Test: `src/jt1078/command_waiter.rs::tests`（新增 5 个）+ `src/handlers/jt1078.rs::tests`（5-6 个 mock 端到端）

**关键代码骨架**：

```rust
// src/jt1078/manager.rs::send_command_and_wait
pub async fn send_command_and_wait(
    &self, phone: &str, msg_id: u16, body: &[u8], timeout_secs: u64,
) -> Result<JtResponse, String> {
    let addr = self.get_terminal_addr(phone).await
        .ok_or_else(|| format!("终端 {} 未连接", phone))?;
    let seq = self.next_seq(addr).await;
    let frame = command::build_jt808_frame(msg_id, phone, seq, body);
    
    // 1) 注册 waiter
    let waiter = JtCommandWaiterHandle::new(phone.to_string(), msg_id, seq, timeout_secs);
    self.command_waiter.register(waiter.clone()).await;
    
    // 2) 发送
    self.send_raw(phone, &frame).await?;
    
    // 3) 等响应
    let resp = waiter.recv().await
        .map_err(|e| format!("wait timeout: {}", e))?;
    
    // 4) 清理 waiter
    self.command_waiter.unregister(&phone, msg_id, seq).await;
    
    Ok(resp)
}
```

```rust
// src/jt1078/manager.rs::send_live_video_and_wait
pub async fn send_live_video_and_wait(
    &self, phone: &str, channel_id: u8, stream_type: u8, close: bool, timeout_secs: u64,
) -> Result<JtResponse, String> {
    let body = command::build_live_video_request(channel_id, stream_type, close);
    self.send_command_and_wait(phone, 0x9101, &body, timeout_secs).await
}
```

```rust
// src/jt1078/command_waiter.rs::try_resolve_by_response
pub async fn try_resolve_by_response(
    &self, phone: &str, msg_id: u16, serial: u16, result: u8,
) -> Option<JtResponse> {
    let key = (phone.to_string(), msg_id, serial);
    if let Some((_, handle)) = self.waiters.remove(&key) {
        let resp = JtResponse { msg_id, serial, result };
        handle.resolve(resp);
        Some(resp)
    } else {
        None
    }
}
```

```rust
// src/handlers/jt1078.rs::ptz 改写
pub async fn ptz(
    State(state): State<AppState>,
    Query(q): Query<PtzQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1);
    let direction = q.direction.clone().unwrap_or_else(|| "stop".to_string());
    let speed = q.speed.unwrap_or(0x80);
    
    if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
        match mgr.send_ptz_and_wait(&phone, channel_id as u8, &direction, speed as u8, 5).await {
            Ok(resp) if resp.result == 0 => Json(build_success("PTZ 命令已应答")),
            Ok(resp) => Json(build_error(&format!("PTZ 失败 result={}", resp.result))),
            Err(e) => Json(build_error(&format!("PTZ 错误: {}", e))),
        }
    } else {
        Json(build_error("JT1078 manager 未初始化"))
    }
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/jt1078/manager.rs` 新增 `Arc<JtCommandWaiter>` 字段（构造函数注入）
- [ ] **Step 2**: 在 `src/jt1078/manager.rs` 新增 `send_command_and_wait`（含 0x0001 通用应答匹配逻辑）
- [ ] **Step 3**: 在 `src/jt1078/command_waiter.rs` 新增 `register` / `unregister` / `try_resolve_by_response` 公共方法
- [ ] **Step 4**: 添加单元测试 `test_command_waiter_register_and_resolve` / `test_command_waiter_timeout` / `test_command_waiter_unknown_serial`（3-5 个）
- [ ] **Step 5**: 在 `src/jt1078/manager.rs` 包装 15+ `send_*_and_wait` 方法（PTZ / live / playback / media_search / set_params / query_location / query_attributes / text / phone / vehicle / take_photo / media_upload / phone_book / wiper / fill_light / terminal_control）
- [ ] **Step 6**: 修改 `src/jt1078/manager.rs::process_payload_for` 在 0x0001 通用应答时调 `command_waiter.try_resolve_by_response`
- [ ] **Step 7**: 修改 `src/handlers/jt1078.rs::ptz` 改用 `send_ptz_and_wait`
- [ ] **Step 8**: 修改 `src/handlers/jt1078.rs::wiper` 改用 `send_wiper_and_wait`
- [ ] **Step 9**: 修改 `src/handlers/jt1078.rs::fill_light` 改用 `send_fill_light_and_wait`
- [ ] **Step 10**: 修改 `src/handlers/jt1078.rs::text_msg` 改用 `send_text_message_and_wait`
- [ ] **Step 11**: 修改 `src/handlers/jt1078.rs::telephone_callback` 改用 `send_phone_callback_and_wait`
- [ ] **Step 12]: 修改 `src/handlers/jt1078.rs::driver_info` 改用 `send_query_attributes_and_wait` + DB 写回
- [ ] **Step 13]: 修改 `src/handlers/jt1078.rs::factory_reset` 改用 `send_terminal_control_and_wait`
- [ ] **Step 14]: 修改 `src/handlers/jt1078.rs::reset` 改用 `send_terminal_control_and_wait`
- [ ] **Step 15**: 修改 `src/handlers/jt1078.rs::connection` 改用 `send_connection_control_and_wait`
- [ ] **Step 16**: 修改 `src/handlers/jt1078.rs::door` 改用 `send_vehicle_control_and_wait`
- [ ] **Step 17**: 修改 `src/handlers/jt1078.rs::media_attribute` 改用 `send_query_attributes_and_wait` + 解析
- [ ] **Step 18]: 修改 `src/handlers/jt1078.rs::media_list` 改用 `send_media_search_and_wait` + 多包聚合
- [ ] **Step 19**: 修改 `src/handlers/jt1078.rs::set_phone_book` 改用 `send_set_phone_book_and_wait`
- [ ] **Step 20**: 修改 `src/handlers/jt1078.rs::shooting` 改用 `send_take_photo_and_wait`
- [ ] **Step 21**: 修改 `src/handlers/jt1078.rs::media_upload_one` 改用 `send_media_upload_and_wait`
- [ ] **Step 22]: 添加集成测试 `test_ptz_command_round_trip`（mock 终端发 0x0001 应答 → handler 返成功）
- [ ] **Step 23**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 24**: Commit `feat(phase-6): JtCommandWaiter integration + 15+ send_*_and_wait methods`

### Task 6.3 — 实时视频 / 回放 / 控制真实链路（P0，32h）

**目标**：
- `live_start` 真发 0x9101 + 等 0x0001 应答 + 等 ZLM 媒体到达（N 秒超时） + 返回真实 RTMP/RTSP URL
- `live_stop` 真发 0x9102 关闭 + 清理 ZLM RTP server
- `playback_start` 真发 0x9201 + 等 0x0001 应答 + ZLM 媒体到达
- `playback_control` 真发 0x9202 + 解析 0x9102 流控（暂停/恢复/倍速/跳转）
- `JtMediaSessionManager` 接入 Jt1078Manager + ZLM on_stream_changed 钩子
- 移除 `127.0.0.1/live/...` 占位 URL（设计文档 §6.1 禁项）

**Files:**
- Modify: `src/jt1078/manager.rs`（持有 `Arc<JtMediaSessionManager>` + `send_live_video_and_wait` 改用 `send_command_and_wait` + 等 ZLM 媒体）
- Modify: `src/jt1078/jt_media_session.rs`（新增 `impl StreamState for JtMediaSession` + `JtMediaSessionManager::wait_for_media(phone, channel_id, timeout)` 用 oneshot::channel 实现）
- Modify: `src/zlm/hook.rs`（`on_stream_changed` / `on_publish` / `on_rtp_server_timeout` 路由到 `JtMediaSessionManager`）
- Modify: `src/state/stream_status.rs`（`StreamState` trait 加 `phone` + `channel_id` getter）
- Modify: `src/handlers/jt1078.rs::live_start`（等 ZLM 媒体 + 返回真实 URL）
- Modify: `src/handlers/jt1078.rs::live_stop`（清理 session + 关 ZLM RTP）
- Modify: `src/handlers/jt1078.rs::playback_start`（等 0x0001 + ZLM 媒体 + 创建 JtMediaSession）
- Modify: `src/handlers/jt1078.rs::playback_control`（真发 0x9202 + 解析 0x9102 + 更新 session state）
- Modify: `src/handlers/jt1078.rs::playback_stop`（真发 0x9202 关闭 + 清理）
- Modify: `src/handlers/jt1078.rs::playback_download_url`（基于 session stream_url 拼真实 URL）
- Test: `src/jt1078/jt_media_session.rs::tests`（5 个）+ `src/handlers/jt1078.rs::tests`（mock ZLM 钩子 + mock 终端应答）

**关键代码骨架**：

```rust
// src/jt1078/jt_media_session.rs::JtMediaSession 实现 StreamState
use crate::state::stream_status::{StreamState, StreamStatus};

impl StreamState for JtMediaSession {
    fn stream_id(&self) -> &str {
        self.zlm_stream_id.as_deref().unwrap_or("unknown")
    }
    fn app(&self) -> &str {
        "jt1078"
    }
    fn status(&self) -> StreamStatus {
        match self.state {
            MediaSessionState::Starting | MediaSessionState::Paused | MediaSessionState::Stopping => StreamStatus::Ready,
            MediaSessionState::Active => StreamStatus::Active,
            MediaSessionState::Stopped => StreamStatus::Stopped,
            MediaSessionState::Failed => StreamStatus::Failed,
        }
    }
    fn set_status(&mut self, status: StreamStatus) {
        self.state = match status {
            StreamStatus::Ready => MediaSessionState::Starting,
            StreamStatus::Pushing | StreamStatus::Active => MediaSessionState::Active,
            StreamStatus::Stopped => MediaSessionState::Stopped,
            StreamStatus::Failed => MediaSessionState::Failed,
        };
    }
    fn media_server_id(&self) -> Option<&str> { None }
    fn device_id(&self) -> Option<&str> { Some(&self.phone) }
    fn channel_id(&self) -> Option<&str> { None }
}
```

```rust
// src/jt1078/jt_media_session.rs::wait_for_media
use tokio::sync::oneshot;
use std::time::Duration;

pub struct MediaWaiter {
    pub phone: String,
    pub channel_id: u8,
    pub sender: Option<oneshot::Sender<JtMediaSession>>,
}

pub struct JtMediaSessionManager {
    sessions: Arc<DashMap<String, JtMediaSession>>,
    waiters: Arc<DashMap<String, MediaWaiter>>,
}

impl JtMediaSessionManager {
    pub async fn wait_for_media(&self, phone: &str, channel_id: u8, timeout: Duration) -> Result<JtMediaSession, String> {
        let key = format!("{}_{}", phone, channel_id);
        let (tx, rx) = oneshot::channel();
        self.waiters.insert(key.clone(), MediaWaiter {
            phone: phone.to_string(),
            channel_id,
            sender: Some(tx),
        });
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(sess)) => Ok(sess),
            Ok(Err(_)) => Err("waiter cancelled".to_string()),
            Err(_) => Err(format!("media wait timeout for {}_{}", phone, channel_id)),
        }
    }
    
    pub fn activate_and_resolve(&self, phone: &str, channel_id: u8, zlm_stream_id: &str) {
        let key = format!("{}_{}", phone, channel_id);
        self.activate(phone, channel_id, zlm_stream_id);
        if let Some((_, mut w)) = self.waiters.remove(&key) {
            if let Some(tx) = w.sender.take() {
                let sess = self.get(phone, channel_id).unwrap();
                let _ = tx.send(sess);
            }
        }
    }
}
```

```rust
// src/jt1078/manager.rs::send_live_video_and_wait 改写
pub async fn send_live_video_and_wait(
    &self, phone: &str, channel_id: u8, stream_type: u8, close: bool, timeout_secs: u64,
) -> Result<JtMediaSession, String> {
    if close {
        // 0x9102 关闭
        let body = command::build_live_video_control(channel_id, 0, true);
        self.send_command_and_wait(phone, 0x9102, &body, timeout_secs).await?;
        self.media_session_manager.stop(phone, channel_id);
        return Err("closed".to_string()); // 不返回 session
    }
    
    // 0x9101 启动
    let body = command::build_live_video_request(channel_id, stream_type, false);
    self.send_command_and_wait(phone, 0x9101, &body, timeout_secs).await?;
    
    // 创建 session + 等待 ZLM 媒体到达
    self.media_session_manager.create_live(phone, channel_id);
    self.media_session_manager.wait_for_media(
        phone, channel_id, Duration::from_secs(timeout_secs),
    ).await
}
```

```rust
// src/zlm/hook.rs::on_stream_changed 路由
if data.app == "rtp" && data.stream.starts_with("jt1078_") {
    let parts: Vec<&str> = data.stream.trim_start_matches("jt1078_").split('_').collect();
    if parts.len() >= 2 {
        let phone = parts[0].to_string();
        let channel_id: u8 = parts[1].parse().unwrap_or(0);
        if let Some(ref state) = state_clone {
            if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
                mgr.media_session_manager().activate_and_resolve(&phone, channel_id, &data.stream);
            }
        }
    }
}
```

```rust
// src/handlers/jt1078.rs::live_start 改写
pub async fn live_start(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(1) as u8;
    let stream_type: u8 = match q.r#type.as_deref() { "sub" => 1, _ => 0 };
    
    if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
        // 1) 打开 ZLM RTP server
        let stream_id = format!("jt1078_{}_{}", phone, channel_id);
        let zlm = match state.zlm_client.as_ref() {
            Some(z) => z,
            None => return Json(build_error("ZLM 未配置")),
        };
        let rtp_info = match zlm.open_rtp_server(&crate::zlm::OpenRtpServerRequest {
            secret: zlm.secret.clone(),
            stream_id: stream_id.clone(),
            port: None,
            use_tcp: Some(false),
            rtp_type: Some(0),
            recv_port: None,
        }).await {
            Ok(i) => i,
            Err(e) => return Json(build_error(&format!("ZLM RTP 失败: {}", e))),
        };
        
        // 2) 终端命令 + 等 ZLM 媒体
        match mgr.send_live_video_and_wait(&phone, channel_id, stream_type, false, 10).await {
            Ok(sess) => Json(serde_json::json!({
                "code": 0,
                "msg": "success",
                "data": {
                    "phoneNumber": phone,
                    "channelId": channel_id,
                    "streamType": stream_type,
                    "rtmpUrl": format!("rtmp://{}:{}/live/{}", zlm.host, zlm.rtmp_port, sess.zlm_stream_id.unwrap_or_default()),
                    "rtspUrl": format!("rtsp://{}:{}/{}", zlm.host, zlm.rtsp_port, sess.zlm_stream_id.unwrap_or_default()),
                    "wsUrl": format!("ws://{}/live/{}", zlm.host, sess.zlm_stream_id.unwrap_or_default()),
                    "stream_id": sess.zlm_stream_id.unwrap_or_default(),
                    "port": rtp_info.port,
                }
            })),
            Err(e) => {
                // 清理 ZLM RTP server
                let _ = zlm.close_rtp_server(&stream_id).await;
                Json(build_error(&format!("实时视频失败: {}", e)))
            }
        }
    } else {
        Json(build_error("JT1078 manager 未初始化"))
    }
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/jt1078/jt_media_session.rs` 加 `MediaWaiter` + `wait_for_media` + `activate_and_resolve` + `StreamState` impl
- [ ] **Step 2**: 添加单元测试 `test_wait_for_media_resolves_on_activate` / `test_wait_for_media_timeout`（2 个）
- [ ] **Step 3**: 在 `src/jt1078/manager.rs` 持有 `Arc<JtMediaSessionManager>` + `media_session_manager()` 访问器
- [ ] **Step 4**: 修改 `src/jt1078/manager.rs::send_live_video_and_wait` 真等 ZLM 媒体
- [ ] **Step 5**: 修改 `src/zlm/hook.rs::on_stream_changed` 路由到 JtMediaSessionManager
- [ ] **Step 6**: 修改 `src/zlm/hook.rs::on_rtp_server_timeout` 路由到 JtMediaSessionManager.stop
- [ ] **Step 7**: 修改 `src/handlers/jt1078.rs::live_start` 改用 `send_live_video_and_wait`
- [ ] **Step 8**: 修改 `src/handlers/jt1078.rs::live_stop` 改用 `send_live_video_control_and_wait` + 清理 session
- [ ] **Step 9**: 修改 `src/jt1078/manager.rs::send_playback_and_wait` 真等 ZLM 媒体
- [ ] **Step 10]: 修改 `src/handlers/jt1078.rs::playback_start` 改用 `send_playback_and_wait` + 创建 session
- [ ] **Step 11**: 修改 `src/jt1078/manager.rs::send_playback_control_and_wait` 真发 0x9202 + 解析 0x9102
- [ ] **Step 12]: 修改 `src/handlers/jt1078.rs::playback_control` 改用 `send_playback_control_and_wait` + 解析流控
- [ ] **Step 13]: 修改 `src/handlers/jt1078.rs::playback_stop` 改用 `send_playback_control_and_wait(close)`
- [ ] **Step 14]: 修改 `src/handlers/jt1078.rs::playback_download_url` 基于 session.stream_url 拼真实 URL
- [ ] **Step 15]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 16]: 跑 `grep -n "127.0.0.1/live" src/handlers/jt1078.rs` 确认占位 URL 已移除
- [ ] **Step 17]: Commit `feat(phase-6): live/playback/control real chain via JtMediaSessionManager + ZLM hooks`

### Task 6.4 — 录像检索 / 下载 / 上传真实链路（P1，24h）

**目标**：
- `record_list` 真发 0x8802 媒体检索 + 多包聚合（仿 Phase 3.3 RecordInfo 等待模板）
- `media_upload_one` 真发 0x8803 + 等上传完成
- `media_attribute` 真发 0x8104 查询 + 解析 0x0107 应答
- `media_list` 支持 0x8802 + 0x0801 多包媒体检索（0x0800/0x0801 协议）

**Files:**
- Modify: `src/jt1078/manager.rs`（新增 `send_media_search_and_wait` + `send_media_upload_and_wait` + `send_query_attributes_and_wait`）
- Modify: `src/jt1078/response_parser.rs`（新增 `parse_attribute_report` 0x0102 / `parse_location_report` 0x0200 / `parse_media_items_first` 0x0801 / `parse_query_params_response` 0x0107）
- Modify: `src/handlers/jt1078.rs::record_list`（真发 0x8802 + 多包聚合 + DB 落库）
- Modify: `src/handlers/jt1078.rs::media_upload_one`（真发 0x8803 + 等上传）
- Modify: `src/handlers/jt1078.rs::media_attribute`（真发 0x8104 + 解析 0x0107）
- Modify: `src/handlers/jt1078.rs::media_list`（真发 0x8802 + 多包聚合 + 0x0800 媒体项列表）
- Test: `src/jt1078/response_parser.rs::tests`（10-12 个）+ `src/handlers/jt1078.rs::tests`（mock 终端应答）

**关键代码骨架**：

```rust
// src/jt1078/response_parser.rs::parse_location_report
pub struct LocationReport {
    pub alarm: u32,
    pub status: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: u16,
    pub speed: u16,
    pub direction: u16,
    pub time: chrono::DateTime<Utc>,
}

pub fn parse_location_report(body: &[u8]) -> Result<LocationReport, String> {
    if body.len() < 28 {
        return Err(format!("location report too short: {}", body.len()));
    }
    let alarm = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let status = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
    let lat_raw = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
    let lng_raw = u32::from_be_bytes([body[12], body[13], body[14], body[15]]);
    let altitude = u16::from_be_bytes([body[16], body[17]]);
    let speed = u16::from_be_bytes([body[18], body[19]]);
    let direction = u16::from_be_bytes([body[20], body[21]]);
    let time_bcd = &body[22..28];
    let time = parse_bcd_datetime(time_bcd)?;
    Ok(LocationReport {
        alarm, status,
        latitude: lat_raw as f64 / 1_000_000.0,
        longitude: lng_raw as f64 / 1_000_000.0,
        altitude, speed, direction, time,
    })
}
```

```rust
// src/jt1078/manager.rs::send_media_search_and_wait
pub async fn send_media_search_and_wait(
    &self, phone: &str, channel_id: u8, start_time: &str, end_time: &str, timeout_secs: u64,
) -> Result<Vec<MediaItem>, String> {
    let body = command::build_media_search(0, channel_id, 0, 
        &command::encode_time_bcd(start_time),
        &command::encode_time_bcd(end_time),
    );
    self.send_command_and_wait(phone, 0x8802, &body, timeout_secs).await?;
    // 0x0801 多包可能为 start/middle/end — 需要等待 media_session 收齐
    // 此处简化：仅首包
    Err("TODO: 6.4 0x0801 多包聚合".to_string())
}
```

```rust
// src/handlers/jt1078.rs::record_list 改写
pub async fn record_list(
    State(state): State<AppState>,
    Query(q): Query<RecordListQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    let channel_id = q.channel_id.unwrap_or(0) as u8;
    let start_time = q.start_time.clone().unwrap_or_default();
    let end_time = q.end_time.clone().unwrap_or_default();
    
    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }
    
    if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
        match mgr.send_media_search_and_wait(&phone, channel_id, &start_time, &end_time, 30).await {
            Ok(items) => {
                // 落库到 gb_jt_media_item
                for item in &items {
                    let _ = crate::db::jt1078::insert_media_item(&state.pool, &phone, channel_id as i32, item).await;
                }
                return Json(serde_json::json!({
                    "code": 0,
                    "data": { "list": items, "total": items.len() }
                }));
            }
            Err(e) => {
                tracing::warn!("JT1078 media search failed for {}: {}", phone, e);
            }
        }
    }
    
    // 兜底：查 ZLM + DB（保留原逻辑）
    Json(serde_json::json!({ "code": 0, "data": { "list": [], "total": 0 } }))
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/jt1078/response_parser.rs` 新增 `LocationReport` / `AttributeReport` / `MediaItem` 类型 + 各自 parse 函数
- [ ] **Step 2**: 添加单元测试 `test_parse_location_report_*` / `test_parse_attribute_report_*` / `test_parse_media_items_first_*`（10-12 个）
- [ ] **Step 3**: 在 `src/jt1078/manager.rs` 新增 `send_media_search_and_wait` + `send_media_upload_and_wait` + `send_query_attributes_and_wait`
- [ ] **Step 4]: 在 `src/db/jt1078.rs` 新增 `insert_media_item` / `list_media_items_by_terminal`（三态 cfg）
- [ ] **Step 5**: 在 `database/init-*.sql` 加 `gb_jt_media_item` 表（id, phone, channel_id, media_id, type, format, start_time, end_time, file_path, file_size, create_time）
- [ ] **Step 6]: 修改 `src/handlers/jt1078.rs::record_list` 改用 `send_media_search_and_wait` + 落库
- [ ] **Step 7]: 修改 `src/handlers/jt1078.rs::media_upload_one` 改用 `send_media_upload_and_wait` + 解析上传进度
- [ ] **Step 8]: 修改 `src/handlers/jt1078.rs::media_attribute` 改用 `send_query_attributes_and_wait` + 解析
- [ ] **Step 9]: 修改 `src/handlers/jt1078.rs::media_list` 改用 `send_media_search_and_wait` + 多包聚合
- [ ] **Step 10]: 添加集成测试 `test_record_list_aggregation`（mock 终端发 3 包 0x0801 → handler 返汇总列表）
- [ ] **Step 11**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 12]: Commit `feat(phase-6): record search/upload/attribute via 0x8802/0x8803/0x8104 + multi-packet aggregation`

### Task 6.5 — 终端参数 / 位置 / OSD 真实链路（P1，20h）

**目标**：
- `config_get` 真发 0x8104 查询终端参数 + 解析 0x0107 应答
- `config_set` 真发 0x8103 设置参数 + 等应答
- `attribute` 真发 0x8107 查询属性 + 解析 0x0102 应答 + DB 写回
- `position_info` 从 DB `gb_jt_terminal_channel.last_lng`/`last_lat`/`last_position_time` 读 + 兜底 0x8201 查询
- 移除 `{longitude: 0.0, latitude: 0.0}` 占位

**Files:**
- Modify: `src/jt1078/manager.rs`（新增 `send_query_params_and_wait` 0x8104 + `send_set_params_and_wait` 0x8103 + `send_query_location_and_wait` 0x8201 + `send_query_attribute_and_wait` 0x8107）
- Modify: `src/jt1078/response_parser.rs`（新增 `parse_query_params_response` 0x0107）
- Modify: `src/db/jt1078.rs`（新增 `get_last_position` / `update_last_position` / `update_attribute` 三态 cfg）
- Modify: `database/init-*.sql`（`gb_jt_terminal` 加 `last_lng` / `last_lat` / `last_position_time` 列 + `gb_jt_terminal` 加 `attribute` JSON 列）
- Modify: `src/handlers/jt1078.rs::config_get`（真发 0x8104 + 解析）
- Modify: `src/handlers/jt1078.rs::config_set`（真发 0x8103 + 等应答）
- Modify: `src/handlers/jt1078.rs::attribute`（真发 0x8107 + 解析 0x0102 + 落库）
- Modify: `src/handlers/jt1078.rs::position_info`（DB 读 + 兜底 0x8201）
- Test: `src/db/jt1078.rs::tests`（5 个）+ `src/handlers/jt1078.rs::tests`（4-5 个）

**关键代码骨架**：

```rust
// src/jt1078/manager.rs::send_query_params_and_wait
pub async fn send_query_params_and_wait(
    &self, phone: &str, timeout_secs: u64,
) -> Result<TerminalParams, String> {
    // 0x8104 body 是空的（按 JT/T 808 规范）
    let resp = self.send_command_and_wait(phone, 0x8104, &[], timeout_secs).await?;
    // resp 是 0x0001 通用应答；真正的参数在 0x0107
    // 终端会再发一个 0x0107 消息 — 需要第二个 waiter
    self.wait_for_msg_id_response(phone, 0x0107, timeout_secs).await
}
```

```rust
// src/handlers/jt1078.rs::position_info 改写
pub async fn position_info(
    State(state): State<AppState>,
    Query(q): Query<PositionQuery>,
) -> Json<serde_json::Value> {
    let phone = q.phone_number.clone().unwrap_or_default();
    if phone.is_empty() {
        return Json(build_error("缺少 phoneNumber"));
    }
    
    // 1) 优先从 DB 读最近位置
    if let Ok(Some(pos)) = crate::db::jt1078::get_last_position(&state.pool, &phone).await {
        return Json(serde_json::json!({
            "code": 0,
            "data": {
                "phoneNumber": phone,
                "longitude": pos.longitude,
                "latitude": pos.latitude,
                "speed": pos.speed,
                "direction": pos.direction,
                "altitude": pos.altitude,
                "time": pos.time,
                "source": "db"
            }
        }));
    }
    
    // 2) 兜底：实时查终端（0x8201 位置查询）
    if let Some(mgr) = state.jt1078_manager.read().await.as_ref() {
        if let Ok(loc) = mgr.send_query_location_and_wait(&phone, 10).await {
            // 落库
            let _ = crate::db::jt1078::update_last_position(&state.pool, &phone, &loc).await;
            return Json(serde_json::json!({
                "code": 0,
                "data": {
                    "phoneNumber": phone,
                    "longitude": loc.longitude,
                    "latitude": loc.latitude,
                    "speed": loc.speed,
                    "direction": loc.direction,
                    "altitude": loc.altitude,
                    "time": loc.time,
                    "source": "device"
                }
            }));
        }
    }
    
    // 3) 实在没有 — 返 404 而非占位
    Json(serde_json::json!({
        "code": 404,
        "msg": "终端无位置数据"
    }))
}
```

```rust
// src/db/jt1078.rs::update_last_position
pub async fn update_last_position(
    pool: &Pool, phone: &str, pos: &LocationReport,
) -> sqlx::Result<()> {
    #[cfg(feature = "sqlite")]
    {
        sqlx::query("UPDATE gb_jt_terminal SET last_lng = ?, last_lat = ?, last_position_time = ? WHERE phone = ?")
            .bind(pos.longitude)
            .bind(pos.latitude)
            .bind(pos.time.to_rfc3339())
            .bind(phone)
            .execute(pool)
            .await?;
    }
    #[cfg(feature = "postgres")]
    {
        sqlx::query("UPDATE gb_jt_terminal SET last_lng = $1, last_lat = $2, last_position_time = $3 WHERE phone = $4")
            .bind(pos.longitude)
            .bind(pos.latitude)
            .bind(pos.time)
            .bind(phone)
            .execute(pool)
            .await?;
    }
    #[cfg(feature = "mysql")]
    {
        sqlx::query("UPDATE gb_jt_terminal SET last_lng = ?, last_lat = ?, last_position_time = ? WHERE phone = ?")
            .bind(pos.longitude)
            .bind(pos.latitude)
            .bind(pos.time.naive_utc())
            .bind(phone)
            .execute(pool)
            .await?;
    }
    Ok(())
}
```

**子任务**：
- [ ] **Step 1**: 在 `src/jt1078/response_parser.rs` 新增 `TerminalParams` 类型 + `parse_query_params_response` 0x0107
- [ ] **Step 2]: 添加单元测试 `test_parse_query_params_response_*`（3-4 个）
- [ ] **Step 3]: 在 `src/jt1078/manager.rs` 新增 `send_query_params_and_wait` + `send_set_params_and_wait` + `send_query_location_and_wait` + `send_query_attribute_and_wait`
- [ ] **Step 4]: 在 `src/db/jt1078.rs` 新增 `get_last_position` / `update_last_position` / `update_attribute`（三态 cfg）
- [ ] **Step 5]: 在 `database/init-*.sql` 加 `last_lng` / `last_lat` / `last_position_time` / `attribute` 列 + 索引
- [ ] **Step 6]: 修改 `src/handlers/jt1078.rs::config_get` 改用 `send_query_params_and_wait`
- [ ] **Step 7]: 修改 `src/handlers/jt1078.rs::config_set` 改用 `send_set_params_and_wait`
- [ ] **Step 8]: 修改 `src/handlers/jt1078.rs::attribute` 改用 `send_query_attribute_and_wait` + 落库
- [ ] **Step 9]: 修改 `src/handlers/jt1078.rs::position_info` 改用 DB 读 + 0x8201 兜底
- [ ] **Step 10]: 添加集成测试 `test_position_info_db_first`（mock 终端位置已落库 → handler 直接返 DB 数据）
- [ ] **Step 11]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 12]: 跑 `grep -n "longitude: 0.0" src/handlers/jt1078.rs` 确认占位已移除
- [ ] **Step 13]: Commit `feat(phase-6): params/position/attribute via 0x8103/0x8104/0x8107/0x8201 + DB persistence`

### Task 6.6 — 横切：JT 终端模拟器 + 三库测试矩阵 + 文档（P1，12h）

**目标**：
- mock JT 终端 TCP 客户端能跑：注册（0x0100） → 应答 0x8100 → 心跳 → 命令 → 应答 0x0001 → 实时视频 0x9101 → 应答 0x0001 → 位置上报 0x0200
- 三库 CI 全绿
- docs/OPERATIONS.md 新增 Phase 6 章节

**Files:**
- Create: `tests/integration/jt1078_e2e_test.rs`（mock 终端端到端）
- Create: `scripts/phase6-test-matrix.sh`（三库 cargo test 脚本）
- Modify: `.github/workflows/ci.yml`（确认三库 job 仍工作）
- Modify: `docs/OPERATIONS.md`（新增 Phase 6 章节）
- Test: 跨 6.1-6.5 集成 case

**子任务**：
- [ ] **Step 1**: 在 `tests/integration/jt1078_e2e_test.rs` 实现 mock JT 终端：
  - 监听 0.0.0.0:0 TCP
  - 接受 TCP 连接 → 发送 0x0100 注册消息
  - 等 0x8100 鉴权应答 → 验证成功
  - 发 0x0002 心跳 → 等应答
  - 等 0x9101 实时视频请求 → 应答 0x0001
  - 发 0x0200 位置上报
  - 等 0x9102 关闭 → 应答
- [ ] **Step 2]: 端到端 case 1：mock 终端注册 → 后端写 DB + 状态 online
- [ ] **Step 3]: 端到端 case 2：mock 终端心跳 → 后端 `last_heartbeat` 更新
- [ ] **Step 4]: 端到端 case 3：mock ZLM 推 on_stream_changed → JtMediaSession 激活
- [ ] **Step 5]: 端到端 case 4：mock 终端位置上报 0x0200 → DB `last_lat`/`last_lng` 更新
- [ ] **Step 6]: 端到端 case 5：mock 终端响应 PTZ 0x9301 通用应答 → handler 返成功
- [ ] **Step 7]: 端到端 case 6：mock 终端发 0x0801 媒体检索多包 → handler 聚合返回
- [ ] **Step 8]: 创建 `scripts/phase6-test-matrix.sh`（复制 phase-5 模板）
- [ ] **Step 9]: 跑 `bash scripts/phase6-test-matrix.sh` 确认三库 exit 0
- [ ] **Step 10]: 在 `docs/OPERATIONS.md` 新增 Phase 6 章节（操作步骤 + 验收命令）
- [ ] **Step 11]: Commit `docs(phase-6): ops doc + 3-DB test matrix script + JT terminal e2e test`

---

## 验收命令

```bash
# 默认（sqlite）
cargo test --lib

# PostgreSQL
cargo test --no-default-features --features postgres --lib

# MySQL
cargo test --no-default-features --features mysql --lib

# Phase 6 关键单测
cargo test --lib jt1078::response_parser::
cargo test --lib jt1078::command_waiter::
cargo test --lib jt1078::jt_media_session::
cargo test --lib handlers::jt1078::

# 集成测试
cargo test --test integration_test jt1078
bash scripts/phase6-test-matrix.sh
```

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/jt1078/manager.rs` | 持有 JtCommandWaiter + JtMediaSessionManager + 15+ send_*_and_wait 方法 | 24h |
| `src/jt1078/response_parser.rs`（新） | 5 类响应解析（注册/位置/属性/参数/媒体检索） | 8h |
| `src/jt1078/command.rs` | 加 0x8100 编码 + 0x0100 解析辅助 | 4h |
| `src/jt1078/server.rs` | 端口配置化 + 注册协议解析 | 4h |
| `src/jt1078/session.rs` | FrameKind 拆分 + 业务消息分发 | 6h |
| `src/jt1078/command_waiter.rs` | register / unregister / try_resolve_by_response 公共方法 | 4h |
| `src/jt1078/jt_media_session.rs` | MediaWaiter + wait_for_media + StreamState impl | 8h |
| `src/handlers/jt1078.rs` | 25+ handler 改用 send_*_and_wait | 16h |
| `src/zlm/hook.rs` | on_stream_changed / on_rtp_server_timeout 路由 | 4h |
| `src/state/stream_status.rs` | StreamState trait 加 phone/channel_id getter | 2h |
| `src/db/jt1078.rs` | auth_code / last_position / media_item / attribute 三态 cfg | 8h |
| `database/init-{sqlite,postgresql,mysql}-2.7.4.sql` | auth_code + last_position + media_item + attribute | 4h |
| `src/config.rs` | Jt1078Config 加 tcp_port / udp_port | 1h |
| `src/lib.rs` | 启动 jt1078::server::start | 1h |
| `tests/integration/jt1078_e2e_test.rs`（新） | mock 终端端到端 | 16h |
| `scripts/phase6-test-matrix.sh`（新） | 三库 cargo test 一键 | 0.5h |
| `docs/OPERATIONS.md` | Phase 6 章节 | 2h |

**总计**：~150h ≈ 4 周编码 + 1 周 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）

- **6.1**：`jt1078::response_parser::tests::test_parse_register_request_*` / `test_bcd_to_string_*` — 6-8 个 case
- **6.2**：`jt1078::command_waiter::tests::test_register_and_resolve` / `test_timeout` — 3-5 个 + handlers 5-6 个
- **6.3**：`jt1078::jt_media_session::tests::test_wait_for_media_*` — 2 个 + handlers 4-5 个
- **6.4**：`jt1078::response_parser::tests::test_parse_location_report_*` / `test_parse_attribute_report_*` / `test_parse_media_items_first_*` — 10-12 个
- **6.5**：`jt1078::response_parser::tests::test_parse_query_params_response_*` — 3-4 个 + `db::jt1078::tests` 5 个

### 集成测试（`tests/integration/jt1078_e2e_test.rs`）

- mock 终端注册（0x0100）→ 后端 0x8100 应答 + DB online
- mock 终端心跳（0x0002）→ 后端 `last_heartbeat` 更新
- mock 终端 PTZ（0x9301）→ 后端等 0x0001 应答 → handler 返成功
- mock 终端位置上报（0x0200）→ DB `last_lat`/`last_lng` 更新
- mock ZLM on_stream_changed → JtMediaSession 激活
- mock 终端 0x0801 媒体检索多包 → handler 聚合

### 端到端（手测，对应设计文档 Acceptance）

- 真实 JT 终端（如海康/同洲车机）启动 → 配置 GBServer IP/port + 鉴权码 → 注册成功
- 真实终端 0x9101 实时视频 → GBServer 等 ZLM 媒体 → 返回真实 RTMP URL
- 真实终端 0x9201 录像回放 → GBServer 等 0x0001 + ZLM → 创建回放 session
- 真实终端 0x0200 位置上报 → GBServer 写 DB → position_info 接口返回最近位置
- 真实终端 0x8802 录像检索 → GBServer 多包聚合 → record_list 返回完整列表
- 真实终端 0x9301 PTZ → GBServer 等 0x0001 → handler 返成功（不返"假成功"）

---

## 衔接说明

### 与 Phase 1 衔接
- **1.x PendingRequestManager**（SIP）→ 6.2 复用模式做 `JtCommandWaiter`（key 改为 `phone+msg_id+serial`）
- **1.x InviteSessionStore**（SIP）→ 6.3 复用模式做 `JtMediaSessionManager`（key 改为 `phone+channel_id`）

### 与 Phase 2 衔接
- **2.x DeviceStatus/Config/Catalog 多包等待** → 6.4 RecordInfo 多包聚合复用 `accumulate_*` 模板
- **2.x SubscriptionLifecycle** → 6.4 终端告警/位置订阅可复用（暂不实现，留 Phase 7）

### 与 Phase 3 衔接
- **3.1 Live Play 媒体等待** → 6.3 实时视频媒体等待复用 `MediaWaiterManager` 模式
- **3.4 DownloadSession** → 6.4 终端录像下载可复用 DownloadSession（暂不实现）

### 与 Phase 4 衔接
- **4.5 StreamStatus 统一接口** → 6.3 `JtMediaSession` 实现 `StreamState` trait
- **4.x ZLM hook 全集** → 6.3 on_stream_changed / on_rtp_server_timeout 路由到 JtMediaSessionManager

### 与 Phase 5 衔接
- **5.x CascadeRegistrar / CascadeService** → 6.1 终端注册鉴权模式可参考 CascadeRegistrar 的"DB 配置 + 状态机"模式
- **5.x SendRtpManager** → 6.3 终端视频若需级联转发，SendRtpManager 复用
- **5.5a/b MobilePosition / Alarm 上行** → 6.5 终端位置上报转发可复用 cascade_forward 的 forward_*_to_all 模式

### 与 Phase 7 衔接
- **7.x Redis StateStore** → 6.2/6.3 终端注册表 / 命令等待 / 媒体会话 跨节点时改用 Redis（本期单节点即可）
- **7.x 跨节点 RPC** → 终端命令跨节点时改用 RPC（本期单节点即可）
- **7.x WebSocket 终端事件** → 终端位置/告警 WebSocket 推送（本期不实现）

---

## 风险与缓解

### R1: 终端命令等待 + ZLM 媒体等待双层等待涉及多模块串通 — **HIGH ⚠️**
- 涉及 Jt1078Manager / JtCommandWaiter / JtMediaSessionManager / ZLM hook / session.rs
- 任一环节失活整链路断
- **缓解**：
  - 6.2 拆 3 个子步骤：注册 waiter（异步 oneshot）→ 发送（独立）→ 等响应（带超时）
  - 6.3 拆 3 个子步骤：创 session（独立）→ 等 0x0001 应答（复用 6.2）→ 等 ZLM 媒体（独立）
  - 每步单独单测；端到端 case 在 6.6 模拟器集成
  - mock ZLM on_stream_changed 推送时即使没真推流也通过

### R2: JtCommandWaiter 已存在但 0 引用 — **MEDIUM**
- 接入工作量 ≈ 重写
- **缓解**：
  - 6.2 保留现有 command_waiter.rs 的 register/unregister API
  - manager.rs 在 `process_payload_for` 中识别 0x0001 通用应答 → 调 waiter.try_resolve_by_response
  - 单元测试覆盖：mock 终端发 0x0001 应答 → handler 返成功

### R3: 多包媒体检索 0x0800/0x0801 协议复杂 — **MEDIUM**
- 终端可能发 0x0800（开始）+ 多个 0x0801（中间）+ 0x0801（结束）
- 需重组 + 按 media_id 排序
- **缓解**：
  - 6.4 简化为：仅 0x8802 + 0x0801 单包聚合（用 0x0801 的 first_pkt 字段）
  - 多包（start+middle+end）实现放 6.4-followup
  - 端到端 case 6 用 mock 终端发 1 包 0x0801 验证

### R4: 鉴权码明文存 DB 有安全风险 — **LOW**
- 当前 `JtTerminal.auth_code` 存明文
- **缓解**：
  - 6.1 仅在测试环境用明文（接受）；生产环境用哈希 + 盐
  - 哈希版放 Phase 7 一起做（用户/JWT 鉴权码统一哈希）

### R5: 终端实时视频 RTP 端口分配可能冲突 — **MEDIUM**
- ZLM 同一 stream_id 第二次调用 open_rtp_server 可能返冲突
- **缓解**：
  - 6.3 在 live_stop 时必 close_rtp_server
  - session manager 加 stream_id → zlm_port 映射表
  - 集成测试覆盖：先开 RTP → 关 → 再开同 stream_id 不冲突

### R6: 三态 cfg 在 JT handler 改动后回归 — **MEDIUM**
- JT 涉及 db::jt1078 + db::cloud_record，可能破坏 mysql / postgres
- **缓解**：
  - 6.4 / 6.5 所有 db 改动都三态 cfg
  - 6.6 跑 `bash scripts/phase6-test-matrix.sh` 必须三库全绿
  - 优先复用既有 db 模块（gb_jt_terminal / gb_jt_terminal_channel）

### R7: 终端 TCP/UDP 监听端口与 SIP 端口冲突 — **LOW**
- 当前 JT 默认 60000，与 SIP 5060 不冲突
- **缓解**：
  - 6.1 端口从 cfg 读，可配置
  - 文档提示：生产部署时 JT 端口应与 SIP/PTZ/Alarm 端口分离

### N1（新增）：模拟器集成测试需要 mock 终端 TCP 客户端 — **LOW**
- 真实 JT 终端启动慢
- **缓解**：
  - 6.6 端到端 case 中 mock JT 终端（tokio TcpStream 客户端）
  - 真实终端集成在 docs/OPERATIONS.md "Phase 6 真实部署" 章节手测

---

## 重新评估后的 P0/P1 优先级

| 任务 | 原优先级 | 重审后 | 理由 |
|---|---|---|---|
| 6.1 标准 JT/T 808 注册 | P0 | **P0** | 设计文档 Acceptance 第 1 条核心 |
| 6.2 JtCommandWaiter 接入 | P0 | **P0** | 后续 4 个任务都依赖命令等待 |
| 6.3 live/playback/control 真实链路 | P0 | **P0** | 设计文档 Acceptance 第 1 条核心 |
| 6.4 record/upload/attribute | P1 | **P1** | 与 6.3 平行但验收压力较小 |
| 6.5 params/position/attribute | P1 | **P1** | 移除占位数据 |
| 6.6 横切 + 三库 + 文档 | P1 | **P1** | 跟着 6.1-6.5 一起做 |

---

## 实施顺序调整

1. **第一批（P0，~80h）**：
   - 6.1 标准 JT/T 808 注册（为后续 4 个任务铺路）
   - 6.2 JtCommandWaiter 接入（基础能力）
2. **第二批（P0，~32h）**：
   - 6.3 live/playback/control 真实链路（核心 R1 风险）
3. **第三批（P1，~44h）**：
   - 6.4 record/upload/attribute（多包聚合）
   - 6.5 params/position/attribute（移除占位）
4. **第四批（P1，~12h）**：
   - 6.6 横切清理 + 三库测试矩阵 + 文档

---

## 完成判定

- **三库 `cargo test --lib` 全绿**（默认 sqlite + `--features postgres` + `--features mysql` 三路 0 失败）
- `scripts/phase6-test-matrix.sh` 一键三库验证脚本 exit 0
- **新增 ≥ 40 个单测**覆盖 6.1-6.5 涉及的所有模块（response_parser / command_waiter / jt_media_session / handlers / db）
- **集成测试 ≥ 6 个**：mock JT 终端（注册/心跳/PTZ/位置/ZLM 媒体/多包媒体检索）
- **真实 JT 终端至少 1 个能完成 register → live → playback → record query → selected controls 全流程**（手测通过）
- `docs/OPERATIONS.md` 新增 Phase 6 章节，操作步骤可复现
- **移除占位数据**：
  - `grep -n "127.0.0.1/live" src/handlers/jt1078.rs` 0 命中
  - `grep -n "longitude: 0.0" src/handlers/jt1078.rs` 0 命中
  - `grep -n "build_success.*成功" src/handlers/jt1078.rs` 仅剩注释
- **路由覆盖**：
  - live/pause、live/continue、live/switch 路径实现
  - playback/speed 多档位（1x/2x/4x/8x/16x）支持
  - record/list 支持 start_time + end_time 范围检索
- `Jt1078Manager` 持有 `Arc<JtCommandWaiter>` + `Arc<JtMediaSessionManager>` + `Arc<Pool>` 三件套
- 端到端 `cargo test --test integration_test jt1078` 全绿
- CI workflow 含三库 job（sqlite 默认 + 显式 postgres/mysql）
- 真实 JT 终端集成测试在 `docs/OPERATIONS.md` 文档化

---

## 后续 Phase 衔接

- **Phase 7 Redis 集群**：终端注册表 / 命令等待 / 媒体会话 跨节点时改用 Redis；当前已用 `Arc<DashMap>` 留出迁移空间
- **Phase 7 WebSocket**：终端位置/告警 WebSocket 推送；JT 终端事件独立于 GB28181 事件
- **Phase 8+**：JT 终端与平台级联（GB → JT 转换）
