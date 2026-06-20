# Phase 5 实施方案 — Platform Cascade Production Parity

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 GBServer 升级为可作为 WVP-Pro Java 或标准 GB 上级平台的"下级平台"——完善级联 REGISTER/Keepalive/重试、上级目录/设备/录像查询响应、上级 INVITE → 本级 INVITE → ZLM SendRtp → 上级媒体转发全链路、订阅（Catalog/MobilePosition/Alarm）跨平台转发。

**Architecture:**
- 状态机收敛到 `CascadeRegistrar`（`src/cascade/register.rs`），`src/sip/gb28181/cascade_service.rs` 标记 deprecated，仅保留与 SipServer 握手接口
- 上级 INVITE 入口走 `cascade_forward.rs::SendRtpManager::handle_upstream_invite`，串通 SipServer::handle_invite 末尾的设备 INVITE → ZLM SendRtp 路径
- 订阅转发走 `cascade_forward.rs` 新增 `forward_mobile_position` / `forward_alarm`，订阅生命周期由 Phase 2 `SubscriptionLifecycle` 触发
- 数据库默认 feature = sqlite；所有 `src/db/*.rs` 改动必须三态 cfg

**Tech Stack:** Rust + Axum + SQLx + ZLMediaKit HTTP API + DashMap（in-memory state）+ Redis（可选，SendRtp 跨节点共享）+ GB28181 SIP

