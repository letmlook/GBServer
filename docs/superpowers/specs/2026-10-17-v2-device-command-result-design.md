# 2.0.0 设备命令结果设计

**状态：** 待审阅；对应路线图 M2 / B14。

## 目标和边界

区分数据库配置保存、SIP MESSAGE 发送、设备业务应答和未确认结果。现有 `send_device_control` 与 `send_device_transport` 只报告发送结果，`device_transport` 又先保存数据库；任何 `code=0`、SIP 200 或 `sipSent=true` 都不能称为设备执行成功。本设计先覆盖可回显 SN 的 `DeviceControl`，同时修正 `ConfigDownload` 查询收到 `DeviceConfig` 应答时的关联。其他命令不自动获得“已确认”语义。

## 协议与关联

- 发送前生成平台进程内不重复的正整数 SN，登记 `(device_id, CmdType, SN)` 的 pending，再发送 XML；保留现有不等待的发送 API 供批量和持续 PTZ 使用。新增可确认发送入口显式返回 SN 与接收器，不让 `send_device_control` 隐式生成无法追踪的秒级时间戳。发送失败立即取消该 pending。
- 设备 `<Response>` 的 `CmdType=DeviceControl`、`SN`、`DeviceID`、`Result` 必须解析；`Result=OK` 为确认成功，非 OK 为设备拒绝并保留安全的结果码。`DeviceConfig` 和 `ConfigDownload` 均可匹配已登记的 `PendingCmdType::DeviceConfig`，但不能匹配 `DeviceControl`。
- 匹配必须同时核对设备、命令类型、SN；同一设备的迟到、重复、错误类型或未知 SN 应答只记录脱敏日志，不完成其他请求。若缺失必要字段，保持未确认。Call-ID 可用于优先定位，但不得绕过上述核对。设备应答中的 `DeviceID` 可能是通道 ID，需以 SIP 来源对应的平台设备 ID 作为设备关联主键，并校验应答 target 是本次设备或通道。
- pending 只在进程内有效，重启后旧 SN 不可与新请求碰撞；使用随机进程前缀加单调计数或等效方案。单次等待预算 5 秒，超时取消 pending；不做跨重启持久化，不把未知结果重试成重复执行。

## HTTP 与页面语义

兼容现有响应字段。传输模式接口继续先保存平台配置，`updated` 与 `streamMode` 只表示 DB 状态，`sipSent` 只表示 SIP 发送；新增 `deviceResult` 对象：`status` 为 `not_sent | sent | confirmed | rejected | unconfirmed`，可附 `sn`、`resultCode`。`sipError` 保持原字段。离线设备是 `not_sent`，不能显示为设备执行成功。

离散操作可选择 `waitResult=true` 等待至多 5 秒并返回最终状态；默认调用保持即时返回 `sent`，避免 PTZ 连续按键阻塞。等待超时为 `unconfirmed`，不是 `rejected`；设备返回失败为 `rejected`。配置查询继续使用原等待式接口，仅修正路由匹配。前端只在得到 `confirmed` 时写“设备已执行”，`sent` 显示“已发送，待确认”，`unconfirmed` 显示“设备结果未确认”。

## 验收

单测覆盖 OK、非 OK、缺字段、错误 CmdType、重复、迟到、超时、SN 并发唯一和发送失败取消；接口测试覆盖 DB 更新成功但设备离线、发送失败及权限。Mock 注入完整 DeviceControl Response；真机至少验证一个离散命令的 SN 与 Result。只有真实设备回执才能宣称“设备已执行”；真机不回报时记为未确认并记录设备型号。

## 代码边界

`src/sip/gb28181/pending_request.rs` 负责类型映射和严格匹配；`src/sip/server.rs` 负责 SN、发送和 Response 路由；`src/handlers/device_control.rs`、`src/handlers/device_stub.rs` 负责 HTTP 状态契约；`web/src/` 对应操作页面负责文案与状态展示。沿用 `ApiResult<T>`、现有鉴权及公开路径，不新增无鉴权入口。
