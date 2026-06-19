# Phase 3 实施方案 — Live / Playback / RecordInfo / Download / Talk-Broadcast

> 基线 commit：`13144e9`（feat(phase-2): DeviceQuery 等待响应 + SubscriptionLifecycle 激活 + Catalog 批量 upsert + Cruise）
> 上游设计：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 3
> 阶段承接：A2（PlaybackSession）+ A3（RecordInfo 多包）+ A4（GB28181 Download INVITE） — 上述子任务已经做了"骨架"，本阶段目标是把骨架串成真实生产闭环

---

## Context

按设计文档 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 3，需把以下五类视频/录像流对齐到 WVP-Pro Java 后端：

1. **Live Play**：SSRC 分配 → ZLM RTP server → SIP INVITE/ACK → ZLM media-arrival hook → 超时清理
2. **Playback**：start/stop/pause/resume/seek/speed 走真实 playback SDP + INVITE 会话
3. **RecordInfo**：等待多包设备响应 → 聚合 → 分页返回
4. **Download**：GB28181 Download INVITE → ZLM MP4 落盘 → 进度追踪 → 受控 stop
5. **Talk/Broadcast**：与 live 分开管理；BYE / RTP 状态正确清理

**Acceptance**（设计文档原文）：
- 实时 / 回放 / RecordInfo / Download / talk / broadcast 在模拟器 + 至少一个真实设备环境跑通
- 主流程不再依赖 placeholder `127.0.0.1/live/...` 代理路径

**当前差距**（代码审计确认，对照 `src/handlers/play.rs` / `playback.rs`）：

| # | 现状 | 缺口 |
|---|---|---|
| 1 | `play_start` 仅等 SIP 200 OK | 没有等 ZLM 媒体到达；`media_waiter_manager` 在 SipServer 持有但 handler 路径未串联 |
| 2 | `play_stop` 失败 fallback 到 `send_talk_bye` | live 和 talk 用错 session；`send_talk_bye` 对 live 完全是错语义 |
| 3 | `playback_start` 走 `rtsp://127.0.0.1/live/{device}/{channel}` 兜底 | 占位 URL，仍依赖本地代理 |
| 4 | `playback_pause` / `playback_resume` 用裸 XML 走 `send_message_to_device` | 应该走 `send_playback_control(Pause/Resume)`，A2 已实现该函数 |
| 5 | `gb_record_query` 真正只查 ZLM MP4 本地文件 | 多包 SIP 响应被忽略；`ResponseRouter::accumulate_record_info` 已存在但 handler 没串联 |
| 6 | `gb_record_download_start` 的 GB28181 路径 hardcode `"progress": 0` | 无 ZLM MP4 落盘状态回调 → 进度永远是 0 |
| 7 | `broadcast_start` 调用 `send_talk_invite`（live 的对偶是 talk，方向相反） | talk 与 broadcast 共享同一会话结构，BYE 时清理方向错 |
| 8 | ZLM hook 已实现 `on_stream_changed` 但没接到 `MediaWaiterManager::resolve_by_stream` | 媒体到达通知无法终结 wait_for_media |

**预估工作量**：~40h（5 个工作日编码 + 1 周 buffer） → 与设计文档 "3-5 周" 估算吻合。

---

## 任务清单

### Task 3.1 — Live Play 真实化（P0，10h + 2h 测试）

**目标**：`/api/play/start` 必须等 SIP 200 OK **且** ZLM 媒体到达（`on_stream_changed` 触发），超时清理一切资源；`/api/play/stop` 仅 BYE live session，不再 fallback 到 talk BYE。

