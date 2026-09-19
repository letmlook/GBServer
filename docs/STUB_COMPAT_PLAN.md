# `stub.rs` / `device_stub.rs` 定位与清退评估

> 📌 **2026-09-19 重写** —— 原「兼容层退役路线」的多条前提经复核**站不住**，
> 本文件已按代码实测重写。**结论：这两个模块是当前生产实现，不是待退役的兼容层，不应清退。**
>
> 上一版（2026-09-07 初版 + 2026-09-19 数字刷新）的错误在 §5 逐条列出。

---

## 1. 实际规模

| 模块 | 行数 | entry | 挂载路由 |
|------|------|-------|----------|
| `src/handlers/stub.rs` | 3834 | 47 | 47 |
| `src/handlers/device_stub.rs` | 1273 | 18 | 18 |
| **合计** | **5107** | **65** | **65** |

复现：

```bash
wc -l src/handlers/stub.rs src/handlers/device_stub.rs
grep -cE '^\s*pub async fn ' src/handlers/stub.rs src/handlers/device_stub.rs
grep -cE 'stub::|device_stub::' src/router.rs      # 65
```

## 2. 它们到底是什么

**是真实实现，不是占位，也不是薄转发。** 逐个 entry 核对结果：

- **`stub.rs` 的 47 个 entry，函数体直接写在 `stub.rs` 内**，调用 `src/db/*` 完成真实落库。
  承载：角色（1）、区域（10）、业务分组（7）、日志（2）、API Key（7）、录像计划（7）、
  云端录像（12）、通道列表（1）。
  例：`record_plan_add` 校验 `planItemList` 非空 + 落库 + 唤醒调度器；
  `user_api_key_enable` 真的 `set_enable`；`cloud_record_list` 332 行。
- **`device_stub.rs` 的 18 个 entry 同样全部是真实实现**：要么落库，要么真的下发 SIP
  信令 / 调用 ZLM，失败时如实返回错误。**没有「返回固定 JSON 的空实现」**
  （该模块文件头对此已有正确声明）。

`stub` 这个名字与「兼容 shim」的自我描述**具有误导性**：这些不是待删的占位，
而是 `db/` 之上的真实 handler 层。

## 3. 当前前端依赖度（决定能不能清退的关键）

当前 Vue 3 前端（`web/src/api/*.ts`，173 处 URL 字面量）与 65 条路由的对应关系：

| 状态 | 路由数 |
|------|--------|
| **当前前端仍在调用** | **49** |
| 无当前前端调用方 | 16 |

也就是说 **49/65 是活跃前端正在依赖的生产路径**（角色、区域、分组、日志、录像计划、
云端录像、设备增删改查/同步/传输模式、直播流查询、通道流标识等）。
另有 WVP API 契约消费方（非本仓库前端）也走这些路径。

**这直接否决了「退役」的前提**：删掉它们会当场打断当前 UI，而不是清理历史包袱。

> 统计口径：`src/router.rs` 中 `stub::` / `device_stub::` 的 65 条路由 ×
> `web/src/api/*.ts` 的 `url:` 字面量（含模板串，`:param` 按通配匹配）。
> 无拼接式 URL，故统计完整。

## 4. 无前端调用方的 16 条（仅这些是候选）

| 类别 | 路由 | 说明 |
|------|------|------|
| API Key（7） | `/api/userApiKey/add`、`/delete`、`/enable`、`/disable`、`/remark`、`/reset`、`/userApiKeys` | 后端能力完整，但**当前前端没有任何 API Key 管理界面**（`web/src/` 中 `userApiKey`/`apiKey` 零引用） |
| 区域 / 分组（5） | `/api/region/description`、`/api/region/base/child/list`、`/api/region/queryChildListInBase`、`/api/group/path`、`/api/group/delete` | 当前 UI 未暴露这些操作（`region.ts` 未调用） |
| 设备（4） | `/api/device/query/channel/one`、`/api/device/query/tree/channel/:device_id`、`/api/device/query/sub_channels/...`、`/api/device/query/subscribe/alarm` | 见备注 |

> 备注：通道单查前端已改走 `/api/common/channel/one`（`common_channel::channel_one`），
> 故 `/api/device/query/channel/one` 冗余；`subscribe/alarm` 属设备能力，当前 UI 未接。
> **「无前端调用方」不等于「无外部调用方」** ——
> 三方对接方可能仍在用，清退前必须做真实调用监控，不能只 grep 本仓库前端。
>
> 统计口径：`src/router.rs` 的 65 条 shim 路由 × `web/src/api/*.ts` 的 173 处
> `url:` 字面量（模板串插值按通配处理）。
> ⚠️ `/api/device/control/record`（`device_stub::control_record`）**有**前端调用方
> （`web/src/api/device.ts`），**不在**候选之列。