**基线 commit:** `fa87cf6`（Phase 4 完成）
**上游设计:** `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 5
**总工作量:** ~100h（约 2.5 周），拆 5 个子任务 + 1 个横切任务。

---

## 全局约束

- **数据库默认 feature = sqlite**：所有 `src/db/*.rs` 新增/修改的表 / 函数必须三态 cfg（`postgres` / `mysql` / `sqlite`）
- **优先复用已有 db 模块**：`gb_platform` / `gb_platform_channel` / `gb_device` / `gb_device_channel` / `gb_send_rtp` 不新建重复表
- **三库 CI 必跑**：`cargo test --lib` + `cargo test --no-default-features --features postgres --lib` + `cargo test --no-default-features --features mysql --lib` 三路全绿
- **状态机收敛**：`CascadeRegistrar`（`src/cascade/`）是唯一级联注册状态机；`src/sip/gb28181/cascade_service.rs` 的 `CascadeService` 标记 `#[deprecated]`，但仍保留以避免一次性改动过大
- **真实协议链路**：不允许用"占位 URL / 假成功响应"绕开协议；每个子任务都必须在测试中验证完整 command → response → 状态更新
- **承接 Phase 4 收尾**：`on_send_rtp_stopped` 在 Phase 4 已有骨架，Phase 5 扩展为按 send_rtp_id 路由到具体 SendRtpManager session

---

## Context

按设计文档 §7 Phase 5，需把 GBServer 级联能力从"骨架"提升到"能作为 WVP-Pro Java 下级平台投产"：

1. **5.1 REGISTER/Keepalive/retry 收敛**：当前 `CascadeRegistrar`（`src/cascade/register.rs`，C3 阶段已完成 80%）+ `CascadeService`（`src/sip/gb28181/cascade_service.rs`）**两套状态机并存**，且后者硬编码 `127.0.0.1:5060` 与本级 GB ID
2. **5.2 上级查询响应**：Catalog/DeviceInfo/DeviceStatus 已实现（comprehensive plan B2 ✅），但 **RecordInfo 上行未做**（上级查本级录像）
3. **5.3 上级 INVITE → SendRtp 全链路**：`cascade_service.rs::handle_upstream_invite` 注释「简化：记录转发会话，返回成功」+ 实际 `cascade_forward.rs:1491-1494` 的真实 SendRtp 启动逻辑**未串通**
4. **5.4 BYE/send_rtp_stopped 清理**：`sip/server.rs:1730` BYE 路径已实现；`on_send_rtp_stopped` 在 Phase 4 已有但**未按 send_rtp_id 路由到具体 session**
5. **5.5 订阅转发**：Catalog 上行已落库（comprehensive plan B2 ✅），但 **MobilePosition / Alarm 上行未做**

**Acceptance**（设计文档原文）：
- Java WVP-Pro 能注册本级为下级平台
- 上级能查目录、点播共享通道、停止流、接收选定的订阅

**当前差距**（代码审计确认）：

| # | 现状 | 缺口 |
|---|---|---|
| 1 | `CascadeRegistrar` 805 行 + 9 个 c3_tests 状态机完整 | 5.1 需把 `run_registration_loop` / `send_keepalive_all` / `detect_keepalive_timeouts` 串到 `lib.rs::run()`；CascadeService 状态机需降级为薄包装 |
| 2 | `CascadeService::build_register_msg` 第 513 行 `response=""` 永远是空 | 5.1 必须改为走 CascadeRegistrar 的 md5_hex digest 实现 |
| 3 | `cascade_service.rs:219` `local_id` 硬编码 `"34020000002000000001"` | 5.1 必须从 SipConfig 读取 |
| 4 | `cascade_service.rs:290-291` `127.0.0.1:5060` 硬编码 | 5.1 必须从 SipConfig 读取 |
| 5 | `handle_upstream_invite`（cascade_service.rs:422-457）注释「简化」+ 只返回字符串 | 5.3 必须真做：解析 SDP → 调 SipServer 向设备 INVITE → 等 200 OK → ZLM SendRtp → 记录 session |
| 6 | `cascade_forward.rs:1491-1494` 已有设备 INVITE → ZLM startSendRtp 触发点 | 5.3 需要把 `cascade_service::handle_upstream_invite` 的入口改为调用此处的设备 INVITE 流程（而非简化为返回字符串） |
| 7 | `on_send_rtp_stopped` 在 `src/zlm/hook.rs` 存在但**未路由**到具体 SendRtpManager session | 5.4 需按 send_rtp_id 查表后清理 |
| 8 | `handle_message` 中 MobilePosition 上行 → 需转发到所有 Active 级联平台 | 5.5a MobilePosition 上行 |
| 9 | `handle_message` 中 Alarm 上行 → 需转发到所有 Active 级联平台 | 5.5b Alarm 上行 |

**预估工作量**：~100h（2.5 周编码 + 1 周 buffer），6 个子任务，4-6 个 PR。

---

## File Structure

| 路径 | 责任 | 状态 |
|---|---|---|
| `src/cascade/register.rs` | 唯一级联注册状态机（已有 805 行） | 改（5.1 串联 + 抽公共 digest） |
| `src/sip/gb28181/cascade_service.rs` | 兼容层（CascadeService 重复实现） | 改（5.1 标 deprecated + 委派给 CascadeRegistrar） |
| `src/sip/gb28181/cascade_forward.rs` | 订阅上行转发 + SendRtp 状态机（已有 650 行） | 改（5.5 + 5.4 路由） |
| `src/sip/gb28181/record_info_upstream.rs`（新） | 上级查本级 RecordInfo 的响应构造 | 增 |
| `src/sip/gb28181/upstream_invite.rs`（新） | 把 cascade_service::handle_upstream_invite 抽成可单测的纯函数 | 增 |
| `src/sip/server.rs` | INVITE 入口 / 平台方向 MESSAGE / 订阅入口 | 改（5.3 串通 + 5.5 上行） |
| `src/zlm/hook.rs` | `on_send_rtp_stopped` 按 send_rtp_id 路由 | 改（5.4） |
| `src/lib.rs` | `run()` 启动 `cascade_periodic_tasks` 与 `SendRtpManager` StateStore 注入 | 改（5.1 + 5.4） |
| `src/db/platform.rs` | `gb_platform` 表（三态 cfg 已存在） | 改（5.1 reload_from_db 入口） |
| `tests/integration/cascade_e2e_test.rs`（新） | 上级平台模拟器 → 注册 → 查目录 → 点播 → BYE | 增 |
| `scripts/phase5-test-matrix.sh`（新） | 三库 `cargo test --lib` 一键验收 | 增 |
| `docs/OPERATIONS.md` | Phase 5 章节 | 改 |

---

## 任务清单

### Task 5.1 — CascadeRegistrar 串联 + CascadeService 收敛（P0，16h）

**目标**：
- `CascadeRegistrar::run_registration_loop` + `cascade_periodic_tasks` 在 `lib.rs::run()` 启动
- `CascadeService` 标记 `#[deprecated]`，所有方法委派给 `CascadeRegistrar`
- 修复 `127.0.0.1:5060` / 本级 GB ID 硬编码 → 从 `SipConfig` 读取
- 修复 `build_register_msg` `response=""` → 走 `CascadeRegistrar::build_register_request`（已有 md5_hex digest）
- 串联 `reload_from_db` 入口到 lib.rs

**Files:**
- Modify: `src/sip/gb28181/cascade_service.rs`（删除 80% 重复实现，保留 SipServer 握手接口）
- Modify: `src/lib.rs:218-260`（串联 `cascade_periodic_tasks` + `reload_from_db` 启动钩子）
- Modify: `src/sip/server.rs:3917-3970`（`register_to_platform` / `send_platform_invite` 改走 CascadeRegistrar）
- Modify: `src/sip/gb28181/cascade.rs`（若存在公共类型，重新指向 cascade::register）
- Test: `src/cascade/register.rs` 末尾新增 `tests::test_periodic_tasks_*`（3-4 个）

**关键代码骨架**：

```rust
// src/sip/gb28181/cascade_service.rs（保留兼容层 + 委派）
#![allow(deprecated)]
//! ## Deprecated
//! 自 Phase 5 起，`CascadeService` 仅为与 SipServer 握手的兼容层；
//! 所有状态机/REGISTER/Keepalive 行为已迁移到 [`crate::cascade::CascadeRegistrar`]。

#[deprecated(
    since = "0.5.0",
    note = "use crate::cascade::CascadeRegistrar instead"
)]
pub struct CascadeService { /* ... */ }

