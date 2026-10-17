# 2.0.0 管理员诊断设计

**状态：** 待审阅；对应路线图 M3 / A2。与旧媒体设计的 A2 章节相同目标，本文件固定安全与接口边界。

## 目标与接口

`GET /api/test/hook/list` 返回最近 ZLM Hook 的最小元数据；`GET /api/test/redis` 返回 Redis 与 StateStore 的真实状态。两个接口必须经过 JWT 与 `authz::require_admin`，统一返回 `ApiResult<T>`，并注册受保护 OpenAPI 路由。既有错误框架中普通用户的 `Error403` 可能表现为 HTTP 200 + `code=403`，测试按实际契约分别断言 HTTP 和业务码。

## Hook 快照

只保存能够证明来自已配置 ZLM 节点的事件。`on_play`/`on_publish` 在现有 secret 验证成功后记录；其他分支若没有同等来源验证，须先补统一来源验证或不纳入快照，不能把收到请求当作可信事件。每种事件类型最多保留最近 1 项，类型总数由固定白名单限定；字段只允许 `hookType`、`receivedAt`、`mediaServerId`、`app`、`stream`、`schema`、`success`，字符串截断至 128 字符。禁止保存原始请求、鉴权 secret、设备密码、token、完整 URL、异常链；未知类型不入库。内存丢失可接受，返回 `retention=in_memory_latest_per_type` 明示范围。读取快照不触发 ZLM 控制动作。

## Redis / StateStore

明确区分 `not_configured`、`connected`、`unavailable`；Redis 配置存在时对实际连接执行 1 秒超时的 PING 并给出耗时毫秒数。StateStore 使用 Redis 后端时，内存回退不能误报“Redis 可用”；返回 `backend=redis|memory` 和有限状态枚举。故障只给固定摘要（如 `ping_timeout`），不得返回 URI、密码或原始驱动错误。诊断本身不修改 Redis。

## 验收

测试未登录、普通用户、管理员；带 secret 的 Hook 输入经鉴权后才留白名单摘要，拒绝的 Hook 不留痕；Redis 未配置、正常、断连、超时均有确定状态和无泄密断言。真实运行时关闭 ZLM/Redis 后应能从返回状态与日志定位节点或依赖，但诊断接口仍能响应。

## 代码边界

`src/zlm/hook.rs` 在已验证事件分支写快照，独立小模块保存有界状态；`src/lib.rs::AppState` 注入共享状态，同时更新 `src/test_support.rs` 和 `src/lib.rs` 测试构造器；`src/handlers/` 添加只读 handler；`src/openapi/routes_system.rs` 注册端点。配置文件不新增 secret；不在浏览器直接访问 ZLM 管理 API。