| 子任务 | 文件 | 改动 |
|---|---|---|
| `MediaWaiterManager` 注入 SipServer 字段 | `src/sip/server.rs:218` | 已存在；新增 `pub fn media_waiter_manager(&self)` 访问器 |
| `play_start` 注册 media waiter | `src/handlers/play.rs:43-110` | `send_play_invite_and_wait` 之前先 `media_waiter_manager.register(call_id, stream_id, "rtp", 15)`；SIP 200 OK 后 `tokio::time::timeout(15s)` 等 `resolve_by_stream` 或 `cleanup_expired` |
| ZLM hook 触发 media waiter 解决 | `src/zlm/hook.rs::sync_stream_changed`（或附近的 `on_stream_changed` 分支） | 检测到 stream 注册时调 `sip.media_waiter_manager().resolve_by_stream(stream, app)` |
| 移除 `play_stop` 的 talk BYE fallback | `src/handlers/play.rs:152-158` | 删除 `send_talk_bye` fallback；只在 `InviteSessionManager::get_by_device_channel` 命中 live 时 BYE |
| `InviteSession` 关联 `stream_id` | `src/sip/gb28181/invite_session.rs:181-220` | `InviteSession::new` 接收 `zlm_stream_id` / `app` 字段；`create` 时一并写入，BYE 时用该字段调 `close_rtp_server` |
| 超时清理 | `src/handlers/play.rs::play_start` | 等 15s 未到达 → `close_rtp_server` + `send_session_bye` + 返回 408 |
| 单测 + 集成 | `media_waiter.rs::tests` + `tests/integration_test.rs` | mock ZLM on_stream_changed → 200ms 内 `resolve_by_stream` 返回 true；超时路径不 panic |

**关键代码骨架**：

```rust
// src/handlers/play.rs::play_start
match sip.send_play_invite_and_wait(&device_id, &channel_id, rtp_server.port, Some(&ssrc)).await {
    Ok(call_id) => {
        // 等媒体到达（最多 15s）—— MediaWaiterManager::register 返回 (key, oneshot::Receiver<MediaWaitResult>)
        let (_waiter_key, media_rx) = sip.media_waiter_manager().register(
            &call_id, &stream_id, "rtp", 15
        );
        match tokio::time::timeout(Duration::from_secs(15), media_rx).await {
            Ok(Ok(crate::sip::gb28181::media_waiter::MediaWaitResult::MediaReady { .. })) => {
                /* 返回完整 play_url */
            }
            _ => {
                let _ = zlm_client.close_rtp_server(&stream_id).await;
                let _ = sip.send_session_bye(&device_id, &channel_id).await;
                return Json(WVPResult::error("Media arrival timeout"));
            }
        }
    }
    Err(_) => { /* 已有清理 */ }
}
```

```rust
// src/zlm/hook.rs on_stream_changed 分支末尾追加
if data.app == "rtp" {
    if let Some(ref sip) = state.sip_server {
        sip.media_waiter_manager().resolve_by_stream(&data.stream, &data.app);
    }
}
```

### Task 3.2 — Playback 真实化（P0，10h + 2h 测试）

**目标**：`playback_start` 不再回退到 `rtsp://127.0.0.1/live/...`；`pause` / `resume` / `speed` / `seek` 全部走 `send_playback_control`；媒体到达检测与 3.1 共享 `MediaWaiterManager`。

| 子任务 | 文件 | 改动 |
|---|---|---|
| `playback_start` 删除占位 URL 分支 | `src/handlers/playback.rs:187-244` | 移除 `rtsp://127.0.0.1/live/...` `AddStreamProxyRequest` 兜底；改为先开 ZLM RTP server (`stream_id=playback_{device}_{channel}_${ts}`) → `send_playback_invite_and_wait`（新增） → 写 `PlaybackInviteSession` → 等媒体到达 → 返回 |
| 新增 `send_playback_invite_and_wait` | `src/sip/server.rs:3398` 附近 | 复制 `send_play_invite_and_wait` 模式，但用 `build_playback_sdp` + Subject 第 4 段 SSRC 前缀 1 |
| `playback_pause/resume` 改 `send_playback_control` | `src/handlers/playback.rs:279-340` | 替换裸 XML `send_message_to_device` 为 `send_playback_control(Pause/Resume)` |
| `PlaybackInviteSession` 关联 media waiter | `src/sip/gb28181/playback_session.rs:36-100` | `PlaybackInviteSession::new` 接收 `zlm_stream_id` / `app`；`start` handler 注册 waiter，hook 触发 resolve |
| 单测 | `playback_session.rs::tests` | `pause/resume/seek/scale` 4 个 case；`PlaybackInviteSession` 状态机 |

**`send_playback_invite_and_wait` 骨架**：

```rust
pub async fn send_playback_invite_and_wait(
    &self, device_id: &str, channel_id: &str,
    start_time: &str, end_time: &str,
    media_port: u16, ssrc: Option<&str>,
) -> Result<String> {
    let call_id = format!("pbk_{}_{}", device_id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let sdp = build_playback_sdp(&self.config.ip, media_port, start_time, end_time);
    let ssrc = ssrc.map(String::from).unwrap_or_else(|| build_playback_ssrc(device_id));
    // 注册 PendingRequest 等 200 OK
    let (req, rx) = self.pending.register_with_receiver(...);
    self.pending.insert(req);
    self.send_invite_with_sdp(device_id, channel_id, &call_id, &sdp, &ssrc, SubjectPrefix::Playback).await?;
    // 已经有 await rx 模板（同 2.1 任务）
    let body = tokio::time::timeout(Duration::from_secs(15), rx).await??;
    self.parse_invite_response(&body)?;
    Ok(call_id)
}
```

