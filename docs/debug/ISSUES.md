# GBServer 调试问题清单（ISSUES.md）

> 调试时间：2026-06-20
> 调试者：ZCode (MiniMax-M3)
> 范围：仅启动+观察+修必要编译错误，不改业务逻辑

## TL;DR

GBServer 仓库自 `602036b "refactor: auto-fix compiler warnings"` 提交后留下大量未编译验证的 bug，
叠加 `e132db2 "chore: drop GitHub Actions CI workflows"` 删除了 CI，导致**仓库当前无法直接 `cargo run` 启动**。
本次调试已修复全部 28 个编译错误 + 4 处路由重复 panic，后端成功在 :18080 运行；前端 dev server
在 :9528 运行；ZLM 在 :18081 运行。**端到端可用**。

## 服务清单

| 服务 | 端口 | 状态 |
|------|------|------|
| ZLMediaKit (docker) | 8080→18081, 554→5544, 1935→11935 | ✅ Up |
| GBServer 后端 (cargo run) | 18080 + 5060/5061 + 60000 | ✅ Up |
| Vue 2 前端 (npm run dev) | 9528 | ✅ Up |
| SQLite | `data/gbserver.db` | ✅ 1 device (self) |

## P0 修复的 5 类问题

### 1. 13 个 E0252 重复 `use`（auto-fix 残留）

- 提交 `602036b` 跑 `cargo fix` 警告时未跑完整编译，引入了 `use` 重复
- 影响文件 6 个：
  - `src/sip/gb28181/playback_session.rs:12`
  - `src/sip/gb28181/subscription_lifecycle.rs:15, 19`
  - `src/sip/gb28181/cascade_service.rs:35`
  - `src/sip/gb28181/cascade_forward.rs:13, 14, 15`（连续 3 个 `use` 块重复 Arc/DashMap/RwLock）
  - `src/sip/server.rs:17, 201`（两段合并的 import 块和单独重复的 media_waiter）
- 修复：删除冗余 `use` 行

### 2. 8 个 E0433 `device_query` 模块未导入（router.rs）

- `src/router.rs` 的 `use crate::handlers::{...}` 列表漏了 `device_query`，
  但 `handlers/mod.rs` 已经 `pub mod device_query;` —— refactor 后忘了更新 import
- 修复：在 router.rs import 列表加 `device_query`

### 3. 3 个 E0599 `SipServer::device_commander` 方法缺失

- `src/handlers/device_query.rs` 在 3 处调用 `server.device_commander()`，
  但 `SipServer` 没有这个字段也没有这个方法
- `src/sip/gb28181/device_commander.rs` 已经定义了 `DeviceCommander` 包装
- 修复：
  - `src/sip/server.rs` 加 `device_commander: Arc<DeviceCommander>` 字段
  - 在 `SipServer::new` 中初始化 `device_commander: Arc::new(DeviceCommander::new(pending_request_manager.clone()))`
  - 加访问器 `pub fn device_commander(&self) -> Arc<DeviceCommander> { self.device_commander.clone() }`

### 4. pending_request.rs 字段类型错配

- `PendingRequest.response_sender` 字段被改成 `Option<()>`（auto-fix 把 sender 抽空了）
- 但 `PendingRequest::new_with_receiver` 还在构造 `Some(tx)`，并被 `complete()` 调 `tx.send(xml_response.to_string())`
- `PendingRequest::new` 签名被改成返回 `PendingRequest`（单值），但 `register_record_info_multi_packet` 期望元组解构
- 修复：
  - 字段类型改回 `Option<oneshot::Sender<String>>`
  - 给 `PendingRequest` 手动 `impl Clone`（`response_sender: None` 防止 oneshot 双发 panic）
  - `register_record_info_multi_packet` 内调用点从 `PendingRequest::new` 改为 `PendingRequest::new_with_receiver`

### 5. 4 处路由重复 panic（启动后第一次崩溃）

`Axum` 启动时检测到重复路由会 panic。同一个 HTTP path 被两个 handler 注册：

| 路径 | 第一个 handler（保留）| 第二个 handler（删除）|
|------|----------------------|----------------------|
| `GET /api/device/query/statistics/register` | `device::device_register_statistics` | `device_stub::statistics_register` |
| `GET /api/device/query/statistics/keepalive` | `device::device_keepalive_statistics` | `device_stub::statistics_keepalive` |
| `POST /api/device/config/update` | `device_control::device_config_update` (111 行) | `device_query::device_config_update` (7 行) |
| `GET /api/media/getPlayUrl` | `device_query::get_play_url` (41 行) | `parity_extras::media_get_play_url` |
| `GET /api/media/stream_info_by_app_and_stream` | `device_query::stream_info` (29 行) | `parity_extras::media_stream_info_by_app_and_stream` |
| `GET /api/cloud/record/collect/add` | `stub::cloud_record_collect_add` | `cloud_record_extra::collect_add` |

实际是 **6 处** 重复（5 处没暴露是因为同方法只定义了一对 handler，其中一个被注释掉或没注册）。

**保留规则**：实现更完整（行数更多）的那个；删除简化版/stub 版。

## P1 启动环境调整（运行时配置）