#[allow(deprecated)]
impl CascadeService {
    pub async fn register(&self, platform_id: &str) -> Result<(), String> {
        // 委派给 CascadeRegistrar
        self.registrar.register(platform_id).await
    }
    // 其余方法同模式
}
```

```rust
// src/lib.rs::run() 末尾追加
let cascade_registrar = Arc::new(CascadeRegistrar::new());
cascade_registrar.set_pool(state.pool.clone());
if let Some(ref sip) = state.sip_server {
    cascade_registrar.set_sip_server(sip.clone()).await;
}
cascade_registrar.load_platforms_from_db(&state.pool, &config.sip.device_id, &config.sip.realm).await;
// 周期任务：注册/Keepalive/reload_from_db/超时检测
tokio::spawn(crate::cascade::register::cascade_periodic_tasks(
    cascade_registrar.clone(),
    config.sip.device_id.clone(),
    config.sip.realm.clone(),
));
// 单独的注册循环
tokio::spawn(cascade_registrar.run_registration_loop());
```

```rust
// src/cascade/register.rs::build_register_request 抽公共 digest
pub fn build_digest_response(
    username: &str, password: &str, realm: &str,
    method: &str, uri: &str, nonce: &str,
) -> String {
    let ha1 = md5_hex(&format!("{}:{}:{}", username, realm, password));
    let ha2 = md5_hex(&format!("{}:{}", method, uri));
    md5_hex(&format!("{}:{}:{}", ha1, nonce, ha2))
}
```

**子任务：**
- [ ] **Step 1**: 在 `src/cascade/register.rs` 抽 `build_digest_response` 公共函数（替换原 inline 逻辑）
- [ ] **Step 2**: 添加单元测试 `test_build_digest_response_known_vector`（RFC 2617 案例）
- [ ] **Step 3**: 修改 `src/sip/gb28181/cascade_service.rs`：删除 `CascadeService::build_register_msg`、`CascadeService::send_keepalive` 内的硬编码，从 `SipConfig` 读取 `device_id` / `local_ip` / `local_port`
- [ ] **Step 4**: 给 `CascadeService` 标注 `#[deprecated]`，所有方法委派给 `CascadeRegistrar`
- [ ] **Step 5**: 修改 `src/lib.rs::run()`：在 startup 串联 `cascade_periodic_tasks` + `run_registration_loop`
- [ ] **Step 6**: 验证 `src/sip/server.rs::register_to_platform` / `unregister_from_platform` / `send_platform_catalog` / `send_platform_invite` 走 CascadeRegistrar 而非 CascadeService
- [ ] **Step 7**: 添加集成测试 `test_cascade_periodic_tasks_wires_up`（mock pool，验证 spawn 后 registrar 状态机可驱动）
- [ ] **Step 8**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 9**: Commit `feat(phase-5): wire CascadeRegistrar as single state machine + deprecate CascadeService`

### Task 5.2 — 上级 RecordInfo 查询响应（P0，12h）

**目标**：当上级平台 SIP MESSAGE 查询本级某通道的 RecordInfo 时，本级真正等待设备 RecordInfo 多包响应 → 聚合 → 返回（替代当前仅"返回 empty 列表"）

**Files:**
- Create: `src/sip/gb28181/record_info_upstream.rs`（upstream 响应构造 + 等待复用）
- Modify: `src/sip/server.rs::handle_message`（识别"platform 方向 RecordInfo 查询"分支）
- Modify: `src/sip/gb28181/pending_request.rs`（`QueryResult::Items` 已在 phase-3，复用）
- Modify: `src/db/cloud_record.rs`（新增 `query_by_device_channel` 三态 cfg）
- Test: `src/sip/gb28181/record_info_upstream.rs::tests`（3-4 个）

**关键代码骨架**：

```rust
// src/sip/gb28181/record_info_upstream.rs
//! 响应上级平台对本级通道的 RecordInfo 查询。
//! 流程：
//! 1. 解析 SIP MESSAGE body → 拿到 (device_id, channel_id, start_time, end_time, sn)
//! 2. 复用 Phase 3.3 的 send_record_info_query_and_wait → 等多包设备响应
//! 3. 落库到 gb_cloud_record（Phase 3.3 已建）
//! 4. 用 WVP-Pro 兼容 XML 拼装 Response 并回送上级

pub async fn handle_upstream_record_info_query(
    sip: &crate::sip::SipServer,
    platform_id: &str,
    channel_id: &str,
    start_time: &str,
    end_time: &str,
    sn: u32,
) -> Result<String, String> {
    // 1. 找到 channel_id 所属的 device_id
    let device_id = sip.device_manager().get_device_id_by_channel(channel_id).await
        .ok_or_else(|| format!("Channel {} not found", channel_id))?;
    // 2. 复用 Phase 3.3 异步 RecordInfo 等待
    let items = sip.send_record_info_query_and_wait(
        &device_id, channel_id, start_time, end_time, sn
    ).await?;
    // 3. 拼装上行 Response XML
    let xml = build_upstream_record_info_response(platform_id, channel_id, sn, &items);
    Ok(xml)
}

fn build_upstream_record_info_response(
    platform_id: &str, channel_id: &str, sn: u32,
    items: &[RecordItem],
) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
<CmdType>RecordInfo</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<Name>{}-records</Name>
<SumNum>{}</SumNum>
<Num>{}</Num>"#,
        sn, channel_id, channel_id, items.len(), items.len()
    );
    for item in items {
        xml.push_str(&format!(
            "<Item><DeviceID>{}</DeviceID><Name>{}</Name><StartTime>{}</StartTime><EndTime>{}</EndTime><Secrecy>0</Secrecy></Item>",
            item.device_id, item.name, item.start_time, item.end_time
        ));
    }
    xml.push_str("</Response>");
    xml
}
```

```rust
// src/sip/server.rs::handle_message 中识别 platform 方向 RecordInfo
if upstream_platform.is_some() && query_target == config.device_id {
    if let Some(cmd) = detect_cmd_type(&body) {
        if cmd == "RecordInfo" {
            let parsed = parse_record_info_query(&body)?;
            return Self::handle_upstream_record_info_query(
                parsed.device_id, parsed.channel_id, parsed.start, parsed.end, parsed.sn
            ).await;
        }
    }
    // ... 现有 Catalog/Info/Status 分支保留
}
```