### Task 3.3 — RecordInfo 多包等待 + 分页（P0，8h + 2h 测试）

**目标**：`/api/playback/{device}/{channel}/record` 真正等 SIP 多包 RecordInfo 响应，调用方可以分页（page / count）。

| 子任务 | 文件 | 改动 |
|---|---|---|
| `send_record_info_query` 改 async + 返回 `Vec<RecordInfoItem>` | `src/sip/server.rs:3161` | 当前是 fire-and-forget；改用 `pending_request.register_with_receiver` + `accumulate_record_info` + `parse_record_info_items`；15s 超时返回已有部分 |
| 新增 `QueryResult::Items(Vec<RecordInfoItem>)` 变体 | `src/sip/gb28181/pending_request.rs` | 替换当前的 `Raw(xml)`；`accumulate_record_info` 返回 true 时解析 → `Items` |
| `gb_record_query` handler 等多包 | `src/handlers/playback.rs:478-536` | `match sip.send_record_info_query(...).await { Ok(items) => 返回分页; Err(_) => ZLM 兜底 }`；page / count 走标准 `slice` |
| ZLM 兜底降级 | 同上 | SIP 失败 / 设备离线 → 当前 ZLM MP4 文件逻辑保留为兼容路径 |
| 单测 + 集成 | `pending_request.rs::tests` | 多包 SumNum=5 乱序到达 → `Items(merge_5_items)`；page=1&count=10 取前 10 |

**RecordInfo 异步骨架**：

```rust
// src/sip/server.rs
pub async fn send_record_info_query_and_wait(
    &self, device_id: &str, channel_id: &str,
    start_time: &str, end_time: &str, sn: u32,
) -> Result<Vec<RecordInfoItem>> {
    let call_id = format!("rec_{}_{}", device_id, sn);
    let (req, mut rx) = self.pending.register_with_receiver(
        device_id, sn, PendingCmdType::RecordInfo, &call_id, 15
    );
    self.pending.insert(req);
    self.send_record_info_query_internal(device_id, channel_id, start_time, end_time, sn).await?;

    let mut items = Vec::new();
    let mut packet_count = 0i32;
    loop {
        match tokio::time::timeout(Duration::from_secs(15), &mut rx).await {
            Ok(Ok(QueryResult::Items(packet))) => {
                items.extend(packet);
                packet_count += 1;
                // 设备在 SumNum=N 包后会发最后一包，检查 SumNum
                if let Some(sum) = self.extract_record_sum_num(&body) {
                    if packet_count >= sum { break; }
                }
            }
            Ok(Ok(QueryResult::Raw(xml))) => {
                items.extend(parse_record_info_items(&xml));
                packet_count += 1;
            }
            _ => break,
        }
    }
    Ok(items)
}
```

### Task 3.4 — Download 真实化（P0，8h + 2h 测试）

**目标**：GB28181 Download 路径下，`/api/playback/.../download/start` 后 `/download/progress` 返回真实进度（来自 ZLM MP4 落盘字节数），`stop` BYE 后清理 ZLM RTP server。

| 子任务 | 文件 | 改动 |
|---|---|---|
| `DownloadSession` 关联 zlm_stream_id | `src/handlers/playback.rs:90-130` | 新增字段 `zlm_stream_id: String`、`zlm_app: String`、`mp4_record_key: Option<String>` |
| GB28181 路径开 RTP server 后注册 media waiter | `src/handlers/playback.rs:563-619` | `media_waiter_manager.register(call_id, stream_id, "rtp", 15)`；等流到达后状态 `inviting` → `downloading` |
| ZLM 落盘进度回调 | `src/zlm/hook.rs::sync_stream_changed` | `if data.app == "rtp" && data.stream.contains("download_")` → `download_manager.update_progress(stream, size, "downloading")` |
| `DownloadSession::update_progress` 用绝对字节数 | `src/handlers/playback.rs:126` | 当前 0..100 模糊语义；改为 (current_bytes, total_bytes) 并对外仍返回百分比 |
| `gb_record_download_stop` 真清理 | `src/handlers/playback.rs:668-700` | 调 `media_waiter_manager.unregister` + `close_rtp_server` + `sip.send_session_bye`；ZLM 本地路径保持 `stop_download` |
| 单测 | `download_session.rs::tests` | 进度更新 0% → 50% → 100%；BYE 后 stream_id 从 media_waiter 移除 |

