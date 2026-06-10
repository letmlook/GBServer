# GBServer 综合实现计划

> 生成时间: 2026-06-10
> 基线: `d286d70`（最新提交，编译通过 / 82 warnings）
> 状态: 规划阶段，无任务开始
> 上游文档: `docs/WVP_PARITY_GAP_ANALYSIS.md`、`docs/parity/wvp-phase-0-parity-audit.md`、`docs/superpowers/plans/2026-06-01-dev-plan.md`

## 目标

将 Rust GBServer 后端从「能跑 / 主要功能可用」推进到「WVP-Pro Java 后端可替换」。范围是 GB28181 SIP 信令、ZLM 媒体面、平台级联、JT1078 车载部标、Redis 状态存储、跨节点 RPC、运维面。

完成定义（DoD）：
- `cargo check` 与 `cargo test --lib` 全部通过
- `tests/integration/sip/device_simulator.rs` 端到端模拟测试纳入 CI
- `docs/parity/wvp-phase-0-parity-audit.md` 中 **Missing = 0**、**Method mismatch = 0**
- `/api/sy/camera/*`、前端 2 个缺失页面补齐
- StateStore RedisBackend 真实现，多节点负载生效
- 操作手册覆盖启动 / 升级 / 灾备

## 进度索引

| 阶段 | 主题 | 任务数 | 进度 |
|---|---|---:|---|
| A | P0：SIP 协议链路闭合 | 18 | 14/18 |
| B | P0：平台级联接入 | 11 | 0/11 |
| C | P1：业务深度（StateStore / 多节点 / 重试） | 13 | 0/13 |
| D | P1：WVP 路由补齐（106 条） | 17 | 0/17 |
| E | P2：运维与质量 | 14 | 0/14 |
| F | P2：兼容性与契约测试 | 8 | 0/8 |
| **合计** | | **81** | **14/81** |

更新规则：每完成一项把 `- [ ]` 改为 `- [x]`，并在"进度"列同步百分比。完成阶段时把阶段标题前缀从 `🟦` 改 `✅`。

## 当前快照

- 路由：290 条挂载（`src/router.rs`）
- SIP 公开方法：25+（`src/sip/server.rs`）
- ZLM Hook：11 种（`src/zlm/hook.rs`）
- 编译：dev profile 1m02s / 0 errors / 82 warnings（其中 36 处 snake_case 命名）

## 阶段 A — P0：SIP 协议链路闭合

A2 已完成，剩余 A3 / A4 / A5 / A6 继续。

依赖：dev-plan Phase 1.3 收口 → A1 → A2 → A3 → A4。
可验证产物：模拟器回放 200 OK 完整链 + `pending_request` 状态机单测。

### A1. ResponseRouter 接入 SipServer 主路径
- [x] `SipServer` 创建 `pending_request_manager: Arc<PendingRequestManager>` 字段
- [x] `handle_message()` 内识别 DeviceInfo / DeviceStatus / RecordInfo / Alarm / ConfigDownload 响应 → 调用 `route_message_response()`
- [x] `handle_response()` 内对 200 OK BYE/CANCEL → `route_response()` 走 `pending_request_manager`
- [x] 启动 `pending_cleanup_loop()` 后台任务（每 10s 清超时请求）
- [x] 给 `pending_request.rs` 加 `ResponseRouter` 单元测试（11 个 case：命中 / 4xx / 未注册 / 空 call_id / 6 类 CmdType / accumulate / BYE-INVITE-CANCEL）

**A1 旁路修复（让 `cargo test --lib` 达到 100/100）**：
- [x] `PendingRequestManager::with_timeout` 是 builder 模式，测试改用 `.new().with_timeout(...)` 风格
- [x] `JtCommandWaiter::with_timeout` 同上
- [x] `XmlParser::get_cmd_type` 嵌套 `<Response>` 时拿不到 CmdType，改用独立 `extract_cmd_type` 字符串扫描
- [x] `XmlParser::count_record_items` / `extract_record_items` 兼容 `<RecordItem>` / `<RecordItem ` / `<Item>` / `<Item ` 四种形式
- [x] `JtCommandWaiter::cleanup_expired` DashMap 死锁（持读锁时调 remove）—— 先收集 expired keys 再统一 remove
- [x] `JtCommandWaiter::test_register_and_complete` 未使用 `key` 绑定 → 改 `_key`
- [x] `MediaWaiterManager::resolve_by_stream` 死锁（同 DashMap）—— 先 clone `call_id` 再调 resolve
- [x] `MediaWaiter` 增加 `waiter_key: String` 字段 + `with_app()` builder，让 `cleanup_expired` 复用同一 key 与 `active_keys` 对齐
- [x] `MediaWaiter` 之前是空壳，测试因 key 不匹配残留；现在 `register` 调用 `with_app(app)` 写入 key
- [x] `CatalogSyncSession::add_packet` GB28181 `<Num>` 是包序号不是 item count；改为 `received_num += 1`，测试期望值 1/2/3
- [x] `CascadeSession::needs_refresh` 把 `remaining > 0` 改为 `remaining <= 60`，覆盖已过期情况
- [x] `cascade_service.rs::test_cascade_session_lifecycle` / `test_cascade_needs_refresh` 改 `#[tokio::test]`（sqlx Pool 需要 tokio 上下文）
- [x] `cascade_forward.rs::test_forward_catalog` 补 `.await` + `#[tokio::test]`
- [x] `cascade_service.rs::dummy_pool()` 改 cfg 分发 postgres / mysql，删除硬编码 mysql

