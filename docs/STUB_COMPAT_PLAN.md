# stub.rs / device_stub.rs 兼容层退役路线

> 本文件跟踪 `src/handlers/stub.rs`（2274 行 / 58 entry）与 `src/handlers/device_stub.rs`（868 行 / 22 entry）两个**前端兼容 shim** 的退役计划。
> 这两个模块不是技术债，是显式设计的向后兼容层；本计划用于在合适时机把它们清空。

---

## 1. 当前定位

| 模块 | 行数 | entry 数 | 角色 |
|------|------|----------|------|
| `src/handlers/stub.rs` | 2274 | 58 | 角色 / 区域 / 分组 / 日志 / API Key / 录像计划 等「真实实现已迁出，但仍挂旧路径」的兼容性 shim |
| `src/handlers/device_stub.rs` | 868 | 22 | 设备 / 通道 / 订阅 / 控制 等「占位接口」（返回空响应但保持前端不报错）的兼容性 shim |

两者总计 **3142 行 / 80 个 entry**，全部挂载在 `src/router.rs`（约 80 个 shim 路由）。

---

## 2. 何时退役

满足下列**全部条件**时启动退役流程：

- [ ] 前端 `web/src/api/*.ts` 中已无对应旧路径调用（grep `'/api/...'`）
- [ ] `web-legacy-vue2/` 已停止维护超过 6 个月
- [ ] 监控无任何客户端调用对应路由 90 天（建议在 `auth_middleware` 加埋点）

启动条件由代码 owner 在评审会议确认。

---

## 3. 退役步骤（每条路由）

1. **冻结期**：在 entry 函数前加 `#[deprecated(note = "请改用 <新路径>，将于 <日期> 移除")]`
2. **通知期**：在 `README.md` 的「API 概览」表格加 ⚠️ 标记；CHANGELOG/RELEASE_NOTES 通告
3. **拦截期**：将 entry 改为返回 `AppError::with_code(ErrorCode::Gone)`（HTTP 410）而非正常响应
4. **移除期**：删除 entry、`router.rs` 中路由注册、相关测试

冻结 + 通知期合计不少于 **90 天**；拦截 + 移除按 PR 一次性提交。

---

## 4. 已迁出（可进入冻结期的候选）

按 `stub.rs` 文件头注释，下列能力**已有真实实现模块**，仅 shim 仍挂载旧路径：

| 旧路径（stub.rs 中） | 新模块 |
|----------------------|--------|
| 角色 / 区域 / 分组 | `handlers::role` / `region` |
| 日志 | `handlers::log` |
| API Key | `handlers::user_api_key` |
| 录像计划 | `handlers::record_plan` |
| 设备控制 | `handlers::device_control` |
| 回放 | `handlers::playback` |

> 这些可以**优先**进入冻结期；具体路由清单见 `git grep -n 'pub async fn' src/handlers/stub.rs`。

---

## 5. 仍为占位（device_stub.rs 全集）

`device_stub.rs` 22 个 entry 多数仍返回空响应（前端兼容 shim）。

| 类别 | entry | 真实实现位置（待） |
|------|-------|--------------------|
| 设备同步 | `sync_status` / `device_sync` / `device_transport` | `handlers::device_query` / `device_control` |
| 设备 CRUD | `device_update` / `device_add` / `device_one` / `device_tree` | `handlers::device`（部分已有） |
| 通道查询 | `channel_one` / `sub_channels` / `tree_channel` | `handlers::common_channel` |
| 订阅类 | `subscribe_mobile_position` / `subscribe_alarm` | 暂无；建议随 GB28181 推送通道建设 |
| 控制类 | `control_record` / `channel_audio` / `channel_stream_identification_update` | `handlers::device_control` |
| 通道流查询 | `query_streams` | `handlers::device_query` |
| 基础参数 | `config_basic_param` | `handlers::device_control` |

完成度按**真实 handler 返回非空 data 且通过集成测试**判定。

---

## 6. 度量

每季度（与 `cargo test` 一同）执行：

```bash
# shim 调用频次（需在 auth_middleware 加埋点后才有意义）
grep -rhE 'fn (sync_status|device_sync|device_transport|...)' src/handlers/stub.rs | wc -l

# 前端依赖项（grep api/* 中旧路径字符串）
cd web && grep -rnE "'/api/(device/sync_status|device/delete|subscribe/catalog)'" src/api/
```

数量不再下降且 ≥ 1 个季度无新前端依赖时，提请评审会议进入 §2 启动条件。

---

## 7. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-07 | 初版：定义兼容层范围、退役触发条件、四步流程 |