### Task 3.5 — Talk / Broadcast 分流（P1，4h + 1h 测试）

**目标**：talk（客户端 → 设备）与 broadcast（设备 → 客户端）使用独立会话管理；`TalkManager` 现有结构保留，broadcast 改用 `InviteSessionManager`（live 风格）。

| 子任务 | 文件 | 改动 |
|---|---|---|
| 新增 `BroadcastManager` | `src/sip/gb28181/broadcast.rs`（新文件） | 复用 `TalkManager` 模板；状态机 Pending/Active/Terminated；Subject 第 4 段 SSRC 前缀 4（与 WVP 兼容） |
| `broadcast_start` 改用 `BroadcastManager` | `src/handlers/play.rs:163-206` | `sip.broadcast_manager().create(...)` + `send_broadcast_invite`（新方法） |
| `broadcast_stop` 调 `send_broadcast_bye`（新方法） | `src/handlers/play.rs:208-236` | 移除 `send_talk_bye`；BYE 走 broadcast session |
| `send_broadcast_invite` / `send_broadcast_bye` | `src/sip/server.rs`（新增 2 个方法） | 与 talk 类似但用 `BroadcastManager` + `build_broadcast_sdp`（音频多播） |
| `TalkManager` 收编 `send_talk_invite` 调用 | `src/handlers/talk.rs:15-90` | 已基本正确；只需确认 talk stop 不影响 broadcast |
| 单测 | `broadcast.rs::tests` | talk 与 broadcast session 不互相影响；broadcast 200 OK 后 `TalkManager::active_count` 不变 |

**Subject 命名规范**（与 WVP Java 一致）：

| 用途 | SSRC 前缀 | 类型 | Manager |
|---|---|---|---|
| Live Play | 0 | Play | InviteSessionManager |
| Playback | 1 | Playback | PlaybackInviteSessionManager |
| Download | 2 | Download | DownloadSession + InviteSessionManager |
| Talk | 3 | Audio | TalkManager |
| Broadcast | 4 | Audio | BroadcastManager |

### Task 3.6 — 横切：清理 placeholder 路径与 dead code（P1，2h）