**子任务：**
- [ ] **Step 1**: 创建 `src/sip/gb28181/record_info_upstream.rs`，定义 `handle_upstream_record_info_query` + `build_upstream_record_info_response`
- [ ] **Step 2**: 添加单元测试 `test_build_upstream_record_info_response_empty` / `test_build_upstream_record_info_response_with_items`（3-4 个）
- [ ] **Step 3**: 在 `src/sip/server.rs::handle_message` 识别 platform + RecordInfo CmdType → 走新函数
- [ ] **Step 4**: 验证 `src/db/cloud_record.rs::query_by_device_channel` 三态 cfg 完整
- [ ] **Step 5**: 添加集成测试 `test_upstream_record_info_aggregation`（mock 设备发 3 包 RecordInfo，验证 response 含 SumNum=3 + 全部 item）
- [ ] **Step 6**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 7**: Commit `feat(phase-5): upstream RecordInfo query aggregation for cascade platforms`

### Task 5.3 — 上级 INVITE → 本级 INVITE → ZLM SendRtp 整链路（P0，24h）

**目标**：
- `cascade_service.rs::handle_upstream_invite` 注释「简化」删除
- 真正执行：解析 SDP → 调 SipServer 向设备 INVITE → 等 200 OK + ZLM 媒体到达 → ZLM SendRtp 指向上级 IP:port
- 串通 `cascade_forward.rs::handle_upstream_invite` 已有逻辑
- 单元测试覆盖 SDP 解析 + SSRC 提取

**Files:**
- Create: `src/sip/gb28181/upstream_invite.rs`（抽成可单测的纯函数）
- Modify: `src/sip/gb28181/cascade_service.rs::handle_upstream_invite`（替换"简化"分支）
- Modify: `src/sip/server.rs::handle_invite`（平台方向 INVITE 走 cascade 入口）
- Modify: `src/sip/gb28181/cascade_forward.rs`（接收上游 INVITE 入口）
- Test: `src/sip/gb28181/upstream_invite.rs::tests`（5-6 个）

**关键代码骨架**：

```rust
// src/sip/gb28181/upstream_invite.rs
//! 把"上级 INVITE → 本级设备 INVITE → ZLM SendRtp"链路抽成可单测的纯函数。
//!
//! 责任：
//! 1. 解析上级 SDP → (upstream_host, upstream_port, upstream_ssrc)
//! 2. 构造本级向设备的 INVITE SDP（"play" 模式）
//! 3. 调 SipServer::send_play_invite_and_wait → 等 200 OK + ZLM 媒体到达
//! 4. 调 ZLM startSendRtp 把设备流推向上级
//! 5. 在 SendRtpManager 登记 session

use crate::sip::gb28181::cascade_forward::SendRtpManager;

pub struct UpstreamInvitePlan {
    pub platform_id: String,
    pub channel_id: String,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub upstream_ssrc: String,
    pub local_stream_id: String,
    pub local_ssrc: String,
}

/// 解析 WVP-Pro 标准 INVITE SDP，提取媒体端点
pub fn parse_invite_sdp(sdp: &str) -> Result<(String, u16, String), String> {
    let mut media_ip = String::new();
    let mut media_port = 0u16;
    let mut ssrc = String::new();
    for line in sdp.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("c=IN IP4 ") {
            media_ip = rest.to_string();
        } else if line.starts_with("m=video ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                media_port = parts[1].parse().map_err(|e| format!("bad port: {}", e))?;
            }
        } else if let Some(rest) = line.strip_prefix("y=") {
            ssrc = rest.chars().take(10).collect();
        }
    }
    if media_port == 0 || media_ip.is_empty() {
        return Err("SDP missing c= or m=video".to_string());
    }
    if ssrc.is_empty() {
        ssrc = "0000000000".to_string();
    }
    Ok((media_ip, media_port, ssrc))
}

pub fn build_upstream_invite_plan(
    platform_id: &str, channel_id: &str, sdp: &str,
) -> Result<UpstreamInvitePlan, String> {
    let (upstream_host, upstream_port, upstream_ssrc) = parse_invite_sdp(sdp)?;
    let local_stream_id = format!("cascade_{}_{}", platform_id, channel_id);
    let local_ssrc = format!("0{:0>9}0", &channel_id[..channel_id.len().min(9)]);
    Ok(UpstreamInvitePlan {
        platform_id: platform_id.to_string(),
        channel_id: channel_id.to_string(),
        upstream_host, upstream_port, upstream_ssrc,
        local_stream_id, local_ssrc,
    })
}
```

```rust
// src/sip/gb28181/cascade_service.rs::handle_upstream_invite 改写
pub async fn handle_upstream_invite(
    &self, platform_id: &str, channel_id: &str, sdp: &str,
) -> Result<String, String> {
    if !self.get_session(platform_id).map(|s| s.is_active()).unwrap_or(false) {
        return Err(format!("Platform {} not active", platform_id));
    }
    // 5.3: 删掉"简化"分支，改走真实链路
    let plan = crate::sip::gb28181::upstream_invite::build_upstream_invite_plan(
        platform_id, channel_id, sdp
    )?;
    let cascade_call_id = plan.local_stream_id.clone();

    // 1) SipServer 调设备 INVITE（走 Phase 3.1 media_waiter）
    let sip = self.sip_server.as_ref()
        .ok_or_else(|| "SIP server not available".to_string())?
        .read().await;
    let (device_call_id, _media_ready) = sip.send_play_invite_and_wait(
        channel_id, channel_id, /* rtp_port */ 0, Some(&plan.local_ssrc)
    ).await.map_err(|e| format!("device INVITE failed: {}", e))?;
    drop(sip);

    // 2) ZLM startSendRtp → 上级 IP:port
    let zlm = self.zlm_client.as_ref()
        .ok_or_else(|| "ZLM client not available".to_string())?;
    zlm.start_send_rtp(&plan.local_stream_id, &plan.upstream_host, plan.upstream_port)
        .await
        .map_err(|e| format!("startSendRtp failed: {}", e))?;

    // 3) SendRtpManager 登记
    let session = SendRtpSession::new(
        cascade_call_id.clone(), platform_id.to_string(), channel_id.to_string(),
        plan.upstream_host.clone(), plan.upstream_port, plan.upstream_ssrc.clone(),
    );
    self.send_rtp_manager.create(session);
    tracing::info!(
        "5.3 cascade upstream INVITE completed: platform={} channel={} -> upstream={}:{}",
        platform_id, channel_id, plan.upstream_host, plan.upstream_port
    );
    Ok(cascade_call_id)
}
```