### A2. PlaybackSession 真实化
- [x] `playback_pause` / `playback_speed` / `playback_seek` 调用 `send_playback_control`（GB28181 PlayBackCtrl XML）
- [x] PlaybackInviteSessionManager 状态机与 SipServer 事件打通（既有 `playback_session.rs`，handler 通过 `playback_manager.{pause,resume,update_speed,update_current_time}` 维护）
- [x] 回放停止走 `send_session_bye`（不再用 talk BYE fallback）—— 既有 `playback_stop` 已直接调
- [ ] MediaWaiter 接 9102 流 → 触发 ZLM `addStreamProxy`（A2 第 4 项留到 A3/A4 配合做）
- [x] 单元测试：build_playback_control_xml 4 个 case（Pause / Resume / Seek / Scale）XML 拼装正确

**A2 额外修复**：
- [x] 新增 `pub enum PlaybackControlCmd { Play, Pause, Resume, Stop, Seek, Scale }` + `pub(crate) fn build_playback_control_xml` 纯函数
- [x] `src/sip/mod.rs` 增加 `pub use server::PlaybackControlCmd`
- [x] 4 个 handler（pause/resume/speed/seek）改用 `send_playback_control`，XML 全部符合 GB28181
- [x] `stream_id` 解析改用 `parse_playback_target` 辅助函数，channel_id 不再被丢弃

### A3. GB28181 RecordInfo 多包查询
- [x] `playback.rs::gb_record_query` 走 `send_record_info_query` 而非 mock（既有）
- [x] SipServer 收到 RecordInfo 响应 → pending_request 完成 → 解析 Item 列表
- [x] 解析结果落 `wvp_cloud_record`（若 SumNum=1）或返回给调用方（`parse_record_info_items` 纯函数 + accumulator 走 `<Response>` 计数；后台任务收到后做 DB 落库是后续 D 阶段任务）
- [x] 集成测试：模拟器返回 SumNum=3 → 接口返回合并列表（`accumulate_record_info_collects_all_packets` + `parse_record_info_response_merges_multi_packet`）

**A3 额外修复**：
- [x] `send_record_info_query` 注册 PendingRequest（cmd_type=RecordInfo, 15s 超时），让 A1 路由能把响应 complete 到正确 entry
- [x] `ResponseRouter::accumulate_record_info` 改语义：GB28181 SumNum/Num 是包序号不是 item 数；改成调用方维护 `&mut i32 packet_count`，每包 +1
- [x] 新增 `parse_record_info_items(xml) -> Vec<RecordInfoItem>` 纯函数 + `RecordInfoItem` 结构体
- [x] `extract_tag_text` 字符串扫描工具函数，避开 `XmlParser::parse_fields` 嵌套 bug
- [x] 3 个新单测：单包解析 / 多包 5 item 合并 / 空 RecordList 边界

### A4. GB28181 录像下载 INVITE 真实化
- [ ] `playback.rs::gb_record_download_start` 走 Subject = `Download` 流程
- [ ] SSRC = 2 + device_id 前 9 位（参考 `send_download_invite` 已有签名）
- [ ] 9102 端口 → ZLM `addStreamProxy` → MP4 落盘
- [ ] `download/progress` 真实查询 ZLM record 状态
- [ ] `download/stop` BYE + 关 ZLM 流

### A5. 设备/通道统计 + 告警订阅
- [ ] `/api/device/query/statistics/register` 真实聚合 `wvp_device.on_line`
- [ ] `/api/device/query/statistics/keepalive` 真实聚合 `wvp_device.keepalive_time`
- [ ] `send_alarm_subscribe` 公开方法 + `/api/device/query/subscribe/alarm` 路由
- [ ] SipServer 报警 NOTIFY → `db::alarm::insert_alarm` 入库
- [ ] WS 广播 `alarm_event` 消息