## 5. 上一版的错误（逐条更正）

| 上一版的说法 | 实测 |
|---|---|
| `device_stub.rs` 的 entry「多数仍返回空响应」 | ❌ **无任何空实现**，全部真实落库/下发 |
| `stub.rs` 里的角色/区域/分组/日志/API Key/录像计划是「真实实现已迁出，仍挂旧路径的 shim」 | ❌ 实现**从未迁出**，函数体就在 `stub.rs` 内，调用 `db/*` |
| 新模块为 `handlers::role` / `region` / `log` / `user_api_key` / `record_plan` / `cloud_record` | ❌ 这些 handler 文件**只有 `role.rs` / `region.rs` 存在**；`group.rs`/`log.rs`/`user_api_key.rs`/`record_plan.rs`/`cloud_record.rs` 均不存在 |
| 「设备控制 / 回放」在 `stub.rs` 里 | ❌ `stub.rs` 无这两个模块；二者在 `device_control.rs` / `playback.rs`，且不经 `stub` |
| 「待前端切到新 API 后再做 deprecation」 | ❌ 前提不成立：当前前端**已经在新架构下**，却仍调用这 49 条旧路径 |
| `device_stub.rs` 文件头称报警/移动位置订阅「实际由 `device_control` 处理」 | ❌ router 实际注册的是 `device_stub::subscribe_alarm` / `device_stub::subscribe_mobile_position`；`device_control` 里只有 `subscribe_mobile_position`（且未被 router 引用），`subscribe_alarm` 根本不存在 |
| §2 启动条件「前端 `web/src/api/*.ts` 中已无对应旧路径调用」 | ❌ 当前 49 条命中，该条件离满足很远 |

> 另：`src/handlers/device_stub.rs` 的模块头注释也需要更正（它把两个订阅 handler
> 的归属写错了）。本轮只记录、未改代码注释（纯文档性瑕疵，不影响行为）。

## 6. 如果将来真要清退

前提条件（**全部**满足才启动）：

- [ ] 49 条「前端仍在调用」的路由已迁到替代端点，且前端已切换
- [x] `web-legacy-vue2/` 已移除（2026-09-19 删除，仅存 git 历史）
- [ ] 对 16 条无调用方候选做过**真实调用监控**（含仓库外对接方），持续 90 天无流量

分两类处理，不要一刀切：

1. **16 条无调用方候选**：先加 `#[deprecated]` + 日志埋点观察，再按下方四步走。
2. **49 条活跃路由**：属于**迁移**而不是清退 —— 必须先有替代端点并完成前端切换，
   否则就是功能倒退。当前不存在这样的替代端点。

四步流程（单条路由）：

1. **冻结期**：`#[deprecated(note = "请改用 <新路径>，将于 <日期> 移除")]`
2. **通知期**：`README.md` API 概览加 ⚠️ 标记 + CHANGELOG / RELEASE_NOTES 通告
3. **拦截期**：改为返回 `ErrorCode::Gone`（HTTP 410）
4. **移除期**：删除 entry + `router.rs` 注册 + 相关测试

冻结 + 通知期合计不少于 **90 天**；拦截 + 移除按 PR 一次性提交。

**另需先解决的结构问题**：这两个文件名为 `stub` 却是核心实现，建议解冻后
按业务域拆分为 `handlers::{region,group,log,user_api_key,record_plan,cloud_record}.rs`
并保留路由路径不变 —— 这是纯搬迁，风险低，且能让「兼容层」这个错误认知消失。
注意 `handlers/mod.rs` 的路由注册引用也需同步。

## 7. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-07 | 初版：定义「兼容层」范围、退役触发条件、四步流程 |
| 2026-09-19（早） | 按实测刷新行数/entry 数（3834/1273 行、47/18 entry、65 条路由） |
| 2026-09-19（本版） | **按代码复核重写**：更正「空实现」与「真实实现已迁出」两类错误前提；补充当前前端依赖度实测（49 条活跃 / 16 条无调用方）；补正被写错的 handler 归属；结论改为「生产实现，非待退役兼容层」 |