**子任务：**
- [ ] **Step 1**: 创建 `src/sip/gb28181/upstream_invite.rs`，定义 `UpstreamInvitePlan` + `parse_invite_sdp` + `build_upstream_invite_plan`
- [ ] **Step 2**: 添加单元测试 `test_parse_invite_sdp_*`（5-6 个 case：标准 WVP-Pro SDP / 缺 c= / 缺 m= / 缺 y= / 端口非法）
- [ ] **Step 3**: 修改 `src/sip/gb28181/cascade_service.rs::handle_upstream_invite` 替换"简化"分支
- [ ] **Step 4**: 验证 `src/zlm/client.rs::start_send_rtp` 公共方法已存在
- [ ] **Step 5**: 验证 `src/sip/server.rs::send_play_invite_and_wait` 公共方法签名
- [ ] **Step 6**: 验证 `src/sip/gb28181/cascade_forward.rs::SendRtpManager::create` 接受 session
- [ ] **Step 7**: 添加集成测试 `test_upstream_invite_full_flow`（mock SipServer + ZLM，验证：解析 SDP → 设备 INVITE → startSendRtp → SendRtpManager 登记）
- [ ] **Step 8**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 9**: Commit `feat(phase-5): upstream INVITE → device INVITE → ZLM SendRtp full chain`

### Task 5.4 — `on_send_rtp_stopped` 按 send_rtp_id 路由到具体 SendRtpManager session（P0，10h）

**目标**：
- ZLM `on_send_rtp_stopped` 推送时，按 `data.stream` 查 SendRtpManager
- 若该 stream_id 属于 SendRtpManager session → 标记 `active=false` + 通知上级 BYE
- 防止 cascade 流在 ZLM 侧异常断开时本级状态残留

**Files:**
- Modify: `src/zlm/hook.rs::on_send_rtp_stopped` handler（按 stream 路由）
- Modify: `src/sip/gb28181/cascade_forward.rs`（新增 `close_by_stream` 方法）
- Modify: `src/sip/server.rs`（SendRtpManager 注入 AppState 后暴露给 hook）
- Test: `src/sip/gb28181/cascade_forward.rs::tests`（3-4 个）

**关键代码骨架**：

```rust
// src/sip/gb28181/cascade_forward.rs::close_by_stream
impl SendRtpManager {
    /// 5.4: 按 stream_id 关闭 SendRtp session（ZLM 异常断开时调用）
    pub fn close_by_stream(&self, stream_id: &str) -> Option<SendRtpSession> {
        // stream_id 可能带 .ts 后缀；用 starts_with 匹配
        let key = self.sessions.iter()
            .find(|entry| entry.value().cascade_call_id == stream_id
                   || stream_id.starts_with(&entry.value().cascade_call_id))
            .map(|entry| entry.key().clone());
        if let Some(k) = key {
            let mut session = self.sessions.remove(&k)?.1;
            session.active = false;
            // 同步到 StateStore
            if let Some(ref store) = self.state_store {
                store.del_cascade_sendrtp(&k);
            }
            return Some(session);
        }
        None
    }
}
```

```rust
// src/zlm/hook.rs::on_send_rtp_stopped 末尾追加
if let Some(state) = state_clone.as_ref() {
    if let Some(ref sip_server) = state.sip_server {
        if let Some(session) = sip_server.send_rtp_manager().close_by_stream(&data.stream) {
            tracing::info!(
                "5.4 on_send_rtp_stopped → closed cascade session platform={} channel={} stream={}",
                session.platform_id, session.channel_id, data.stream
            );
            // 可选：发 BYE 给上级平台（如果上级未主动 BYE）
        }
    }
}
```

**子任务：**
- [ ] **Step 1**: 在 `src/sip/gb28181/cascade_forward.rs::SendRtpManager` 新增 `close_by_stream` 方法
- [ ] **Step 2**: 添加单元测试 `test_close_by_stream_match_exact` / `test_close_by_stream_match_prefix` / `test_close_by_stream_no_match` / `test_close_by_stream_state_store_sync`（4 个）
- [ ] **Step 3**: 验证 `src/sip/server.rs` 中 `send_rtp_manager` 已暴露访问器
- [ ] **Step 4**: 修改 `src/zlm/hook.rs::on_send_rtp_stopped` handler：先按 `data.app == "rtp"` 判断，再调 `close_by_stream`
- [ ] **Step 5**: 添加集成测试 `test_send_rtp_stopped_cascade_cleanup`（mock ZLM 推 on_send_rtp_stopped → SendRtpManager session 标记 inactive）
- [ ] **Step 6]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 7**: Commit `feat(phase-5): on_send_rtp_stopped routes to SendRtpManager by stream_id`

### Task 5.5a — MobilePosition 上行转发（P1，8h）

**目标**：当本级设备上报 MobilePosition（MESSAGE）时，自动转发到所有 Active 级联平台订阅了位置的上级