### A6. JT1078 实时/回放真实化
- [ ] `jt1078::live_start` 调 `Jt1078Manager::send_live_video`（9101）后再开 ZLM RTP
- [ ] `jt1078::live_stop` 调 `send_live_video_control(stop)` + 关 ZLM RTP
- [ ] `jt1078::playback_start` 调 `send_playback_stream`（9102）+ MP4
- [ ] `jt1078::playback_stop` / `playback_control` / `playback_download_url` 真实化
- [ ] 单元测试：模拟 JT1078 终端应答 9101/9102 流

## 阶段 B — P0：平台级联接入

依赖：A1 完成。链式：B1 → B2 → B3 → B4。
可验证产物：mock 上级平台能注册 + 查询目录 + 点播 + BYE 全流程。

### B1. CascadeRegistrar 启动到 lib.rs
- [ ] `CascadeRegistrar::load_platforms_from_db` 在 `lib.rs::run()` 启动时调用
- [ ] `run_registration_loop` 启动为后台 task
- [ ] `enable=true` 的平台自动注册；`enable=false` 的自动 UNREGISTER
- [ ] 401 触发 digest 重试
- [ ] 单元测试：3 个平台（Active / WaitingAuth / Offline）的状态转换

### B2. 上级方向 MESSAGE 路由
- [ ] SipServer 收到平台方向的 Catalog/Info/Status 查询 → 查 DB → 回复
- [ ] SipServer 收到平台方向 NOTIFY → 走 pending_request
- [ ] 上级设备列表推送 → `wvp_platform_channel` 落库
- [ ] 集成测试：模拟器作为上级能查到本级目录

### B3. 上级点播 → 设备 INVITE → SendRtp
- [ ] `cascade_forward.rs` 状态机接到 SipServer INVITE 入口
- [ ] 设备 INVITE 200 OK → 拿 SSRC/port → ZLM `startSendRtp` 指向上级 IP:port
- [ ] 收到上级 BYE/CANCEL → 停 SendRtp + 设备 BYE
- [ ] 单元测试：完整 INVITE / 200 OK / ACK / BYE 流程
- [ ] 集成测试：模拟器点播能看到 SSRC/RTP 转发

### B4. 平台级联缺失路由补齐
- [ ] `GET /api/platform/info/:id`
- [ ] `POST /api/role/add`
- [ ] `DELETE /api/role/delete`
- [ ] `GET /api/proxy/one`
- [ ] `GET /api/push/forceClose`
- [ ] `GET /api/region/one`
- [ ] `GET /api/region/page/list`
- [ ] `GET /api/region/sync`

## 阶段 C — P1：业务深度

依赖：A1、B1 完成。链式：C1 → C2 → C3 → C4 → C5 → C6。
可验证产物：双 ZLM 节点 + Redis 跑通负载均衡；前端 2 个缺失页面挂上。

### C1. StateStore RedisBackend 真实现
- [ ] 替换 `state_store.rs:238` 处 `RedisBackend` 的所有 no-op 方法
- [ ] 实现 `device_online_{set,get,all}` 用 `wvp:device:online:{id}` 带 TTL
- [ ] 实现 `stream_{set,get,del,all}` 用 `wvp:stream:{id}` JSON 编码
- [ ] 实现 `invite_{set,get,del}` 用 `wvp:invite:{id}` JSON 编码
- [ ] 实现 `position_{set,get}` 用 `wvp:position:{id}`
- [ ] 实现 `media_server_{set,get,all,select_least_loaded}` 用 `wvp:ms:streams:{id}` 计数 + ZSET 排序
- [ ] 实现 `cascade_sendrtp_{set,get,del}` 用 `wvp:sendrtp:{id}`
- [ ] `StateStore::redis(url)` 在 `lib.rs::run()` 中创建并注入 `AppState`
- [ ] 单元测试：所有方法 Redis mock 测试（用 `testcontainers` 起 redis）

### C2. 多节点 ZLM 真正生效
- [ ] `get_zlm_client(None|"auto")` → `StateStore::select_least_loaded_server()` → 缓存 5s
- [ ] `play_start` 走负载均衡而非硬编码第一个节点
- [ ] `playback_start` 同上
- [ ] `send_play_invite` 同上
- [ ] 集成测试：起 2 个 mock ZLM，10 次连续播放至少 4/6 分配到节点 2

