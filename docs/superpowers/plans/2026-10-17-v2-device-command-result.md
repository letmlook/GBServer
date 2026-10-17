# 2.0.0 设备命令结果 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让设备控制与传输模式调用区分发送、设备确认、拒绝及未确认结果。

**Architecture:** SIP 层在发送前登记唯一 SN 和 pending；ResponseRouter 严格匹配设备、类型和 SN；HTTP 保留旧字段并增量暴露结果状态。持续 PTZ 默认即时返回，可选短等待用于离散命令。

**Tech Stack:** Rust、Tokio、Axum、SQLx、Vue 3、Playwright。

**Spec:** `docs/superpowers/specs/2026-10-17-v2-device-command-result-design.md`

## Global Constraints

- 版本 `2.0.0`；保留既有公开路径及 `updated`、`streamMode`、`sipSent`、`sipError` 字段。
- `confirmed` 只能来自匹配 SN 的 `Result=OK`；SIP 200 和发送成功均不等价。
- 等待最多 5 秒；超时为 `unconfirmed`，不可自动重发设备命令。
- HTTP 返回 `ApiResult<T>`；不改变现有管理员鉴权方式。

## Review Focus

- 同设备并发命令的 SN 不得碰撞；在任务 1 的并发测试覆盖。
- Call-ID 正确但 CmdType 错误的应答不能完成请求；在任务 1 的路由测试覆盖。
- 通道级 PTZ 应答 `DeviceID` 为通道编码时仍匹配 SIP 来源设备；在任务 1 的路由测试覆盖。
- DB 保存成功但设备离线须返回 `not_sent`；在任务 3 的接口测试覆盖。
- 设备迟到或重复应答不能改变新请求；在任务 1 的超时测试覆盖。

---

### Task 1: 严格关联 DeviceControl Response

**Files:** Modify `src/sip/gb28181/pending_request.rs`、`src/sip/server.rs`；Test 同文件测试模块。

**Interfaces:** 产生 `PendingCmdType::DeviceControl`；`ResponseRouter::route_message_response_for_device(body, call_id, device_id)` 只完成三元组匹配的 pending。保留 `ConfigDownload` 查询与 `DeviceConfig` 应答的同一 pending 类型。

- [ ] **Step 1: 写失败测试。** 分别断言 OK、非 OK、`DeviceConfig`、错误类型、错误设备、通道 target、缺 SN、重复、迟到；并发创建 100 个 SN 断言唯一。
- [ ] **Step 2: 验证失败。** 运行 `cargo test --lib sip::gb28181::pending_request -- --nocapture`，记录当前不识别 DeviceControl 或错误完成的断言。
- [ ] **Step 3: 最小实现。** 扩展 CmdType 映射；完成前比对设备、类型、SN 与 target；仅从完整、匹配的 Response 提取 Result。SN 由进程唯一前缀和单调计数器产生，重启后不复用上一代前缀。
- [ ] **Step 4: 验证通过。** 运行 `cargo test --lib sip::gb28181::pending_request` 与 SIP server 定向测试；上述用例全过。
- [ ] **Step 5: 提交。** `git add src/sip && git commit -m "fix(sip): correlate device control results"`

### Task 2: 提供可等待的发送入口

**Files:** Modify `src/sip/server.rs`、`src/sip/gb28181/device_commander.rs`；Test 同文件测试模块。

**Interfaces:** 新增 `send_device_control_confirmable(device_id: &str, channel_id: &str, cmd_type: &str, body: &str) -> Result<(u32, tokio::sync::oneshot::Receiver<String>)>`；既有 `send_device_control(...) -> Result<()>` 保持可用。发送失败取消 pending；接收器交给调用方在 5 秒内等待并解释 Result。

- [ ] **Step 1: 写失败测试。** 断言先登记后发送、发送失败 pending 被取消、超时后迟到 Response 不会完成新请求。
- [ ] **Step 2: 验证失败。** 运行 `cargo test --lib send_device_control_confirmable`，确认入口缺失。
- [ ] **Step 3: 最小实现。** 复用任务 1 的 SN/路由，构造与旧入口相同的 Control XML；保留旧入口用于无需等待的调用方。
- [ ] **Step 4: 验证通过。** 运行 `cargo test --lib send_device_control_confirmable` 和 `cargo test --lib sip::`。
- [ ] **Step 5: 提交。** `git add src/sip && git commit -m "feat(sip): expose confirmable device control"`

### Task 3: HTTP 结果契约与页面文案

**Files:** Modify `src/handlers/device_control.rs`、`src/handlers/device_stub.rs`、对应 `web/src/api/device.ts` 和设备操作页面；Test handler 测试模块、相应 Playwright 用例。

**Interfaces:** `deviceResult.status` 使用 `not_sent | sent | confirmed | rejected | unconfirmed`；可选 `sn`、`resultCode`。`waitResult=true` 最多等待 5 秒；旧字段保留。

- [ ] **Step 1: 写失败测试。** 离线 DB 已保存断言 `not_sent`，发送成功无回执断言 `sent` 或等待后的 `unconfirmed`，OK/非 OK 分别断言 `confirmed`/`rejected`；页面只在 confirmed 时显示“设备已执行”。
- [ ] **Step 2: 验证失败。** 运行相关 handler 定向 `cargo test --lib device_transport`、`cd web && npm run build`，记录缺字段或 UI 断言失败。
- [ ] **Step 3: 最小实现。** 扩充响应而非替换旧字段；离散操作按参数调用任务 2 入口，持续 PTZ 保持即时；配置查询复用任务 1 映射。
- [ ] **Step 4: 验证通过。** 运行 `cargo test --lib device_transport`、`cargo test --lib device_control`、`cd web && npm run build` 与设备控制 Playwright 用例。
- [ ] **Step 5: 提交。** `git add src/handlers web/src && git commit -m "feat(device): surface command confirmation state"`

### Task 4: Mock 与真机验收记录

**Files:** Modify `mock/` 中现有 GB28181 模拟器及测试、`docs/STATUS.md`、`docs/OPEN_ISSUES.md`。

**Interfaces:** Mock 回显相同 SN 和 Result；真机记录设备型号、请求 SN、SIP 200、Response Result 四项证据。

- [ ] **Step 1: 写失败集成断言。** Mock 分别回 `OK`、非 OK 和不回报，断言 API 三种结果不混淆。
- [ ] **Step 2: 验证失败。** 运行 Mock 对应测试和控制 E2E，保留失败日志。
- [ ] **Step 3: 完成 Mock 和验收脚本。** 运行一台真机的离散命令；无法取得设备时明确写“真机未验证”，不宣称 B14 关闭。
- [ ] **Step 4: 验证并记录。** 运行 `cargo fmt --check && cargo test --no-fail-fast`、`cd web && npm run build`、相关 E2E；逐项写入实际结果和环境。
- [ ] **Step 5: 提交。** `git add mock docs && git commit -m "test(device): verify command result evidence"`