**Files:**
- Modify: `src/sip/gb28181/cascade_forward.rs`（新增 `forward_mobile_position_to_all`）
- Modify: `src/sip/server.rs::handle_message`（MobilePosition 解析后转发）
- Modify: `src/sip/gb28181/cascade_service.rs`（暴露"已订阅位置的上级列表"查询）
- Test: `src/sip/gb28181/cascade_forward.rs::tests`（3 个）

**关键代码骨架**：

```rust
// src/sip/gb28181/cascade_forward.rs
/// 5.5a: 转发 MobilePosition 到所有订阅了该通道的上级平台
pub async fn forward_mobile_position_to_all(
    &self, channel_id: &str, body: &str, sn: u32,
) -> Result<usize, String> {
    // 1) 查所有 Active 级联平台
    let platform_ids = self.cascade_service.active_platform_ids_with_position_sub();
    let mut sent = 0;
    for platform_id in platform_ids {
        // 2) 拼装 MobilePosition 上行 XML（保留原始 body + 替换 DeviceID/SN）
        let xml = wrap_mobile_position_for_upstream(&platform_id, channel_id, body, sn);
        // 3) SipServer 调 send_message_to_platform
        if let Some(ref sip) = self.sip_server {
            sip.send_message_to_platform(&platform_id, &xml).await
                .map_err(|e| tracing::warn!("forward MobilePosition to {} failed: {}", platform_id, e))
                .ok();
            sent += 1;
        }
    }
    Ok(sent)
}
```

**子任务：**
- [ ] **Step 1**: 在 `src/sip/gb28181/cascade_service.rs::CascadeService` 新增 `active_platform_ids_with_position_sub`
- [ ] **Step 2**: 在 `src/sip/gb28181/cascade_forward.rs` 新增 `forward_mobile_position_to_all` + `wrap_mobile_position_for_upstream`
- [ ] **Step 3**: 在 `src/sip/server.rs::handle_message` 解析 MobilePosition 后调 forward
- [ ] **Step 4]: 添加单元测试 `test_forward_mobile_position_*`（3 个：active 平台 / 无 active / 转发失败不 panic）
- [ ] **Step 5**: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 6]: Commit `feat(phase-5): MobilePosition upstream forwarding to active cascade platforms`

### Task 5.5b — Alarm 上行转发（P1，8h）

**目标**：当本级设备上报 Alarm（MESSAGE）时，自动转发到所有 Active 级联平台订阅了告警的上级

**Files:**
- Modify: `src/sip/gb28181/cascade_forward.rs`（新增 `forward_alarm_to_all`）
- Modify: `src/sip/server.rs::handle_message`（Alarm 解析后转发）
- Modify: `src/sip/gb28181/cascade_service.rs`（暴露"已订阅告警的上级列表"）
- Test: `src/sip/gb28181/cascade_forward.rs::tests`（3 个）

**关键代码骨架**：同 5.5a 但 `wrap_alarm_for_upstream` 字段名不同

```rust
pub async fn forward_alarm_to_all(
    &self, device_id: &str, body: &str, sn: u32,
) -> Result<usize, String> { /* 同 5.5a 模板 */ }
```

**子任务：**
- [ ] **Step 1**: 在 `src/sip/gb28181/cascade_service.rs` 新增 `active_platform_ids_with_alarm_sub`
- [ ] **Step 2]: 在 `src/sip/gb28181/cascade_forward.rs` 新增 `forward_alarm_to_all` + `wrap_alarm_for_upstream`
- [ ] **Step 3]: 在 `src/sip/server.rs::handle_message` 解析 Alarm 后调 forward
- [ ] **Step 4]: 添加单元测试 `test_forward_alarm_*`（3 个）
- [ ] **Step 5]: 跑全量 `cargo test --lib` 确认无回归
- [ ] **Step 6]: Commit `feat(phase-5): Alarm upstream forwarding to active cascade platforms`

### Task 5.6 — 横切：级联模拟器 + 三库测试矩阵 + 文档（P1，6h）

**目标**：
- 上级平台模拟器（mock SIP socket）能跑：REGISTER → 200 OK → Catalog 查询 → 目录响应 → INVITE → 200 OK → BYE
- 三库 CI 全绿
- docs/OPERATIONS.md 新增 Phase 5 章节

**Files:**
- Create: `tests/integration/cascade_e2e_test.rs`（上级平台模拟器端到端）
- Create: `scripts/phase5-test-matrix.sh`（三库 cargo test 脚本）
- Modify: `.github/workflows/ci.yml`（已有三库 job；本阶段确认仍工作）
- Modify: `docs/OPERATIONS.md`（新增 Phase 5 章节）
- Test: 跨 5.1-5.5 集成 case

**子任务：**
- [ ] **Step 1**: 在 `tests/integration/cascade_e2e_test.rs` 实现 mock 上级平台：
  - 监听 0.0.0.0:0 UDP
  - 接受 REGISTER → 401 challenge
  - 接受 digest REGISTER → 200 OK
  - 等待 KEEPALIVE（MESSAGE Keepalive）→ 200 OK
  - 发起 Catalog 查询 → 收到 Response
  - 发起 INVITE（带 SDP） → 收到 200 OK
  - 发起 BYE → 收到 200 OK