### C3. CascadeRegistrar 自动重试 + 离线恢复
- [ ] 401 鉴权 → 重新 digest → 重试
- [ ] Keepalive 超时（> 3 次） → 转 Offline → 周期重试（30s）
- [ ] 平台 disable 立即 UNREGISTER
- [ ] 单元测试：状态机 5 种转换

### C4. `/api/sy/camera/*` 9 条海康/宇视定制
- [ ] `GET /api/sy/camera/list` → `wvp_device` + `wvp_device_channel` 联查
- [ ] `GET /api/sy/camera/list-with-child` → 含子通道
- [ ] `GET /api/sy/camera/list-for-mobile` → 移动端精简字段
- [ ] `GET /api/sy/camera/cont-with-child` → contract 版本
- [ ] `GET /api/sy/camera/list/box`、`list/circle`、`list/polygon`、`list/address`、`list/ids`
- [ ] `GET /api/sy/camera/control/play`、`control/stop`、`control/ptz`
- [ ] `GET /api/sy/camera/meeting/list`

### C5. 云录像 collect / zip / list-url（5 条）
- [ ] `GET /api/cloud/record/collect/add` → `wvp_cloud_record_collect` 表
- [ ] `GET /api/cloud/record/collect/delete`
- [ ] `GET /api/cloud/record/download/zip` → ZLM 文件归档
- [ ] `GET /api/cloud/record/list-url` → DB 查询返回 URL 列表
- [ ] `GET /api/cloud/record/zip` → 同 download/zip

### C6. 前端 2 个缺失页面
- [ ] `web/src/views/alarm/index.vue` + 路由 `/alarm`
- [ ] `web/src/views/play/share.vue` + 路由 `/play/share`
- [ ] 后端 `/api/play/share` 鉴权 token 端点

## 阶段 D — P1：WVP 路由补齐（106 条）

依赖：B4、C1。链式：D1 → D2 → D3 → D4 → D5。
可验证产物：`docs/parity/wvp-phase-0-parity-audit.md` 重跑后 Missing = 0。

### D1. JT1078 区域/线路/控制（≈ 30 条）
- [ ] 区域 circle：`add`、`edit`、`update`、`delete`、`query`
- [ ] 区域 polygon：`set`、`delete`、`query`
- [ ] 区域 rectangle：`add`、`edit`、`update`、`delete`、`query`
- [ ] 线路 route：`set`、`query`、`delete`
- [ ] `live/continue`、`live/pause`、`live/switch`
- [ ] `record/start`、`record/stop`
- [ ] `snap`
- [ ] `temp-position-tracking`
- [ ] `confirmation-alarm-message`
- [ ] `playback/download`
- [ ] `media/upload/one/delete`
- [ ] `terminal/channel/delete`、`terminal/channel/one`

### D2. RTP/PS 控制（≈ 10 条）
- [ ] `POST /api/rtp/receive/open` → ZLM `openRtpServer`
- [ ] `POST /api/rtp/receive/close/*path` → ZLM `closeRtpServer`
- [ ] `POST /api/rtp/send/start` → ZLM `startSendRtp`
- [ ] `POST /api/rtp/send/stop/*path` → ZLM `stopSendRtp`
- [ ] `POST /api/ps/receive/open`、`close`
- [ ] `POST /api/ps/send/start`、`stop`
- [ ] `GET /api/ps/getTestPort`

### D3. 报警清理（3 条）
- [ ] `DELETE /api/alarm/clear`（清全部）
- [ ] `DELETE /api/alarm/delete`（按 id 批量）
- [ ] `GET /api/alarm/snap/:param`（截图）

### D4. 媒体 tile / CommonChannel（3 条）
- [ ] `GET /api/common/channel/map/tile/:x/:y/:z`
- [ ] `GET /api/common/channel/map/thin/tile/:x/:y/:z`
- [ ] `GET /api/front-end/common/:cmd/:ch`

### D5. 媒体 / 配置（4 条）
- [ ] `GET /api/media/getPlayUrl`
- [ ] `GET /api/media/stream_info_by_app_and_stream`
- [ ] `GET /api/server/config` → 当前脱敏配置
- [ ] `GET /api/server/shutdown` → 真触发进程退出
- [ ] `GET /api/server/version` → 真实 git 版本

## 阶段 E — P2：运维与质量

依赖：C1、C2 完成。链式：E1 → E2 → E3 → E4 → E5。
可验证产物：CI 跑全集成测试 + 警告清零。

### E1. StateStore 接入 cascade_forward + scheduler + 多节点
- [ ] `cascade_forward.rs` 用 StateStore 跟踪 SendRtp 状态
- [ ] `scheduler/record_plan.rs` 用 StateStore 跟踪 active 录像
- [ ] `playback_session` / `invite_session` 用 StateStore 跟踪会话
- [ ] 删除 `src/cache.rs` 中重复的 5 个函数（与 StateStore 合并）

