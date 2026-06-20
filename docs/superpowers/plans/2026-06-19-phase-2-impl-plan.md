# Phase 2 实施方案 — Device Query / Catalog / Subscriptions / PTZ

## Context

按设计文档 `docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md` §7 Phase 2，需把以下四类 GB28181 设备命令对齐到 WVP-Pro Java 后端：

1. **DeviceStatus / DeviceInfo / ConfigDownload 查询**：等待设备响应
2. **Catalog 多包同步 + 行政区划 / 业务组**
3. **Catalog / MobilePosition / Alarm 订阅生命周期**
4. **PTZ / Preset / Guard / FrontEnd / Cruise 路径点**

**Acceptance**（设计文档）：
- 模拟设备可注册、同步 Catalog、应答设备查询、接受 PTZ、推送订阅的位置 / 告警事件
- 删除 / 重命名 / 隔离 compatibility-empty handlers

**当前差距**（代码审计确认）：
- `DeviceCommander` 已实现 3 个 query 注册方法，但 `SipServer` 缺 `send_device_info_query` / `send_device_status_query` 公共方法；`send_device_config_query` 走 `send_message_to_device` 不走 PendingRequest
- `PendingRequest::new` 把 `response_sender` 设为 None，调用方拿不到结果
- `SubscriptionLifecycle::get_needing_renew` 有但全代码库无人调用 → 后台续订任务缺失
- Catalog / Alarm NOTIFY 无 Redis 广播
- PTZ 编码路径分裂（`build_ptz_xml` 用 GB/T 28181 标准 hex，`PtzEncode::direction_8` 用 A5/AF 厂商格式）
- Cruise 路径点 CRUD 缺失（SetPoint / DeletePoint / SetSpeed / SetTime）

**预估工作量**：~30h（4 工作日编码 + 1 周 buffer）→ 与设计文档 "2-3 周" 估算吻合。

---

## 任务清单

### Task 2.1 — Device Query 等待响应（P0，10h + 2h 测试）

| 子任务 | 文件 | 改动 |
|---|---|---|
| 改造 `PendingRequest` 持 receiver | `src/sip/gb28181/pending_request.rs:84-109` | 新增 `register_with_receiver` 返回 `(PendingRequest, oneshot::Receiver<String>)`；保留原 `register` 为 fire-and-forget |
| `DeviceCommander` 三个 query 改 async | `src/sip/gb28181/device_commander.rs:62-97` | 返回 `Result<DeviceQueryResult>`（Ok / Timeout / ParseError / DeviceOffline），内部 `tokio::time::timeout` + `await rx` |
| `SipServer` 新增 3 个公共方法 | `src/sip/server.rs:2818-2832` | `send_device_info_query` / `send_device_status_query` / `send_device_config_query`，统一走 `DeviceCommander` |
| `SipServer` 暴露 `device_commander()` | `src/sip/server.rs:221-258` | 新增 `device_commander: Arc<DeviceCommander>` 字段 + 访问器 |
| `handlers/device_control.rs::device_config_query` 改造 | `src/handlers/device_control.rs:272-322` | 改 wait-for-response 模式 |
| 单测 | `device_commander.rs::tests` | timeout / 解析错误 / 正常路径；`pending_request.rs` oneshot cancel 不 panic |

**关键代码骨架**：
```rust
// pending_request.rs
pub fn register_with_receiver(
    &self, device_id: &str, sn: u32, cmd: PendingCmdType,
    call_id: &str, timeout_secs: u64,
) -> (PendingRequest, oneshot::Receiver<String>) {
    let (tx, rx) = oneshot::channel();
    let req = PendingRequest::new(device_id, sn, cmd, call_id, Some(tx), timeout_secs);
    // ... 同原 register 逻辑
    (req, rx)
}
```

```rust
// device_commander.rs
pub async fn query_device_info(&self, device_id: &str, sn: u32)
    -> Result<DeviceInfoData, DeviceQueryError>
{
    let call_id = format!("dc_info_{}_{}", device_id, sn);
    let (req, rx) = self.pending.register_with_receiver(...);
    self.pending.insert(req);
    self.send_query(device_id, &call_id, "DeviceInfo", ...).await?;
    match tokio::time::timeout(Duration::from_secs(15), rx).await {
        Ok(Ok(xml)) => self.parse_device_info(&xml).ok_or(ParseError),
        Ok(Err(_)) => Err(Timeout),  // sender dropped
        Err(_) => Err(Timeout),
    }
}
```

