# Findings & Decisions

## Requirements
- 输入：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`（§7 Phase 5） + `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md` + `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md`
- 输出：`docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`
- 要求：与既有计划风格一致；可被 subagent 任务级执行；含 File Structure、子任务、关键代码骨架、验收命令、风险与衔接
- 风格：中文；技术名词保持英文；表格化呈现

## Research Findings

### 设计文档 §7 Phase 5 原文要点
1. **Goal**：operate as a lower-level platform for WVP-Pro or standard GB upstream platforms.
2. **Tasks (5)**：
   - 5.1 REGISTER refresh、UNREGISTER、keepalive、retry/backoff、multi-platform state
   - 5.2 响应上级 Catalog / DeviceInfo / DeviceStatus / RecordInfo 查询
   - 5.3 上级 INVITE → 本级 play → ZLM SendRtp → 上级媒体
   - 5.4 BYE/CANCEL + send_rtp_stopped → 清理 SendRtp
   - 5.5 跨平台订阅转发（Catalog / MobilePosition / Alarm）
3. **Acceptance**：Java WVP-Pro 能注册本级为下级平台；上级能查目录、点播、停止、收订阅
4. **Estimate**：3-5 周

### Phase 0 审计（docs/parity/interface-coverage-phase-0.md）相关
- /platform 页面挂载在 WVP-Pro 路由 → 平台级联相关接口存在但深度参差
- B 段（comprehensive plan）已完成：B1 CascadeRegistrar 启动、B2 上级 MESSAGE 路由、B3 上级点播 + SendRtp、B4 平台级联路由补齐
- C3（comprehensive plan）「CascadeRegistrar 自动重试 + 离线恢复」标记未完成（401 digest 重试、Keepalive 超时转 Offline、disable UNREGISTER、状态机 5 种转换）

### 当前代码现状（直接来自仓库）
- `src/cascade/`（C3 实现）：`register.rs` 805 行，`CascadeRegistrar` 完整状态机（NotRegistered / Registering / Challenged / Registered / Failed）、401 digest 重试、Keepalive 超时检测、reload_from_db、unregister_and_remove、build_keepalive_message、`note_liveness`、9 个 `c3_tests::*` 单元测试
- `src/sip/gb28181/cascade_service.rs`（683 行）：与 CascadeRegistrar **功能重复**——CascadeState（CascadeRegistrar 是 RegistrationStatus）、CascadeSession 状态机（Idle/Registering/Active/WaitingAuth/Refreshing/Offline/Failed）、register / keepalive / unregister / 401 challenge / refresh / INVITE 转发骨架、`handle_upstream_invite`（**未串通** INVITE→ZLM SendRtp 全链路）、9 个单测
- `src/sip/gb28181/cascade_forward.rs`（650 行）：`SendRtpManager` + `SendRtpSession`（cascade_call_id / platform_id / channel_id / upstream_host/port/ssrc）+ StateStore 跨节点同步、E1 已注入 StateStore
- `src/sip/server.rs`（~4000 行）：
  - `register_to_platform` / `unregister_from_platform` / `send_platform_catalog` / `send_platform_invite` 已实现
  - `handle_packet` 中平台方向 SIP 已能路由到 `handle_catalog_for_platform` / `handle_device_info_for_platform` / `handle_device_status_for_platform`
  - 上级 INVITE → 设备 INVITE → ZLM SendRtp 全链路在 `handle_invite` 末尾实现（cascade startSendRtp log 在 sip/server.rs:1494）
  - 上级 BYE/CANCEL → 停 SendRtp + 设备 BYE（log 在 sip/server.rs:1730-1736）
  - 仍硬编码 `127.0.0.1:5060` 作为本级 Via/From host（line 290-291 in cascade_service.rs）
- `src/handlers/platform.rs`（225+ 行）：`platform_query` / `sync_platform_registration` / `push_platform_channels` / `refresh_platform_catalog` + 5+ 路由已挂

### 与 Phase 4 衔接点
- phase-4 文档明确：「5.x 级联用 `on_send_rtp_stopped` → Phase 4 在此 hook 加更细致的事件分发（按 send_rtp_id 路由到具体平台）」
- 当前 `src/zlm/hook.rs` 中 `on_send_rtp_stopped` 已存在；需要核对其是否按 send_rtp_id 路由到具体 SendRtpManager session
- `least_load` 跨平台场景在 phase-5 不是主任务，phase-4 6.3 已经覆盖

### Phase 5 子任务映射

| 设计文档 | 当前完成度 | Phase 5 子任务 |
|---|---|---|
| 5.1 REGISTER/Keepalive/retry | CascadeRegistrar (src/cascade) 80% 完整；CascadeService (src/sip/gb28181) 与之重复 | 5.1 收敛两套状态机 + CascadeRegistrar 串联 run_registration_loop + 主库三态 cfg |
| 5.2 上级 Catalog/Info/Status/RecordInfo | Catalog/Info/Status 已实现；RecordInfo 走本级设备 → 暂未做「向上级回 RecordInfo」 | 5.2 补 RecordInfo 上行 + handler 串通 |
| 5.3 上级 INVITE → 本级 → SendRtp | `handle_upstream_invite` 仅打印 log，未串通；cascade_forward.rs:1491-1494 有设备 INVITE → SendRtp 真实逻辑 | 5.3 串通：上级 INVITE → SipServer::handle_invite → cascade_forward 入口 → 设备 INVITE → ZLM SendRtp |
| 5.4 BYE/CANCEL/send_rtp_stopped 清理 | BYE 已实现（sip/server.rs:1730）；`on_send_rtp_stopped` 需核 | 5.4 验证 BYE 路径 + 完善 on_send_rtp_stopped 路由到具体 session |
| 5.5 订阅转发 | Catalog 通知已部分实现；MobilePosition/Alarm 上行未做 | 5.5 拆 5.5a（MobilePosition 上级转发）+ 5.5b（Alarm 上级转发） |

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| CascadeRegistrar 作为唯一状态机，src/sip/gb28181/cascade_service.rs 标记 deprecated | 避免两套实现 drift；cascade_service.rs 仅保留兼容方法 |
| 5.3 串通路径以 cascade_forward.rs::handle_upstream_invite 为入口 | 已有 SendRtpManager + StateStore 注入，扩展而非重写 |
| 沿用 phase-4 的三库 cfg 强约束 | 项目已沉淀的模式 |
| 验收脚本 `scripts/phase5-test-matrix.sh` 复制 phase-3 模板 | 一致性 |
| 上级 By-pass 路由（Catalog/Info/Status）保留 `handle_*_for_platform`，phase-5 在此基础上加 RecordInfo | 与既有架构一致 |
| 5.5 订阅转发不做 WVP 全字段对齐（只 forward 必要字段：device_id/ch_id/coords/alarm_type/time） | 留出 phase-7 Redis 跨节点空间 |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| `cascade_service.rs::build_register_msg` 第 513 行 `response=""` 永远是空 | 5.1 必须修复：需要 CascadeRegistrar 的 digest 实现 + 在 cascade_service 也支持 |
| `cascade_service.rs:219` `local_id` 硬编码 `"34020000002000000001"` | 5.1 必须改为从 SipConfig 读取 |
| `cascade_service.rs:290-291` `127.0.0.1:5060` 硬编码 | 5.1 必须从 SipConfig 读取 |
| `handle_upstream_invite` 注释「简化：记录转发会话，返回成功」→ 实际并未做设备 INVITE | 5.3 重写 |

## Final Deliverable
- `docs/superpowers/plans/2026-06-20-phase-5-impl-plan.md`（~500 行）
- 5 子任务 + 1 横切：5.1 状态机收敛 / 5.2 RecordInfo 上行 / 5.3 上级 INVITE 整链路 / 5.4 send_rtp_stopped 路由 / 5.5a MobilePosition 上行 / 5.5b Alarm 上行 / 5.6 横切+三库+文档
- 估时 ~100h（2.5 周编码 + 1 周 buffer）

## Resources
- 设计文档：`docs/superpowers/specs/2026-05-30-wvp-java-parity-design.md`
- 上游基线：WVP-Pro 2.7.4 / commit b760458
- 既有计划：
  - `docs/superpowers/plans/2026-06-19-phase-3-impl-plan.md`（3.x 视频/录像）
  - `docs/superpowers/plans/2026-06-20-phase-4-impl-plan.md`（4.x ZLM）
  - `docs/superpowers/plans/2026-06-10-comprehensive-impl-plan.md`（B/C/D/E/F 综合）
- 关键代码：
  - `src/cascade/{mod.rs, register.rs}` — 状态机
  - `src/sip/gb28181/{cascade.rs, cascade_service.rs, cascade_forward.rs}` — 应用层
  - `src/sip/server.rs` — SIP 入口（cascade 相关方法 3917+ 行）
  - `src/handlers/platform.rs` — /api/platform/*
  - `src/db/platform.rs` — `gb_platform` 表
  - `src/db/platform_channel.rs` — `gb_platform_channel` 表
  - `src/zlm/hook.rs::on_send_rtp_stopped` — SendRtp 终止回调
- 数据库：`database/init-{sqlite,postgresql,mysql}-2.7.4.sql`

## Visual/Browser Findings
- （无图像/浏览器内容）

---
*2-Action Rule：每 2 次视图/搜索后立即更新；本任务以代码阅读为主，关键发现已落到上方表格*