### E2. 跨节点 RPC（src/rpc.rs）
- [ ] 用 reqwest 实现 JSON-RPC over HTTP
- [ ] 4 个方法：`forward_invite`、`stop_session`、`get_session_state`、`broadcast_event`
- [ ] 单元测试：mock 远端节点返回

### E3. 完整集成测试
- [ ] `tests/integration/sip/device_simulator_test.rs` 端到端跑通
- [ ] 加进 `scripts/run-backend-tests.sh`
- [ ] GitHub Actions workflow 每日 + 每次 PR 跑
- [ ] 跑通后输出 JSON 测试报告

### E4. 警告清理
- [ ] 修 36 处 snake_case 命名（`parentDeviceId` → `parent_device_id`）
- [ ] 修 `drop(&socket)` 1 处
- [ ] 修 unused variable / unused import
- [ ] `cargo clippy -- -D warnings` 通过

### E5. 配置 / 部署完善
- [ ] `config/application.yaml` 补 `MapConfig`、`redis.pool_size`、多 ZLM 配置示例
- [ ] `docker-compose.yml` 加 mock 上级 SIP 平台（go sip 或者 opensips）
- [ ] `README.md` 加「升级到 v2」的章节
- [ ] `docs/OPERATIONS.md` 新建：启动 / 升级 / 灾备 / 监控

## 阶段 F — P2：兼容性与契约测试

依赖：D 阶段完成。链式：F1 → F2 → F3 → F4。
可验证产物：parity audit 自动对账 + 双数据库 CI。

### F1. WVP 前端契约测试
- [ ] 写 `scripts/parity-audit/contract-test.js`
- [ ] 对每个 Rust 路由打 fixture + 期望字段
- [ ] 与 `docs/parity/wvp-phase-0-parity-audit.md` 对账
- [ ] 输出 HTML 报告

### F2. MySQL/PostgreSQL 双库实测
- [ ] GitHub Actions 跑两次：MySQL 8.0 + PostgreSQL 16
- [ ] 修 dialect 差异（`ON CONFLICT` vs `ON DUPLICATE KEY`、`self` 列名）
- [ ] 验证 schema 自动初始化在两库都通过

### F3. 安全加固
- [ ] JWT secret 强制环境变量，启动时缺省即退出
- [ ] API Key 接入 `auth_middleware`（`X-API-Key` 头）
- [ ] 敏感日志脱敏（密码、secret、token）
- [ ] rate-limit 中间件（`tower-governor`）

### F4. 大文件收尾
- [ ] `stub.rs` 55 个函数按真实程度拆分到各模块
- [ ] `device_stub.rs` 同样拆分
- [ ] 删除 `.rs.bak` 备份文件
- [ ] 文件名不再带 `stub` 后缀

## 风险与回滚

| 风险 | 缓解 | 回滚 |
|---|---|---|
| A 阶段改动 SIP 主路径，可能引入回归 | 严格走模拟器测试 | git revert 到 `d286d70` |
| C1 RedisBackend 实现涉及 state_store.rs 大改 | 先在 InMemoryBackend 写相同 trait 测试 | feature flag `state-store-redis` 默认关 |
| C4 9 条 sy/camera 路由可能和现有 device API 冲突 | 路由独立，handler 复用 device.rs 函数 | 删除路由 |
| D 阶段大量路由新增可能引入字段兼容问题 | 走 F1 契约测试 | 路由可挂可拆 |
| E 改动 `src/cache.rs` 可能影响现有调用 | 先加 `state-store-redis` feature flag | git revert |
| F2 双库 dialect 差异可能阻塞 CI | 修一个测一个 | 单库跑 |

## 配套基础设施

- 进度文档：本文件
- 任务跟踪：使用 `update_plan` 工具在每个会话中显示当前在哪个阶段
- 目标追踪：使用 `create_goal` / `update_goal` 跟踪总体进度
- 集成脚本：`scripts/run-backend-tests.sh` 每完成一段必跑
- 对账脚本：`node scripts/parity-audit/extract-wvp-parity.js` 每完成 D 阶段必跑

## 完成判定

- 所有 81 个 checkbox 全部 `[x]`
- `docs/parity/wvp-phase-0-parity-audit.md` 重跑后 **Missing = 0**、**Method mismatch = 0**
- GitHub Actions 主线绿（含双库 + 集成测试）
- 操作文档发布到 `docs/OPERATIONS.md`
- 在仓库根写一个 `RELEASE_NOTES_v2.md` 总结变更