### Task 2.2 — Catalog 多包 + BusinessGroup + WS 广播（P1，5h + 1h 测试）

| 子任务 | 文件 | 改动 |
|---|---|---|
| `XmlParser` 提取 BusinessGroup | `src/sip/gb28181/xml_parser.rs` | `parse_catalog_channels` 增加 BusinessGroup 提取 |
| `db_device::upsert_channel_from_catalog` 增量字段 | `src/db/device.rs` | business_group 列 + migration 脚本 |
| 消除 handle_notify 与 CatalogSyncManager.flush_to_db 重复 | `src/sip/server.rs:1989-2013` | 改为调 `catalog_sync_manager.handle_packet(device_id, body).await`；保留 B2 平台分支 → `gb_platform_channel::batch_add_channels` |
| CatalogSyncManager 注入 WsState | `src/sip/gb28181/catalog_sync.rs:128-247` | 多包收齐后广播 `catalogChanged` 事件 `{deviceId, count, completed}` |
| 单测 | `catalog_sync.rs::tests` | 3 包乱序到达也能 Done；BusinessGroup 字段解析 |

### Task 2.3 — Subscription Lifecycle 续订 + Redis 广播（P0/P1，8h + 2h 测试）

| 子任务 | 文件 | 改动 | 优先级 |
|---|---|---|---|
| `SubscriptionLifecycle` 注入 SipServer | `src/sip/server.rs:221-258` | 新增 `subscription_lifecycle: Arc<SubscriptionLifecycle>` 字段 | P0 |
| `send_subscribe` 注册到 SubscriptionLifecycle | `src/sip/server.rs:2839-2912` | 末尾 `self.subscription_lifecycle.register(device_id, event, &call_id, expires)` | P0 |
| 收到 SUBSCRIBE 200 响应时 renew | `src/sip/server.rs:2089-2170` | handle_response 中 `if cseq.contains("SUBSCRIBE")` 分支调 `renew()` | P0 |
| **后台续订任务** 替换 server.rs:529-601 两段循环 | `src/sip/server.rs:523-601` | 单一 `subscription_renewal_loop` 周期 30s，调 `lifecycle.get_needing_renew()` → `send_subscribe_internal` | P0 |
| handle_catalog_notify 增加 Redis 广播 | `src/sip/gb28181/subscription_lifecycle.rs:168-204` | publish 到 `catalog:{device_id}` 频道 | P1 |
| handle_alarm_notify 增加 Redis 广播 | `src/sip/gb28181/subscription_lifecycle.rs:274-315` | publish 到 `alarm:{device_id}` 频道 | P1 |
| 修 handle_position_notify 死代码警告 | `src/sip/gb28181/subscription_lifecycle.rs:255-256` | 删除未用变量 | P1 |
| 单测 + 集成 | `subscription_lifecycle.rs::tests` | 3 订阅 → 270s → `get_needing_renew`=3 | P0 |

**后台续订骨架**（照搬 C3 cascade_periodic_tasks 模式）：
```rust
pub async fn subscription_renewal_loop(
    sip: Arc<RwLock<SipServer>>,
    state_store: Arc<StateStore>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        let s = sip.read().await;
        let lifecycle = s.subscription_lifecycle().clone();
        let needing = lifecycle.get_needing_renew();
        for (device_id, event, call_id, expires) in needing {
            if let Err(e) = s.send_subscribe(&device_id, &event, expires).await {
                tracing::warn!("renewal failed for {}/{}: {}", device_id, event, e);
            }
        }
    }
}
```

### Task 2.4 — PTZ / Preset / Guard / FrontEnd / Cruise（P1，10h + 2h 测试）

| 子任务 | 文件 | 改动 | 优先级 |
|---|---|---|---|
| 修 `CLE_PRESET` 拼写 | `src/sip/gb28181/ptz.rs:112` | 改为 `CLEAR_PRESET`（WVP 规范一致） | P0 |
| 统一 PTZ 编码路径到 A5/AF | `src/handlers/device_control.rs:233-249` | `build_ptz_xml` 删除；改用 `PtzEncode::direction_8` | P0 |
| SipServer 暴露 4 个 thin 包装 | `src/sip/server.rs:2801-2816` | `send_ptz_command` / `send_preset_command` / `send_guard_command` / `send_front_end_command`（fire-and-forget） | P1 |
| Cruise 路径点 4 个 API | `src/handlers/front_end.rs` | `SetPoint` / `DeletePoint` / `SetSpeed` / `SetTime` → `FrontEndCommand::to_xml` | P1 |
| device_ptz / device_preset / device_guard 迁移 | `src/handlers/device_control.rs:22-143` | 改用 SipServer 新方法；`build_ptz_xml` / `build_preset_xml` 标记 `#[allow(dead_code)]` | P1 |
| 单测 | `ptz.rs::tests` | 4 个 FrontEndCommand XML 校验、A5/AF 头格式 | P1 |