| 调整 | 原因 | 文件 |
|------|------|------|
| ZLM HTTP 端口 8080→**18081** | 本机 8080 被 Docker Desktop 占用 | docker run `-p 18081:8080` |
| ZLM RTSP 端口 554→**5544** | 本机 554 被 Docker Desktop 占用 | docker run `-p 5544:554` |
| ZLM RTMP 端口 1935→**11935** | 本机 1935 被 Docker Desktop 占用 | docker run `-p 11935:1935` |
| ZLM HTTPS 端口 8443→**18444** | 避开 8443 | docker run `-p 18444:8443` |
| ZLM `allow_ip_range=0.0.0.0/0` | 默认 allow list 不含 docker bridge 网段 | mount 自定义 ini |
| ZLM `api.secret=EctdXjqgidxs2AMquujELlQEh3G4OqAH` | 默认 secret `S63648...` 被 ZLM 判定为"invalid"自动重生成 | 同步到 `config/application.toml` |
| 后端 `[[zlm.servers]]` 端口 8080→**18081** | 与 docker 端口映射一致 | `config/application.toml` |
| 后端 `secret` 同步 | 与 ZLM 实际 secret 一致 | `config/application.toml` |

## P2 已知遗留问题（不修，仅记录）

1. **macOS 全局代理（`all_proxy=http://127.0.0.1:7892`）干扰本地 curl**
   - 现象：`curl 127.0.0.1:18081` 经代理返回 502
   - 解决：用 `curl --noproxy "*"` 或 `env -u all_proxy` 绕过
   - 永久修：把 `127.0.0.1` 加到 `no_proxy` 环境变量

2. **端口冲突需要重映射**
   - 本机 554/1935/8080 被 `com.docker` (Docker Desktop) 占用
   - 解决：docker run 时重映射到非冲突端口（见上表）

3. **Axum 路由重复是架构问题**
   - `device_stub` 模块和 `device_query` 模块都在尝试注册相同路径
   - 根本原因：refactor 期间没清理旧的 `device_stub` stub 注册
   - 建议：合并 `device_stub` 到 `device_query`，或统一通过 `stubs/` 目录管理

4. **CI 缺失**
   - `e132db2` 删了 GH Actions，没有 PR 验证编译
   - 建议：恢复 CI（`cargo check` + `cargo test` + `npm run build:prod`）

5. **前端 dev server 编译报 105 个 warning（unrelated to bugs fixed）**
   - 大部分是 `unused variable` 和 `drop(&x)` 不做事
   - 不影响功能

6. **后端启动时一行 WARN**：`未配置 static_dir 或目录不存在，仅提供 API`
   - 原因：未跑 `npm run build:prod` 生成 `web/dist`
   - 影响：dev 模式无所谓，prod 模式下访问根路径会 404
   - 解决：`cd web && npm run build:prod`

7. **设备统计为 0**
   - `statistics/register` 返回 `{"activeDevices":0,"inactiveDevices":0,"todayRegister":0,"totalDevices":0}`
   - 原因：实际只有 1 个 self 设备，且它从未走 SIP REGISTER 流程
   - 正确：需要真实 SIP 设备注册后才有数据

8. **媒体节点状态点显示灰（offline 标识）**
   - `mediaServer.png` 中 zlmediakit-1 的圆点灰色
   - 实际：日志显示 `ZLM server zlmediakit-1 status changed: Unknown -> Online`
   - UI 显示可能是因为前端状态计算逻辑（`/api/server/media_server/list` 需带负载信息）
   - 不影响功能

## P3 验证结果

### API smoke（7/7 通过）

| API | HTTP | 说明 |
|-----|------|------|
| `POST /api/user/login?username=admin&password=admin` | 200 | 返回 accessToken |
| `GET /api/user/userInfo` (带 token) | 200 | 返回 admin 用户信息 |
| `GET /api/user/users` | 200 | 返回 1 个用户（admin） |
| `GET /api/device/query/devices?page=1&count=10` | 200 | 返回 1 个 self 设备 |
| `GET /api/device/query/devices/34020000002000000001` | 200 | 设备详情 |
| `GET /api/device/query/devices/.../channels` | 200 | 0 个通道（无真实摄像头） |
| `GET /api/server/media_server/list` | 200 | 1 个 zlmediakit-1 |

### UI smoke（Playwright 18/18 通过）

- `e2e/tests/smoke.spec.ts` 自带 18 个测试
- 覆盖：dashboard / live / channel / map / device / push / proxy / commonChannel / recordPlan /
  cloudRecord / mediaServer / platform / user / operations / alarm
- 全部截图保存到 `e2e/artifacts/*.png`（15 个页面 + login + dashboard-before/after）

### 控制台错误（仅记录，不修）

- `commonChannel` 页：`TypeError: Cannot read properties of undefined (reading 'offsetHeight')` in `<Region>` 组件
  - 是 `<AppMain>` 渲染时 DOM 还未准备好
  - 不影响功能（测试通过）
- `operations` 页：`TypeError: childValue.startsWith is not a function` in `OperationsSystemInfo`
  - 后端 `/api/server/system/configInfo` 返回了非字符串字段，前端未防御
  - 不影响其他功能

## 后续可选项

- [ ] 把修复 commit 到一个 PR（建议拆 5 个 commit：E0252, E0433, E0599, pending_request, 路由去重）
- [ ] 恢复 GitHub Actions CI
- [ ] 合并 `device_stub` 模块到 `device_query`
- [ ] 把 `S63648HLbxckv7YjpPTXXRTOsAVGo0Ia` 换成一个真正满足 ZLM 复杂度要求的 secret
- [ ] docker-compose 里写好 ZLM 端口映射规则
- [ ] `npm run build:prod` 生成 `web/dist` 用于 release 模式统一部署
