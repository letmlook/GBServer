# 媒体播放、录像可靠性与运维诊断设计

**状态：** 已确认，待实施计划

## 目标

在不改变现有 GB28181 接入、直播播放和 WebRTC 已可用行为的前提下，消除 B1 的无效 RTP 回连与不透明失败，稳定录像计划和 `cloudRecord` 端到端测试，并补齐最小、受控的运维诊断能力。

## 范围与边界

本设计覆盖 `OPEN_ISSUES.md` 的 A2、B1、B2、B3、B4、B5。

- A1（中亿视图 SY）继续保留在待办，保留现有 `/api/sy/*` 路由和实现，但不在本轮补齐其缺失端点。
- A3（第三方 `/api/v1/*` 签名协议）明确范围外；不复用 JWT/API-Key handler 进行拼接式兼容。
- B2、B3 已有生产修复。本轮只补回归测试、诊断证据和文档状态，不替换已验证的运行逻辑。
- B4 是测试进程编排问题，不改变服务器的正常退出或信号处理行为。

## 成功标准

1. 设备 SDP 中的媒体端口与 INVITE 中的 ZLM RTP 端口不同、但媒体已抵达 ZLM 时，点播不再请求 `connectRtpServer`，也不产生误导性的错误日志。
2. 真正需要 ZLM 主动连接设备的 TCP-PASSIVE 点播，在当前 ZLM 不支持时返回可诊断的业务失败，并且不遗留 RTP Server 或 SIP 会话。
3. 录像计划能区分“流尚未注册、可重试”和其他启动录制错误；这些分支有自动化测试。
4. `cloudRecord` E2E 只在已形成录像文件并确认落库后成功，不以接口调用成功或短暂 sleep 作为成功依据。
5. E2E 能区分产品失败与后端/Mock 进程异常退出。
6. 管理员可读取脱敏后的最近 Hook 事件和 Redis 连通性状态；普通用户不能访问。

## 设计

### B1：按实际收流状态决定 RTP 回连

`send_play_invite_and_wait_media` 成功代表 ZLM 已收到媒体。普通直播路径在这个时点继续根据 200 OK SDP 的 `m=` 端口调用 `connectRtpServer`，既不会改善已成功的收流，也会在 UDP/不兼容模式下产生“仅支持 tcp 主动模式”的错误。

普通路径改为：

1. 保留 SDP 媒体端口解析，用于诊断；
2. 若它与 ZLM 分配端口不同，记录 `info` 级结构化日志，包含设备、通道、SDP 端口、ZLM 端口与“media_already_ready”原因；
3. 不调用 `connect_rtp_server`，直接返回既有播放地址。

TCP-PASSIVE 路径仍须在获得设备 SDP 后由 ZLM 向设备端口主动建立连接。该路径失败时：关闭该 `stream_id` 的 RTP Server、按 `StreamType::Play` 结束对应 SIP 会话，并返回包含传输模式和 ZLM 错误摘要的业务错误。不得把失败伪装成播放成功，也不得重新分配 RTP 端口或自动篡改设备传输模式。

这一区分不建立“按设备学习/缓存端口行为”的状态；单设备画像和自动模式切换会引入持久化策略与失效规则，超出本轮所需的最小修复。

### B2 与 B5：录像从“请求成功”改为“产物成立”

保留 `RecordPlanScheduler::start_record_with_retry` 的 25 次、800ms 间隔有界重试：仅 `can not find the stream` 或等价“流未出现”错误可重试；其他错误立即失败；“already recording”仍为幂等成功。

补足两个层面的证据：

- 调度器单元测试模拟：首次流不存在后成功、连续不存在直至预算耗尽、已经录制三种结果；断言不会重试不可恢复错误。
- `cloudRecord` E2E 使用事件驱动的确定性流程：创建计划并等待流出现/开始录制 → 保持至少一个可配置的最短录制窗口 → 明确停止录制或撤销计划 → 在总超时内轮询 `gb_cloud_record` 对应记录，必要时以 ZLM 录像目录文件作为同等证据。失败报告必须包括计划、流、Hook 与文件状态。