**Cruise XML 骨架**（基于现有 FrontEndCommand）：
```rust
// ptz.rs:FrontEndCommand::to_xml 增加 4 个变体
FrontEndCommand::SetPoint { cruise_id: u32, point_id: u32, preset_id: u32 }
    => format!("<Control>...<FrontEndCmd>SetPoint {} {} {}</FrontEndCmd></Control>", cruise_id, point_id, preset_id)
```

### Task 2.5 — 横切清理 compatibility-empty handlers（P1，3h）

| 子任务 | 文件 | 改动 |
|---|---|---|
| 清理 stub.rs | `src/handlers/stub.rs` | grep "TODO" "unimplemented" + 无实际实现的 device handler → 移到 `legacy/` 或 `#[deprecated]` |
| 清理 device_stub.rs | `src/handlers/device_stub.rs:779` | alarm_subscribe 调用迁移到 `device_control.rs` 的 alarm 订阅 |
| 移除 parity_extras.rs 残留 | `src/handlers/parity_extras.rs` | 删除无实现项 |

---

## 关键文件改动清单

| 文件 | 改动 | 估时 |
|---|---|---|
| `src/sip/gb28181/pending_request.rs` | `register_with_receiver` + oneshot 持有 | 2h |
| `src/sip/gb28181/device_commander.rs` | 3 个 query 改 async + 结构化返回 | 3h |
| `src/sip/server.rs` | `device_commander` 字段 + 4 个公共方法 + 后台续订循环 | 5h |
| `src/handlers/device_control.rs` | config_query 改 wait-for-response；PTZ 路径统一 | 3h |
| `src/handlers/front_end.rs` | 4 个 Cruise 路径点 API | 3h |
| `src/sip/gb28181/ptz.rs` | 修 `CLE_PRESET` 拼写 | 0.1h |
| `src/sip/gb28181/catalog_sync.rs` | 注入 WsState + 落库去重 | 1.5h |
| `src/sip/gb28181/subscription_lifecycle.rs` | `handle_catalog_notify` 加 redis/ws 参数；修 Redis 死代码 | 2h |
| `src/handlers/stub.rs` `device_stub.rs` `parity_extras.rs` | 隔离/重命名/删除空 handler | 3h |
| `src/db/device.rs` | upsert_channel_from_catalog 加 business_group 字段 | 1h |
| 测试 `tests/integration_test.rs` `tests/e2e_test.rs` | 新增 4 个跨子任务集成用例 | 4h |

**总计**：~30h ≈ 4 个工作日（纯实现 + 测试）+ 4-5 天 review/buffer。

---

## 验收测试

### 单元测试（每子任务必跑）
- **2.1**：`device_commander.rs::tests` — timeout 路径、oneshot cancel 不 panic、parse 错误返回
- **2.2**：`catalog_sync.rs::tests` — 3 包乱序到达 SumNum 收齐、BusinessGroup 字段解析
- **2.3**：`subscription_lifecycle.rs::tests` — 3 订阅 → 270s → `get_needing_renew`=3
- **2.4**：`ptz.rs::tests` — 4 个 FrontEndCommand XML 校验、A5/AF 头格式

### 集成测试（`tests/integration_test.rs`）
- 注册虚拟设备 → DeviceInfo 查询 → 200ms 内收到结果
- 5 包 Catalog SumNum=5 乱序到达 → `gb_device_channel` 行数 = 解析 Item 数
- Catalog 订阅 5s 后 mock NOTIFY → `SubscriptionLifecycle` 续期成功
- PTZ 全方向 + Stop 共 9 个命令 → 设备收到 9 条 SIP MESSAGE
- Fire-and-forget vs wait-for-response 区分：PTZ stop 无需响应、DeviceInfo 必须有响应

### 端到端（手测，对应设计文档 Acceptance）
- 真实 IPC 注册 → Catalog 同步 → 控制台显示通道列表
- 调用 `/api/device/ptz?...&command=LEFT` → 设备云台左转
- 调用 `/api/device/subscribe/mobilePosition` → 设备推送位置 → Redis `position:xxx` 收到消息
- `/api/device/config/query` 返回设备配置 XML