- [ ] **Step 2]: 端到端 case 1：本级启动 → 注册到模拟上级 → 看到 Registered 状态
- [ ] **Step 3]: 端到端 case 2：模拟上级发 Catalog 查询 → 本级返回目录
- [ ] **Step 4]: 端到端 case 3：模拟上级发 INVITE（mock 设备应答 + mock ZLM startSendRtp）→ SendRtpManager 登记
- [ ] **Step 5]: 端到端 case 4：模拟上级发 BYE → SendRtpManager session 关闭
- [ ] **Step 6]: 端到端 case 5：mock ZLM 推 `on_send_rtp_stopped` → SendRtpManager 标记 inactive
- [ ] **Step 7]: 创建 `scripts/phase5-test-matrix.sh`（复制 phase-3 模板）
- [ ] **Step 8]: 跑 `bash scripts/phase5-test-matrix.sh` 确认三库 exit 0
- [ ] **Step 9]: 在 `docs/OPERATIONS.md` 新增 Phase 5 章节（操作步骤 + 验收命令）
- [ ] **Step 10]: Commit `docs(phase-5): ops doc + 3-DB test matrix script + cascade e2e test`

---

## 验收命令

```bash
# 默认（sqlite）
cargo test --lib

# PostgreSQL
cargo test --no-default-features --features postgres --lib

# MySQL
cargo test --no-default-features --features mysql --lib

# Phase 5 关键单测
cargo test --lib cascade::
cargo test --lib sip::gb28181::upstream_invite::
cargo test --lib sip::gb28181::record_info_upstream::
cargo test --lib sip::gb28181::cascade_forward::

# 集成测试
cargo test --test integration_test cascade
bash scripts/phase5-test-matrix.sh
```

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/cascade/register.rs` | 抽 build_digest_response 公共函数 | 2h |
| `src/sip/gb28181/cascade_service.rs` | 标 deprecated + 委派给 CascadeRegistrar + 删硬编码 | 6h |
| `src/sip/gb28181/cascade_forward.rs` | close_by_stream + forward_mobile_position_to_all + forward_alarm_to_all | 10h |
| `src/sip/gb28181/upstream_invite.rs`（新） | UpstreamInvitePlan + parse_invite_sdp + build_upstream_invite_plan | 6h |
| `src/sip/gb28181/record_info_upstream.rs`（新） | handle_upstream_record_info_query + build_upstream_record_info_response | 6h |
| `src/sip/server.rs` | handle_message 加 RecordInfo / MobilePosition / Alarm 分支 + INVITE 串通 cascade | 8h |
| `src/zlm/hook.rs` | on_send_rtp_stopped 按 stream 路由 | 3h |
| `src/lib.rs` | 串联 cascade_periodic_tasks + run_registration_loop | 3h |
| `src/db/cloud_record.rs` | query_by_device_channel 三态 cfg | 1h |
| `tests/integration/cascade_e2e_test.rs`（新） | mock 上级平台端到端 | 8h |
| `scripts/phase5-test-matrix.sh`（新） | 三库 cargo test 一键 | 0.5h |
| `docs/OPERATIONS.md` | Phase 5 章节 | 2h |

**总计**：~100h ≈ 2.5 周编码 + 1 周 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）

- **5.1**：`cascade::register::tests::test_build_digest_response_known_vector`（RFC 2617 案例）+ `test_periodic_tasks_wires_up`（3-4 个）
- **5.2**：`sip::gb28181::record_info_upstream::tests::test_build_upstream_record_info_response_*`（3-4 个）
- **5.3**：`sip::gb28181::upstream_invite::tests::test_parse_invite_sdp_*`（5-6 个：标准 / 缺 c= / 缺 m= / 缺 y= / 端口非法 / SSRC 长度）
- **5.4**：`sip::gb28181::cascade_forward::tests::test_close_by_stream_*`（4 个：精确 / 前缀 / 不匹配 / StateStore 同步）
- **5.5a**：`sip::gb28181::cascade_forward::tests::test_forward_mobile_position_*`（3 个）
- **5.5b**：`sip::gb28181::cascade_forward::tests::test_forward_alarm_*`（3 个）

### 集成测试（`tests/integration/cascade_e2e_test.rs`）

- mock 上级平台接受本级 REGISTER → 401 → digest 重试 → 200 OK
- mock 上级平台发 Catalog 查询 → 收到 Response
- mock 上级平台发 INVITE → 收到 200 OK + SendRtpManager 登记
- mock 上级平台发 BYE → SendRtpManager session 关闭
- mock ZLM 推 `on_send_rtp_stopped` → cascade session 标记 inactive

### 端到端（手测，对应设计文档 Acceptance）

- 真实 WVP-Pro Java 启动 → 配置本级为下级 → 看到本级设备目录
- WVP-Pro Java 点播本级通道 → 拉流成功
- WVP-Pro Java 停止 → 本级 SendRtp 关闭
- WVP-Pro Java 订阅本级告警/位置 → 收到上报

---

## 衔接说明

### 与 Phase 3 衔接

- **3.3 RecordInfo 多包等待** → 5.2 上行 RecordInfo 复用 `send_record_info_query_and_wait` + `accumulate_record_info` 模板
- **3.1 Live Play 媒体等待** → 5.3 上级 INVITE 触发设备 INVITE 时复用 `media_waiter_manager`

### 与 Phase 4 衔接

- **4.5 StreamStatus 统一接口** → 5.4 `close_by_stream` 中 SendRtpSession 状态字段复用 StreamStatus::Stopped
- **4.x ZLM hook 全集** → 5.4 在 `on_send_rtp_stopped` 上扩展路由

### 与 Phase 6 衔接

- **6.x JT1078 终端会话** → 本阶段 SendRtpManager / CascadeRegistrar 抽象不变；Phase 6 直接复用
- **6.x 终端告警/位置** → 5.5a/5.5b 上行转发可被 JT 设备告警/位置复用

### 与 Phase 7 衔接

- **7.x Redis StateStore 扩展** → 5.4 `close_by_stream` 已使用 `state_store.del_cascade_sendrtp`（E1 已实现）
- **7.x 跨节点 RPC** → SendRtpManager 跨节点时由 Redis 同步；本期单节点即可

---

## 风险与缓解

### R1: 上级 INVITE 整链路涉及 4 个模块串通 — **HIGH ⚠️**
- 涉及 SipServer::handle_invite、cascade_service、cascade_forward、ZLM
- 任一环节失活整链路断
- **缓解**：
  - 5.3 拆 3 个子步骤：解析 SDP（纯函数）→ 设备 INVITE（复用 Phase 3.1）→ ZLM startSendRtp（独立）
  - 每步单独单测；端到端 case 在 5.6 模拟器集成
  - mock ZLM startSendRtp 返回 Ok 时即使没真推流也通过

### R2: CascadeService 标 deprecated 但仍保留 — **MEDIUM**
- 双套状态机并存风险
- **缓解**：
  - 5.1 把 CascadeService 所有方法改成"委派" + 编译期 `#[deprecated]` 警告
  - 后续 Phase 8+ 删除 CascadeService 整体