| 子任务 | 文件 | 改动 |
|---|---|---|
| `grep "127.0.0.1/live"` | `src/handlers/*.rs` `src/sip/*.rs` | 替换为 `format!("rtsp://{}/live/...", media_ip)` 或走 ZLM `getStreamProxy` 返回真实流地址 |
| 删除 `playback_start` 的 `rtsp://127.0.0.1/...` 占位 | `src/handlers/playback.rs:188` | 整段删除 |
| 删除 `play_stop` 的 talk BYE fallback | `src/handlers/play.rs:152-158` | 整段删除 |
| 文档更新 | `docs/OPERATIONS.md` | 新增 "Phase 3 真实视频/录像闭环" 章节 |

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/handlers/play.rs` | play_start 等媒体到达；play_stop 去 talk fallback；broadcast 用 BroadcastManager | 6h |
| `src/handlers/playback.rs` | playback_start 去占位 URL；pause/resume 改 send_playback_control；download 进度真值 | 8h |
| `src/sip/server.rs` | media_waiter_manager 访问器；send_playback_invite_and_wait；send_broadcast_invite/bye；record_info_query_and_wait | 6h |
| `src/sip/gb28181/invite_session.rs` | InviteSession::new 接收 zlm_stream_id/app | 2h |
| `src/sip/gb28181/playback_session.rs` | PlaybackInviteSession 关联 media waiter | 1h |
| `src/sip/gb28181/broadcast.rs`（新） | BroadcastManager + build_broadcast_sdp | 3h |
| `src/sip/gb28181/talk.rs` | 收编 talk 与 broadcast 不互通 | 1h |
| `src/sip/gb28181/pending_request.rs` | QueryResult::Items 变体；多包 SumNum 自终结 | 3h |
| `src/zlm/hook.rs` | on_stream_changed 触发 media_waiter 解决；download 进度更新 | 3h |
| `src/db/record.rs` | 新增 record_list_page（按 device/channel/time 范围分页查询本地缓存） | 2h |
| 测试 `tests/integration_test.rs` `tests/e2e_test.rs` | 新增 5 个跨子任务集成用例 | 4h |
| `docs/OPERATIONS.md` | Phase 3 章节 | 1h |

**总计**：~40h ≈ 5 个工作日（纯实现 + 测试）+ 5-6 天 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）

- **3.1**：`media_waiter.rs::tests` — resolve_by_stream 命中；register 后 cleanup_expired 不 panic；双 stream 注册独立清理
- **3.2**：`playback_session.rs::tests` — PlaybackInviteSession 状态机（Pending → Active → Paused → Active → Sought → Stopped）
- **3.3**：`pending_request.rs::tests` — 多包 5 item SumNum=3 乱序到达 → 合并正确；page=1&count=2 取前 2
- **3.4**：`download_session.rs::tests` — 进度 0→50→100；BYE 后 stream_id 从 media_waiter 移除
- **3.5**：`broadcast.rs::tests` — talk 与 broadcast session 不互通；同 device_id 不冲突

### 集成测试（`tests/integration_test.rs`）

- 注册虚拟设备 → `/api/play/start` → 200ms 内返回 play_url（mock ZLM on_stream_changed）
- 同上但 mock ZLM 不触发 → 15s 后返回 408 + RTP 端口已关
- `/api/playback/start` → 等真实 SDP 通过；pause/resume/speed/seek 各自发 SIP INFO，设备收到正确 XML
- `/api/playback/.../record` SumNum=3 多包 → 合并 ≥10 item
- `/api/playback/.../download/start` → 30s 后 `/download/progress` 返回真实百分比（非 0）

### 端到端（手测，对应设计文档 Acceptance）

- 真实 IPC 注册 → `/api/play/start` → 浏览器拉流可看（rtsp://, http://...flv, ws://...flv, hls）
- 真实 IPC + 历史录像 → `/api/playback/start?startTime=...&endTime=...` → 可倒放/暂停/调速
- 真实 IPC → `/api/playback/.../record` → 列表与 IPC 本地录像一致
- 真实 IPC → `/api/playback/.../download/start` → `/download/progress` 持续上升 → `/download/stop` 收尾
- 真实 IPC + 客户端话筒 → `/api/talk/start` → 设备端听到
- 真实 IPC + 多个客户端 → `/api/broadcast/start` → 所有客户端听到设备端

---

## 衔接说明

### 与已完成的 Phase 1/2 衔接

- **2.1 PendingRequest receiver** — 3.3 复用 `register_with_receiver` 模板直接拼装 RecordInfo 异步等待
- **2.3 SubscriptionLifecycle** — 3.1 不涉及（live play 与订阅无关）；3.5 talk/broadcast 与 catalog/position subscription 平行
- **A2 PlaybackSession** — 3.2 在其基础上加 SDP / media waiter 等待，不重写
- **A3 RecordInfo 多包** — 3.3 把 fire-and-forget 改成 async 等待
- **A4 Download INVITE** — 3.4 在其基础上加进度回调

### 与 Phase 4 (ZLM hook) 衔接

- 3.1 / 3.4 借 `on_stream_changed` 实现媒体到达检测
- Phase 4 真正完成 `on_rtp_server_timeout` 时，3.1 的超时清理逻辑可以共用

### 与 Phase 7 (Redis StateStore) 衔接

- 3.4 的 DownloadSession 进度目前只在内存；Phase 7 用 Redis 持久化跨节点
- 3.5 的 BroadcastManager 同理

---

## 风险与缓解

### R1: 媒体到达超时导致假阴性 — **MEDIUM**
- 当前 `media_waiter_manager` 默认 15s；某些低带宽 IPC 推流首包会 > 15s
- **缓解**：超时时间按 ZLM 节点 `rtp_proxy_timeout` 配置动态调整（30-60s），新增 `play_start` query 参数 `mediaTimeoutMs`

### R2: PlaybackInviteSession 与 InviteSession 双 manager 重复 — **MEDIUM**
- 当前 A2 阶段引入了 `PlaybackInviteSessionManager`（330 行），与 `InviteSessionManager` 大量重叠
- 3.2 不可避免地要继续扩展前者
- **缓解**：3.2 不重写；记录技术债，Phase 7 用 StateStore 抽象合并

### R3: DownloadSession 内存状态不可跨节点恢复 — **LOW**
- 集群部署时，A 节点 BYE 后 B 节点 `/download/progress` 查不到 session
- **缓解**：3.4 加 `download_session` 字段到 RedisBackend（Phase 7 范畴，本期仅留接口占位）

### R4: Broadcast 与 Talk 共用 9100/9101 端口冲突 — **HIGH ⚠️**
- 现有 `TalkManager::build_talk_sdp` 写死 9100/9101
- 同时启动 talk + broadcast → ZLM RTP 端口冲突
- **缓解**：
  - broadcast 改用 9103/9104（区分 talk）
  - 或 ZLM RTP server 用 `port=0` 让 ZLM 自动分配（最稳）
  - 本期强制走 ZLM 自动分配

### R5: on_stream_changed 触发过频，命中其它 stream — **MEDIUM**
- `data.app == "rtp"` 但 stream 不一定是当前 live session
- **缓解**：只在 `data.stream == expected_stream_id` 时 `resolve`；其它 stream 忽略
- 测试覆盖：mock 同时推送两个 stream，只有匹配的 resolve

### R6: 旧 `send_talk_bye` fallback 删除破坏手测脚本 — **LOW**
- 当前 `play_stop` 在 live session 缺失时 fallback；删除后必须 live session 一定存在
- **缓解**：live 路径 3.1 保证 `InviteSession` 一定创建；`play_stop` 找不到 session 时返回 `success_empty()`（与现状一致），不报错

### N1（新增）：`build_playback_sdp` start/end_time 传 "0" 表示即时
- 当前 `send_download_invite` 走 `build_playback_sdp(&self.config.ip, 0, "0", "0")`；start/end=0 在 SDP 里是历史回放语义
- WVP Java 的 Download SDP 用 `t=0 0`，download 实时拉取
- **缓解**：3.4 不动 SDP，仅确保 DownloadSession 不依赖 start/end_time 字段（确认后删除冗余）

### N2（新增）：`playback_start` 的 `stream_id` 包含 `ts` 后无法精确匹配 `get_by_stream`
- `playback_session.rs::get_by_stream(stream_id)` 用 `split('_')` 解析 device/channel
- ts 在末尾，split 后 device/channel 正确，但 channel 后还有 ts 片段
- **缓解**：3.2 改用 `playback_manager.find_by_device_channel(device_id, channel_id)` 新方法

---

## 重新评估后的 P0 优先任务

| 任务 | 原优先级 | 重审后 | 理由 |
|---|---|---|---|
| 3.1 Live Play 真实化 | P0 | **P0** | 设计文档 Acceptance 第一条 |
| 3.2 Playback 真实化 | P0 | **P0** | 占位 `127.0.0.1/live/...` 必删 |
| 3.3 RecordInfo 多包 | P0 | **P0** | A3 fire-and-forget 必须改为等待 |
| 3.4 Download 真实化 | P0 | **P0** | 进度永远是 0 是严重缺陷 |
| 3.5 Talk/Broadcast 分流 | P1 | **P1** | 功能正确但能 work |
| 3.6 横切清理 | P1 | **P1** | 跟着 3.1/3.2 一起做 |

---

## 实施顺序调整（基于重审）

1. **第一批（P0，~16h）**：
   - 3.1 Live Play 真实化（媒体到达 + BYE 去 fallback）
   - 3.3 RecordInfo 多包等待（直接复用 2.1 register_with_receiver 模板）
   - 3.4 Download 进度真值（on_stream_changed 回调）

2. **第二批（P0/P1，~18h）**：
   - 3.2 Playback 真实化（占位 URL 删 + pause/resume 用 send_playback_control）
   - 3.5 Talk/Broadcast 分流（独立 BroadcastManager）

3. **第三批（P1，~6h）**：
   - 3.6 横切清理 + 文档

---

## 完成判定

- `cargo test --lib` 全绿，新增 ≥ 15 个单测覆盖 live/playback/record/download/talk/broadcast
- `tests/integration_test.rs` 新增 ≥ 5 个跨子任务用例
- 真实 IPC / NVR 至少 1 个能完成 play → playback → record → download → talk → broadcast 全流程（手测通过）
- `docs/OPERATIONS.md` 新增 Phase 3 章节，操作步骤可复现
- 主流程代码搜索 `rtsp://127.0.0.1/live/...` 返回 0 命中（除测试 fixture）