---

## 衔接说明

### 与已完成的 Phase 1 衔接
- **1.1 PendingRequest** — 2.1 在其基础上加 receiver（**前向兼容**：保留 `register` 签名）
- **1.3 PendingRequest cleanup** — 2.1 必须保证 `cleanup_expired` 在 sender drop 时调 `sender.send("")` 或 skip
- **B2 handle_notify** — 2.2 收编重复逻辑但保留 B2 `gb_platform_channel` 落库分支

### 与 C3 CascadeRegistrar 衔接
- 2.3 的后台续订循环复用 C3 `cascade_periodic_tasks` 模式：`tokio::spawn` + `Arc<RwLock<SipServer>>` + 30s interval

### 与 E1 StateStore 衔接
- 2.3 的 `SubscriptionLifecycle` 可选注入 `StateStore` 做跨节点订阅状态共享（Phase 3+ 范畴，本次不强求）

---

## 风险与缓解（重新评估版）

### R1: send_subscribe socket 读锁死锁 — **MEDIUM（已部分缓解）**
**状态**：仍存在但概率低
- `send_subscribe`（`server.rs:2839-2912`）内 `socket = self.socket.read().await` 持读锁到 `socket.send_to(...).await` 调用结束
- 现有调用方（`device_control.rs` handlers）不持 `&mut self`，但若将来有嵌套调用可能死锁
- `send_subscribe_internal`（`server.rs:3821-3896`）已存在，签名 `(&Arc<UdpSocket>, ...)`，**不获取 SipServer 内部锁**，续订循环使用它是安全的
**缓解**：维持现状，所有后台循环必须用 `send_subscribe_internal` 而非 `send_subscribe`

### R2: PendingRequest oneshot 与 cleanup 竞争 — **ELIMINATED ✅**
**状态**：不是真实风险
- `pending_request.rs:91-103`：`PendingRequest::new` 创建 oneshot 后立即 `take()` 并丢弃 receiver
- `complete()`（`pending_request.rs:166-180`）不涉及 oneshot
- `device_commander` 调用方只用 `pending_count()` / `has_pending_for_device()`，**从未 await receiver**
- oneshot 是预留但从未接通的扩展点
**结论**：当前实现与风险无关；2.1 任务的"新增 register_with_receiver"是新增能力而非修旧 bug

### R3: 后台续订 spam — **MEDIUM**
**状态**：已部分缓解（`server.rs:529-566` 有 `device.online` 检查），仍缺退避
- 60s 重试间隔限制 spam 上限
- **但**：设备离线 → SUBSCRIBE 失败 → 下个 60s 又失败 → 永循环无 backoff
- 也无 per-device 失败计数器
**缓解**：
- 加 per-device `failure_count` 字段，超过阈值（如 5 次）后停止重试 N 分钟
- 或者用 `SubscriptionLifecycle::unregister` 标记 inactive（**当前 R6 显示 SubscriptionLifecycle 是死代码**，需要先激活它）
- 短期方案：仅依赖 `device.online` 检查，保留 60s 重试

### R4: Catalog NOTIFY 路径重复 — **LOW**
**状态**：两条路径并存但功能分工清晰
- `server.rs:1989-2013` (handle_notify 内联循环)：设备目录同步 → `gb_device_channel`
- `server.rs:1965-1987` (B2 上游平台检测)：`gb_platform_channel::batch_add_channels`
- `catalog_sync.rs::CatalogSyncManager::flush_to_db` / `handle_packet` **从未被调用**（模块未被 SipServer 实例化）
- `handle_notify` 的 O(n) per-channel DB 调用对大目录性能不佳但功能正确
**缓解**：
- **不强行统一**两条路径（B2 上游 + 设备目录本质不同）
- 仅优化：把 per-channel DB 循环改为单条 `INSERT ... ON CONFLICT DO UPDATE`（PG）/ `INSERT ... ON DUPLICATE KEY UPDATE`（MySQL）
- `catalog_sync.rs::CatalogSyncManager` 整体删除（死代码 ~371 行）