### R3: send_rtp_stopped 路由可能误关非 cascade 流 — **MEDIUM**
- `data.stream` 可能与 live / playback 流同名
- **缓解**：
  - `close_by_stream` 仅在 SendRtpManager sessions 中查找（不会命中 InviteSessionManager / PlaybackInviteSessionManager）
  - 测试覆盖：mock live / playback 流同时存在，close_by_stream 不误关

### R4: 订阅转发可能洪泛 — **LOW**
- 1000 个上级平台 × 100 设备/秒告警 = 10 万上行/秒
- **缓解**：
  - 5.5a/5.5b 转发用 tokio::spawn + bounded mpsc，不阻塞 handle_message
  - 后续 Phase 7 用 Redis pub/sub 优化

### R5: `local_id` 仍可能硬编码 — **MEDIUM**
- 5.1 必须修复，但 5.5 上行 XML 拼装时也要用 `local_id`
- **缓解**：
  - 5.1 统一在 SipConfig 加 `device_id` 字段（已有）
  - 5.5 全部从 SipConfig 读取，不允许 inline 字符串

### R6: 三态 cfg 在 cascade_service 改动后回归 — **MEDIUM**
- CascadeService 涉及 db::platform，可能破坏 mysql / postgres
- **缓解**：
  - 5.1 仅委派方法签名不变，db 调用无新增
  - 5.6 跑 `bash scripts/phase5-test-matrix.sh` 必须三库全绿

### N1（新增）：上级平台模拟器集成测试需要 mock ZLM startSendRtp — **LOW**
- 真实 ZLM 启动慢
- **缓解**：
  - 5.6 端到端 case 3 中 mock ZLM startSendRtp → 返回 Ok
  - 真实 ZLM 集成在 docs/OPERATIONS.md "Phase 5 真实部署" 章节手测

---

## 重新评估后的 P0/P1 优先级

| 任务 | 原优先级 | 重审后 | 理由 |
|---|---|---|---|
| 5.1 CascadeRegistrar 串联 | P0 | **P0** | 双套状态机不收敛，后续必崩 |
| 5.2 RecordInfo 上行 | P0 | **P0** | WVP Java 点播依赖录像查询 |
| 5.3 上级 INVITE 整链路 | P0 | **P0** | 设计文档 Acceptance 第 2 条核心 |
| 5.4 send_rtp_stopped 路由 | P0 | **P0** | 资源泄漏 + 状态不一致 |
| 5.5a MobilePosition 上行 | P1 | **P1** | 功能正确但能 work |
| 5.5b Alarm 上行 | P1 | **P1** | 与 5.5a 平行 |
| 5.6 横切 + 三库 + 文档 | P1 | **P1** | 跟着 5.1-5.5 一起做 |

---

## 实施顺序调整

1. **第一批（P0，~52h）**：
   - 5.1 CascadeRegistrar 串联（为后续 4 个任务铺路）
   - 5.4 send_rtp_stopped 路由（最小代价，清理残留）
2. **第二批（P0，~36h）**：
   - 5.2 RecordInfo 上行（复用 Phase 3.3 模板）
   - 5.3 上级 INVITE 整链路（核心 R1 风险，需要更多调试时间）
3. **第三批（P1，~16h）**：
   - 5.5a MobilePosition 上行
   - 5.5b Alarm 上行
4. **第四批（P1，~6h）**：
   - 5.6 横切清理 + 三库测试矩阵 + 文档

---

## 完成判定

- **三库 `cargo test --lib` 全绿**（默认 sqlite + `--features postgres` + `--features mysql` 三路 0 失败）
- `scripts/phase5-test-matrix.sh` 一键三库验证脚本 exit 0
- **新增 ≥ 25 个单测**覆盖 5.1-5.5 涉及的所有模块（CascadeRegistrar / record_info_upstream / upstream_invite / cascade_forward / SendRtpManager）
- **集成测试 ≥ 5 个**：mock 上级平台 REGISTER / Catalog / INVITE / BYE / send_rtp_stopped
- **真实 WVP-Pro Java 至少 1 个能完成 register → catalog → play → stop 全流程**（手测通过）
- `docs/OPERATIONS.md` 新增 Phase 5 章节，操作步骤可复现
- `CascadeService` 标注 `#[deprecated]`，所有方法委派
- 端到端 `cargo test --test integration_test cascade` 全绿
- 主流程代码搜索 `127.0.0.1:5060` 在 cascade_service.rs 中**仅剩 0 命中**（已迁 SipConfig）
- CI workflow 含三库 job（sqlite 默认 + 显式 postgres/mysql）