测试不得依赖“建完计划立刻删除”来触发 MP4 回调；ZLM 仅在关闭有效 MP4 后触发 `on_record_mp4` 是该测试的必要前置。

### B3 与 B4：将测试环境故障显式化

Mock 保持实现 `connection_lost` 和 `error_received`。测试编排层负责：

1. 分别启动后端、ZLM（如由测试管理）和 SIP Mock；
2. 记录每个进程 PID、日志路径和退出码；
3. 在运行用例前检查 HTTP health/ready 与 Mock 可达性；
4. 用例运行中若任一依赖进程退出，立即以“测试环境失效”失败，并附带相应日志；
5. 清理阶段只终止本次启动记录中的 PID/进程组，不能使用会波及同组长驻服务的通用 job 终止方式。

这使 B3 的协议回调问题与 B4 的执行环境问题分别可见，避免被误判成录像或 RTP 产品缺陷。

### A2：受保护、脱敏的诊断端点

增加两个仅管理员可访问的只读端点：

- `GET /api/test/hook/list`：返回各 ZLM Hook 类型的最近一次接收时间与经脱敏/截断后的载荷摘要；
- `GET /api/test/redis`：返回 StateStore/Redis 是否配置、连通性、延迟或失败摘要，绝不返回 URI 密码、token、完整连接串或原始异常链。

端点统一调用 `authz::require_admin`。Hook 数据保存为有界内存快照，按事件类型只保留最新一项并限制载荷大小；诊断接口故障返回正常的 `ApiResult` 业务错误，不泄露内部配置。

## 文件边界

- `src/handlers/play.rs`：B1 的普通点播与 TCP-PASSIVE 分支、资源清理和日志。
- `src/sip/server.rs` 或现有测试模块：播放会话与媒体就绪行为的回归测试。
- `src/scheduler/record_plan.rs`：B2 重试逻辑及其单元测试。
- `e2e/tests/cloudRecord.spec.ts` 与 E2E 共享 fixture/helper：B5 录像产物断言、B3/B4 运行态检查。
- `mock/tools/sip-device/sip_device_mock.py` 及其测试位置：Mock 传输关闭/错误不退出的验证。
- `src/zlm/*` hook 接收位置、`src/handlers/*`、`src/openapi/*`、`src/router.rs`：A2 有界 Hook 快照和受保护诊断路由。
- `docs/OPEN_ISSUES.md`、`docs/STATUS.md`：以实际提交和验证结果更新 B2–B5/A2 状态；A1 和 A3 的范围决定保持明确。

## 错误处理与安全约束

- 所有新 HTTP 响应使用 `ApiResult<T>`；管理员判定仅使用 `authz::require_admin`。
- 不把原始 SDP、设备密码、Redis URL、Authorization 或 Hook 完整载荷直接返回给客户端。
- B1 清理必须按业务类型定位 SIP 会话，使用现有精确会话 API，不得恢复已修复的粗粒度 BYE 查找。
- 重试必须有固定次数和固定/可配置间隔，不得无限循环。

## 验证策略

- Rust：针对 B1 分支和 B2 重试分支运行定向测试，最后运行 `cargo test --lib`；在可绑定回环端口的环境中确认完整结果。
- 前端/E2E：运行 `cloudRecord` spec 与新增的测试环境健康检查；成功证据包含最终录像记录或文件，而不只是 HTTP 200。
- A2：验证未登录与普通用户为 401/403，管理员返回脱敏状态；Redis 不可用时响应受控错误且不泄露连接信息。
- 质量门禁：`cargo fmt --check`、`git diff --check`，并按改动范围运行 TypeScript 检查/构建。

## 实施顺序

1. B1 播放路径与会话清理回归测试；
2. B2 调度器测试和 B5 云录像 E2E 的确定性闭环；
3. B3/B4 进程健康与清理编排；
4. A2 受保护诊断端点；
5. 文档状态回写和完整回归。

各任务独立提交；A2 可在 B1–B5 稳定后单独发布。