### R5: `CLE_PRESET` 拼写错误 + A5/AF 路径不可达 — **HIGH ⚠️**
**状态**：未修复且影响预置位删除
- `ptz.rs:112` 仍写 `CLE_PRESET`（与 WVP 规范和所有 handler 不一致：`device_control.rs:256` 和 `front_end.rs:96` 均用 `CLEAR_PRESET`）
- 设备收到 `CLE_PRESET` 会忽略，导致预置位删除实际不工作
- `PtzEncode::direction_8`（`ptz.rs:147-176`）定义的 A5/AF 编码（`A5{:02X}...AF`）**没有任何 handler 调用**
**缓解**：
- P0 修复 `CLE_PRESET` → `CLEAR_PRESET`（一个字符改动）
- 文档化：A5/AF 路径暂保留作为扩展点，不在 Phase 2 范围内激活

### R6: SubscriptionLifecycle / NotifyDispatcher 死代码 — **MEDIUM**
**状态**：~371 行死代码
- `subscription_lifecycle.rs` 整个模块（SubscriptionLifecycle、NotifyDispatcher）从未被 SipServer 实例化
- 续订后台循环（`server.rs:529-566`）独立实现，不调用 `SubscriptionLifecycle::get_needing_renew`
- `NotifyDispatcher::handle_position_notify` 与 `server.rs:2328-2374::handle_mobile_position` 逻辑重复
- `handle_catalog_notify` 与 `server.rs:1989-2013` 内联逻辑重复
**风险**：维护负担 + 新人理解混乱（两套订阅管理）
**缓解**：
- 方案 A（推荐）：删除 `subscription_lifecycle.rs` 整个文件，把 `get_needing_renew` 等纯函数逻辑搬到 `CatalogSubscriptionManager`（或新建 `SubscribeManager`），让后台循环调用它
- 方案 B（保留）：在 `subscription_lifecycle.rs` 头加注释明确"服务器视角"，并把 SipServer 接入该模块
- 短期建议方案 A（更彻底）

### N1（新增）：MobilePosition 临时 manager
- `server.rs:589` 每轮 renew 都 `Arc::new(CatalogSubscriptionManager::new())`，状态不跨周期保留
- DB 查询驱动实际 renew，in-memory 状态对 MobilePosition 无效
- 风险低；建议删除临时构造，直接用 `state.catalog_subscription_manager` 单例

### N2（新增）：SubscriptionManager call_id 累积
- `send_subscribe_internal`（`server.rs:3884-3893`）每次 renew 都 `CatalogSubscription::new(...)` 创建新 entry
- `CatalogSubscriptionManager.subscribe` 不去重 → 旧 call_id 累积
- 需要：基于 (device_id, event) 去重；或定期清理过期 entry

### N3（新增）：handle_mobile_position 与 handle_position_notify 重复
- `server.rs:2328-2374` 与 `subscription_lifecycle.rs:206-271` 几乎一字不差
- 与 R6 同步处理：删 `subscription_lifecycle.rs` 即可消除

---

## 重新评估后的 P0 优先任务

| 任务 | 原优先级 | 重审后 | 理由 |
|---|---|---|---|
| 修 `CLE_PRESET` 拼写 | P1 | **P0** | 影响预置位删除功能 |
| 删除/激活 `subscription_lifecycle.rs` 死代码 | P1 | **P0** | 371 行死代码 + 与 catalog 模块重复 |
| R3 续订 spam 加退避 | P1 | **P0** | 现网长期运行后会无限 spam |
| Device Query 等待响应（R2 已消除，仅新增能力） | P0 | P0 | 新功能，非修 bug |
| Catalog NOTIFY 路径优化 | P1 | P2 | 不影响功能，仅性能 |
| A5/AF 编码路径激活 | P1 | **删除（不实施）** | 文档化为扩展点 |

---

## 实施顺序调整（基于重审）

1. **第一批（P0，~6h）**：
   - 修 `CLE_PRESET` → `CLEAR_PRESET`
   - 删除 `subscription_lifecycle.rs` 死代码（保留 `get_needing_renew` 等纯函数搬到 `CatalogSubscriptionManager`）
   - 修 R3 续订退避（per-device failure_count）

2. **第二批（P0/P1，~15h）**：
   - 2.1 Device Query 等待响应（新增 oneshot receiver 能力）
   - 2.3 SubscriptionLifecycle 续订调用 get_needing_renew（替代 server.rs:529-566 独立循环）

3. **第三批（P1/P2，~10h）**：
   - 2.2 Catalog NOTIFY O(n) 优化（合并 INSERT）
   - 2.4 PTZ / Cruise 路径点 API（除 CLE_PRESET 已修）

4. **第四批（P1，~3h）**：
   - 2.5 横切清理 stub.rs / device_stub.rs