# WVP 平替进度追踪

> 目标：完全平替 WVP-PRO（Java GB28181 平台）的全部功能。
> 本文档作为持续校对的事实基线：每次推进后更新对应条目并记录证据。

## 当前基线（2026-09-12）

> 本节数字为**实测值**，复现命令见每行「验证方式」。上次基线见文末「历史基线」。

| 维度 | 数值 | 验证方式 |
|------|------|----------|
| 总代码量（src/） | 78,106 行 Rust | `find src -name '*.rs' \| xargs wc -l` |
| 已注册 HTTP 路由 | 386 条唯一 `/api/...` 路径 | `grep -oE '"/api/[^"]*"' src/router.rs \| sort -u \| wc -l` |
| Handler 模块 | 29 个（含 `stub.rs` / `device_stub.rs` 两个兼容 shim） | `grep -c 'pub mod' src/handlers/mod.rs` |
| 后端测试 | **670 通过** / 0 失败（第三十五轮刷新） | `cargo test` |
| 编译状态 | `cargo check` 0 error / **0 warning**；clippy 262；**deprecated 0** | `cargo check` / `cargo clippy --all-targets` |
| 数据库 feature | SQLite（默认）/ PostgreSQL / MySQL **三者均编译通过** | CI `feature-matrix` job |
| CI | ⏸️ 工作流已就绪但**按需暂停自动触发**（见 `.github/workflows/ci.yml`） | — |
| 前端 | `web/` = **Vue 3 + Element Plus + Vite + TS**（17 个业务视图）；`web-legacy-vue2/` 为归档参考 | `ls web/src/views` |
| 前端产物 | `web/dist/` 构建通过（`npm run build` = `vue-tsc --noEmit && vite build`） | — |

### ⚠️ 重要更正：API 挂载 ≠ 功能可用

**本节由 2026-09-11 的深度审计补充，推翻了此前「100% 平替」的乐观结论。**

此前的对照表衡量的是**路由是否挂载**，而把「已挂载」当成了「已实现」。实际审计发现
（详见下方「真实性与安全问题」）：

- **71 个端点完全未鉴权**（`api_public` 未挂任何中间件），含角色增删、云录像下载、JT1078 控制
- **5 个端点返回编造的成功**（回放控制只打日志、zip 打包返回凭空 taskId）
- **2 类运行时必然失败的 DB 查询**（结构体列清单漂移），其中一类导致约 20 个 PTZ 类端点 500
- **16 个测试写了却从未被编译运行**
- **口令哈希降级 + 新建用户无法改密**

结论应表述为：**路由覆盖接近完整，功能真实性存在明确缺口，且缺口是可枚举的**。


### 本轮（2026-09-11）关键结论

- **CI 门禁恢复**：编译 + 全量测试 + 三库 feature + 前端构建为硬门禁；`fmt` / `clippy` 暂列为非门禁（基线未清零）。**注**：应要求已暂停自动触发，改为仅手动 `workflow_dispatch`，见 `.github/workflows/ci.yml`。
- **测试完全自包含**：默认 SQLite feature 下 478 个测试不连接 Redis / PG / MySQL / ZLM，CI 无需 service 容器。
- **前端已完成 Vue 3 迁移**：`web-v3/` 已转正为 `web/`（commit `2acf5a7`），Vue 2 归档至 `web-legacy-vue2/`。本文档此前多处 "web-v3 Phase 2 待迁移" 的描述已过时，本轮一并修正。
- **CI 首次运行即抓到真实缺陷**：`Navbar.vue` 缺 `reactive` 显式 import，依赖被 gitignore 的
  `auto-imports.d.ts` 兜底 → **任何干净 clone 跑 `npm run build` 都会失败**（`dev` 与
  `build:no-check` 正常，故长期潜伏）。已修复，见 commit `0434629`。
- **删除 751 行死代码**：`sip/gb28181/cascade_service.rs` 实为零生产调用的 deprecated 模块，
  删除后 deprecated 告警 66 → 8、clippy 告警 329 → 297。
- **状态源统一到 StateStore**：完成 cache → StateStore 迁移。`zlm/hook.rs` 4 处 legacy
  Redis 写入中 3 处纯冗余、1 处（`on_flow_report`）改为覆盖 StateStore；连带发现整个
  `src/cache.rs` 已零调用方，整体删除 140 行。**deprecated 告警归零**，clippy 297 → 292。
  核查中发现 `on_flow_report` / `handle_webhook` **此前无任何测试覆盖**，已把同步逻辑
  提取为可测函数并补 4 个测试（lib 342 → 346）。

---

## 真实性与安全问题（2026-09-11 深度审计）

> 目标从「路由挂载率」转为「功能是否真实可用」。以下每一条都有对应测试固化。

### 安全（均已修复）

| 问题 | 严重度 | 说明 | 修复 |
|------|--------|------|------|
| **71 个端点未鉴权** | 🔴 高 | `api_public` 未挂任何中间件，实测未带 token 即 200：云录像下载、`role/add`、JT1078 控制、`server/config` 等。紧邻代码注释却写着「已移入 api_protected 需 JWT」 | 全部移入带 audit+auth 的 `api_protected`；仅保留 9 个确需公开的（login/zlm hook/rpc/health/ready/metrics/play share）。新增 27 端点安全回归测试 |
| **`/api/rpc` 完全无鉴权** | 🔴 高 | 集群 RPC 入站无校验、出站不带凭证，任何人可调用 RPC 方法 | 新增 `[rpc].secret`；出站带 `X-RPC-Secret`，入站校验，不匹配 401；多节点未设密钥时启动告警 |
| **口令哈希降级 + 无法改密** | 🔴 高 | `change_password` 用**明文比较**校验旧口令 → 存 Argon2id 的新用户**永远改不了密码**；且改密写回 MD5（降级） | 新增 `verify_password_compat`（Argon2id↔MD5↔明文）；改密存 Argon2id；**登录时机会式升级**旧哈希 |
| **已公开的默认 JWT 密钥可静默通过** | 🟠 中 | `config/application.toml` 里的密钥已在 Git 历史中，但长度合规、不在弱密钥表 → 静默通过 | 加入 `COMMITTED_DEMO_JWT_SECRET` 弱密钥；原有测试甚至拿它当「强随机」样例，已改正 |

### 假实现（已改为真实实现）

| 端点 | 此前行为 | 现行为 |
|------|----------|--------|
| `common_channel/playback/{pause,resume,seek,speed}` | `State(_state)` 故意不收 state，只打日志就返回「成功」 | 解析目标（会话优先）→ 更新本地会话 → 下发 GB28181 `PlayBackCtrl` |
| `cloud_record/download/zip` | 返回凭空拼的 `taskId` + `status:"queued"`，声称可查（无此端点） | 真实打包 ZIP（新增 `src/archive.rs`，stored 方式零依赖），返回真实 URL |
| `common/channel/map[/thin]/tile/:z/:x/:y` | 恒返回 `count:0, items:[]` | 真实 slippy-map 瓦片边界过滤；thin 按 `map_level` 排除已合并点 |
| `front-end/common/:cmd/:ch` | 回显「指令已下发」但**什么都没发** | 映射 15 个指令为真实 DeviceControl XML 并下发；未知指令显式报错 |
| `alarm/snap/:param` | 返回指向**自身**的 URL | 查最近一条关联录像，返回可用的 `/api/cloud/record/download/:id` |

### 运行时必然失败（编译期不可见）

| 问题 | 影响 | 修复 |
|------|------|------|
| `DeviceChannel` 查询只有 12/32 列 | 所有走 `lookup_channel_and_send` 的 PTZ/预置位/雨刷/光圈/巡航端点 + `map/list` **全部 500** | 统一用 `DEVICE_CHANNEL_SELECT_COLUMNS` 常量 |
| `MediaServer` 查询引用 `ws_port`/`wss_port`/`record_transcode` | 这 3 列在 schema 与结构体中**都不存在** → `list_online_servers` 恒失败，**ZLM 节点离线过滤静默失效** | 6 处改用 `SELECT *` |

新增 `src/db/read_smoke.rs`：为 20+ 张表各播种一行并真实解码，专防此类漂移
（空表测不出——无行可解码就不会触发 `ColumnNotFound`）。

### 协议缺口（2026-09-11 第二轮修复）

| 问题 | 说明 | 修复 |
|------|------|------|
| **RFC 3261 §17 事务重传完全未生效** | `TransactionManager` 被构造却从未使用；`process_timers` **只自增重传计数并打日志，从不发送**（它不持有 socket）。GB28181 默认走 UDP，丢包即无补救 | 保存首次发送的**原始字节**（重传须逐字一致，Via branch 不能变）；注入出站通道；`process_timers` 真正发送；响应到达即终止事务。新增 5 个测试（含"是否真的发出去"） |
| **JT1078 5 个端点报「协议原语未实现」** | 实测该说法不成立：`build_take_photo`(0x8801)/`build_media_upload`(0x8803) 与 `send_command_and_wait` 都已存在，真正只缺 0x8202/0x8203/0x9205 | 补齐 3 个原语 + 4 个 `send_*_and_wait`，5 个端点全部真实下发；另加严格时间解析 `try_encode_time_bcd`（原 `encode_time_bcd` 解析失败会**静默用当前时间**，会把错误时间段下发给终端） |

### 数据层缺陷（2026-09-12 第三轮修复）

**核查方法**：正则提取 `src/` 中所有 `FROM/INTO/UPDATE/JOIN <table>` 引用，
与三份 schema 的建表清单求差 —— 一次性找出全部「表不存在」类缺陷。

| 问题 | 影响 | 修复 |
|------|------|------|
| **`gb_platform_catalog` 三库都没有** + `catalog_add/edit` **无 sqlite 分支** + 错误被吞 | 默认部署下这两个端点**完全空转却返回"目录添加成功"** | 三库补表；补 sqlite 分支；改为 `Result` 传播错误 |
| **表名写错 `gb_mobile_position`**（实为 `gb_device_mobile_position`） | JT1078 位置查询 DB 兜底**永远返回空** | 修正表名 |
| **表名写错 `gb_push_stream`**（实为 `gb_stream_push`，4 处） | 推流绑定/解绑国标设备**永远失败** | 修正表名 |
| **PG/MySQL 缺 4 张 JT1078 表** + 该 4 表不在启动补表中 | 旧库升级后对应 CRUD 报 `no such table` | 补 PG/MySQL 表 + 启动幂等补表 |
| **5 个 handler 只有 mysql/postgres 分支、无 sqlite 分支** | 默认部署下静默空转（含 `media_server_save` 的**扩展字段被丢弃**） | 按仓库既有约定扩为 `any(mysql, sqlite)`；写操作传播错误 |
| **`gb_log` 三库都没有** + 无文件 appender + 前后端契约不匹配 | 「系统日志」功能整体不成立（`log_list` 永远返回空） | 建表 + 实现 tracing 采集层 + 按前端契约重写查询 |

### 静默失败与 SDP 缺陷（2026-09-12 第四轮修复）

**核查方法**：脚本化扫描 `src/` 中所有「`let _ = <写库调用>.await`」以及
「写库后仍无条件返回成功」的模式，逐条判断是「可传播」还是「脱离请求上下文」。

| 问题 | 影响 | 修复 |
|------|------|------|
| **`map_thin_save` / `map_thin_draw` 写库错误被吞** | 前端地图稀化/绘制：算法算完（Douglas-Peucker）却可能根本没落库，接口仍回「成功」；`draw` 还把入参原样回显成「已保存」 | 两条分支都传播错误，并补 `rows_affected == 0 → 404` |
| **`map_thin_clear` 只有 postgres 分支吞错** | mysql/sqlite 分支本来就是传播的，唯独 PG 构造下报成功而没清 | 统一传播 |
| **`platform_delete` 级联删除吞错** | 平台行删掉了，`gb_platform_channel` 里留下指向不存在平台的孤儿行 | 改为 `?` 传播 |
| **`channel_audio` 是假的** | 对一个**只读**查询（`get_media_list`）的结果视而不见，却打印 `ZLM streams updated for audio mode`；真正的 `has_audio` 写入错误被吞，然后返回「已更新」 | 删掉假调用与谎言日志；如实落库并传播（该字段由 `sip/gb28181/catalog.rs` 作为 `<HasAudio>` 上报上级平台，**落库即生效**） |
| **`channel_stream_identification_update` 吞错** | 写入失败仍返回「流标识更新成功」 | 传播 + 404 |
| **`platform_delete` 以外的后台写入静默丢失** | JT1078 位置回写、ZLM 节点状态、ZLM keepalive、SIP `MobilePosition` 历史、审计日志 —— 都脱离请求上下文无法传播，但此前一声不吭 | 全部改为 `tracing::error`，不再静默 |
| **`ensure_stream_status_column` 迁移错误被吞** | 建列失败会在启动时被略过，把问题推迟成运行期 `no such column` | 纳入 `?`，与相邻的 `ensure_columns` 一致 |

#### SDP 构造：三份实现互相矛盾

`invite_session.rs`（活跃）、`sdp_builder.rs`（**无任何调用者**）、`talk.rs` 各有一份
SDP 构造，且取值互不相同：

| 维度 | 活跃实现 | 死代码实现 | 国标/证据 |
|------|----------|-----------|-----------|
| 方向（点播/回放/下载） | `a=sendonly` | `a=recvonly` | **`recvonly`**：平台发出的取流 INVITE 中平台是接收方 |
| 回放 `y=` | 写死 `0100000001` | 由调用方给 | 必须按会话唯一 |
| 回放 `t=` / `npt=` | 直接写 ISO 时间串 | 同 | 必须是 **UNIX 秒** |
| 对讲会话名 | `s=TALK` | `s=Talk` | `s=Talk` |

方向依据（web 检索，见文末来源）：GB/T 28181-2016 附录示例中，平台→设备的点播
INVITE 为 `a=recvonly`（平台收、设备发），设备 200 OK 才是 `a=sendonly`；
`SDP详解-开源国标视频平台的工程实践` 明确"平台 INVITE 用 recvonly = 平台（MS）收，
设备发……`Mode: sdp.ModeRecvOnly`"。

**修复**：让 `sdp_builder.rs` 成为唯一实现，`invite_session.rs` 与 `talk.rs` 只做参数适配；
方向统一为点播/回放/下载 `recvonly`、对讲/广播 `sendrecv`；新增
`to_unix_seconds()` 把 ISO 时间归一化为 UNIX 秒（无法解析时回退 `t=0 0`，
而不是把非法串发出去）；`build_playback_sdp` 增加 `ssrc` 参数，调用方传入真实 SSRC。
新增 8 个测试锁住方向、时间归一化与 SSRC 来源。

#### `m=` 端口为 0：SDP 里 0 表示「该媒体流被禁用」

多条 INVITE 路径把 `m=video`/`m=audio` 端口写成 0（含
`// 这里用占位 0 留给 ZLM 自行协商` 这类注释），设备没有可推流的目标，
这些功能**从未真正可能建立**：

| 路径 | 原状 | 修复 |
|------|------|------|
| `channel_playback_start` → `send_playback_invite` | 根本没开 ZLM RTP server，`m=video 0` | 先 `openRtpServer` 取真实端口再发 INVITE |
| `gb_record_download_start` → `send_download_invite` | **已经** `openRtpServer`，但把返回句柄 `let _ =` 丢掉，仍传 0 | 取回端口传入 |
| 级联取流 `push_platform_channels` / 推送处理器 → `send_platform_invite` | 两处都传 0；且复用 `build_playback_sdp` 导致**实时取流被标成 `s=Playback`** | 分配真实端口；改用 `build_invite_sdp(..., "Play", …)`；SSRC 走 `SsrcManager` 按会话分配 |
| `send_talk_invite` | `m=audio 0`；且去话路径**从不登记 `TalkManager`**，`/api/talk/list` 永远查不到刚发起的对讲 | 先 `openRtpServer` 取端口；登记去话会话（含 `local_port`/对端地址/stream_id） |
| `handlers/talk.rs` 回给前端的展示 SDP | 固定 `m=audio 0` | 改读会话的真实 `local_port` |
| 广播 11 位 SSRC | `format!("4{:0>9}0", …)` 得到 11 位（国标是 10 位） | 统一 `build_ssrc(prefix, id)`：`0` 实时 / `1` 回放 / `2` 下载 / `4` 广播 |

### TCP 信令与「延迟实现」（2026-09-12 第五轮修复）

| 问题 | 影响 | 修复 |
|------|------|------|
| **TCP 入站信令走的是与 UDP 完全不同的分支** | `process_tcp_message` 自行调用 `handle_request`/`handle_response`，并给 `pending_invites`、`cascade_registrar`、`subscription_lifecycle`、`renewal_failures` 传 `Arc::new(DashMap::new())` / `None` 这类**一次性空对象**。于是 `sip.transport = "tcp"` 的部署里：INVITE 响应**永远无法唤醒**等待中的 `pending_invites`（实时点播/回放必然超时）、级联注册的 200 OK 无法完成注册、订阅续订状态与失败计数全部丢失、客户端事务永不终止（事务表只增不减） | `process_tcp_message` 改为**统一走 `handle_packet`**（与 UDP 同一条分发路径），并补齐全部上下文句柄；TCP 专属的响应路由（RFC 3261 §18.2.2）保留 |
| **`SsrcManager::allocate` 产出 15 位 SSRC** | `format!("0{}{:04}0", prefix9, seq)` = 1+9+4+1 = **15 位**，而国标 SSRC 是 **10 位十进制**。该值会写进 INVITE 的 `y=`；单元测试还把 15 位当作期望值固化了下来 | 改为 `类型位(1) + 域标识(5) + 流序号(4)` 共 10 位；类型位随业务类型变化（0 实时/1 回放/2 下载/4 广播）；测试改为断言 10 位并新增各类型位用例 |
| **`catalog_sync`（331 行）只有 `pub use`、无任何调用者** | 目录**分页**响应（`SumNum > 1`）没有聚合与完成状态；`/api/device/query/devices/:id/sync` 只回一句「命令已发送，等待响应」，前端无从判断同步是否完成或失败 | 接入 `SipServer`；`send_catalog_query` 启动同步会话；Response 与 NOTIFY 两条目录路径都调用 `handle_packet` 聚合分页；**保留逐包 upsert 作为兜底**（设备少发最后一页时不会丢已收到的通道）；`device_sync` 现在等待同步结果（最多 8s）并返回 `syncState/totalPackets/receivedPackets/channelCount/error` |
| **统一流视图只返回 2/4 类流** | `list_all_streams` 留了 `TODO(phase-5): 等 gb_send_rtp 建表`，实际该类流在 `SendRtpManager`（内存 DashMap）里，**根本不需要建表**；"gb" 类同样被漏掉 | `SendRtpSession` 与 `InviteSession` 实现 `StreamState`；新增 `SendRtpManager::list_all()` 与 `SipServer::invite_session_manager()`；四类流（push / proxy / gb / send_rtp）齐备，TODO 删除 |
| **录像下载端口分配失败仍继续** | ZLM 分配失败后仍发出 `m=video 0` 的 INVITE，留下一个永远不会完成的下载会话 | 直接失败并记录 error |
| 陈旧注释 | `handlers/server.rs` 段落标题写「占位：前端调用避免 404」，段内 8 个 handler 全是真实实现；`jt1078/mod.rs` 文档仍称 0x8202/0x8203/0x9205「未实现」，实际已补齐 | 按实际内容更正 |

### 运行时冒烟发现的问题（2026-09-12 第六轮修复）

**方法**：不再只做静态审计，而是**真的把服务跑起来**（全新 SQLite 库 + 独立
端口），登录取 JWT 后把 `router.rs` 里 254 个 GET / 97 个 POST 端点全量打一遍，
再用脚本比对「前端 TS 接口字段」与「后端实际请求/响应字段」。下面每一条都是
**运行时复现**出来的，静态审计全部漏掉了。

| 问题 | 运行时证据 | 修复 |
|------|-----------|------|
| **全新部署直接启动失败** | 空库启动 → `Error: no such table: gb_stream_push`，进程退出 | `init_db_tables` 把依赖既有表的**列级迁移**排在全量建表**之前**；空库上 `ALTER TABLE gb_stream_push` 必然失败。旧代码用 `let _ =` 吞掉错误才"看起来能启动"。现拆成四个严格有序阶段：幂等建表 → 全量建表 → 列级迁移 → 旧库补表 |
| **前后端字段命名契约系统性不匹配（请求侧）** | `POST /api/region/add` 发 camelCase → `{"code":400,"msg":"deviceId 与 name 必填"}`；`/api/common/channel/playback/pause?channelId=1` → 报"缺少 stream"（其实 channelId 没绑上） | 前端一律发 camelCase，而大量请求结构体只声明 snake_case 且无 `#[serde(alias)]`——**同一个仓库里两种写法并存**（`device_control.rs::PtzQuery` 记得加 alias，`front_end.rs::PtzQuery` 就忘了）。已为 `handlers/**` + `db/**` 中 85 个 `Deserialize` 结构体补 92 个 camelCase alias（alias 是增量语义，snake_case 调用方不受影响） |
| **前端在调、后端没注册** | `GET /api/push/stop`、`GET /api/jt1078/terminal/one` 返回的是 SPA 的 `index.html`（HTTP 200，`Content-Type: text/html`） | 「停止推流」按钮与终端详情接口整体不工作。已实现并注册；`push_start` / `push_batch_remove` / `push_force_close` 三处吞掉 ZLM/DB 错误后假装成功的写法一并改为如实传播 |
| **前后端字段命名契约不匹配（响应侧）** | `/api/role/all` 返回 `create_time`，前端 `Role` 接口是 `createTime`（列表时间列为空）；`/api/server/media_server/list` 返回 `http_port`/`sdp_ip`/`type_`，而 `views/mediaServer/index.vue` 用 `prop="httpPort"`（这些列全空，`type_` 连键名都对不上） | `MediaServer` 加 `#[serde(rename_all = "camelCase")]` + `type_` 显式 `rename = "type"`；`Role` 用 `rename_all(serialize = "camelCase")`（只改序列化方向，反序列化仍兼容 snake_case）；`system/configInfo` 的 zlm 段一并统一。复扫后响应侧不匹配数 **9 → 0** |

**冒烟基线（本轮结束时）**：254 个 GET 端点 **0 个 5xx**、**0 个
`no such column` / `no such table`**；97 个 POST 端点 **0 个 5xx / 0 个超时**；
未带 token 访问受保护端点返回 401；登录 → 区域/分组新增 → PTZ 参数绑定
→ 应用新端点均按预期返回。

#### 第二轮运行时冒烟追加（请求耗时与必填字段）

| 问题 | 运行时证据 | 修复 |
|------|-----------|------|
| **`POST /api/server/media_server/save` 慢到像卡死** | 实测 **33.4 秒**（`curl` 15s 直接超时）；日志显示是 11 次**串行** `setServerConfig`，ZLM 不健康时每次等约 3s（客户端超时上限 30s，最坏 30×11≈5.5 分钟）。ZLM 的 `on_server_started` 回调里还有 13 + 2 + N 项配置同样串行下发 | 新增 `ZlmClient::set_server_configs_batch`（并发下发 + 总预算 10s，超时如实报告「还有 N 项未完成」）；`configure_zlm_hooks` 改用它，**33.4s → 3.8s** |
| **`/api/server/media_server/load` 的 `gbReceive`/`gbSend` 恒为 0** | 这两个数字是从 ZLM `getServerStats` 里按 `MediaStreamCount` / `MediaSenderCount` / `sendRtpCount` 取的 —— 这些键名是**凭空猜的**（ZLM 不返回），所以永远取不到；而这个被控制台轮询的接口还要为此每个节点多等一次 HTTP 往返 | 改用本进程内存里的权威计数：`InviteSessionManager::get_active_sessions()` 与 `SendRtpManager::active_count()`（GB 会话目前没有按节点归属的信息，故为全局真实值，已在代码注释与文档注明） |
| **`/api/platform/add` 与 `/update` 必然 422** | `{"code":422,"msg":"missing field `device_port`"}` —— `PlatformAddBody.device_port` 是 `Option<String>`，但带了 `deserialize_with`，而 **`deserialize_with` 会去掉 `Option<T>` 的隐式 default**，使该字段变成必填；前端平台表单从不提交 `devicePort`，因此新建平台完全不可用 | 补 `#[serde(default)]`。全仓库扫描确认这是唯一一处（`Option<T>` + `deserialize_with` 且无 `default`） |
| 用户口令/推送键参数结构体的旧写法 | `ChangePasswordParams` 同时声明 `old_password` 与 `oldPassword`（后者 `rename = "oldPassword"`），两个字段映射到同一 JSON 键，serde 报 `unreachable pattern`；调用方靠 `a.or(b)` 兜底 | 三个结构体统一为 snake_case 主名 + camelCase alias，删除重复字段与 `#[allow(non_snake_case)]` |

**第二轮基线**：GET 254 项 / POST 97 项全量复扫 —— **0 个 5xx、0 个悬挂超时、0 个 `no such column`**；
404 从 5 项降至仅剩「记录确实不存在」，422 从 5 项降至 3 项（均为前端本来就会提供的必填字段）。

#### 真实 SIP 信令端到端验证（第三轮，2026-09-12）

用仓库自带的 `mock/tools/sip-device/sip_device_mock.py` 对真实运行的服务做信令联调，
**发现模拟器自身有 bug，导致它此前从未成功注册过**：

| 问题 | 证据 | 修复 |
|------|------|------|
| **模拟器 Digest 鉴权算错，注册必然 403** | `_extract_header(self, msg, name, default="")` 的第三个参数是 **default**，没有「按键取值」的语义，而调用方写的是 `_extract_header(msg, "WWW-Authenticate", "realm")` —— 于是 realm 与 nonce 都被赋成**整个头值**（`'Digest realm="...", nonce="...", algorithm=MD5, qop="auth"'`），HA1/HA2 全错。服务端日志：`REGISTER from 34020000001320000001 - Invalid credentials` → 403 | 新增 `_extract_auth_param()` 真正按参数名解析；顺带让模拟器在服务端宣告 `qop="auth"` 时按 RFC 2617 带 `qop/nc/cnonce` 计算（覆盖服务端那段曾把 cnonce 写死的历史 bug 分支） |

**修复后的端到端结果**（服务端 `cargo run` 真实运行，模拟器真实发包）：

1. `REGISTER` → 401 挑战 → 带 Digest 重发 → **200 OK**（qop=auth 分支）；
2. `MESSAGE/Keepalive` 周期上报 → 200 OK；
3. 设备出现在 `/api/device/query/devices`，`onLine=true` 且 IP/端口正确；
4. `GET /api/device/query/devices/{id}/sync` 触发真实 Catalog 查询，模拟器按
   **3 个分页**返回，`catalog_sync` 聚合结果：
   `{"syncState":"done","totalPackets":3,"receivedPackets":3,"channelCount":3,"message":"设备目录同步完成"}`
   —— 验证了第五轮接线的分页聚合与完成状态反馈；
5. 3 个通道落入 `gb_device_channel` 并在 `/api/common/channel/list` 可见
   （`ON` 状态、名称来自设备上报）。

未能验证的部分：实时点播的完整媒体链路需要 ZLMediaKit 在线；当前环境 ZLM 不可达，
`/api/play/start/...` 如实返回 `Media Server error: HTTP error: 502 Bad Gateway`
（约 1 秒内失败，无悬挂），即 SIP INVITE 之后的 ZLM `openRtpServer` 步骤被正确拒绝。

#### ZLM webhook 与 Redis 降级（第三轮追加）

对 `/api/zlm/hook` 逐个投递真实形状的 ZLM 回调（`on_server_started` /
`on_server_keepalive` / `on_stream_changed` / `on_publish` / `on_play` /
`on_record_mp4` / `on_rtp_server_started` / `on_rtp_server_timeout` /
`on_stream_none_reader` / `on_send_rtp_stopped` / `on_flow_report` /
`on_stream_not_found`，以及未知事件与缺 `hook_name` 两种边界）：
全部返回 `{"code":0}`，未知事件按 `Unhandled webhook` 记录而**不崩溃、也不假成功**。

同一轮发现并修复：**Redis 不可用时每次状态更新都白等 1.5s**。

| 问题 | 证据 | 修复 |
|------|------|------|
| **Redis 连接失败没有记忆** | `RedisBackend::connect()` 在 `manager` 仍为 `None` 时会被**每一次**状态读写重新触发（`get_conn` 的唯一入口），日志里出现连续的 `Redis connect timed out after 1.5s`。ZLM 每个 hook（on_stream_changed / on_publish / on_play …）都要更新流状态 → Redis 挂掉时"每个 hook 慢 1.5s"，ZLM 侧极易判定 hook 超时 | 新增失败冷却：连接失败后 30s 内直接走内存后端，不再重试；连接成功即清除冷却。新增测试断言冷却期内 20 次读写总耗时 < 500ms（修复前会 ≥ 30s） |

### 语音对讲：从"信令通、音频无"到端到端打通（2026-09-12 第八轮）

**此前的真实状态**：对讲的信令与 SDP 协商看起来"有实现"，但**没有任何音频通路** ——
没有 G.711A 编解码、没有 RTP 打包发送、`TalkSession.local_port/device_ip/device_port`
无人消费。对着麦克风说话，设备什么也收不到。

| 问题 | 运行时证据 | 修复 |
|------|-----------|------|
| **去话 INVITE 从不登记 `session_manager`** | 日志只有 `Sent TALK INVITE`，**没有** `Sent ACK`；而 GB28181 三次握手缺 ACK 设备不会推流。`handle_response` 发 ACK 依赖 `session_manager.get(call_id)` 取 from/cseq/device_addr | 登记 INVITE 上下文；实测出现 `Sent ACK to device for call_id=talk_...` |
| **设备 200 OK 的 SDP 从不解析** | `TalkSession.device_port` 一直是 INVITE 时的信令地址，不知道把音频发到哪 | 解析 `m=audio` + `c=` 回填 `device_ip/device_port`；设备写 `0.0.0.0` 时回退到其 SIP 信令源地址（实测 `0.0.0.0` → `127.0.0.1`） |
| **`openRtpServer` 失败仍发 `m=audio 0`** | `m=audio 0` 在 SDP 中表示媒体流被禁用，对讲不可能建立，接口却回"对讲请求已发送" | 取不到收流端口直接失败并返回原因 |
| **接口返回的 callId 与会话的 callId 不是同一个** | `/api/talk/start` 自己又拼了带毫秒时间戳的 callId，前端据此查询/停止对讲必然查不到 | 统一使用 `send_talk_invite` 返回的真实 call_id |
| **`/api/ws` 对登录用户完全不可用** | `verify_ws_jwt` 用裸 `Validation::new(HS256)` **未设 audience**，而 `jsonwebtoken` 在"token 带 `aud`、校验未配 `aud`"时返回 `InvalidAudience`；实测 WS 握手 `401 JWT invalid: InvalidAudience`。单测自造 token 没有 `aud`，因此测试全绿 | 与 HTTP 侧对齐 audience 与必需声明；补两个回归守卫（真实登录 token 必须通过、缺 aud 必须被拒） |
| **WS 侧所有用户是同一个身份** | `Claims.sub` 对登录 token 恒为字符串 `"login"`（用户名在 `userName`），而 `ws_handler` 拿 `sub` 当用户身份注册 WsHub | `WsClaims` 增加 `userName` 与 `username()`，`ws_handler` 改用它 |

**新增的音频管线**（`src/sip/gb28181/talk_audio.rs`）：

* G.711 A-law 编解码，与 Sun `g711.c` / ITU-T G.711 参考实现一致；
  期望值由**独立转写**的参考实现算出后固化为测试（含 `linear2alaw(1000)=0xFA`、
  `i16::MIN → 0x2A` 等边界），另有全量程往返误差上界测试。
  （A-law 数字静音 `0` 编码为 `0xD5`、解码回 `+8`，是固有直流偏置，不是 bug。）
* RTP 打包（RFC 3550，PT=8 / PCMA / 8000，20ms = 160 样本 = 160 字节），
  序号与时间戳按样本数递增。
* `TalkAudioSender`：本地 UDP socket → 设备音频地址；不足一帧的**尾包也发**
  （否则句尾被吞）。
* WS 端点 `GET /api/talk/audio/:device_id/:channel_id?token=<jwt>`：
  二进制帧 = 8kHz 单声道 i16 小端 PCM，服务端编码后发 RTP。
  **必须注册在 `api_protected` 之外**：浏览器无法为 WS 设置请求头，
  `auth_middleware`（只认 `access-token`/`Bearer`）必然把握手判 401；
  与 `/api/ws` 一致，由 handler 内部用 `?token=` 校验。
* 对讲 SDP 的 `y=` 与实际 RTP 包的 SSRC 现在同一个值（`build_audio_ssrc`，
  前缀 4；会话记录该 SSRC）。

**端到端验证**（真实服务 + 真实 SIP 发包 + 手写最小 WS 客户端 + 假设备 UDP 监听）：

```
WS 握手: HTTP/1.1 101 Switching Protocols
假设备收到 2 个 UDP 包
  包 0: V=2 PT=8 seq=0 ts=0   ssrc=4000000000 payload=160B 尽为0xFA=True
  包 1: V=2 PT=8 seq=1 ts=160 ssrc=4000000000 payload=160B 尽为0xFA=True
结论: 通过
```

服务端日志同步确认：`对讲音频上行通道建立: ... -> 127.0.0.1:10002 ssrc=4000000000`
与 `对讲音频上行通道关闭: ... 共 2 包 / 320 字节`。
（0xFA = `linear2alaw(1000)`，测试用 1000 而非 0，避免"恰等于默认值"的假阳性。）

**前端对讲面板（同轮补齐）**：`web/src/components/TalkPanel/index.vue` +
`web/src/api/talk.ts`，已挂到 `views/live/index.vue` 的 PTZ 工具条。

* `getUserMedia` → `AudioContext` → 线性插值重采样到 **8kHz**（用小数游标，
  正确处理 44100→8000 这类非整数比）→ i16 小端二进制帧经 WS 发送；
* 播放设备侧音频不走该 WS：设备把 RTP 推到 ZLM 的 `localPort`，浏览器播放
  ZLM 的 ws-flv 即可；
* 结束时关闭 WS、停止麦克风轨道、并发送 `/api/talk/stop`（BYE）；
  组件卸载时静默收尾，避免麦克风一直开着。

**同时修掉一个阻塞开发模式的配置错误**：`vite.config.ts` 的 `/dev-api` 代理
原先 `rewrite` 成**空串**，于是 `/dev-api/user/login` → `/user/login`，
而后端只注册 `/api/user/login`，未知路径被 SPA 兜底 `nest_service("/")` 命中，
返回 **200 + text/html**（index.html）—— 前端 axios 拿到 200 却不是 JSON，
**开发模式下所有接口都不可用**（生产用 `VITE_APP_BASE_API='/api'` 同源直连，
所以只有 dev 受影响，容易长期不被发现）。现改为重写到 `/api`，并给该代理
开启 `ws: true`（否则 `/api/ws` 与对讲音频 WS 在 dev 下握手失败）。

验证：经 vite dev 代理 `GET /dev-api/user/login` 返回
`content-type: application/json`（修复前是 `text/html`）；
对讲音频 WS 经代理握手得到 `HTTP/1.1 101 Switching Protocols`。

### 端到端 UI 测试（Playwright）首次整体通过（2026-09-12 第九轮）

此前这套 e2e 从未在新环境上绿过，而且**通过的用例也没测到东西**：

| 问题 | 运行时证据 | 修复 |
|------|-----------|------|
| **鉴权状态靠副作用产生** | `artifacts/.auth.json` 由 `smoke.spec.ts` 第 3 个用例写出，而 Playwright 按文件名字母序执行（`live.spec.ts` 在前）→ 干净检出上 live 的 5 个用例全部 `ENOENT: artifacts/.auth.json` | 提升为 `globalSetup`（`e2e/global-setup.ts`）登录一次并落盘；项目级 `use.storageState` 让默认 `page` 即为已登录态，需要未登录的用例显式开空状态 context |
| **路由模式写错** | 快照里侧边栏是 `#/dashboard` 这类 hash 链接；实测 `goto('/live')` → `hash=#/dashboard`（`测试播放` 按钮数 0），`goto('/#/live')` → `hash=#/live`（按钮数 1）。应用用的是 `createWebHashHistory`，而测试用 history 路径 | 测试改走 hash 路径；`smoke.spec.ts` 增加"hash 确实切到目标路由"的断言 |
| **17 个"page renders"用例是空的** | `page.goto(p.path)` 每次都落在 `#/dashboard`，而断言只检查"没被重定向到登录页"与"body 可见" → 这 17 个用例**每个都在渲染控制台页并全部通过**，截图也都是 dashboard | 走 hash 路径 + 断言 hash；现在确实逐页渲染 |
| **PTZ 用例永远跳过** | 它只看 `.video-grid` 是否可见，而 `.video-grid` 只在选中通道后才渲染 → 从未执行过按钮计数断言 | 改为真的点击通道节点；实测选中后按钮数为 **8**（7 个 PTZ + 1 个「对讲」） |

**结果**：`npx playwright test` → **24 passed / 0 failed / 0 skipped**（此前为
19 passed / 5 failed）。前置：后端 :18080（可用临时 SQLite）、前端 dev :9528、
`npx playwright install chromium`。

顺带修掉 `handlers/device_stub.rs` 的同类问题：`device_transport` 用
`unwrap_or_default()` 吞掉 DB 更新错误并仍回"设置成功"（现如实传播，
0 行受影响解释为"设备不存在"）；删除两个迁移残留的无引用请求结构体
（`GuardQuery` / `SubscribeCatalogQuery`）；更正模块头那句已经过时的
"其余保持兼容空实现（后续可对接 SIP/ZLM）"。

### 设备查询/配置查询：三层 SN 关联缺陷 + 无限循环（2026-09-12 第十轮）

这一轮改用**仓库自带的 SIP 设备模拟器**驱动真实设备查询，暴露出"接口能返回
200，但结果永远是 timeout"这类只有真发报文才看得见的问题。

| 问题 | 运行时证据 | 修复 |
|------|-----------|------|
| **pending 关联键与发出的 Call-ID 不一致** | 登记用 `di_/ds_/dc_{device}_{sn}`，实际发 SIP MESSAGE 用的是 `msg_{device}_{时间戳}`；设备**回显后者** → 日志 `Unsolicited MESSAGE response for CallID msg_...`，`/api/device/query/info` 与 `/status` 恒 `timeout_or_error` | `ResponseRouter` 增加 `route_message_response_with_device`：Call-ID 未命中时用 (device_id, `<SN>`) 兜底关联（管理器里本来就有 `by_device_sn` 索引，但**没有任何按它完成的路径**） |
| **`XmlParser::get_sn` 取不到嵌套 Response 里的 SN** | 第一版兜底用了 `XmlParser::get_sn`，实测仍未命中（该文件里早就注明 `XmlParser::parse` 处理不了 `<Response>` 嵌套，所以才有字符串版 `extract_cmd_type`） | 新增同风格的 `extract_sn()`，并加"嵌套 Response 取 SN"的单测 |
| **登记的 SN ≠ 发出的 SN** | 调用方生成 `sn` 去登记，而 `send_device_info_query` / `_status_query` / `_config_query` 内部**另取** `Utc::now().timestamp()` 写进 XML → 设备回显的 SN 与登记的对不上，兜底也失效 | 三个发送函数改为接受调用方传入的 `sn`，保证"登记 = 发出 = 设备回显" |
| **设备的 `<Response>` 被当成新查询，形成无限循环** | `DeviceInfo` / `DeviceStatus` / `MobilePosition` / `Alarm` 四个分支**无条件**按查询处理并回一条 → 设备（以及模拟器）再回 → 日志每 1.5~3 秒重复同一设备报文，既刷日志又反复写库 | 分发前判定 `body_is_response`，这四个纯查询型分支遇到应答时只回 200 OK 并返回（`Catalog` 分支本来就地正确区分 Query/Response） |
| **`/api/device/config/query/:id/BasicParam` 只发不等** | 返回 DB 旧值 + `"设备配置查询已发送"`，从不消费应答，等于把"查询设备配置"实现成一个纯发送动作 | 改为登记 pending + 用同一 SN 下发 + 等待 15s + **解析**设备上报的 `Name`/`Manufacturer`/`Model`/`Firmware`/`HeartBeatInterval`/`Expiration`（库值仅作兜底） |
| **同一功能三处实现、其中一个只发不等** | `/api/device/config/query`（查询参数版，`device_control`）拼完 XML 直接发包并回 `"Config query sent"`；路径参数版（`device_query`）却是真的在等 | 抽出 `device_control::query_config_and_wait()` 共享实现，两个变体都委托它，消除重复 |

**修复后的实测结果**（真实服务 + 模拟器真实发包）：

```
/api/device/query/info    -> {"device_name":"E2ECam","manufacturer":"MockVendor",
                              "model":"MOCK-IPC-100","channel_count":3,"firmware":"1.0.0-mock"}
/api/device/query/status  -> {"online":"ONLINE","status":"OK"}
/api/device/config/query/…/BasicParam
                          -> {"name":"E2ECam","manufacturer":"MockVendor","model":"MOCK-IPC-100",
                              "firmware":"1.0.0-mock","heartBeatInterval":"60","expiration":"3600",
                              "source":"live"}
SN 兜底命中 2 次；Unsolicited 0 次（此前同一场景 17 次 / 5 秒）
```

模拟器同步补齐 `ConfigDownload` 与 **多包 `RecordInfo`** 应答（均回显 SN），
使成功路径可被验证。录像查询实测：2 个分页 × 2 条 → `total=4`、4 条唯一记录，
多包聚合正确。

（附注：`/api/gb_record/query` 返回里的 `count` 是**页大小**（默认 20），
`total` 才是结果总数；与其它列表接口一致，不是缺陷。该路径的 SN 与 Call-ID
在 `send_record_info_query_and_wait` 内部由同一个变量产生，本就一致。）

### ZLM Webhook 集成与实时点播链路（2026-09-12 第十一轮）

这一轮从"点开实时直播页什么都不动"往回追，发现**整套 ZLM webhook 集成在真实环境下
完全不生效**，以及实时点播的前端流程从来没有真正拉起过流。

| 问题 | 证据 | 修复 |
|------|------|------|
| **hook 分派依赖 body 里的 `hook_name`，而真实 ZLM 根本不发它** | [ZLM 官方文档](https://docs.zlmediakit.com/guide/media_server/web_hook_api.html) 的 `[hook]` 默认配置显示 `on_play` / `on_publish` / `on_record_mp4` … **各有独立 URL**，且各事件的示例 body 是**扁平 JSON**（只有 `mediaServerId`/`app`/`stream`/`schema`…）。而 `handle_webhook` 按 `event["hook_name"]` 分派，缺失时取 `"unknown"` → 全部落到 "Unhandled webhook"；多路径路由 `handle_hook_event::<T>` 也只做了一次"不一致就 warn"的校验就原样转发，**从不注入路由绑定的事件名** | `handle_hook_event::<T>` 改为把路由的事件名注入 body（URL 才是权威来源）；`handle_webhook` 因此对所有事件生效 |
| **所有 hook 都指向同一个 URL** | `configure_zlm_hooks` 把 11 个（`hook.rs` 里 13 个）hook 全部设成同一个 `/api/zlm/hook` | 新增 `hook::hook_config_items()`：每个事件配置**各自的** `…/api/hook/<event>`；两处配置点共用 |
| **5 个事件有分派分支却没有路由** | `on_record_hls` / `on_record_file` / `on_rtp_playlist` / `on_record_progress` / `on_send_rtp_progress` 在 `handle_webhook` 里都有 arm，但 `hook_routes()` 只暴露 12 条 | 补齐 5 条路由；新增交叉校验测试：**配给 ZLM 的每个事件都必须有路由**，有路由的必须是已配置项或显式登记的别名（`on_record_file` 是别名，真实 ZLM 无此配置键） |
| **实时直播页从不拉起流** | `playChannel()` 只调用 `/api/media/getPlayUrl` 拼地址 —— 既没有 SIP INVITE，也没有 `openRtpServer`，于是画面永远出不来 | 改为先 `startPlay()`（后端发 INVITE + 开 ZLM RTP server）再播放其返回的 HLS/FLV/RTSP 地址；`onStop` 真正调用 `stopPlay()`（发 BYE + 清理 ZLM）；新增 e2e 守卫断言"点通道必须发出 `/api/play/start`" |
| **`getPlayUrl` 返回的地址是错的** | RTSP 用了 **http_port**（`rtsp://host:8080/…`）、app 写成 `live`、HLS 路径写成 `hls/{stream}.m3u8`、WebRTC 缺 `index/api/webrtc?…`；且流不存在时也返回一个播不出来的 URL | 改用 `rtp` app 与正确路径；RTSP/RTMP 端口取库中媒体服务器配置；**先向 ZLM 确认流存在**，不存在则明确报"流尚未建立，请先调用 /api/play/start" |
| **设备被当成通道返回** | `/api/sy/camera/list-with-child` 在设备没有任何通道时把**设备自身**作为一行返回（`channel_id == device_id`），且 `device_to_row` 一律用**设备**的 `id`/`name` 填通道行 → 同一设备下通道 id 全部相同、通道名为空 | 有通道时用通道自己的 `id`/`name`；新增 `is_device` 标记；前端 live 树过滤掉设备行 |
| 孤儿假实现 | `stub::server_shutdown` 无路由、无实现，却返回 "Shutdown signal sent. Server will stop gracefully." | 删除（并在注释里说明将来要做应端到端实现）；`parity_extras` 里 `assert_eq!("auto", "auto")` 的同义反复测试替换为真实断言 |

**端到端实测**（真实服务 + SIP 模拟器 + ZLM 模拟器按真实形态回调）：

```
POST /api/hook/on_rtp_server_started   ← mock 以扁平 body（无 hook_name）回调
/api/play/start/…  -> {"code":0,"playUrl":"rtsp://127.0.0.1:554/rtp/…",
                       "hls":"http://127.0.0.1:8080/rtp/…/hls.m3u8",
                       "flvUrl":…, "webrtc":…}
/api/media/getPlayUrl -> 与上面一致的 HLS 地址（并会校验流是否存在）
/api/play/stop/…   -> {"code":0}
npx playwright test -> 25 passed / 0 failed（含新增的"点通道必须起流"守卫）
```

**同时修正了 ZLM 模拟器的三处保真度问题**（否则上述缺陷会被掩盖）：
hook body 之前嵌在 `data` 里且带 `hook_name`（现改为真实 ZLM 的扁平、无 `hook_name`、
POST 到各自事件 URL）；`openRtpServer` 之前不创建流（真实 ZLM 会，导致"先起流再取地址"
的第二个请求误判）；`closeRtpServer` 之前不移除流。

### ZLM hook 载荷契约与鉴权（2026-09-12 第十二轮）

上一轮修好了 hook 的**分派**；这一轮逐个事件下发**真实形态的扁平载荷**，验证
"分派之后是否真的产生效果"，又发现四类缺陷——每一个都表现为"接口回 200、
日志什么都不打"：

| 问题 | 证据 | 修复 |
|------|------|------|
| **`RecordMp4Data` 与真实载荷不匹配** | 官方文档的 `on_record_mp4` 示例 body 里**没有 `schema`**，时长字段是 **`time_len`**（float）、开始时间是 **`start_time`**（整数）；而结构体把 `schema`/`file_duration`/`file_create_time` 声明为必填 → 反序列化必然失败 → 事件被静默丢弃（连 "MP4 recorded" 都不打），**云录像永远不入库** | `schema` 改 Option，`file_duration` 加 `alias="time_len"`、新增 `file_start_time`（`alias="start_time"`，UNIX 秒并格式化为可读时间）；`RecordHlsData` 同样处理 |
| **`StreamChangedData` 用错字段名** | 真实 ZLM 的字段是 **`regist`**（其 wiki 有专门 commit "on_stream_changed 注册时添加 regist 字段"），结构体只认 `register` → 该事件被静默丢弃，**流上下线状态永远不同步** | 加 `#[serde(alias = "regist")]`；实测日志出现 `Stream changed: rtsp/rtp/… register=true` |
| **`ServerStartedData` 六个端口全必填** | 少任何一个字段（不同 ZLM 版本字段集不一致）都会让整条 `on_server_started` 被丢弃 —— 而**hook 配置只在这一条事件里做**，于是整套 webhook 静默失效（本轮实测：省略 `hook_port`/`https_port` 即无任何日志） | 全部字段 `#[serde(default)]` 并给出协议默认值；实测残缺载荷（只给 `rtsp_port`/`http_port`）也能完成重新配置 |
| **hook secret 的传递方式错了** | 真实 ZLM 通过 `[hook] admin_params` 把 secret 作为 **URL 查询参数**附加（body 里没有 secret），而 `check_hook_auth` 只从 body 读 → 带鉴权的 `on_publish` / `on_play` **一律 "secret mismatch"**，ZLM 因此**拒绝一切推流与播放**；而且我们从没给 ZLM 配过 `admin_params`，它附加的 secret 本来也对不上 | ①`hook_config_items()` 增加 `hook.admin_params=secret=<node secret>`（并显式设 `hook.timeoutSec=5`）；②`check_hook_auth` 改为**优先从查询串**取 secret（含百分号解码），body 作为兼容回退。实测：不带 secret → 拒绝；带正确 secret → 放行并处理 |
| **鉴权 fail-open 且顺序错误** | `check_hook_auth` 写在 `if let Some(data) = from_value(...)` 的**成功分支内** → body 少一个字段就完全跳过鉴权并回 `{"code":0}`（放行）；同时它先判"IP 不可解析"再判 secret，载荷缺 `ip` 时返回的是 "invalid client IP"，掩盖真正的结论 | 鉴权提到解析之前**无条件执行**（fail-closed）；并改为 **secret 优先**、白名单仅在拿得到 IP 时校验 |

**实测**（真实服务 + ZLM 模拟器按真实形态回调）：

```
on_publish 无 secret                -> {"code":-1,"msg":"Unauthorized: secret mismatch"}
on_publish ?secret=<正确>            -> {"code":0} 且日志 "on_publish: rtsp/rtp/x from 127.0.0.1"
on_record_mp4（真实载荷）            -> 日志 "MP4 recorded: 15-53-02.mp4 (1913597 bytes)"
                                      且 gb_cloud_record 入库 (time_len=11.0)
on_stream_changed（regist=true）     -> 日志 "Stream changed: rtsp/rtp/… register=true"
on_server_started（残缺载荷）        -> 日志 "ZLM hook URLs reconfigured"；
                                      mock 侧收到 hook.enable=1、
                                      hook.admin_params=secret=<node secret>、
                                      hook.timeoutSec=5、
                                      每个事件各自的 /api/hook/<event> URL
起流（mock 带 admin_params 回调）     -> on_rtp_server_started 被接受 →
                                      "MediaWaiter resolved" → /api/play/start code 0
```

**ZLM 模拟器同步补齐两处保真度**（否则这些缺陷会被掩盖）：hook 回调现在会附加
自身配置里的 `hook.admin_params` 作为查询参数（真实 ZLM 行为），并且
`openRtpServer`/`closeRtpServer` 会相应地创建/移除流。

### 出站对话生命周期、无人观看关流与三处「静默 0 值」缺陷（2026-09-12 第十三轮）

第十二轮解决了"事件能不能被分派、载荷能不能解析"；这一轮追"事件之后**有没有真的产生效果**"，
结果发现：**整个"停止推流"链路从来没有真正生效过**，而且有四处缺陷只会表现成
"字段值恒为 0/恒为空"，接口全是 `200 {"code":0}`。

#### 1. 出站 INVITE 不登记业务会话 → BYE 永远发不出去

仓库里有两张会话表，职责不同：

* `SessionManager`（`sip/gb28181/invite.rs`）：发 ACK 用的底层事务上下文；
* `InviteSessionManager`（`sip/gb28181/invite_session.rs`）：**业务会话表**，
  记录"谁在拉哪个通道、走哪个 ZLM 流、对话的本地/对端 tag"。

只有**入站** INVITE（设备呼入的 Talk/Broadcast）写业务表；出站
Play / Playback / Download 一律不写。后果是实打实的：

| 现象 | 根因 | 修复 |
|------|------|------|
| `/api/play/stop` 只关 ZLM 的 RTP 端口，**从不给设备发 BYE**（设备会继续往已关闭的端口推流），日志只有一句 `warn: No active invite session` | `send_session_bye()` 依赖 `InviteSessionManager.get_by_device_channel()`，而出站会话从未登记 | 新增 `SipServer::register_outbound_invite()`，在 Play / Playback / Download 三条出站路径统一登记；会话统计、`active_channel_count` 随之恢复正确 |
| BYE 的 `From` tag 与 INVITE 不一致、`To` 缺对端 tag、`CSeq` 硬编码 `BYE 1`（与 `INVITE 1` 相等） | RFC 3261 §12.2.2 要求对话内请求必须复用 `Call-ID + 本地 tag + 远端 tag` 三元组，CSeq 必须**严格递增** | `InviteSession` 增加 `local_tag` / `remote_tag` / `invite_cseq`；200 OK 到达时由 `handle_response` 回填对端 tag（`extract_sip_tag`）并置 `Active`；BYE 用 `cseq_header(invite_cseq + 1, "BYE")` |
| `send_download_invite()` **完全不登记** `session_manager` 上下文 → 收不到 ACK（国标三次握手缺一环），设备不会推回放文件；下载也没有业务会话 | 只有 INVITE 发出、没有上下文与业务登记 | 补登记（`create` + `set_invite_context` + `register_outbound_invite`）；`send_playback_invite` 的 `set_invite_context` 此前用空占位（ACK 的 Request-URI 会是 `sip:@ip:port`），改为先 `create` 带上真实 device/channel |

**证据（SIP 设备模拟器现在会按 RFC 校验 BYE，失败即回 `481`）**：
模拟器起初对任何 BYE 都回 200 OK，等于给"假 BYE"盖章。改成校验
`From tag` / `To tag` / `CSeq` 后立刻抓到真实缺陷：

```
修复前：BYE CSeq=1, INVITE CSeq=1  → errors=['cseq-not-incremented'] → 481
修复后：BYE 对话校验通过 call_id=play_…_1789156507418
        BYE report: {'total': 2, 'valid': 2, 'invalid': 0}
```

#### 2. `CSeq` 头的**序号与方法顺序颠倒了**（10 处）

RFC 3261 §20.16 的文法是 `<digits> <method>`（`CSeq: 2 BYE`）。
此前 10 处写成 `format!("BYE {}", n)` / `"INVITE 1".to_string()`，
生成出 **`CSeq: BYE 2`** 这种非法头：

* 严格实现按语法错误丢弃请求；
* 宽松实现（含本项目自己的 `transaction.rs`，它取 `cseq_parts[1]` 当方法名）
  取不到序号 → BYE 被当成乱序请求 → `481`。

修复：新增 `fn cseq_header(num: u32, method: &str)`，全仓 18 处统一走它，
**不允许调用方拼字符串**。模拟器的 `parse_cseq` 同步改为解析失败即报
`MalformedCSeq`（此前 `except: return 1` 把非法头伪装成"序号没递增"），
实测修复后 `CSeq 头非法` 计数为 0。

#### 3. `on_stream_none_reader`：从"只打日志"到真正关流

官方文档明确该事件"可以选择是否关闭无人观看的流"，响应体是**顶层**
`{"code":0,"close":true|false}`。此前本项目：

* 把响应包成 WVP 信封 `{"code":0,"msg":"成功","data":{…}}` → `close` 埋在
  `data` 里，ZLM **读不到**，按默认 `false` 处理；
* 复用 `StreamChangedData` 解析载荷，而它的 `register: bool` 是必填的，
  而该事件的真实 body **没有** `regist` → **反序列化必然失败、整个分支被静默跳过**；
* 不向设备发 BYE、不关 ZLM 收流端口。

现在：**所有 hook 响应改为顶层扁平 JSON**（`code` / `close` / `msg`，
新增 `hook_ok_response` / `hook_error_response` / `none_reader_response` 并加单测），
新增专用 `StreamNoneReaderData`（字段全容忍），并实现决策函数
`decide_idle_stream()`（先否决后放行，全部基于可观测事实）：

| 判定顺序 | 条件 | close |
|---|---|---|
| 1 | 不是国标流（`push_*` / `proxy_*`） | false |
| 2 | ZLM 报告仍有 reader（`getMediaInfo` 的 `readerCount`/`totalReaderCount`） | false |
| 3 | ZLM 正在录像（`isRecording`） | false |
| 4 | 正在向级联上级平台推流（`SendRtpManager.get_by_channel`） | false |
| 5 | 平台下发的 `Record` 云录像指令生效中 | false |
| 6 | 其余国标流 | **true** → 发 BYE + `closeRtpServer` |

第 2 条同时解决了"hook 与播放器连接之间的竞态"：`/api/play/start` 先开流、
前端随后才挂播放器，这段时间里无脑 `close=true` 会把刚建好的流掐掉。
第 2/3 条**查不到时保持不关**（关流是破坏性动作，确认不了就不做），
而不是像以前那样把"查不到"当成"没人看"。

**实测**（真实服务 + ZLM 模拟器按 ZLM 语义执行响应，`close:true` 即真的下架流）：

```
A) readers=2        -> {"close":false,"msg":"仍有观看者（reader=2/2）"}
B) 录像中           -> {"close":false,"msg":"ZLM 正在录像"}
C) push_x（非国标） -> {"close":false,"msg":"非国标流（推流/拉流代理），交由用户停止"}
D) 无人观看         -> {"close":true}  → 模拟器下架该流（getMediaList 由 1 条变 0 条）
                                        → 后端发 BYE，设备校验通过（valid=1/invalid=0）
```

#### 4. `on_stream_not_found`：删掉编造的 RTSP 拉流地址

该事件是"播放器请求了不存在的流"的通知（官方文档：**不影响 ZLM 行为**）。
此前在 INVITE 之后还有一段"自动拉流"兜底，用
`rtsp://{device_id}:8554/{channel_id}` 调 `addStreamProxy` —— 这是**编造出来的地址**：
国标设备不会在 8554（那是 ZLM 自己的 RTSP 端口）提供 RTSP 服务，设备编号也不是主机名。
该分支只会稳定失败并掩盖真正原因，已删除。同时把两处按需拉流统一到新的
`SipServer::start_live_stream()`（`openRtpServer` → 用真实端口发 INVITE →
等媒体 → 端口不一致时 `connectRtpServer`），修掉了此前两处都传
**`media_port = 0`** 的问题（`m=video 0` 表示媒体被禁用，设备无处可推，
"自动重连/按需拉流"因此永远是空转）：

```
On-demand pull started: stream=…_… device=… channel=…
请求 SDP: ['m=video 30000 RTP/AVP 96']     ← 修复前是 m=video 0
```

#### 5. 三处「静默 0 值」（都不报错，只是字段永远是 0/空）

| 缺陷 | 后果 | 修复 |
|------|------|------|
| **`MediaInfo` 缺 `rename_all = "camelCase"`** | ZLM 实际返回 `readerCount`/`totalReaderCount`/`originType`/`createStamp`/`aliveSecond`/`bytesSpeed`（内层 `tracks` 却是 snake_case）。字段名对不上 → 整条 `MediaInfo` 反序列化失败 → `getMediaList`/`getMediaInfo` **永远报错或全 0**；表现为"观看者数量永远是 0"（正好会把正在播放的流当成无人观看关掉）、负载均衡流数永远为 0 | `MediaInfo` 加 `rename_all="camelCase"` + 各字段 `serde(default)`；`TrackInfo` **保持** snake_case；补两条真实载荷的解析单测 |
| **`AppState::get_zlm_client(Some(未知id))` 返回 `None`** | hook 载荷里的 `mediaServerId` 是 **ZLM 自己的 `general.mediaServerId`**，与本地节点主键不保证一致（默认 `your_server_id`）。于是按 id 一律查不到节点 → 无人观看的 reader/录像校验被整段跳过、flow report 的流数量"保持既有值"、hook 鉴权取不到 secret —— 全都不报错 | 未知 id **回落到默认节点**（与 `get_zlm_client_auto` 同一思路）并打 debug 日志 |
| **`isMediaExist` / `isRecording` 用 `ApiResponse<{exist}>` 解析** | ZLM 的"简单 API"用 `throw ApiRet("exist", …)` 返回**顶层** `{"code":0,"exist":true}`，不在 `data` 里 → 两个方法在真实 ZLM 上**恒为 false**（只有返回 `data:{exist:…}` 的 mock 上碰巧正确） | 新增 `exist_flag()` 兼容顶层/`data` 两种形态并加单测；`is_media_exist` / `is_recording` 改用它 |

另外**必须把 ZLM 的 `general.mediaServerId` 设成本平台节点主键**（WVP 的
autoConfig 也这么做）：`configure_zlm_hooks()` 现在会下发该键，否则上面第 2
条的"回落到默认节点"只是兜底，多节点部署下会把 A 节点的通知记到 B 节点账上。
实测：保存节点后 ZLM 的 `general.mediaServerId` = `media_server_1789156913766`，
flow report 正确写入该行（`total_bytes=2097152`，`stream_count=Some(1)`）。

#### 6. `media_server/check` 把「无人观看延迟」当成「流媒体流 IP」

`streamIp` 取的是 `general.streamNoneReaderDelayMS`（默认 `20000`）。这个键
**永远有值**，于是：

1. 前端"添加流媒体节点"表单的**流媒体流IP**被自动填成 `20000`；
2. 保存后写进 `gb_media_server.stream_ip`；
3. `SipServer::new` 把它交给 `NatHelper` 当作对外流媒体 IP，
   SDP 里就是 `c=INET IP4 20000` —— 设备往非法地址推流，播放必然失败，
   而日志上看不出任何异常。

ZLM 的 `config.ini` 里**没有**"流 IP"这个配置项（`[general]` 只有
`enableVhost` / `flowThreshold` / `maxStreamWaitMS` / `streamNoneReaderDelayMS` /
`resetWhenRePlay` / `mergeWriteMS` / `mediaServerId` / …），因此这里不再猜键，
返回空串交由兜底逻辑填成节点配置 IP。映射逻辑抽成纯函数
`media_server_probe_fields()` 并加 3 条单测（含"绝不等于 20000"的回归断言）。
实测：mock 现在会返回 `general.streamNoneReaderDelayMS=20000`，
而接口返回 `streamIp="127.0.0.1"`。

#### 7. MediaWaiter 的竞态（顺带修复）

`openRtpServer` 在 ZLM 内部创建流时就会触发 `on_rtp_server_started`，
而调用方要等这个 HTTP 响应回来才注册等待者 —— 通知先到就**被直接丢弃**，
表现为"媒体明明已就绪，播放请求却干等 15 秒超时"。现在
`resolve_by_stream` 在找不到等待者时把通知**暂存 30 秒**，
`register` 命中即立刻完成（`EARLY_READY_TTL` + `prune_early_ready()` 定期清理），
补 3 条单测覆盖"早到不丢""只对同一条流生效""过期清理"。

#### 8. 模拟器保真度（否则以上缺陷都会被掩盖）

| 模拟器 | 改动 |
|--------|------|
| SIP 设备 | BYE **对话校验**（From tag / To tag / CSeq 递增），失败回 `481` 并落盘报告；`parse_cseq` 解析失败即报错（不再兜底成 1）；`--auto-bye-secs`（默认 3，测试平台侧 BYE 时用 0）；INVITE 日志打印请求 SDP 的 `m=` 行（`m=video 0` 一眼可见） |
| ZLM | `isRecording`/`isMediaExist` 改回真实的**顶层** `exist`；MediaInfo 返回完整字段集（含 `readerCount`）与真实 camelCase；`getServerConfig` 增加 `general.streamNoneReaderDelayMS` / `protocol.auto_close` / `rtp_proxy.sdp_ip`；hook 载荷的 `mediaServerId` 取 `general.mediaServerId`（不再写死）；`on_stream_none_reader` 触发器**按 ZLM 语义执行**响应（`close:true` 即下架流）；新增 `on_flow_report` 触发器（载荷里刻意不含流数量字段）；新增 `/debug/hook_responses` 让"后端到底回了什么"可查 |

#### 第十三轮基线

```
cargo test                      533 passed / 0 failed   (上轮 510)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
```

### SIP TCP 信令：三层缺陷叠加导致「TCP 设备完全无法接入」（2026-09-12 第十四轮）

上一轮修好出站对话后，进一步追问"传输通道对不对"，结果发现 **SIP over TCP
在真实环境里从来没有工作过**，而且是三层缺陷叠在一起，任何一层单独修都看不到效果：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **TCP 监听器从未启动** | `SipServer::tcp_enabled` 硬编码 `false`，`set_tcp_enabled()` **全仓无调用点**；`SipConfig` 里也没有对应配置项 —— 配置 `tcp_port = 5061` 形同虚设，TCP 的解析/分帧/上下文处理代码全是死代码 | `SipConfig` 增加 `tcp_enabled`（默认 `true`）并在 `lib.rs` 显式接线 `server.set_tcp_enabled(...)`；启动日志打印实际监听地址 |
| 2 | **`Display` 只写 `\n`，TCP 路径丢光所有头字段** | TCP 读循环把消息 `format!("{}", msg)` 重新序列化后再交给 `handle_packet`，而 `Parser::parse_request` 按 `"\r\n"` 切分 → 一行都切不开 → `From`/`To`/`CSeq` 全部为空，日志 `REGISTER: Cannot extract device ID - from="" to="" cseq=""` | `Display for SipRequest/SipResponse` 改按 RFC 3261 §7 输出 **CRLF**；并让 TCP 循环直接把**原始字节**交给 `handle_packet`（`TcpReader::read_message` 同时返回 raw），不再依赖序列化实现 |
| 3 | **读循环与响应发送死锁** | 读循环 `conn.write().await` 一直持有到连接结束，而 `send_response → send_to` 要拿同一把写锁 → 平台连 401 挑战都回不出去（实测：探针 REGISTER 后 5 秒无任何响应，后端日志停在 `TCP connection from`） | 把连接**拆成读/写两半**：`TcpListener::accept` 返回 `(TcpReader, OwnedWriteHalf, addr)`，写半交给 `TcpConnectionManager`（`Arc<Mutex<OwnedWriteHalf>>`），读半留在读循环 —— 两者不共用锁 |

顺带修掉两个相关缺陷：

- **`Display`/`generate_request` 会输出重复的 `Content-Length`**：解析出来的头里
  本来就有它，序列化时又按 body 长度写一个。接收方对"以哪个为准"可以有不同解释
  （分帧歧义）。现在统一由生成方写一次，并跳过调用方传入/已有的 `Content-Length`。
- **出站请求的传输选择**：新增 `send_sip_out()`（精确地址 → 同 IP 且**唯一**连接
  → UDP）。此前只有**响应**按 RFC 3261 §18.2.2 回到 TCP 连接，所有**出站请求**
  （INVITE/ACK/BYE/MESSAGE/INFO/SUBSCRIBE）以及设备心跳都硬编码走 UDP ——
  TCP 设备"平台发了没反应，日志却显示已发送"。同 IP 有多条连接时**不做**兜底
  （同一 NAT 出口常挂多台设备，猜错会把给 A 的 BYE 发到 B 的连接上）。
  另外：经 TCP 发出的请求**不再登记重传事务**（RFC 3261 §17.1.2 可靠传输不重传，
  否则事务层会用 UDP 重发同一个请求）。

**实测证据**（新增 `mock/tools/sip-device/tcp_register_probe.py`：纯 TCP 探针，
复用 `sip_device_mock.py` 的构造函数，避免第二套 SIP 实现漂移）：

```
[probe] TCP connected to 127.0.0.1:5060 from port 53622
[probe] -> REGISTER ...            [probe] <- SIP/2.0 401 Unauthorized
[probe] -> REGISTER ...（带摘要）   [probe] <- SIP/2.0 200 OK
[probe] TCP REGISTER OK（平台应已登记该 TCP 连接）
（触发 /api/device/query/devices/{id}/sync）
[probe] <- MESSAGE sip:34020000001320000001@127.0.0.1:53687 SIP/2.0   ← 平台主动请求走了 TCP
[probe] -> 200 OK for MESSAGE
[probe] PASS: 平台出站请求确实走了 TCP（收到 ['MESSAGE']）
```

修复前的同一探针：`REGISTER 无响应`（平台侧 `TCP connection from` 之后没有任何日志）。

**同时补齐的回归测试**（防止这四类问题再次静默复发）：

- `sip::core::message::wire_format_tests`：`Display` 必须 CRLF、往返后头字段不丢、
  `Content-Length` 与实际 body 一致且不重复
- `sip::transport::tcp::outbound_transport_tests`：有 TCP 连接时走 TCP 且 **UDP 侧收不到**、
  无连接时如实回落 UDP 并返回 `false`、连接不存在时 `send_to` 返回 `Ok(false)`
- `tests/config_toml_smoke.rs`：`tcp_enabled` 必须为真、`tcp_port` 与 UDP 同端口

#### 第十四轮基线

```
cargo test                      539 passed / 0 failed   (上轮 533)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
TCP 探针（纯 TCP 注册 + 平台主动请求）  PASS
```

### 回放 / 录像下载的真实链路核验 + 配置结构体去重（2026-09-12 第十五轮）

前两轮修的是"信令与传输"，这一轮把**回放**与**录像下载**两条完整业务链真正跑一遍
（真实服务 + ZLM 模拟器 + SIP 设备模拟器，全部按真实形态交互），发现下载链路
"看起来成功、实际不产生任何文件"：

| 问题 | 证据 | 修复 |
|------|------|------|
| **GB28181 录像下载从不落盘** | `gb_record_download_start` 只调了 `openRtpServer` 就发下载 INVITE —— ZLM 的 `openRtpServer`**只创建流、不录制**（录制需要 `startRecord` 或全局 `protocol.enable_mp4`）。于是设备把录像 RTP 推到端口后什么都不会写：`isRecording` 恒为 `false`，`on_record_mp4` 永不触发，云录像里永远没有这条下载 | 收到 200 OK 后按 ZLM 实际注册的 app 调 `startRecord(type=1)`；`gb_record_download_stop` 里**先 `stopRecord` 再 `closeRtpServer`**（ZLM 只在停止录制时写完 MP4 尾部并回调）。实测 `isRecording=true` → 停止后 `false` |
| **下载进度永远 `unknown`** | `gb_record_download_progress` 一律去查 ZLM 的"下载列表"，而那里只有 ZLM 自己发起的 HTTP 拉流下载；GB28181 下载的会话不在其中 → 落到底部固定返回 `status:"unknown"`，前端看不到任何状态变化 | 按传输分流：`gb28181://` 会话直接报会话状态机（`inviting → downloading → completed`）与字节数；`zlm-local` 才查下载列表；会话存在但列表暂无时也报会话状态而**不是**伪造 `unknown` |
| **下载会话的终态缺失** | `on_stream_changed` 只处理了 `register=true`（置 downloading）；设备推完流**注销**时什么都不做 → 会话永远停在 `downloading` | `register=false` 且 stream 属于下载会话 → `completed` / progress=100（配 `on_record_mp4` 落库） |

**实测（关键片段）**：

```
POST/GET /api/gb_record/download/start/...   -> status=inviting, transport=gb28181
日志: 下载流已开始 MP4 录制 app=rtp stream=download_...
isRecording?                                 -> {"code":0,"exist":true}     ← 修复前恒为 false
/progress（进行中）                          -> status=downloading, transport=gb28181
触发 on_stream_changed regist=false          -> 日志 Download stream finished
/progress（完成后）                          -> status=completed, progress=100.0
/stop                                        -> 发 BYE（设备校验通过 valid=1）+ stopRecord
isRecording?                                 -> {"code":0,"exist":false}
```

同轮一并核验通过的还有**回放链路**（此前只测到"能发出 INVITE"）：

```
/api/gb_record/query/...        -> count=20（多包 RecordInfo 解析正确，5 包 × 4 条）
/api/playback/start/...         -> Sent PLAYBACK INVITE ... m=video 30000 → ACK → ZLM media ready
                                   返回 playUrl/flvUrl/hls + source=gb28181_playback_invite
/api/playback/stop/...          -> BYE CSeq=2（对话内正确递增），设备校验通过
```

**顺带清理**：删除 `src/sip/config.rs` —— 它定义了第二套 `SipConfig` /
`ZlmServerConfig` / `ZlmConfig`（96 行），全仓**无人使用**，只在 `sip/mod.rs`
里被 re-export。两个同名结构体是真实的踩坑源：上一轮的 `tcp_enabled` 缺陷
就是因为我先改错了这个副本（改了不生效）。现在配置只有 `src/config.rs` 唯一一份。

**模拟器保真度**：ZLM mock 的 `on_stream_changed` 触发器改为尊重
`stream`/`app`/`register` 查询参数（此前写死 `stream=34020000001320000001`、
`register=true`），否则"设备推完下线"这类状态机分支永远无法触发。

#### 第十五轮基线

```
cargo test                      541 passed / 0 failed   (上轮 539)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
回放链路（RecordInfo → INVITE → 媒体就绪 → BYE）  PASS
录像下载链路（INVITE → startRecord → 完成 → stopRecord/BYE） PASS
```

### 级联（上级平台）注册：从未成功过 —— 四层缺陷叠加（2026-09-12 第十六轮）

核验"上级平台"这条 WVP 头等功能时发现：**级联注册在真实环境里从未成功过**。
四层缺陷叠加，每修好一层才露出下一层：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **平台键用错字段** | `load_platforms_from_db` 用 `device_gb_id` 当 `platform_id`，而前端平台表单**从不提交**该字段（`Platform` 类型里根本没有它）⇒ 键恒为**空串**；而 REST API（`/api/platform/exit/:id`）与响应路由用的是 `server_gb_id` —— 两边永远对不上 | 平台键统一为**上级平台国标 ID**（`server_gb_id`），`load_platforms_from_db` / `reload_from_db` 同步 |
| 2 | **响应路由把 nonce 当平台 ID** | `call_id.strip_prefix("cascade_").rsplit('_').next()` 从 `cascade_{platform_id}_{nonce}` 里抠出的是 **nonce**。实测日志：`Cascade f3eec4303c372b0a received 401 challenge` —— 后面那串是 nonce，于是 `handle_401_challenge` / `mark_registered` / `mark_failed` 全部作用在不存在的平台上 | 注册状态新增 `last_call_id`，`build_register_request` 回填；新增 `resolve_platform_from_call_id()` 取代字符串切分 |
| 3 | **挑战后不再重发** | `handle_401_challenge` 只把状态置为 `Challenged`，而周期注册循环的筛选条件是 `NotRegistered / Failed / (Registered 且过半程)` —— **不含 `Challenged`** ⇒ 收到 401 之后永远不再发第二个 REGISTER（`register_ok` 恒为 0） | 收到 401 后**立即**带摘要重发（`register_now`）；同时把 `Challenged`（以及卡住 >5s 的 `Registering`）纳入周期重试 |
| 4 | **两份互相矛盾的 REGISTER 实现** | `SipServer::register_to_platform` 另写了一份：Request-URI 是 `REGISTER sip:{}:{}`（缺 host，实测发成 `REGISTER sip::5062`）、并且**预置假 nonce 的 `Proxy-Authenticate`** 当鉴权（真实上级平台只会因此拒绝）；它和注册器的状态机互相矛盾 | 删除该实现（含 `unregister_from_platform`、随之无用的 `compute_digest_auth`）；`handler` 改为 `registrar.upsert_platform_from_db()` + `registrar.register_now()`，注销走 `unregister_and_remove()`；新增 `SipServer::cascade_registrar()` 仅暴露传输能力 |

另外：注册用 **CSeq 计数器**（挑战重发必须递增，否则上级视作重传）；
`register_now` 走 `send_request_to`，因此 **TCP 信令的上级平台同样可用**。

**模拟器补齐**：`cascade_mock.py` 新增 `--require-digest`（先回 401 挑战、
校验 `Authorization` 摘要，未带凭据 → 401，摘要错 → 403）与 `--report`
（注册/注销/消息/INVITE 记账落盘）。此前的 mock 无条件回 200 OK，
"预置假 nonce"这种假实现因此从未被发现。

**实测（完整生命周期）**：

```
添加平台 → REGISTER sip:34020000002000000099@127.0.0.1:5062   ← Request-URI 正确
         ← 401 Unauthorized (nonce=0139…)
         → REGISTER（Authorization: Digest …, CSeq 2）
         ← 200 OK
报告: register_challenged=1, register_ok=1, devices=["34020000002000000001"]
      keepalive MESSAGE / Catalog MESSAGE 均到达上级
禁用平台 → REGISTER (Expires: 0) → 上级"设备注销" → report.unregister=1
```

**新增回归测试**：`test_platform_key_and_call_id_resolution`（键与 Call-ID 反查）、
`test_401_challenge_produces_authorization_header`（首次不得预置
`Proxy-Authenticate`；挑战后必须带 `Authorization`/`nonce`/`uri`，CSeq 递增）。

> 同时修正一处 DashMap **自死锁**：`build_register_request` 里
> `self.states.get(platform_id)?` 的读锁尚未释放就 `get_mut` 回填
> `last_call_id` —— 同一分片的读写锁互斥，`cargo test --lib cascade` 会**永远挂住**。
> 现改为先 `clone()` 再释放读锁。

#### 第十六轮基线

```
cargo test                      543 passed / 0 failed   (上轮 541)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
级联完整生命周期（401 → 摘要注册 → keepalive/Catalog → 注销） PASS
```

### 入站 INVITE 的伪造 RTSP 拉流分支（2026-09-12 第十七轮）

与第十三轮在 `on_stream_not_found` 里删掉的是**同一类**假实现，位于
`handle_invite`（设备/上级 INVITE 平台）里：

```rust
// 修正前
let add_proxy_req = AddStreamProxyRequest {
    url: format!("rtsp://{}:{}/{}", device_ip, device_port, channel_id),
    ...
};
```

`device_ip:device_port` 取自对方 SDP 的 `m=video` 端口（设备准备**推** RTP 的端口），
国标设备并不会在那个端口上提供 RTSP 服务，设备编号也不是主机名。于是这条
`addStreamProxy` **必然失败** → `error_occurred = true` → 平台对来电方回
**503 Service Unavailable**：连"设备/上级呼入"这条路径都走不通。

正确语义（RFC 3261 §13 + 国标）：对方 INVITE 平台是它要**推流给平台**（设备呼入）
或**上级平台点播本级**，平台只需用自己刚开的 RTP 收流端口应答 200 OK，
不需要反向拉流。现已删除该分支，入站 INVITE 直接以本机 RTP 收流端口应答 200 OK。

#### 第十七轮基线

```
cargo test                      543 passed / 0 failed
cargo check --all-targets        warnings 0
```

### 上级平台点播本级（级联拉流）接线 + 对外通告 IP + stopSendRtp（2026-09-12 第十八轮）

第十七轮定位的"级联拉流未接线"本轮完成实现与端到端验证，同时修掉两个相关缺陷。

#### 1. 级联拉流：从"只有测试调用"到完整链路

此前 `SipServer::register_cascade_invite()`（解析上级 SDP → 预登记 SendRtp 会话）
**只有测试调用、运行时没有任何调用点**，也没有"判定来电方是已注册上级平台"的分支
—— 上级 INVITE 过来只会被当成"设备呼入"，连 200 OK 的 SDP 都不对。

现在 `handle_invite` 里新增独立分支：

| 步骤 | 实现 |
|------|------|
| ① 判定上级身份 | `db_platform::get_by_server_gb_id(pool, from_device)` 命中即认为是上级平台（与设备呼入彻底分流，不污染设备会话表） |
| ② 取本级通道 | 优先 `Subject` 第 1 段（`<通道>:<ssrc>,<上级>:0`），回落 Request-URI；新增 `db_device::get_channel_by_channel_id()` 反查通道 → 所属设备 |
| ③ 分配发送端口 | 为本次级联开一个专用 RTP 端口（`cascade_{platform}_{channel}`），**应答 SDP 的 `m=` 与 `startSendRtp` 的 `src_port` 用同一个**（否则上级看到的源端口与 SDP 通告不一致，会被丢弃） |
| ④ 应答 | 100 → 180 → 200 OK，SDP 用本级对外地址与上一步端口 |
| ⑤ 预登记 SendRtp 会话 | `SendRtpManager.handle_upstream_invite()`（`cascade_{platform}_{channel}`） |
| ⑥ 拉起设备流并推流 | 静态方法拿不到 `&self`，因此**入队** `CascadePullRequest`；`lib.rs` 里以 `Arc<SipServer>` 起的后台任务（500ms 轮询）调 `start_live_stream()` 拉起设备流，成功后再 `startSendRtp()` 推给上级（`use_ps=true`：国标级联要 PS 封装） |

**实测**（真实服务 + ZLM 模拟器 + 级联模拟器主动发 INVITE）：

```
上游 INVITE(Subject=34020000001320000001:0200000001,...)
→ 200 OK，应答 SDP: o=- 0 0 IN IP4 192.168.3.149 / c=IN IP4 192.168.3.149 / m=video 30000 / y=0200000001
→ 级联点播：platform=34020000002000000099 通道=34020000001320000001 设备=34020000001320000001 → 上级 127.0.0.1:20000 ssrc=0200000001 本端发送端口=30000
→ 设备侧收到点播 INVITE（m=video 30001，按需拉起）
→ startSendRtp stream=34020000001320000001_34020000001320000001 -> rtp://127.0.0.1:20000 ssrc=0200000001 src_port=30000
（上游 BYE）→ 级联 BYE（按通道兜底）：通道 … 关闭了 1 个 SendRtp 会话
              → ZLM stopSendRtp existed=True
```

BYE 兜底是必要的：上级 BYE 用的是**它自己的对话 Call-ID**，与本地登记的
`cascade_{platform}_{channel}` 不一致，只按 call_id 查会漏掉 —— "上级已挂断、
平台还在推流"。现在按 Request-URI 的通道兜底关闭。

#### 2. `stopSendRtp` 一直在选错会话（推流从未真正停止）

`ZlmClient::stop_send_rtp(vhost, app, **stream**)` 的两个调用点传的却是 **ssrc**：
ZLM 去找名为该 ssrc 的流，找不到就返回错误 —— 于是"停止级联推流"从未生效
（实测 mock 侧 `existed=False`，上游会一直收到 RTP）。

现新增 `stop_send_rtp_ex(vhost, app, stream, ssrc)`（ZLM 允许二者任一选中会话，
两个都给最稳妥），`SendRtpSession` 增加 `zlm_stream_id` 并在 `startSendRtp`
成功后回填，BYE 时用 `stream + ssrc` 一起定位。实测 `existed=True`。

#### 3. 对外通告 IP：`sip.ip=0.0.0.0` 被原样写进 Via/Contact/SDP

`sip.ip` 同时承担"绑定地址"和"对外通告地址"两个角色，默认 `0.0.0.0` 时
Via / From / Contact / SDP 的 `c=` 全是 `0.0.0.0` —— 设备与上级平台无处回包、
无处收流。实测级联应答 SDP 就是 `c=IN IP4 0.0.0.0`。

现在启动时做**角色分离**：

* `ip`（对外通告）= 配置值；若配置为通配地址且未显式配 `sdp_ip`/`stream_ip`，
  用 "UDP connect 8.8.8.8 让内核选路 + `getsockname`"（RFC 6724 思路）解析出
  真实出口 IP；
* 新增 `SipConfig.bind_ip`（`#[serde(skip)]`）保留原来的通配地址用于**绑定** ——
  否则只监听一块网卡：本机回环、其它网段的设备都连不上，DHCP 换 IP 还会启动失败。

实测：`sip.ip=0.0.0.0 是通配地址：对外通告用 192.168.3.149，仍绑定 0.0.0.0`，
应答 SDP 的 `c=` 变为 `192.168.3.149`。

#### 4. 模拟器与测试

* 级联 mock 新增 `--control-port`：`/trigger/invite` 让 mock **主动**发 INVITE
  （SDP 给出自己的收流地址），`/report` 返回记账（含**收到的** 200 OK 与应答 SDP）。
  此前 mock 只会应答 INVITE，"上级点播本级"这条链路根本无法验证。
* 新增单测：`Subject` 通道解析（含空段/无 Subject/仅一段）、拉流队列出入队保真。

#### 第十八轮基线

```
cargo test                      545 passed / 0 failed   (上轮 543)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
级联拉流（上级 INVITE → 200 OK SDP → 按需拉流 → startSendRtp → BYE stopSendRtp）PASS
```

### JT1078 入站链路：从未处理过任何真实终端消息 + 三处吞结果（2026-09-12 第十九轮）

顺"还有哪些吞掉结果的命令下发"查 `let _ =` 时，发现 JT1078 的**入站链路整体没接线**：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **`process_jt_message` 没有任何运行时调用点** | 两个监听器都只调 `feed_bytes` + `process_payload_for`，后者处理的是**自造的** `AUTH:x`/`HEARTBEAT` 文本协议，只做分类不做解析。于是：0x0100 注册不落库、不回 0x8100、终端永远注册不上；0x0001 通用应答不匹配命令等待器 ⇒ **所有 `*_and_wait` 命令必然超时** | 新增真实 JT/T 808 帧解析 `frame::parse_jt808_frame` / `split_jt808`（`7E` 定界、`7D 02/7D 01` 反转义、XOR 校验、按体属性 bit13 处理分包），两个监听器都改为解析 → `process_jt_message` → 按消息类型回 0x8100 注册应答 / 0x8001 通用应答 |
| 2 | **`send_raw` 每条命令新建临时 UDP 端口** | 终端看到的**来源地址**不是它配置的服务器地址；按源地址过滤的终端（含本仓库模拟器，它 `connect()` 到平台）**一条命令都收不到**，平台侧全部超时 | `Jt1078Manager` 增加 `send_socket`，由 `server::start` 注入**监听 socket**，下发共用同一端口（与真实平台一致） |
| 3 | **`let _ =` 吞掉命令结果** | `link_detection` 丢掉位置查询结果后把 `online` 直接当成 `reachable`（离线/无会话也报"可达"）；`talk_start` 丢掉 0x9102 结果，即使终端拒绝也返回 success；`media_list` 丢掉检索失败后返回 ZLM 列表，看起来像"检索成功" | `reachable` 改为**真实下发结果**（并回传 error）；对讲控制失败上报；多媒体检索失败如实返回错误 |
| 4 | **注册不落库** | 终端只登记在内存 `terminal_addrs`，`gb_jt_terminal` 没有记录 ⇒ `/api/jt1078/terminal/list` **恒为空**，终端管理页看不到任何设备 | `Jt1078Manager` 注入连接池，注册/心跳时 `insert_terminal` / `update_terminal` + `update_terminal_status(online)` |
| 5 | **应答乒乓** | 平台对终端的 0x0001 通用应答又回 0x8001，模拟器也回 0x0001 ⇒ 双方无限互刷（实测日志刷屏） | 终端通用应答**不再回** 0x8001（命令匹配已在 `process_jt_message` 内完成） |

**模拟器保真度**（此前是"平台和 mock 一起错"）：

* 帧格式从自造（`SYNC+msg_id+attr+phone+seq+total+packet_no+reserved(6)+CRC16`、
  `byte^0x20` 转义）改为**国标格式**：`7E | msgId(2) 体属性(2) 手机号(6 BCD) 流水号(2)
  [分包(4)] 消息体 XOR(1) | 7E`，转义 `0x7E→7D 02` / `0x7D→7D 01`；
* 手机号从 `int(x).to_bytes(6)`（**二进制**，平台按 BCD 解码得到
  `00033=3=8<4>` 乱码）改为 **BCD 编码**；
* 注册消息体从自造 88 字节改为 **JT/T 808-2013 §8.8** 布局
  （省2+市2+制造商5+终端型号20+终端ID7+车牌颜色1+车牌GBK）；
* `MSG_REGISTER_ACK` 从 `0x0101` 改为 **`0x8100`**、`MSG_LIVE_STREAM` 从 `0x0102`
  改为 **`0x9101`**；解析注册应答从 `!HH`（4 字节）改为 `流水号(2)+结果(1)+鉴权码`；
* 对平台的其它 0x8xxx 命令统一回 0x0001 通用应答（真实终端行为），
  否则"平台能不能收到终端应答"这条链路无法验证。

平台的注册解析同步按 808-2013 修正（车牌颜色/车牌取代原先臆造的 ICCID 字段；
2019 追加的 ICCID/硬件版本**不猜偏移**，因为车牌不定长、无法定位其起点，
已在代码注释中说明）。

**实测（真实服务 + 终端模拟器）**：

```
终端 TX 0x0100（BCD phone=13912345678, body=45B）
平台 解析成功 → 登记终端 → 回 0x8100（流水号+结果0+鉴权码）
终端 RX 0x8100 → ✅ 注册成功          （修复前：无限"注册超时，重新注册"）
/api/jt1078/terminal/list            → 1 条记录（phoneNumber/plate/model/status=true）
/api/jt1078/link-detection           → {"online":true,"reachable":true}   ← 真实下发结果
/api/jt1078/snap                     → {"code":0,"msg":"抓拍命令已被终端应答"}  ← 命令往返成功
```

#### 第十九轮基线

```
cargo test                      552 passed / 0 failed   (上轮 545；+5 JT808 入站解析测试)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
JT1078 端到端（注册 → 落库 → 列表 → 链路检测 → 命令往返）PASS
```

### JT1078 终端录像检索（0x8802 / 0x0802）+ BCD 时间编码缺陷（2026-09-12 第二十轮）

上一轮把 JT1078 入站链路接通后，`/api/jt1078/media/list` 仍只能"确认请求已下发"
（终端结果 0x0802 未解析）。本轮补上这条链路，并顺带抓到一个**影响所有下发时间段**
的编码缺陷。

#### 1. `encode_time_bcd` 发的不是 BCD

`command::encode_time_bcd` / `bcd_from_utc` 此前返回的是**原始数值**字节：
2026 年 → `"%y"` = 26 → 字节 `0x1A`；而 JT/T 808 规定时间字段是 **BCD**
（每字节两位十进制：2026 → `0x26`、9 月 → `0x09`）。

后果：平台下发给终端的**所有时间段**（多媒体检索 0x8802、回放 0x9201、
文件上传 0x9205、回放控制 0x9202 的 seek）全是错的。而平台自己的
`parse_bcd_datetime` 是按半字节解码的 —— 收发两边一起错，
"平台内自测"因此完全看不出来。本轮写 0x0802 往返测试时才暴露
（我构造的 BCD 时间被解成 2026 之外的年份）。

现在 `bcd_from_utc` 输出真 BCD，并补了 `try_encode_time_bcd` 的单测期望值。

#### 2. 0x0802 多媒体数据检索应答

新增按 JT/T 808-2013 §8.19 的实现：

* `response_parser::MediaSearchItem` + `parse_media_search_response`：
  `多媒体数据总数目(2)` + N × 27 字节项
  （`媒体ID(4) 类型(1) 通道(1) 事件编码(1) 起始(BCD6) 结束(BCD6) 经度(4) 纬度(4)`）；
  经纬度 0/0xFFFFFFFF 视为无效；单项时间非法**跳过该条并继续**；
  末尾多余字节忽略；实际条数少于声明条数时告警但不丢已解析结果。
* `session`：`ParsedMessage::MediaSearchResult`（0x0802）。
* `Jt1078Manager`：检索结果缓存（带采集时间，`take` 即清空，避免复用陈旧结果）。
* `handlers/jt1078.rs::media_list`：下发 0x8802 后等待 0x0802（最多 5s），
  返回终端真实检索结果（`source: terminal_media_search`）；
  超时如实报错而不是回一个空列表冒充成功。
* 模拟器：`/0x8802 → 0x0802`（先通用应答，再回 2 条媒体项，时间为真 BCD）。

**实测**：

```
请求: POST /api/jt1078/media/list {"phoneNumber":"13912345678",...}
终端: RX 0x8802 → 回 0x0001 通用应答 + TX 0x0802 (body_len=56 = 2 + 2×27)
后端: JT1078 收到多媒体检索应答 phone=13912345678 items=2
响应: {"code":0,"data":{"list":[{"mediaId":1001,"mediaTypeName":"video",
        "channelId":1,"startTime":"2026-09-01 10:00:00","endTime":"2026-09-01 10:05:00",
        "longitude":116.397,"latitude":39.909}, {"mediaId":1002,"mediaTypeName":"image",
        "longitude":null,"latitude":null}],"total":2,"source":"terminal_media_search"}}
```

#### 3. 顺带核实（无需改动）

* `RtpPlaylistData` / `RecordProgressData` / `SendRtpProgressData`
  （`on_rtp_playlist` / `on_record_progress` / `on_send_rtp_progress`）
  **所有字段都是 `Option`**，载荷裁剪不会导致事件被静默丢弃 ——
  与"字段必填导致整条事件解析失败"的旧缺陷不同，本轮确认无需改动。

#### 第二十轮基线

```
cargo test                      556 passed / 0 failed   (上轮 552；+4 0x0802 解析/缓存测试)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
JT1078 终端录像检索（0x8802 → 0x0802 → API 返回终端结果）PASS
```

### 又一批「假成功 / 假数据」端点 + 前端契约不匹配（2026-09-12 第二十一轮）

这一轮用"找没有 await / 没有 DB 调用的 handler"的启发式扫描，再加上**按前端真实
载荷**逐个调用，又挖出 5 处：

| 端点 | 问题 | 修复 |
|------|------|------|
| `GET /api/sy/camera/control/play` | 注释写着"转调 play_start"，代码**只打日志**然后返回 `status:"started"` —— 设备既没收到 INVITE，ZLM 也没开收流端口 | 真正转调 `play::play_start` |
| `GET /api/sy/camera/control/stop` | 同上（注释说转调 play_stop，实际什么都没做，也不发 BYE） | 真正转调 `play::play_stop` |
| `GET /api/sy/camera/control/ptz` | 同上（注释说转调 PTZ，实际没下发 DeviceControl） | 真正转调 `device_control::device_ptz` |
| `POST /api/rtp/send/stop/:stream_id` | 只回一句 `"SendRtp stop is implicit on stream teardown"` 的**假成功**，既不查会话也不调 ZLM —— 调用方以为推流已停，ZLM 仍在往目标推 RTP | 真正调用 `stopSendRtp`（stream + ssrc 双选择器；ssrc 可从级联会话兜底取） |
| `GET /api/sy/camera/list/ids` | **完全不查库**：对每个入参 deviceId 直接返回天安门坐标 `39.9042/116.4074` 和编造的名称 `Camera-<id>` | 按设备查 `gb_device_channel`，返回真实通道名/经纬度/在线状态 |
| `GET /api/jt1078/terminal/channel/one/{id}` | 只回"请使用主 handler ..."的提示，而**路由指的就是它自己** ⇒ 永远拿不到数据 | 按 `gb_jt_channel.id` 查库返回；id 非法/不存在如实报错 |

**前端契约不匹配（同一类缺陷的又一簇）**：JT 设备页的字段名与后端 DTO 不一致，
而旧代码把"参数没绑定"当成"没传"静默返回空/成功，页面因此完全不可用却看不出原因：

| 调用 | 前端字段 | 后端旧字段 | 现象 | 修复 |
|------|----------|------------|------|------|
| `terminal/channel/list` | `terminalDbId` | `device_id` | 列表恒为空 | 增加 `terminalDbId` 别名，并支持按 `gb_jt_terminal.id` 查询（仍兼容手机号） |
| `terminal/channel/add` | `phoneNumber` + `channelName` | `device_id` + `name` | 恒报"终端不存在" | 增加 `phoneNumber`/`deviceId`/`channelName` 别名 |
| `terminal/channel/update` | `channelName` | `name` | 名称改不了；且 `let _ =` 吞掉错误后仍报"更新成功"（id 不存在也报成功） | 增加别名；id≤0 报错；DB 错误/0 行影响如实返回 |
| `sy/camera/control/*` | `deviceId`/`channelId` | `device_id`/`channel_id` | 参数不生效（旧实现无论如何都回假成功） | 增加 camelCase 别名（`presetIndex` 亦可） |
| `sy/camera/list/ids` | `deviceIds` | `device_ids` | 恒空列表 | 增加 `deviceIds`/`deviceId` 别名 |

**实测**（真实服务 + SIP/ZLM 模拟器）：

```
/api/sy/camera/control/play?...  → code 0, stream=34020000001320000001_...   （真发 INVITE）
/api/sy/camera/control/ptz?...   → "PTZ command sent"                        （真发 DeviceControl）
/api/sy/camera/control/stop?...  → callId=play_...                           （SIP 侧收到 BYE）
/api/rtp/send/stop/...           → {"stopped":true,"app":"rtp","ssrc":...}    （真调 stopSendRtp）
/api/sy/camera/list/ids?deviceIds=3402...0001 → total=4（真实通道名，非 Camera-<id>）
/api/jt1078/terminal/channel/add {phoneNumber,channelName} → 成功
/api/jt1078/terminal/channel/list?terminalDbId=1 → 1 条
/api/jt1078/terminal/channel/one/1 → 真实行（含改名后的 name）
/api/jt1078/terminal/channel/update {id:99999} → {"code":1,"msg":"通道不存在: 99999"}
```

#### 第二十一轮基线

```
cargo test                      560 passed / 0 failed   (上轮 556；+3 JT1078 DTO 契约测试 +1 camelCase)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
```

### 回放控制：报文族与对话都错了（2026-09-12 第二十二轮）

核验"回放暂停/拖动/倍速"时发现：三个接口都返回 success，但**在真实设备上完全无效**。

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **报文族错了**：回放控制用的是 `build_playback_control_xml`，即 `<Control><CmdType>DeviceControl...>` 的 **MANSCDP** 报文（那是云台/报警/录像的设备控制族），而 GB/T 28181 §9.10 规定回放控制是 **MANSRTSP**（`PLAY`/`PAUSE`/`TEARDOWN` + `Scale:`/`Range: npt=`），`Content-Type: Application/MANSRTSP` | 实测 SIP 侧收到的 INFO 正文是 `<?xml version="1.0" ...?>`，设备只能当作无法识别的控制命令 | 新增 `build_playback_control_mansrtsp()`：Play/Resume→`PLAY`、Pause→`PAUSE`、Stop→`TEARDOWN`、Seek→`Range: npt=<秒>-`、Scale→`Scale: <倍率>` |
| 2 | **不是对话内请求**：走 `send_message_to_device` 会**新建 Call-ID 和新 From tag**，设备无法把它关联到正在播放的回放会话（RFC 3261 §12.2.2 要求 Call-ID + 本地 tag + 远端 tag 一致、CSeq 严格递增） | 实测旧实现 Call-ID 与回放 INVITE 不同 | 改为从 `InviteSessionManager` 取出该路回放会话，复用其 Call-ID / 本地 tag / 对端 tag，CSeq 用 `bye_cseq()` 递增并回写会话 |

**实测**（真实服务 + SIP 设备模拟器）：

```
回放 INVITE call_id=playback_34020000001320000001_1789165834347
pause  → INFO 正文: PAUSE RTSP/1.0 / CSeq: 2                    call_id 同上
seek 60→ INFO 正文: PLAY RTSP/1.0  / CSeq: 3 / Range: npt=60-   call_id 同上
speed 2→ INFO 正文: PLAY RTSP/1.0  / CSeq: 4 / Scale: 2         call_id 同上
```

修复前：INFO 正文是 MANSCDP XML，且 Call-ID 与回放对话无关。

#### 第二十二轮基线

```
cargo test                      562 passed / 0 failed   (上轮 560；+2 MANSRTSP 报文测试)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
```

### 前端设备控制（云台/镜头/预置位）指令格式全错（2026-09-12 第二十三轮）

顺着"接口回 success 但设备可能无效"的线索核对**下发到设备的字节**，发现
云台/镜头/预置位控制的指令格式全都不符合国标，而且实现有三份、互相不一致：

| 位置 | 旧实现 | 问题 |
|------|--------|------|
| `handlers/device_control.rs::build_ptz_xml` | `0501000000{ss}FF` | 6 字节；非 `0xA5` 起始；无累加校验 |
| `handlers/common_channel.rs::build_ptz_xml/build_fi_xml/build_preset_xml` | 同上格式，且聚焦/光圈/预置位一律塞进 `<PTZCmd>` | 同上 + 元素名错误 |
| `handlers/device_batch.rs` | 硬编码 `"A500000000AF"` | 5 字节、校验错误 |

国标（GB/T 28181 附录 A.2）要求 `PTZCmd` 是**固定 8 字节**：

```text
字节1 0xA5   起始码
字节2 组合码1（高4位版本=0，低4位校验位=0xF）
字节3 地址低8位
字节4 指令码（bit0 右/bit1 左/bit2 下/bit3 上/bit4 变倍+/bit5 变倍-）
字节5 数据1（水平速度）
字节6 数据2（垂直速度）
字节7 组合码2（高4位=数据3，低4位=地址高4位）
字节8 校验码 = 前 7 字节之和 % 256
```

且**聚焦/光圈与预置位在国标里是独立元素**：`<FICmd>`（FocusNear/FocusFar/
IrisOpen/IrisClose）与 `<PresetCmd>`+`<PresetIndex>`（SetPreset/CallPreset/
DelPreset），不能借用 `<PTZCmd>`。

现在三者统一到新模块 `src/sip/gb28181/front_end_control.rs`，单测与参考资料
给出的标准样例**逐字节对齐**：

```
向上 A50F0108 001F00 DC     向下 A50F0104 001F00 D8
向左 A50F0102 1F0000 D6     向右 A50F0101 1F0000 D5
放大 A50F0110 000010 D5     缩小 A50F0120 000010 E5
停止 A50F0100 000000 B5（速度必须清零，否则云台会一直转）
```

**实测**（SIP 设备模拟器现在会把控制元素原样打日志，便于外部核对）：

```
up&speed=31      → DeviceControl 收到: PTZCmd=A50F0108001F00DC
zoom_in&speed=1  → DeviceControl 收到: PTZCmd=A50F0110000010D5
stop             → DeviceControl 收到: PTZCmd=A50F0100000000B5
focus_in         → DeviceControl 收到: FICmd=FocusNear
goto_preset 7    → DeviceControl 收到: PresetCmd=CallPreset PresetIndex=7
```

修复前模拟器收到的是 `PTZCmd=0501000000{ss}FF` 这类非法指令（聚焦/预置位也
塞在 PTZCmd 里），真实设备只能拒绝或忽略。

#### 第二十三轮基线

```
cargo test                      567 passed / 0 failed   (上轮 562；+5 PTZ/FICmd/PresetCmd 报文测试)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
```

### JT1078 下行：TCP 接入的终端首次可被控制（2026-09-12 第二十四轮）

上一轮把 JT1078 入站接通后，剩余缺口之一是"**命令只经 UDP 下发**"——而 JT/T 808
终端大量以 **TCP** 接入。本轮补上，并顺手修掉 TCP 应答丢失：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **TCP 终端永远收不到平台命令**：`send_raw` 一律 `UdpSocket::send_to` | 上一轮实测 UDP 终端注册/抓拍正常，但 TCP 终端（同一条链路）命令全部超时 | `Jt1078Manager` 增加 `tcp_senders`（对端地址 → mpsc 发送端）；TCP 连接建立时把写半交给独立任务消费队列；`send_raw` **优先走该终端的 TCP 通道**，无 TCP 通道才回落 UDP（仍用监听 socket 保证来源端口） |
| 2 | **TCP 终端收不到 0x8100 注册应答**：读写拆分后，应答原本直接写在读循环持有的 socket 上 | 后端日志显示"终端注册成功"，但终端侧无限 "注册超时，重新注册" | 应答统一改走 `Jt1078Manager::send_raw`（即"该终端自己的下行通道"），TCP/UDP 共用一条下发路径 |
| 3 | 连接断开后残留发送端 | —— | 读/写任务结束时 `remove_tcp_sender`，避免往死连接写 |

**模拟器**：JT1078 终端 mock 新增 `--transport tcp`（JT/T 808 over TCP 与 UDP
共用 `0x7E` 定界，因此只需按 `0x7E` 切帧、`sendall` 发送）。

**实测**（同一后端同时接入 TCP 与 UDP 终端）：

```
TCP 终端：connect 127.0.0.1:60000 → TX 0x0100 → RX 0x8100 → ✅ 注册成功
          /api/jt1078/link-detection → {"online":true,"reachable":true}
          /api/jt1078/snap           → "抓拍命令已被终端应答"
          终端日志：平台命令 0x8201 / 0x8801（均经 TCP 收到）
UDP 终端：注册成功 + 抓拍成功（回归无破坏）
终端列表：两个终端都在（status=true）
```

**新增单测**：`test_send_raw_prefers_tcp_channel`（TCP 通道优先且数据一致）、
`test_send_raw_udp_uses_listen_socket`（UDP 来源端口必须是服务端口）、
`test_send_raw_reports_unknown_terminal`（未连接要显式报错）。

#### 第二十四轮基线

```
cargo test                      570 passed / 0 failed   (上轮 567；+3 下行通道测试)
cargo build --features mysql     OK
cargo build --features postgres  OK
cargo check --all-targets        warnings 0
npx playwright test             25 passed / 0 failed / 0 skipped
JT1078 TCP + UDP 双通道（注册/查询/抓拍）PASS
```

### 录像计划：从来不录像，且按 UTC 匹配时间窗（2026-09-12 第二十五轮）

`record_plan`（录像计划）是本项目对外承诺的功能之一，前端 `/#/recordPlan`
可以增删计划与时间条目。第二十五轮之前，这个功能**从未产生过任何录像文件**：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **计划到点后直接对不存在的流调 `startRecord`**：调度器只做 `zlm.start_record(stream)`，而该设备的流**根本没人拉过**（实时点播要等用户点播才 INVITE） | ZLM 侧 `isRecording` 永远 `{"code":0,"exist":false}`；`/api/cloud-record/list` 永远空 | tick 里先按通道调 `start_live_stream()`（真实向下游设备发 INVITE 并 `startSendRtp` 到 ZLM），等流建立后再 `start_record`；拉流失败只 warn 不 panic，下个 tick 重试 |
| 2 | **时间窗按 UTC 匹配**：调度器用 `chrono::Utc::now()` 取「当天第几秒」，而计划条目里的 `start`/`stop` 是**用户本地时间**（前端按本地时间选择） | 本机 `local = UTC-8`，一个 06:54–07:20 的计划在 UTC 下变成 22:54–23:20，**整整偏 8 小时**——白天录不上、半夜乱录 | 改用 `chrono::Local::now()`；窗口语义保持半开区间 `[start, stop)` |
| 3 | 计划条目匹配逻辑内联在 tick 里，无法单测 | —— | 抽成纯函数 `schedule_matches(items, weekday, seconds)`，补 3 个测试（半开区间边界、多窗口任一命中、缺字段不误命中） |

**实测**（真实后端 + SIP 设备 mock + ZLM mock）：

```
# 把计划窗口设成覆盖"现在"，等一个 tick（30s）
22:55:29 INFO 已按期拉起设备流 34020000001320000001_34020000001320000001（channel=1）
22:55:29 INFO started MP4 recording for channel 1 stream rtp/34020000001320000001_34020000001320000001 (node zlmediakit-1)
SIP mock: Sent PLAY INVITE ... port=30000          # 真的向设备发了点播
/api/zlm MediaServer isRecording → {"code":0,"exist":true}

# 把计划窗口移到过去，等一个 tick
INFO stopped MP4 recording for channel 1 stream rtp/34020000001320000001_34020000001320000001
/api/zlm MediaServer isRecording → {"code":0,"exist":false}
```

窗口用的是本地时间这一点也被这次实测证实：计划里填的是本地 06:54 附近，
若仍按 UTC 匹配（当时 UTC 已 22:55）该 tick **不会**命中——但它命中了。

#### 第二十五轮基线

```
cargo test                       573 passed / 0 failed   (上轮 570；+3 调度匹配测试)
cargo check --features mysql     OK
cargo check --features postgres  OK
cargo check --all-targets        本项目 0 warning（仅 redis 0.25.4 的 future-incompat 提示）
npx playwright test              25 passed / 0 failed / 0 skipped
录像计划（拉起设备流 → 录制 → 到点停止）PASS
```

### 录像计划：前端契约全错、时段口径全错、删除路由不可用（2026-09-12 第二十六轮）

第二十五轮修好了"到点会去拉流并录像"，但那只解决了**调度器**这一半。
这一轮把 WVP-PRO 的真实实现拉下来逐行对照后（见下方"证据来源"），发现整条
录像计划链路从**前端到后端**都跟 WVP 对不上，用户在界面上根本建不出一个能生效的计划：

| # | 缺陷 | 证据 | 修复 |
|---|------|------|------|
| 1 | **前端提交的字段后端根本不认**：前端发 `{planType, startTime, endTime, enable, mon..sun}`，后端（= WVP 契约）只认 `planItemList:[{start,stop,weekDay}]` | 实测 `POST /api/record/plan/add` 带旧载荷返回 `{"code":0,"msg":"成功"}`，但 `gb_record_plan_item` **一行都没写** | 前端按 WVP 契约重写：`RecordPlanItem{start,stop,weekDay}`；后端补上 WVP 的校验 —— 空 `planItemList` 直接 400「添加录制计划时，录制计划不可为空」（此前是静默成功） |
| 2 | **时段口径错**：`start/stop` 是**当天第几分钟**（0..1440）且是**闭区间**，我们按"当天第几秒 + 半开区间"比 | WVP `RecordPlanMapper.queryRecordIng`：`index = hour*60 + minute`、`where wrpi.start <= #{index} and stop >= #{index}` | `schedule_matches` 改为分钟 + 闭区间；补回归测试 `seconds_are_not_minutes` 钉死这个坑 |
| 3 | **星期口径错**：WVP 用 `LocalDateTime.getDayOfWeek().getValue()` = **ISO 1..7（周一=1）**，我们用 `num_days_from_monday()` = 0..6 | 同上 `queryCurrentChannelRecord()`；WVP `edit.vue` 里 `weekDay: i + 1` | 改用 `number_from_monday()`；周几不合法（0/8）直接 400 |
| 4 | **删除从来没成功过**：前端用 GET 调 `/api/record/plan/delete`，路由只注册了 DELETE（`router.rs`），且 handler 只读 `id` 不读 `planId` | 实测 `GET .../delete?planId=N` → **405**；`DELETE .../delete?planId=N` → **400「缺少 id」** | 前端改 DELETE；handler 同时接受 `id`/`planId`；删除时**一并清理**时段条目与通道上的 `record_plan_id`（对齐 WVP `delete()` 的 `removeRecordPlanByPlanId + cleanItems + delete`） |
| 5 | **关联通道完全无效**：前端传通道**主键**（WVP `CommonGBChannel.gbId`），后端拿它当 `gb_device_id` 字符串去查 → 一条都查不到 → 循环体不执行 → **仍返回成功** | 通道列表接口的 `gbId` 也返回 `gb_device_id` 字符串，语义整体错位 | `gbId` 改为通道主键（与 WVP `wdc.id as gb_id` 一致）；`link` 支持主键/国标编号两种输入，**有任何一个不存在就 400**，`allLink` 缺 `planId` 也报错 |
| 6 | **列表页 6 列全是空的**：前端渲染 `planType/startTime/endTime/mon..sun/enable`，后端从不返回这些字段 | — | 列表改为 WVP 的列：名称 / 录像时段汇总 / **关联通道数** / 更新时间 / 创建时间；`query` 与 `get` 都补上 `channelCount`（WVP 的 `(select count(1) ...)`）与 `planItemList`，并支持 `query` 名称检索 |
| 7 | 关联/取消关联后要等下一个 60s tick 才生效 | WVP `link()` 里**同步**调用一次 `execution()` | 增加进程级 `Notify`：`link`/`add`/`update`/`delete` 后立即唤醒调度器（实测关联后 **2 秒内** `isRecording=true`） |
| 8 | `channel/list` 把推流/代理通道也列出来 | WVP 硬编码 `where wdc.channel_type = 0` | 只列国标通道；`hasLink=false` 对齐 WVP = `record_plan_id IS NULL`（此前把"已关联到其它计划"的通道也当成未关联）；名称/编号/状态走 `coalesce(gb_xxx, xxx)`（目录同步写的是老列，`gb_*` 列为空） |
| 9 | 前端没有"按设备关联"入口，也没有已关联列表 | WVP `linkChannelRecord.vue` | `link` 保留 `deviceDbIds`；前端"关联通道"对话框补上未关联/已关联切换、关键字、在线筛选、添加/移除/全部添加/全部移除 |

**顺带修掉的界面问题**：`main.ts` 没有设置 Element Plus 语言包，`ElMessageBox`
显示英文 `OK`/`Cancel`（有截图证据），与整站中文界面不一致 —— 已设置 `zh-cn`。

**证据来源（本次真正拉取了 WVP-PRO 源码逐行比对，不再靠推测）**：

- `RecordPlanMapper.java`：`queryRecordIng(week, index)` 的完整 SQL；
- `RecordPlanServiceImpl.java`：`execution()` / `queryCurrentChannelRecord()` /
  `stopStreams()` / `add()` / `update()`（跳过字段不全的条目）/ `delete()` /
  `link()` / `linkAll()` / `cleanAll()`；以及 `recording(app,stream)` 被
  `MediaServiceImpl.closeStreamOnNoneReader()` 用来**保护正在录像的流不被"无人观看"关掉**
  —— 我们的 `zlm/hook.rs::decide_idle_stream` 第 3 步的 `isRecording` 判定与之等效；
- `RecordPlanController.java`：6 个端点的参数名与校验（`planId`、`allLink`、
  `hasLink`、`channelType`）；
- `web/src/views/recordPlan/{index,edit}.vue` + `common/weekTimePicker.vue`：确认
  `start/stop` 是分钟（`getTrackStyle` 用 `100/24/60 * track.start`）、`weekDay = i + 1`；
- `CommonGBChannelMapper.queryForRecordPlanForWebList`：`wdc.id as gb_id` + `channel_type = 0`。

> `RecordPlan.snap`（"是否开启定时截图"）在 WVP master 里**只存不用**
> （全仓库 `grep getSnap()` 无调用），所以我们同样只保留字段、不发明行为，
> 界面上明确标注"服务端暂未接线"。

**实测**（真实后端 + SIP 设备 mock + ZLM mock，全新 DB）：

```
0) 目录同步             → channelCount=4
1) 未关联通道列表       → total=4，gbId=4/3/2/1（**主键**，不是国标编号字符串）
2) 旧前端载荷 add       → 400 添加录制计划时，录制计划不可为空（此前：code:0，但 0 条时段）
3) 新契约 add           → 成功；ISO 周6 第 438 分钟 → 计划 [408,528]
4) start>stop           → 400 时段起点 600 大于终点 300
   空 planItemList      → 400 添加录制计划时，录制计划不可为空
5) get                  → name=E2E计划 channelCount=0 planItemList=[{start:408,stop:528,weekDay:6}]
6) link(channelIds=[4]) → 成功；channelCount=1
7) link([999999])       → 400 以下通道不存在: 999999
8) 关联后 t=2s          → ZLM isRecording {"code":0,"exist":true}
   后端日志             → 已按期拉起设备流 3402...0004（channel=4）
                          started MP4 recording for channel 4 stream rtp/34020000001320000001_34020000001320000004
   SIP 设备 mock        → RX INVITE sip:34020000001320000004 → 200 OK (video/Play, m=video 30000)
9) GET  delete(planId)  → 405（旧前端写法）
   DELETE delete(planId)→ 200；plans=0 items=0 linked_channels=0（时段与通道关联都被清干净）
```

**新增测试 22 个**：

- 调度匹配 6 个（闭区间边界、**秒/分钟回归保护**、ISO 周一=1、多窗口、缺字段、全天窗）；
- `db::record_plan` 5 个（条目校验/`is_complete`、`replace_items` 跳过不全条目、
  删除清理条目+通道关联、关联/取消/全部、名称检索分页）；
- handler 11 个（空时段 400、倒置时段 400、add→get 往返、更新/删除不存在的计划 400、
  **按 planId 删除**、按主键关联真的落库、未知通道/未知计划 400、
  `gbId` 为主键 + `hasLink` 语义、全部关联/全部取消、query 的 channelCount 与检索）；
- 占位符改写 2 个（postgres `?`→`$n` 编号、LIMIT/OFFSET 必须接在 WHERE 参数之后）。

**新增 e2e 2 个**（`e2e/tests/recordPlan.spec.ts`）：断言**真实请求体**是 WVP 契约
（`planItemList` + 分钟 + ISO 星期）、列表能读回时段、编辑生效、**删除走 DELETE**；
另一用例验证非法时段被前端拦截。

#### 第二十六轮基线

```
cargo test                       595 passed / 0 failed   (上轮 573；+22 录像计划)
cargo check --features mysql     OK
cargo check --features postgres  OK
cargo check --all-targets        本项目 0 warning
npx playwright test              27 passed / 0 failed / 0 skipped  (上轮 25；+2)
录像计划（新增/编辑/列表/关联/删除 + 立即录像）PASS
```

### 云端录像全链路打通（2026-09-12 第二十九轮）

`cloudRecord` 是契约审计里条数最多（14 条）且**有真实调用方**的模块，
本轮全部修完，并顺带挖出 5 个审计没覆盖到的深层缺陷：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | 删除走 `GET /cloud/record/delete?id=` | 后端只注册 DELETE 且从 **body** 读 `{ids:[...]}` → 405 + 参数进不去。前端改 DELETE + body；后端同时接受**数字主键**与历史组合串，并**先删文件再删库**（反了会留下谁也删不掉的孤儿文件） |
| 2 | 列表只扫 ZLM 的 `app=record&stream=record` | **设备录像一条都查不到**。改为**以 `gb_cloud_record` 为准**（WVP 也是读自己的表），`on_record_mp4` 钩子落库的记录才查得到；显式给了 app+stream 时仍保留 ZLM 文件扫描兜底 |
| 3 | `play/path` 传 `id`，后端只认 `recordId` | 永远返回空路径 → 点播放只看到"无可播放路径"。现同时接受 `id`/`recordId`/`cloudRecordId`（数字主键与组合串都能解析） |
| 4 | 列表里的 `httpPath` 是**磁盘绝对路径** | 前端点播放打开的是 `/opt/media/bin/www/...`，必然打不开。现按 ZLM 的 HTTP 根推导：`http://ip:port/record/{app}/{stream}/{date}/{file}`（实测 **200 / 532944 字节**），并给出 https 与下载地址 |
| 5 | ISO 时间参数被解析成 0 | 前端发 `toISOString()`（带毫秒和 Z），旧解析只认两种格式 → 失败返回 0 → **选了结束时间列表整页空白**。现支持 RFC3339/ISO/本地时间/秒/毫秒时间戳，且解析失败时**忽略该条件并告警**（不再静默清空结果） |
| 6 | `deviceId`/`channelId` 后端 DTO 里没有 | 被 serde 静默丢弃。现已实现，按流名 `{设备}_{通道}` 用 `substr` **精确**前后缀匹配（不用 LIKE，避免 `_` 通配符误匹配） |
| 7 | 打包下载传组合串 → "missing ids" | 列表现在返回**数字主键**，打包/删除/播放口径统一；`/list-url` 与 `download/:id` 的地址也真的可用 |
| 8 | 容器化部署下按**本机路径**找文件 | ZLM 在容器里、后端在宿主机，`file_path` 在本机永远不存在 → 打包"全部不可用"、单条下载 404。现改为**从 ZLM 的 HTTP 服务回拉**：打包时流式落盘再压缩，单条下载直接**代理转发**（透传 Range，实测 206 + 1024 字节，视频可拖动进度条） |
| 9 | ZLM 文件列表 API 根本调不通 | 端点名大小写错（真实是 `getMP4RecordFile`）、缺 `vhost` 参数、且响应结构是 `{rootPath, paths[]}`（不传 period 是日期目录、传了才是文件名）—— 三处任一都让列表恒空。已按真实响应重写并支持"先取日期再展开文件" |
| 10 | 删除文件的 API 在新版 ZLM 不存在 | `/index/api/deleteRecord` 实测 404；正确的是 `/index/api/deleteRecordDirectory`（`vhost/app/stream/period/file_name`）。已切换并实测删除成功 |
| 11 | `task/add` 前端 POST、后端只有 GET；`date/list`/`list-url`/`loadRecord` 的返回类型声明与后端不符 | 前端全部对齐 WVP 契约（task/add 改 GET；date/list 是裸字符串数组；list-url 用 `CloudRecordFile`；loadRecord 返回单条流对象） |
| 12 | 页面把毫秒时间戳直接渲染 | 「开始/结束」列改为 `yyyy-MM-dd HH:mm:ss`，并新增「设备/通道」列（从流名解析） |

**实测（真实 ZLM + 真实录像文件）**：

```
列表         → total 1，id=3，deviceId/channelId 已解析，size=532914
play/path    → httpPath=http://127.0.0.1:8080/record/rtp/<dev>_<ch>/2026-09-12/xxx.mp4
               实测 HTTP 200 / 532944 字节；Range 请求 206 / 1024 字节
打包下载     → fileCount 1 / totalBytes 502944，ZIP 可下载并用 unzip -l 校验内容
删除         → DELETE {ids:["3"]} → deleted:["3"]，容器内文件与库记录同时消失
Playwright   → 新增 cloudRecord.spec.ts 3 个用例：列表可读时间、播放地址可访问
               （含后端代理 2xx）、删除走 DELETE 且真的移除；空环境会**自建一段录像**
               （建计划→拉流→录制→落库→停录），跑完整套 34 passed
```

#### 第二十九轮基线

```
cargo test                       634 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              34 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 设备页：在线状态键名、设备表单字段、同步状态、通道过滤（2026-09-12 第三十轮）

`device` 模块 7 条修完，都是"接口回成功但数据不对/没写进去"：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | **在线设备显示"离线"**：前端读 `row.online`/`row.status`，后端（与 WVP `Device.onLine` 一致）返回 `onLine` | 前端按 `onLine` 读取（保留 `online` 兼容）。连带把"撤防"按钮不可达一并修好（按钮本来靠 `isOnline` 判断） |
| 2 | 新增/编辑设备的 `ip/port/password/expires/heartBeatInterval/heartBeatCount` **后端 DTO 里没有** | 六列在有 `gb_device` 表里一直存在，但 DTO 与 INSERT/UPDATE 都没有 → 填了静默丢弃。现在全部落库；UPDATE 用 `COALESCE(NULLIF(?, ''), password)` 保证"编辑时不回填密码"不会把已有密码清空 |
| 3 | 设备详情只返回 12 个键（无 `id`） | 编辑弹窗靠 `props.device?.id` 判断新增/编辑，缺 `id` 会把"编辑"退化成"新增"。改为直接序列化 `Device`（与列表同源） |
| 4 | `heartBeatInterval/heartBeatCount` 永远读不到 | 列在库里，但 `Device` 结构体与 `DEVICE_SELECT_COLUMNS` 都没有 → 前端类型声明是空的。已补进结构体与全部 15 处显式 SELECT |
| 5 | `sync_status` 无 `total/current/errorMsg` | 前端（含 legacy 同步进度弹窗）靠它们算百分比。现按 WVP `SyncStatus` 补齐（另有 `syncIng`） |
| 6 | 设备通道列表忽略 `query`/`online`/`channelType` | 三个参数被静默忽略（返回该设备全部通道）。已实现（`substr`/LIKE 参数化，行查询与计数共用一套 WHERE） |
| 7 | `update` 对不存在的设备回成功 | 现在返回 404（affected == 0），不再假装成功 |

**实测**：

```
POST /api/device/query/device/add {ip,port,password,expires,heartBeat*} → 读回全部字段
列表         → onLine:false（键名正确）、heartBeatInterval:45、heartBeatCount:2
详情         → 带 id=27 与 expires/heartBeatInterval
sync_status  → {total:0,current:0,errorMsg:null,status:"idle"}
通道过滤     → query=通道2 → 1 条；online=false → 0 条
Playwright   → 34 passed（无回归）
```

#### 第三十轮基线

```
cargo test                       637 passed / 0 failed
cargo check --all-targets        本项目 0 warning
npx playwright test              34 passed / 0 failed / 0 skipped（真实 ZLM）
```

### JT1078 终端与围栏/路线：字段名全线错位 + 一个 500（2026-09-12 第三十一轮）

`jtDevice` 模块 13 条修完。除契约错位外，还连带发现一个更严重的类型缺陷：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | 终端删除用 GET（后端只有 DELETE）+ 传 `id`（后端只认 `phoneNumber`） | 前端改 DELETE；后端同时接受 `id`（前端列表传的主键）与 `phoneNumber`，不存在的 id 返回 404 而不是成功 |
| 2 | 终端 `plateNo/plateColor/makerId/provinceId/cityId` 后端 DTO 里没有 | 编辑框填了静默丢弃。已补齐并落库（车牌颜色兼容数字码与中文名） |
| 3 | **`/terminal/list` 会 500** | `gb_jt_terminal.province_id/city_id` 是 **TEXT** 列，而结构体是 `Option<i32>` —— NULL 能解码，写入真实值后整行解码失败。已把字段改成字符串、写入时把数字码转成字符串，并把 16 处 `SELECT *` 换成显式列（`CAST(... AS TEXT)`） |
| 4 | 列表 `status` 是布尔，前端按 `=== 1` 比较 | 在线终端永远显示"离线"。前端改按布尔判断 |
| 5 | 通道列表缺 `phoneNumber`/`status`，键名是 `name` 而前端读 `channelName` | 后端补 `phoneNumber`/`status` 并同时给出 `name`+`channelName`；前端优先读 `name` |
| 6 | 通道列表 `terminalDbId` 是 `i32`：筛选框填手机号直接 400 | 改成字符串并兼容"主键或手机号"（`opt_string_flexible` 同时接受数字与字符串） |
| 7 | **围栏/路线新建必然失败**：前端发 `phoneNumber/radiusM/pointsJson/waypointsJson`，后端只读 `phone/radius/points/waypoints` | 四个端点全部改成"两种命名都收"，错误文案也改成前端字段名；`pointsJson` 是 JSON 字符串时会解析成数组再存 |
| 8 | 区域/路线查询响应是 snake_case | 前端表格"手机号/中心经纬度/半径/点位"整列空白。给四个结构体加 `#[serde(rename_all = "camelCase")]` |

**实测**：

```
终端列表      → status:true（布尔）、plateNo/plateColor/makerId/provinceId/cityId 全部有值
编辑终端      → 车牌/颜色/厂商/省域/市域真的落库；未提交字段保持原值
DELETE ?id=1  → code:0，终端列表归零（不存在的 id → 404）
通道列表      → terminalDbId 传手机号不再 400
围栏/路线     → phoneNumber/radiusM/pointsJson/waypointsJson 全部接受并落库
查询响应      → circle: [centerLat, centerLon, phoneNumber, radiusM...]；route: waypointsJson
Playwright    → 34 passed（无回归）
```

#### 第三十一轮基线

```
cargo test                       641 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              34 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 推流管理：方法/体型错位 + 一个稳定 500（2026-09-12 第三十二轮）

`streamPush` 模块 12 条修完：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | 删除用 `DELETE /api/push/remove`，后端（与 WVP）只注册 POST + query id | 405，推流记录删不掉。前端改 POST |
| 2 | 「批量删除」把 ids 放 query，后端要求 DELETE + **JSON body** | 415。前端改 `data: {ids}` |
| 3 | 状态列读 `row.status === 1`，后端返回**布尔** | 永远显示"停止"、「停止」按钮恒禁用。前端改按布尔判断 |
| 4 | 列表读 `mediaServerId`/`url`，后端是 snake_case 且**没有 url 列** | 「媒体节点」列空白、「源 URL」列空白。后端结构体改 camelCase；前端不再假装有"源 URL"，改为展示后端算出的**推流地址**（`rtmp://<节点>:1935/app/stream`） |
| 5 | 编辑框要求必填"源 URL"，而 WVP 的推流编辑框只有 App/Stream/节点/拉起离线推流 | 去掉该字段（WVP 里不存在），并补上"拉起离线推流"开关 |
| 6 | `query` 参数被 DTO 收下却从未用于过滤 | 已实现（`app`/`stream` 模糊匹配，行查询与计数共用一套 WHERE） |
| 7 | `save_to_gb` / `remove_form_gb` **更新不存在的列** → 稳定 500 `no such column: device_id` | 改为写入真实存在的 `gb_device_id`/`gb_channel_id`（三份 schema 补列 + 启动时 ALTER 兼容旧库），并在列表返回 |
| 8 | `remove_from_gb` 用 GET、`save_to_gb` 只发 query；`upload` 发 JSON 而后端要 multipart | 前端分别改为 DELETE+body / POST+body / multipart(`file`) |

**实测**（真实后端）：

```
POST /api/push/add                → 成功
GET  /api/push/list               → camelCase：mediaServerId/status(false)/startOfflinePush(true)/pushUrl=rtmp://127.0.0.1:1935/push/cam1
query=cam1 → 1 条；query=nope → 0 条（此前 query 被忽略）
DELETE /api/push/batchRemove {ids:[1]} → removed:1
POST /api/push/remove?id=2        → 成功（此前 DELETE 405）
POST /api/push/save_to_gb {id,deviceId,channelId} → saved:1，列表出现 gbDeviceId/gbChannelId
DELETE /api/push/remove_form_gb {id} → removed:1，绑定清空（此前两者都 500）
Playwright                        → 新增 streamPush.spec.ts 2 个；整套 36 passed
```

#### 第三十二轮基线

```
cargo test                       644 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              36 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 拉流代理：字段名全线错位，功能整块不可用（2026-09-12 第三十三轮）

`streamProxy` 模块 10 条修完。这一页此前**从"新增"到"状态显示"没有一步是对的**：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | 弹窗提交 `{url, enabled, destUrl}`，后端 DTO 只有 `src_url`/`srcUrl` | 新增必失败（`Stream and src_url are required`）。后端 DTO 加 `url` 别名，前端改用 WVP 的 `srcUrl` |
| 2 | 编辑保存时 `src_url` 为 NULL → `COALESCE` 保留旧值 | 「源 URL 静默改不动」。同 1 |
| 3 | `enable` 未落库；`add` 还写死 `enable = false` | 「启用」开关保存无效。DTO 收 `enable`（别名 `enabled`），`add` 写真实值 |
| 4 | `type`/`timeout`/`ffmpegCmdKey`/`rtspType`/`enableAudio`/`enableMp4`/`noneReader` 全部无处安放；`destUrl` 是前端虚构字段 | 后端换 `StreamProxyWrite`（14 列，新增/更新共用），`update` 的 SQL 从 5 列补到 14 列；前端删掉"目标 URL"，按 WVP 补齐真实字段 |
| 5 | 列表读 `row.url`，后端返回 `src_url` | 「源 URL」整列空白。`StreamProxy` 加 `rename_all = "camelCase"` |
| 6 | 列表读 `row.enabled`，后端返回 `enable` | 启用列恒关 |
| 7 | 列表读 `row.status === 1`，后端只有布尔 `pulling` | 状态恒"停止"、**「停止」按钮恒禁用**。前端改读 `pulling`；`start`/`stop` 真实维护 `pulling` + `stream_status`（active/ready/failed） |
| 8 | `/save` 复用只认 `srcUrl` 的 DTO，一旦被调用必失败 | `proxy_save` 改 upsert（存在则更新） |
| 9 | `/one` 不查库，凭空拼 `proxy-{id}` 与假 URL | 支持 `?id=` 与 WVP 的 `?app=&stream=`，返回真实行，无则 404 |
| 10 | `query` 被 DTO 收下却从未使用 | `DynWhere` 统一 WHERE（app/stream/name/srcUrl/type 模糊匹配），行查询与计数共用；前端补搜索框 |

**连带发现并修掉的缺陷**：

* `ZlmClient::add_stream_proxy` 发的是 `enable_aac`，而 ZLM 只认 **`enable_audio`**
  （对未知参数**静默忽略**）→ 勾了"开启音频"从未生效。已核对镜像内
  `/opt/media/bin/www/swagger/openapi.json` 与二进制字符串。
* `gb_stream_push` 的 `query` 同样被忽略；且 postgres 下 `pushing = ?` 绑 `Int`
  会 `operator does not exist: boolean = integer` → 整个推流列表 500。
  为此给 `dyn_where::BindValue` 增加 `Bool` 变体，`pulling`/`pushing` 一并改用。
* `GET /api/proxy/ffmpeg_cmd/list` 返回 4 条硬编码中文说明（"默认转码模板"…），
  并不是 ZLM 的模板键 —— 用户选中的 key 在 ZLM 上不存在。改为真实读节点
  `getServerConfig` 的 `ffmpeg.cmd*`，并补 `DELETE /api/proxy/del?app=&stream=`（WVP 签名）。
* `ProxyListPage` 补 `pageNum`/`pageSize`/`pages`（PageHelper `PageInfo` 兼容）。
* `TerminalQuery` 里 `phoneNumber` 被 `device_id` 与 `phone_number` 同时声明为别名，
  serde 生成的重复分支让后者**不可达** → `/api/jt1078/terminal/query?phoneNumber=`
  恒返回 null。改为各自只认一个键，并消除该编译警告。

**实测**（真实后端 + 真实 ZLM，源流由 ffmpeg 推到 `live/src`）：

```
GET  /api/proxy/ffmpeg_cmd/list?mediaServerId=zlmediakit-1
     → {"ffmpeg.cmd":"%s -re -i %s -c:a aac …"}   （真实 ZLM 配置）
POST /api/proxy/add {url:"rtsp://127.0.0.1:554/live/src", enabled:true}
     → 成功，返回 camelCase：srcUrl/enable/type/streamStatus=ready
POST /api/proxy/add（同 app+stream）      → APP+STREAM 已存在
POST /api/proxy/add（缺 srcUrl）          → 源 URL(srcUrl) 不能为空
GET  /api/proxy/start?id=1
     → streamKey=__defaultVhost__/proxy/legacy1
     → ZLM getMediaList: proxy/legacy1 (rtsp/rtmp/fmp4), originUrl=rtsp://127.0.0.1:554/live/src
     → 列表 pulling=true / streamStatus=active
GET  /api/proxy/stop?id=1                 → pulling=false，ZLM 侧流数 0
POST /api/proxy/add {type:"ffmpeg", ffmpegCmdKey:"ffmpeg.cmd", stream:"fe1"} + start
     → ZLM originTypeStr = ffmpeg_pull（走 addFfmpegSource 分支）
GET  /api/proxy/list?query=…/pulling=true/false → 过滤真实生效
GET  /api/proxy/one?id=1 与 ?app=proxy&stream=legacy1 → 同一真实行
DELETE /api/proxy/del?app=&stream= 与 /api/proxy/delete?id= → 各删 1 行
Playwright                                → 新增 streamProxy.spec.ts 4 个；整套 40 passed
```

#### 第三十三轮基线

```
cargo test                       658 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              40 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 级联平台：国标ID 拼写 + expires 类型 + 三个虚构字段（2026-09-12 第三十四轮）

`platform` 模块 11 条修完。这一页此前**新增平台必然 422**、列表国标ID空白、「注销」静默失效：

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1 | 前端 `serverGbId`（小写 b）vs 后端/WVP `serverGBId` | 列表国标ID空白、「注销」被 `if (!row.serverGbId) return` 静默拦掉。前端改回 `serverGBId`，后端兼容两种拼写 |
| 2 | 新增时国标ID 绑不上 → 写空串，仍回"成功" | 平台不可用（级联注册/`get_by_server_gb_id` 全以空串为键）。加必填 + 唯一性校验，空值 400 |
| 3 | `expires` 前端发 number、后端 DTO 只要 string | 反序列化 422，**请求进不到 handler**。`deserialize_opt_int_string` 两者都收，响应回数字（WVP `int expires`） |
| 4 | 「域名」用 `realm`，后端字段 `serverGBDomain` | 静默丢弃、重开编辑框域名消失。前端改 `serverGBDomain`，后端加 `realm` 别名 |
| 5/6 | 「注册间隔/心跳间隔/心跳次数」是**凭空发明**的字段（WVP `Platform.java` 与本仓库 schema 都没有） | 换成真实的 `expires`（注册周期）+ `keepTimeout`（心跳周期，列 `keep_timeout` 本就存在）；列表/详情统一输出 |
| 7/8 | `catalog/add`/`catalog/edit` 无 alias，`platformId`/`parentId`/`civilCode`/`businessGroup` 全绑不上；编辑还因 `unwrap_or_default()` 用空串**清空**已有值 | DTO 改 camelCase + `parent` 别名；更新改为 `Option` 直接绑定（未传 = NULL） |
| 9 | `/platform/exit/:deviceGbId` 按 `device_gb_id` 查、只回布尔、**没有任何注销动作** | 改按 `serverGBId` 定位，发 `Expires: 0` REGISTER + `enable/status=false`；不存在 404 |
| 10 | `server_config` 声明类型 `{ip,port,id,realm}` 与实际键完全不符 | 前端类型对齐；后端修正 `serverGBId`（此前填的是 `realm`，本平台 20 位编码从未暴露） |
| 11 | `/platform/info/:id` 只回 10 个字段 | 与列表共用 `platform_row_json` |

**实测**（真实后端）：

```
POST /api/platform/add  {serverGbId + realm + heartBeatInterval + expires:3600}
     → 成功；列表回 serverGBId=34020000002000000901 / serverGBDomain=3402000901
       / expires=3600(数字) / keepTimeout=45 / heartBeatInterval 不再重复输出
POST /api/platform/add  {serverGBId:…902}（WVP 字段 + expires:"1800", keepTimeout:"30"）→ 成功
POST /api/platform/add  重复国标ID → 400 平台国标ID已存在
POST /api/platform/add  缺国标ID   → 400 国标ID(serverGBId)不能为空
POST /api/platform/update {id, serverGBDomain, keepTimeout:77, expires:1200} → 成功并落库
GET  /api/platform/info/1  → 与列表同一套键（serverGBDomain/expires/keepTimeout/channelCount…）
GET  /api/platform/exit/34020000002000000901 → sipWarning=null，
     日志：Cascade platform 34020000002000000901 unregistered and removed；
     之后 enable=false status=false（此前只回一个布尔且无动作）
GET  /api/platform/exit/<不存在>            → 404
POST /api/platform/catalog/add {platformId,parentId,civilCode,businessGroup} → affected:1 且字段真的落库
POST /api/platform/catalog/edit {id,name}    → parent/civil_code/business_group 保持原值（不再被清空）
GET  /api/platform/server_config → serverGBId=34020000002000000001 / serverGBDomain=3402000000
Playwright → 新增 platform.spec.ts 4 个；整套 44 passed
```

#### 第三十四轮基线

```
cargo test                       665 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              44 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 行政区划 / 业务分组：删除 405 + 编辑抬根 + 模块没有界面（2026-09-12 第三十五轮）

`region` 模块 8 条契约修完，另外**把这个模块缺失的界面补上了** —— 此前 12 个 API
函数里只有 `getRegionTreeList` 有调用方（地图页），区域/分组的增删改在 Vue3 前端不可达。

| # | 缺陷 | 修复 / 证据 |
|---|------|------|
| 1/2 | `deleteRegion` / `deleteGroup` 用 GET，后端只注册 DELETE | 405，删除永远失败。前端改 DELETE（实测 `GET=405 / DELETE=200`） |
| 3/4 | `RegionUpdate`/`GroupUpdate` 没有 camelCase 别名 → 只有 `name` 生效；`parent_id = ?` 直接把 NULL 写进去 | **只改名字会把节点从子级抬到根级**。DTO 加 camelCase；`parent_id` 改 `COALESCE`；移到顶级用前端既有的 `-1` 哨兵（`build_region_tree` 本来就认） |
| 5/6 | `tree/query` 的参数 DTO 只有 page/count，前端传的 `parentId` 被静默丢弃 | 永远返回"全量第 1 页"。新增 `TreeNodeQuery`（`parentId` + WVP 的 `query`），真正按父节点/关键字过滤并分页 |
| 7/8 | 前端把 `tree/query` 的返回类型声明成数组，实际是 `{total, list}` | 改 `TreeNodePage<T>`；后端补 `pageNum/pageSize/pages`（与 WVP 的 `PageInfo` 一致） |
| + | `GET /api/group/one` 不存在（只有 `region/one`） | 补上 |
| + | **整个模块没有界面** | 新增 `web/src/views/region/`（两个 tab、树形增删改、过滤、一键同步行政区划）+ 路由 `/region` + 侧边栏入口 |

**实测**（真实后端）：

```
POST /api/region/add  顶级(parentId=-1) + 子级(parentId=1) → 成功
GET  /api/region/tree/query?parentId=-1   → total 1（只有顶级）
GET  /api/region/tree/query?parentId=1    → total 1（只有子级）
GET  /api/region/tree/query?query=测试市  → total 1（关键字）
POST /api/region/update {id, name}        → parent_id 仍为 1（此前被清成 NULL）
POST /api/region/update {id, deviceId, parentDeviceId} → 两个字段都真的落库
GET  /api/region/delete?id=2 → 405；DELETE → 200
GET  /api/group/one?id=1 → 真实行（此前 404）
POST /api/group/update {businessGroup:"3", civilCode:"340200"} → 落库且 parent_id 保留
GET  /api/region/sync → {"count":1,...}（真实同步行政区划）
Playwright → 新增 region.spec.ts 5 条；整套 50 passed
```

新页面在真渲染下还暴露了两个只在浏览器里才看得见的问题（已一并修掉）：两个 tab 的
`el-tree` 不能共用一个 `ref` 名（Vue 只保留最后注册的那个，`filter()` 会作用在隐藏的
那棵树上，搜索框看起来完全没反应），也不能共用一个 `data`（`el-tabs` 两个 pane 同时
存在于 DOM，切换后隐藏的树也会跟着渲染另一棵树的数据）。

#### 第三十五轮基线

```
cargo test                       670 passed / 0 failed
cargo check --all-targets        本项目 0 warning
cargo check --features mysql/postgres  OK
npx playwright test              50 passed / 0 failed / 0 skipped（真实 ZLM）
```

### 前端↔后端契约审计：已完成 12 个模块，剩余 5 个模块 / 24 条（2026-09-12 第三十五轮刷新）

第二十六轮用"一个模块一个 agent"的方式把 16 个前端 API 模块逐个对后端路由/DTO
做了一遍审计（证据文件在 `docs/audit/*.md`，共 **130 条**），并按影响排序逐批修复。
当前已修 12 个模块（106 条），剩 `log`(7) / `mediaServer`(7) /
`playback`(3) / `syCamera`(6) / `talk`(1) 共 **24 条**：

| 模块 | 条数 | 状态 |
|------|------|------|
| recordPlan（第二十五/二十六轮，非审计产物） | — | ✅ 已修（含真实调度录像） |
| user | 6 | ✅ 已修（口令写入约定 → 新建/重置账号能登录；role 列；分页） |
| live | 6 | ✅ 已修（PTZ 参数名 + 8 字节报文；streams 的 deviceId/channelId；webrtc 方法） |
| channel | 8 | ✅ 已修（新增/编辑全字段落库 + schema 缺列；三个下拉 {name,code}；真实点播） |
| alarm | 10 | ✅ 已修（清除/批量清除/处理/级别/时间筛选/关键字；handle_result 落库） |
| cloudRecord | 14 | ✅ 已修（第二十九轮，另发现 5 个深层缺陷） |
| device | 7 | ✅ 已修（第三十轮） |
| jtDevice | 13 | ✅ 已修（第三十一轮） |
| log | 7 | ❌ 未修 |
| mediaServer | 7 | ❌ 未修 |
| platform | 11 | ✅ 已修（第三十四轮） |
| playback | 3 | ❌ 未修 |
| region | 9 | ✅ 已修（第三十五轮，另补了整块缺失的界面） |
| streamProxy | 10 | ✅ 已修（第三十三轮） |
| streamPush | 12 | ✅ 已修（第三十二轮） |
| syCamera | 6 | ❌ 未修 |
| talk | 1 | ❌ 未修 |

**剩余模块里"有真实调用方、用户可见"的高优先级项**（按严重度）：

1. `cloudRecord`：删除走 GET（后端只有 DELETE → 405）；`play/path` 传 `id`
   而后端只认 `recordId`（点播放永远拿不到地址）；`list` 的 `startTime/endTime`
   发 ISO 串被解析成 0（选择结束时间后列表整页空白）；`download/zip` 把
   组合串当数字 id（ZIP 永远不生成）；`deviceId/channelId` 过滤被丢弃。
2. `device`：列表在线状态读 `online` 而后端返回 `onLine` → **在线列恒为离线**、
   撤防按钮不可达；新增/编辑设备的 `ip/port/password/expires` 后端 DTO 里没有
   （填了不落库）；`sync_status` 缺 total/current/errorMsg；`channels` 的
   query/online/channelType 过滤被忽略。
3. `jtDevice`：终端删除方法+参数双错（405 / 缺 phoneNumber）；终端
   `plateNo/plateColor/makerId/provinceId/cityId` 被静默丢弃；通道列表缺
   `channelName/phoneNumber/status`；区域/路线四个接口的请求字段名
   （前端 `phoneNumber/radiusM/pointsJson/waypointsJson` vs 后端 `phone/radius/points/waypoints`）
   → **新增围栏/路线必然失败**；区域查询响应是 snake_case（表格多列空白）。
4. ~~`streamPush` / `streamProxy`~~：✅ 已分别于第三十二/三十三轮修复
   （删除/批量删除方法或体型不符 405/415、`url`/`gbId` 后端 DTO 不存在、
   列表字段名错位；拉流代理还补齐了真实 `start`/`stop` 与 `query` 过滤）。
5. ~~`platform`~~：✅ 已于第三十四轮修复（`serverGbId` 拼写、`expires` 数字/字符串、
   `realm`→`serverGBDomain`、心跳三参数换成真实的 `expires`/`keepTimeout`、
   注销改为真的发 `Expires: 0` REGISTER、列表与详情统一字段）。
6. `log` / `mediaServer` / `syCamera` / `playback` / `talk`：
   主要是响应键名与筛选参数不匹配（系统信息页内存/磁盘/版本恒为 0 或 '-'、
   媒体节点“检测”探测错地址、仪表盘“重点通道”卡片跳转失败、
   录像列表“名称”列空白、对讲起播与音频 WS 的时序竞争）。
   ~~`region`~~：✅ 第三十五轮已修（删除 405、update 抬根、tree/query 忽略 parentId，
   并补上了整块缺失的行政区划/业务分组管理界面）。
   其中 region/playback/mediaServer 的多项**当前无调用方**。

**运行期核验缺口（本轮新增/仍未闭环）**：

- ~~**真实 ZLM 收流链路尚未端到端跑通**~~ **已跑通（第二十八轮）**：
  详见上方第二十八轮小节。剩余未在真实 ZLM 上验证的：级联向上推流
  （startSendRtp 到上级平台）、HLS/WebRTC 的实际播放、多节点负载均衡。
- MySQL/PostgreSQL 仍只做**编译**验证（无实例可跑运行期 SQL）。
- `docs/audit/*.md` 里的"无调用方"条目虽已定位，但为保持 API 契约正确仍应修完。

### 真实 ZLMediaKit 上的端到端媒体验证跑通了（2026-09-12 第二十八轮）

此前所有媒体相关验证都跑在自研 ZLM mock 上（mock 只回 HTTP，不做真实收流），
所以"配置下发到底生效没有""真实 ZLM 能不能收到设备的 RTP"这类问题一直没被覆盖。
本轮把真实容器（`docker compose` 里的 `zlmediakit/zlmediakit:master`）接通，
一路修到 **真实的 SIP INVITE → 设备 RTP/PS → ZLM 出流 → hook → 播放地址/录像文件**
全链路跑通：

| # | 缺陷（都是"静默无效/静默超时"型） | 证据与修复 |
|---|------|------|
| 1 | **setServerConfig 全部静默失效** | ZLM 只认查询参数/表单的 `键=值`，客户端发的是 `{secret,key,value}` JSON body → ZLM 忽略 body、因 secret 合法回 `code:0`。所有 autoConfig（hook 地址、端口范围、protocol、mediaServerId）都下发不进去。改为 GET + 查询参数，并加 `set_server_config_verified` 回读校验（不支持的键明确告警） |
| 2 | **hook secret 下发不到** | 官方靠 `[hook] admin_params`，而该镜像的 config.ini **没有这个键**（回读为空）→ 钩子请求不带 secret → 后端 `hook auth: secret mismatch` 拒掉 → 媒体就绪事件永远收不到（表现为 INVITE 200 OK 后收流超时）。现在 secret 直接写在**每个事件 URL** 的查询串上 |
| 3 | **general.mediaServerId 是占位串** | 镜像默认 `your_server_id`，钩子全被标成未知节点。节点上线时下发为节点主键（`zlmediakit-1`），已回读确认 |
| 4 | **收流端口池与容器映射不一致** | 镜像 `rtp_proxy.port_range` 默认 30000-35000，compose 只发布 30000-30100，且端口池**启动时**建立（运行期改不生效）。实测端口落在 33842 → 设备 RTP 到不了容器。现在仓库挂载 `docker/zlm/config.ini`（收窄为 30000-30100，并把 api.secret 对齐），`handlers/play.rs` 在端口越界时 error 级告警 |
| 5 | **getRtpInfo 的响应形态解析错** | 真实 ZLM 返回**扁平**结构（`exist`/`identifier`/`local_port`…），代码按 `data.stream_id` 解析 → 恒为 None。导致"流已存在就复用"永远走不通：第二个观看者拿到 `-300 This stream already exists` 而失败。已兼容两种形态 + 单测 |
| 6 | **同一通道第二个观看者必失败** | 上面两条修完后，`/api/play/start` 对已存在的流改为**复用并直接返回播放地址**（幂等），不再把正常情况当失败 |
| 7 | 测试替身不发媒体 | `mock/tools/sip-device/sip_device_mock.py` 新增 `--send-rtp`：收到 PLAY INVITE 后用 ffmpeg 合成 H264，按 `-f mpeg`(PS) 切片成 RTP 发给平台宣告的地址，BYE 时停止。这样"设备不推流"类问题才可能被发现 |

**实测（真实 ZLM 容器 + 真实 redis + SIP mock --send-rtp）**：

```
GET  /index/api/getServerConfig → general.mediaServerId=zlmediakit-1，
     hook.enable=1，11 个 on_* URL 带 secret，rtp_proxy.port_range=30000-30100
POST /api/play/start/...        → code:0，返回 rtsp/flv/hls/webrtc 地址
GET  /index/api/getMediaList    → rtp/34020000001320000001_34020000001320000001，2 条 track
第二次 play/start               → code:0（复用现有流，不再 -300）
录像计划：新增+关联通道 → 到点拉起 INVITE → isRecording status:true
     容器内落盘 /opt/media/bin/www/record/rtp/<dev>_<ch>/2026-09-12/2026-09-12-22-09-45-0.mp4
删计划后 isRecording → cannot find the stream（录制已停止，文件保留）
npx playwright test            → 31 passed / 0 failed（跑在真实 ZLM 上）
```

#### 第二十八轮基线

```
cargo test                       626 passed / 0 failed
cargo check --all-targets        本项目 0 warning
npx playwright test              31 passed / 0 failed / 0 skipped（真实 ZLM）
真实 ZLM：点播（含第二个观看者复用）/ 收流 / 出流 / 录像文件 全链路 PASS
```

#### 第二十七轮基线

```
cargo test                       623 passed / 0 failed   (上轮 595；+28)
cargo check --all-targets        本项目 0 warning
npx playwright test              31 passed / 0 failed / 0 skipped  (上轮 27；+4)
vue-tsc --noEmit                 通过
真实 ZLM（容器 zlmediakit/zlmediakit:master）实测：
  setServerConfig 改为查询参数后可下发（13/20 项生效，7 项该版本无此键并明确告警）
  rtp_proxy.port_range 已按配置写入；收流端口不在发布范围时 error 级告警
```

### 仍未解决 / 需真实设备核验

以下是本轮**已定位但未改动**的项，均在代码中留有注释或在此登记，
不应被视为"已实现"：

1. **`f=` 媒体描述行的结构**：当前所有实现都发 `f=v/1/96/1/2/1/1/0`，
   而国标模板是 `f=v/<编码>/<分辨率>/<帧率>/<码率类型>/<码率大小>a/<音频编码>/<码率>/<采样率>`
   —— 该串**缺少 `a/` 音频段标记**，结构不完整。改动需要确定各字段取值，
   在没有真实设备可核验前不宜臆造（`f=` 是建议性字段，设备可忽略）。
2. **`Subject` 头形状**：仓库内存在两种写法，活跃实时点播路径用
   `serverGbId:ssrc,deviceGbId:0`（代码内注释即如此），回放/下载路径用
   `localId:channelId,localId:flag`。国标示例为
   `<通道编码>:<发送端序列号>,<接收方编码>:<ssrc>`。实时点播路径可能正在实际互操作，
   改动风险大于收益，故保留现状并在此登记。
3. ~~对讲/广播的媒体面~~ **已实现（第八轮）**：见下方「语音对讲」小节。
4. ~~**`log_file_download`** 仍是文件路径下载；前端 `getLogFile` 定义了但从未调用。~~
   **已修复（第七轮）**：该端点此前固定去 `./logs/<name>` 找文件，而本进程
   **不写日志文件**（tracing 采集层直接落 `gb_log` 表）、`logs/` 目录也从未创建
   —— 因此它是**必然 404 的死路径**，且 `PathBuf::from("./logs").join(file_name)`
   存在**目录穿越**：axum 的路径参数会做百分号解码（实测
   `/api/log/file/%67bserver-log.csv` 返回 200，证明 `%67`→`g`），
   所以 `..%2f..%2fetc%2fpasswd` 会被还原成 `../../etc/passwd` 并读到仓库外文件。
   现改为：`gbserver-log.csv` / `gbserver-log.json` 直接导出 `gb_log`（支持
   `query`/`level`/时间范围过滤）；真实文件走严格文件名校验（单段、纯
   `[A-Za-z0-9._-]`、不以点开头、不含 `..`），非法名返回 400 而非静默 404；
   读取错误不再一律伪装成 404，IO/权限错误如实返回 500。
   新增 3 个测试（含穿越用例与 CSV 转义）。
5. **`catalog_sync` 的完成判定依赖设备如实上报 `SumNum`**：若设备声明
   `SumNum=N` 却只发更少的包，会话会一直停在 `Receiving`（`device_sync`
   8 秒后如实返回该状态）。已保留逐包 upsert 兜底，因此不会丢通道，
   但"同步完成"无法判定。
6. ~~**两套 SSRC 机制并存**~~ **已统一（第十三轮）**：新增唯一的
   `build_ssrc(prefix, device_id)`（10 位 = 1 位类型 + 设备号前 9 位），
   `build_play_ssrc`（实时，前缀 0）/ `build_playback_ssrc`（回放，前缀 1）/
   `build_download_ssrc`（下载，前缀 2）/ `build_audio_ssrc`（对讲，前缀 4）
   全部转发到它。此前 `send_play_invite_and_wait` 的兜底与 `handlers/play.rs`
   各自算的是 `0{id9}0`（**11 位**），与 SsrcManager 口径不一致 ——
   同一设备在不同路径会拿到不同长度的 SSRC。
7. **TCP 信令**已与 UDP 统一分发，但 `handle_packet` 的参数已达 23 个，
   后续应改为上下文结构体，否则每次新增能力都要再穿一遍全部调用点。
8. ~~**`send_session_bye` 仍只走 UDP socket**~~ **已修复（第十四轮）**：
   新增 `sip::transport::tcp::send_sip_out()` 作为**出站请求的唯一发送口**，
   按对端地址选 TCP/UDP；`send_session_bye` / `send_talk_bye` / `send_broadcast_bye`
   / INVITE / ACK / MESSAGE / SUBSCRIBE 与设备心跳全部改走它。
9. ~~**JT1078 终端侧媒体列表（0x0802）未解析**~~ **已实现（第二十轮）**：
   按 JT/T 808-2013 §8.19 解析 0x0802，`/api/jt1078/media/list` 返回终端真实
   检索结果。**仍需真实终端核验**的是 2011/2019 版本差异（本实现按 2013）。
10. ~~**JT1078 命令只经 UDP 下发**~~ **已实现（第二十四轮）**：
    `Jt1078Manager::send_raw` 现按终端接入方式选择 TCP 通道或 UDP 监听 socket；
    模拟器新增 `--transport tcp`，TCP/UDP 两条链路的注册/查询/控制均已实测通过。
11. **巡航/扫描/辅助开关的报文形态待核验**（2026-09-12 第二十三轮定位）：
    `handlers/front_end.rs` 当前发的是**属性式** XML：
    `<CruiseCmd id="1" preset="5" action="add" />`、`<ScanCmd id="1" action="start" />`。
    GB/T 28181-**2022** §A.3.5/A.3.6/A.3.7 把巡航/扫描/辅助开关规定为
    **8 字节二进制 PTZCmd**（字节4 指令码：0x84~0x88 巡航 / 0x89、0x8A 扫描 /
    0x8C、0x8D 辅助），与 2016 的 XML 元素风格都**不是**属性式写法。
    在没有真实设备或权威 2016 元素表可核对前不臆造 —— 现在这批端点的报文
    形态**明确登记为未核验**，不作为已实现。
12. **FI（聚焦/光圈）与预置位存在两套并存编码**（同上）：
    本实现按 **GB/T 28181-2016** 用独立元素 `<FICmd>` / `<PresetCmd>`+`<PresetIndex>`
    （与 WVP 参考实现一致，因此满足平替目标）；
    **2022** §A.3.3/A.3.4 则把它们并入 8 字节 `PTZCmd`
    （字节4 高 2 位=01 表示 FI；预置位用字节4=0x81/0x82/0x83、编号在数据2，
    例如调用预置位 5 = `A50F0182000500xx`）。
    部分 2022 设备可能只认后者 —— 需要真实设备确认后再决定是否追加兼容分支。
13. **`on_rtp_playlist` / `on_record_progress` / `on_send_rtp_progress`**
   的载荷结构未与真实样本核对（官方文档未给出示例）。
14. **多节点下 `general.mediaServerId`** 现在会在 autoConfig 时下发为节点主键；
    但**手工在 ZLM 侧改过该键**的既有部署仍需重新保存节点才会对齐。
15. ~~**上级平台点播本级（级联拉流）尚未接线**~~ **已实现并端到端验证（第十八轮）**：
    见上方第十八轮小节。当前实现用进程级队列 + `Arc<SipServer>` 后台任务
    解耦静态信令路径与 `&self` 媒体路径；后续若继续加级联能力（如上级云台控制
    转发、级联录像回放），建议把 `start_live_stream` 抽成"按部件调用"的自由函数，
    让静态路径可以直接复用，而不再绕队列。

### 工程问题

- **16 个测试写了却从未运行**：`tests/integration/sip/{integration,cascade_integration_test}.rs`
  位于子目录，cargo 只自动发现 `tests/*.rs` 与 `tests/<dir>/main.rs`。已注册为 `[[test]]`
  并修正与 API 的漂移（`with_timeout` builder 化、`accumulate_record_info` 增参等）
- 删除纯占位测试 `device_api_test.rs`（3 测试 0 断言 6 处 TODO，且从未编译）
- 移除 `axum-test` dev 依赖：其 7.x 依赖 **axum 0.6**（与本项目 0.7 不兼容，依赖图里有两个 axum）
- 新增 `test_support`（内存 SQLite + 生产 schema + 可构造 `AppState`），
  使 handler/router 级测试成为可能，并新增「路由可构建」测试防住历史上的启动 panic


## 历史基线（2026-08-23）

| 维度 | 数值 |
|------|------|
| 总代码量（src/） | 61,095 行 Rust |
| 已注册 HTTP 路由 | 369 条唯一 `/api/...` 路径 |
| Handler 模块 | 21 个（其中 `stub.rs`/`device_stub.rs` 主要是 shim 与少量占位） |
| 后端测试 | **573 通过**（lib 510 + 集成 63）/ 3 忽略 / 0 失败（第二十五轮刷新） |
| 编译状态 | `cargo check` 0 error / 55 warning |
| 前端 | `web/` Vue 2 现状稳定；`web-v3/` Phase 1 完成（脚手架+登录+控制台） |
| 数据库 | SQLite/PostgreSQL/MySQL 三选一，默认 SQLite |

## 已实现功能矩阵（按 WVP 模块划分）

| 模块 | 路由数 | 状态 | 证据 |
|------|------|------|------|
| 用户/认证 (`user/`) | 8 | ✅ 完整 | `tests/integration/sqlite_compat.rs::sqlite_user_auth_login_succeeds` |
| 设备 CRUD (`device/`) | 12 | ✅ 完整 | 包含统计、tree、status、channels |
| 设备控制 PTZ/Preset/Guard/Record | 14 | ✅ 完整 | `device_control.rs` + `front_end.rs`（含扫描/巡航/雨刷/光圈/聚焦/预置位） |
| 通道 (`common_channel/`) | 50+ | ✅ 完整 | 含 civilCode、parent、map tile、industry、network identification |
| 直播 (`play/`) | 8 | ✅ 完整 | start/stop/snap/ssrc/share/broadcast/webrtc |
| 回放 (`playback/`) | 7 | ✅ 完整 | start/stop/pause/resume/seek/speed |
| 云录像 (`cloud_record/`) | 14 | ✅ 完整 | `cloud_record_extra.rs` + `stub.rs` |
| 推流/代理 (`stream/`) | 18 | ✅ 完整 | push + proxy + ffmpeg_cmd |
| 上级平台 (`platform/`) | 14 | ✅ 完整 | add/update/delete + 级联 catalog/channel/server_config |
| ZLM (`server/media_server/*`) | 10 | ✅ 完整 | list/one/save/online/check/load/media_info/record_check |
| 系统 (`server/*`) | 9 | ✅ 完整 | system_info/config/map/info/version/resource_info/stream_all |
| 区域/分组 (`region/group/`) | 16 | ✅ 完整 | tree/path/addByCivilCode/sync |
| 录像计划 (`record_plan/`) | 6 | ✅ 完整 | add/update/delete/query/link/channel_list；分钟/ISO 星期口径 + 到点拉流录制 + 前端契约对齐 WVP（第二十五/二十六轮） |
| API Key (`userApiKey/`) | 7 | ✅ 完整 | add/delete/enable/disable/remark/reset/list |
| 角色 (`role/`) | 3 | ✅ 完整 | all/add/delete |
| 日志 (`log/`) | 2 | ✅ 完整 | list + file download |
| 报警 (`alarm/`) | 9 | ✅ 完整 | list/before/detail/clear/handle/snap/device/batch/delete |
| 位置 (`position/history/`) | 1 | ✅ 完整 | history query |
| WebRTC (`webrtc/`) | 1 | ✅ 完整 | play/webrtc |
| 对讲 (`talk/`) | 6 | ✅ 完整 | invite/start/stop/ack/bye/status/list |
| RTP/PS (`rtp/`, `ps/`) | 6 | ✅ 完整 | send/receive + getTestPort |
| 服务器配置 (`server/config`) | 1 | ✅ 完整 | config |
| 移动位置订阅/目录订阅 (`device/query/subscribe/*`) | 3 | ✅ 完整 | catalog/mobile-position/alarm |
| 设备配置查询 (`device/config/query/*`) | 3 | ⚠️ 3 个 fire-and-forget | 详见"已知缺口" |
| 设备配置更新 (`device/config/update`) | 1 | ⚠️ 返回 Not Implemented | 详见"已知缺口" |
| JT1078 车载终端 | 50+ | ✅ 路由齐全 | 含 area/polygon/rectangle/route/telephone/playback/snap/ptz 等 |
| 中亿/SY 视图 (`sy/camera/*`) | 12 | ✅ 完整 | 列表/控制/盒/圆/多边形/会议 |
| 移动端列表 (`sy/camera/list-for-mobile`) | 1 | ✅ | |
| Map tile (`common/channel/map/thin/tile`) | 1 | ✅ | |

## 已知缺口（按优先级倒序）

### P0 · 实际功能回归

- [x] **SIP 上行 XML 解析漏属性形式 DeviceID**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/xml_parser.rs::get_device_id`
  - 症状：真实设备发送 `<Query CmdType="Catalog" DeviceID="...">` 时被错认为"未知请求"
  - 修复：新增属性形式回退 + `find_first_element`/`find_first_attr` 辅助函数
  - 测试：`sip::server::upstream_message_tests::test_xml_parser_extracts_query_target_device_id` ✅
- [x] **PendingRequestManager cleanup_expired 不按 TTL 清理**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/pending_request.rs::register`
  - 症状：`with_timeout(1)` 后 2s 仍返回 0 移除项；`PendingRequest::new` 硬编码 `unwrap_or(30)`，忽略 manager 配置
  - 修复：`register()` 中 `timeout_secs.unwrap_or(self.default_timeout_secs)` 替代硬编码
  - 测试：`sip::gb28181::pending_request::tests::test_cleanup_expired` ✅
- [x] **oneshot Sender 因 Clone 永远丢失，P1 await 链路完全跑不通**（2026-08-23 修复）
  - 文件：`src/sip/gb28181/pending_request.rs`
  - 根因：`PendingRequest::Clone` 显式将 `response_sender` 置 None（避免 oneshot 双发 panic），
    导致 `register_with_receiver` 把 req.clone() 插入 DashMap 后，`complete()` 取到的 sender 永远是 None
  - 修复：新增独立的 `senders: DashMap<call_id, oneshot::Sender<String>>`，
    `register_with_receiver` 把 sender 从 req.take() 后存入 `senders` map；
    `complete()` / `cleanup_expired` / `cancel_for_device` / `cancel_all_for_device` 全部同步清理 senders
  - 测试：5 个新增 commander 测试（`register_with_receiver_resolves_on_complete`、
    `await_response_returns_timeout_when_no_reply`、`query_device_info_and_parse_end_to_end`、
    `query_device_info_and_parse_send_failure`、`query_device_info_and_parse_timeout`）✅
  - 影响：所有 P1 实装的"等待 SIP 响应"端点（device_info / device_status / device_config_query）
    现在端到端可用，不再是无声 fire-and-forget

### P1 · 异步查询未等待响应

- [x] **`GET /api/device/query/info/{device_id}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_info`
  - 现状：使用 `commander.query_device_info_and_parse(...)` + 15s 超时；超时返回带 `"status":"timeout_or_error"`
- [x] **`GET /api/device/query/status/{device_id}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_status`
  - 同样模式：`commander.query_device_status_and_parse(...)` + 15s 超时
- [x] **`GET /api/device/config/query/{device_id}/{config_type}` 实际等待响应**（2026-08-23 修复）
  - 文件：`src/handlers/device_query.rs::device_config_query`
  - 使用 `register_device_config_with_receiver` + `await_response` + 透传原始 XML（配置结构多样不强解析）

### P2 · 设备配置更新

- [x] **`POST /api/device/config/update` 死代码已删**（2026-08-23 清理）
  - `device_query.rs::device_config_update` 从未被注册，router 使用 `device_control::device_config_update` 真实 111 行实现
  - 删除 `device_query.rs` 中的占位函数

- [x] **设备控制 Transport 协议消息**（2026-08-23 完成）
  - 文件：[server.rs](src/sip/server.rs) 新增 `send_device_transport` + [device_stub.rs](src/handlers/device_stub.rs) `device_transport` 升级
  - 现状：handler 现在更新 DB **并**向设备下发 SIP Control/Transport 消息
  - 已加：mode 合法性校验（必须 TCP/UDP/TCP-ACTIVE/TCP-PASSIVE）+ 在线判定 + sipSent/sipError 字段

### P3 · JT1078 区域/路由 HTTP 端点（GBServer 扩展，超 WVP 范围但 Stop hook 明确指出）

- [x] **JT1078 圆形围栏 CRUD**（2026-08-23 实装）
  - 表：[init-sqlite-2.7.4.sql](database/init-sqlite-2.7.4.sql) 新增 `gb_jt_area_circle`
  - DB：[jt1078.rs](src/db/jt1078.rs) 新增 `JtAreaCircle` struct + insert/update/delete/list
  - Handler：[jt1078_extra.rs](src/handlers/jt1078_extra.rs) 重写 5 个端点为真实 DB 持久化
  - 测试：5 个 CRUD roundtrip 集成测试 ✅
- [x] **JT1078 多边形围栏 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_area_polygon` + `JtAreaPolygon` + insert/delete/list
  - Handler：3 个端点（set/delete/query）
- [x] **JT1078 矩形围栏 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_area_rectangle` + `JtAreaRectangle` + insert/update/delete/list
  - Handler：5 个端点（add/edit/delete/query/update）
- [x] **JT1078 路线 CRUD**（2026-08-23 实装）
  - 表：`gb_jt_route` + `JtRoute` + insert/delete/list
  - Handler：3 个端点（set/query/delete）
- [x] **JT1078 协议操作层 12 个端点**（live/record/snap/temp_position_tracking/confirmation_alarm/playback_download/media_upload_delete/terminal_channel_*）（2026-09-11 全部接线，本轮**复核确认**）
  - 现状：全部经 `src/jt1078/` 的 `Jt1078Manager` **真实下发并等待终端通用应答**，失败即返回错误；已无"已受理"占位响应
  - 关联模块：[src/jt1078/](src/jt1078/) 5 个子模块、4,850 LOC
  - 平替评估：WVP-PRO 没有 JT1078 协议层；这部分是 GBServer 独有扩展，已不再是"silently do nothing"

### P4 · 前端 WVP 业务页迁移

- [x] **Vue 3 前端迁移全部完成**（2026-08-23，commit `2acf5a7`）
  - `web-v3/` 转正为 `web/`；Vue 2 归档至 `web-legacy-vue2/`（仅参考）
  - 17 个业务视图已落地：channel / live / playback / map / mediaServer / recordPlan /
    platform / streamProxy / streamPush / cloudRecord / alarm / device / jtDevice /
    dashboard / login / operations / user
  - 迁移细节见 [web/MIGRATION.md](../web/MIGRATION.md)（该文件已标注 Phase 2+ 表格为历史记录）
- [ ] **前端仅剩体验类收尾**（非迁移阻塞项）：`commonChannel` 页 `<Region>` 的 `offsetHeight`
      报错、`operations` 页 `childValue.startsWith` 未防御非字符串（见 `docs/debug/ISSUES.md`）

### P5 · 代码质量

- [x] **`src/cascade/register.rs:346,540`** + **`src/sip/server.rs` 6 处** —— 移除 `drop(&X)` no-op（2026-08-23 清理，6 处全部删除）
- [x] **`CLAUDE.md` 顶部"default database feature is PostgreSQL"过期描述**（2026-08-23 修复）
  - 同步 5 处描述（顶部命令说明、MySQL 示例、PostgreSQL 示例、db 模块描述、init schema 描述）
  - 当前与 `Cargo.toml` 的 `default = ["sqlite"]` 完全一致
- [x] **`unused imports` 11 个**（2026-08-23 清理）
  - `server.rs` / `system.rs` / `subscription.rs` / `sip_server.rs` / `lib.rs` / `record_plan.rs` / `common_channel.rs` / `jt1078.rs` 中全部删除
- [x] **`field \`code\` never read` 11 个**（2026-08-23 清理）
  - `zlm/client.rs` 中 9 处 `struct Resp { code: i32 }` 加 `#[allow(dead_code)]`
  - `StreamListResp` / `VersionResp` 同处理
- [x] **`ambiguous glob re-exports` 11 个**（2026-08-23 清理）
  - `db/mod.rs` 加 `#[allow(ambiguous_glob_reexports)]` 到每个 `pub use module::*`
- [x] **`unused variable` 14 个**（2026-08-23 清理）
  - `platform.rs` 4 函数、`jt1078.rs` 6 函数、`common_channel.rs` 1 函数、`stream.rs` 2 函数：加 `#[allow(unused_variables)]`（feature-gated SQL 路径下 sqlite 不使用部分参数）
  - `cascade/register.rs`、`sip/server.rs` 手动 `drop(&sip)` 改为引用作用域结束自动释放
- [x] **`mut not needed` 2 个**（2026-08-23 清理）
  - `ws/hub.rs` 中 4 处 `let (tx, mut rx)` 检查后**保留** mut（`recv()` 需要）
  - `pending_request.rs:348` `mut req` 去除（take() 已不需要，sender 在 senders map 中）
- [x] **`dead/unreachable code`**（2026-08-23 清理）
  - `sip/server.rs:1178-1179` 重复的 `SipMethod::Options/Info` match 臂删除（前者已覆盖）
  - `sip/server.rs:4839` `waiter_key` 改为 `_waiter_key`（分配但未读）
- [x] **cascade_service 30 条 deprecated 字段/结构 warning**（2026-09-11 解决）
  - 原判断是「架构性：应迁移到 `crate::cascade::CascadeRegistrar`，影响大需独立 PR」
  - **实测该判断有误**：`CascadeService` 早已零生产调用（唯一构造点全在它自己的单测里），
    模块文档也自述「生产路径不再使用本类型」。因此正确做法不是迁移而是**直接删除**
  - 已删除 `src/sip/gb28181/cascade_service.rs`（751 行）+ `mod.rs` 的 `pub mod` 与
    `#[allow(deprecated)] pub use`（那处 allow 正是此前压住告警的补丁）
  - 能力对照确认 `cascade/register.rs` + `cascade_forward.rs` 为超集（含 11 个
    `c3_*` / `phase5_*` 等价测试）
- [x] **cache → StateStore 迁移完成（deprecated 告警清零）**（2026-09-11）
  - 原判断「8 条 warning，需连同 fallback 策略一起设计」方向正确，但**范围被低估**
  - `zlm/hook.rs` 的 4 处 legacy Redis 写入中，3 处是纯冗余（StateStore 写入就在同一段
    代码前面，Redis 那几行自带注释「will be removed in Phase 7.6」）；仅 `on_flow_report`
    一处真需迁移 —— 已改为用 ZLM 上报的权威绝对计数覆盖 `StateStore.stream_count`
  - `lib.rs::select_least_loaded` 的 Redis fallback（原 Step C）随之删除
  - 连带发现：**整个 `src/cache.rs` 已零调用方**（StateStore 提供 device_online /
    stream / media_server / recording 全部等价 API），已整体删除 140 行 + `pub mod cache;`
  - ⚠️ 更正：原记录写「`lib.rs:755` fallback 读该 Redis 计数，属活跃负载均衡逻辑」——
    该读取确实存在，但它读的正是上面那几处写入的**冗余副本**，与 Step A 的 StateStore
    查询重复，因此可以安全删除（回退链改为 StateStore → ZLM 实时计数 → 首个节点）
- [ ] **剩余 clippy 告警 292 条**（2026-09-11 实测，原 329）：主要为
  `too_many_arguments` 106（handler 多参数，宜在 `Cargo.toml [lints]` 显式放行）、
  `borrow_deref_ref` 86、`unnecessary_unwrap` 40、`unnecessary_cast` 34 等机械项
  - 建议下一批：`cargo clippy --fix` 清机械项 + `[lints]` 配置结构性项，
    清零后再把 CI `hygiene` job 提升为硬门禁

## 已验证 · 端到端冒烟（2026-06-20 历史记录）

来源：`docs/debug/SMOKE_REPORT.md`

| 类型 | 通过 | 备注 |
|------|------|------|
| 后端 health | 1/1 | `{"status":"alive"}` |
| 后端 metrics | 1/1 | Prometheus 端点正常 |
| 后端 API smoke | 7/7 | 登录/用户/设备/媒体 |
| Playwright UI smoke | 18/18 | 15 页面 + login + dashboard |
| ZLM HTTP API | 1/1 | `code:0` |

## 测试基线（每次推进后回填）

- 2026-09-12 第三十二轮：`cargo test` —— **644 通过 / 0 失败**（推流 12 条）
- 2026-09-12 第三十三轮：`cargo test` —— **658 通过 / 0 失败**（拉流代理 10 条 + `enable_audio`/`TerminalQuery` 连带修复；e2e 40）
- 2026-09-12 第三十四轮：`cargo test` —— **665 通过 / 0 失败**（级联平台 11 条；e2e 44）
- 2026-09-12 第三十五轮：`cargo test` —— **670 通过 / 0 失败**（行政区划/业务分组 8 条 + 补全管理界面；e2e 50）
- 2026-09-12 第三十一轮：`cargo test` —— **641 通过 / 0 失败**（JT1078 终端/围栏 13 条）
- 2026-09-12 第三十轮：`cargo test` —— **637 通过 / 0 失败**（设备页 7 条）
- 2026-09-12 第二十九轮：`cargo test` —— **634 通过 / 0 失败**（云端录像全链路）
  - 同时：`npx playwright test` 34 通过 / 0 失败 / 0 跳过
- 2026-09-12 第二十七轮：`cargo test` —— **623 通过 / 0 失败**（+28：契约修复与真实 ZLM 集成）
  - 同时：`npx playwright test` 31 通过 / 0 失败 / 0 跳过
- 2026-09-12 第二十六轮：`cargo test` —— **595 通过 / 0 失败**（lib 532 + 集成 63；+22 录像计划）
  - 同时：`npx playwright test` 27 通过 / 0 失败 / 0 跳过；mysql/postgres feature 构建 OK
- 2026-09-12 第二十五轮：`cargo test` —— **573 通过 / 0 失败**（lib 510 + 集成 63；+3 录像计划调度匹配）
- 2026-08-23 第四次推进：`cargo test --no-fail-fast` —— **395 通过 / 2 忽略 / 0 失败**
  - lib: 348（+5：JT1078 area/route CRUD 集成测试）
- 2026-08-23 第三次推进：`cargo test --no-fail-fast` —— **392 通过 / 2 忽略 / 0 失败**
  - lib: 345（+5：commander 端到端 await 测试 + register timeout 回归保护）
- 2026-08-23 第二次推进：`cargo test --no-fail-fast` —— **387 通过 / 2 忽略 / 0 失败**
  - lib: 340（+9：xml_parser 7 + pending_request 2）
- 2026-08-23 首次推进：`cargo test --no-fail-fast` —— **378 通过 / 2 忽略 / 0 失败**

## 平替决策记录

- **保留 `stub.rs` / `device_stub.rs` 作为 shim**：这些文件已演化为 WVP API 兼容层而非纯占位，不删除以保持前端路由兼容
- **`parity_extras.rs` 已清空**：ISSUES.md 提到的 6 处路由重复已被 `device_query`/`device_control` 完整实现版本取代
- **JT1078 模块**：作为 GBServer 独有扩展（超越 WVP），路由齐全，纳入平替范围
## WVP-PRO 路由对照（2026-08-23 核对）

按 WVP-PRO Java 控制器分类（项目知识 + 公开源码 API 表），逐条核对当前实现的 370 路由：

| 模块 | WVP-PRO 端点数 | 已覆盖 | 状态 |
|------|--------------|-------|------|
| Auth/User | 11 | 11 | ✅ 完整 |
| User API Key | 7 | 7 | ✅ 完整 |
| Device CRUD | 18 | 18 | ✅ 完整 |
| Device Control (PTZ/Preset/Guard/Record/Reboot/Batch) | 8 | 8 | ✅ 完整 |
| Device Config (query/update) | 4 | 4 | ✅ 完整（含 SIP 等待响应）|
| Device Statistics/Tree/Stream | 6 | 6 | ✅ 完整 |
| Channel CRUD + Civil Code + Industry + Network Ident | 14 | 14 | ✅ 完整 |
| Channel Play + Playback (含 seek/pause/speed) | 9 | 9 | ✅ 完整 |
| Channel Map (tile/level/thin) | 7 | 7 | ✅ 完整 |
| Channel Group/Region 绑定 | 6 | 6 | ✅ 完整 |
| Live (play/snap/ssrc/share/broadcast/webrtc) | 8 | 8 | ✅ 完整 |
| Playback | 6 | 6 | ✅ 完整 |
| Cloud Record (list/play/zip/date/seek/speed) | 14 | 14 | ✅ 完整 |
| GB Cloud Record (device query/download) | 4 | 4 | ✅ 完整 |
| Record Plan | 7 | 7 | ✅ 完整 |
| Push/Proxy (ffmpeg) | 17 | 17 | ✅ 完整 |
| Platform/Cascade (含 catalog/channel) | 14 | 14 | ✅ 完整 |
| Server/Media Server | 12 | 12 | ✅ 完整（含 health check / media_info）|
| Region/Group/Role | 19 | 19 | ✅ 完整 |
| Alarm | 9 | 9 | ✅ 完整 |
| Talk | 7 | 7 | ✅ 完整 |
| Position History | 1 | 1 | ✅ 完整 |
| RTP/PS send/receive + getTestPort | 9 | 9 | ✅ 完整 |
| WebRTC | 1 | 1 | ✅ 完整 |
| Media (getPlayUrl/stream_info) | 2 | 2 | ✅ 完整 |
| Logs | 2 | 2 | ✅ 完整 |
| System (info/version/stats/online-users) | 4 | 4 | ✅ 完整 |
| SY Camera (中亿视图，WVP 扩展) | 12 | 12 | ✅ 完整 |
| Health/Ready/RPC/WS/ZLM Hook | 5 | 5 | ✅ 完整 |
| Front End PTZ/Preset/Scan/Tour/FI/Wiper | 20 | 20 | ✅ 完整 |
| JT1078 区域/路由/控制（GBServer 扩展，**超出 WVP 范围**） | 26 | 16 (CRUD) + 10 (协议) | ✅ DB 层实装 + 协议层 stub |

**结论**：WVP-PRO 公开 API 端点 100% 已挂载到 router.rs（共 370 条），端点路径 + 参数 + 响应 schema 与 Java 版对齐。JT1078 部分为 GBServer 独有扩展，区域/路由 CRUD 已实装 DB 层，协议操作层保留"已受理"响应（需要在线终端 + JT/T 808/1078 协议栈）。

**待 PR/独立 sprint 闭环的剩余工作**（不属于"功能平替"范畴，而是工程化收尾）：

1. ~~`web-v3/` Phase 2 业务页迁移（前端，~5 周）~~ → ✅ 已完成（2026-08-23）
2. ~~cascade_service → CascadeRegistrar 迁移（30 个 deprecated warning，独立 PR）~~
   → ✅ 2026-09-11 解决：实为**零调用的死代码**，直接删除模块（751 行）而非迁移
3. ~~cache::set_media_server_streams → StateStore 迁移~~ → ✅ 2026-09-11 完成
   （deprecated 归零；连带删除已零调用的整个 `src/cache.rs`）
4. 清零 `cargo fmt` 差异（约 2.6 万行）与剩余 clippy warning，随后把 CI `hygiene` job 提升为门禁

## WVP-PRO 真实源码对照（2026-08-23 第 5 次推进）

⚠️ **重要更正**：上一版本对照表基于助手自身的 WVP-PRO 知识（不可验证）。本节用 web_search 实际检索到的 WVP-PRO 仓库源码片段逐条核对，来源包括：
- 648540858/wvp-GB28181-pro 公开 README
- DeepWiki 自动生成的 REST API Controllers 文档（基于 Java 源码扫描）
- gitee.com 上的 wvp-pro 镜像分支（苏叶/wvp-pro、easyaiot 等）
- CSDN 上引用 WVP-PRO @RequestMapping 注解的二次开发指南

### 来自 [DeviceQueryController.java](https://gitee.com/shanghai-internet-of-things_1/easyaiot) 与 [RuoYi-Wvp Device Management DeepWiki](https://deepwiki.com/cbnbcbnb/RuoYi-Wvp/4.1-device-management) 真实证据

| WVP-PRO 端点 | HTTP | GBServer 路由 | 状态 |
|------|------|------|------|
| `/api/device/query/devices` | GET | `/api/device/query/devices` | ✅ |
| `/api/device/query/devices/{deviceId}` | GET | `/api/device/query/devices/:device_id` | ✅ |
| `/api/device/query/devices/{deviceId}/sync` | POST | `/api/device/query/devices/:device_id/sync` | ✅ |
| `/api/device/query/devices/{deviceId}/delete` | DELETE | `/api/device/query/devices/:device_id/delete` | ✅ |
| `/api/device/query/device/add/` | POST | `/api/device/query/device/add` | ✅ |
| `/api/device/query/device/update/` | POST | `/api/device/query/device/update` | ✅ |
| `/api/device/query/transport/{deviceId}/{streamMode}` | POST | `/api/device/query/transport/:device_id/:stream_mode` | ✅（已实装 DB + SIP Transport） |
| `/api/device/query/sub_channels/{deviceId}/{parentId}/channels` | GET | `/api/device/query/sub_channels/:device_id/:parent_channel_id/channels` | ✅ |
| `/api/device/query/sync_status` | GET | `/api/device/query/sync_status` | ✅ |
| `/api/device/query/streams` | GET | `/api/device/query/streams` | ✅ |
| `/api/device/query/subscribe/catalog` | GET | `/api/device/query/subscribe/catalog` | ✅ |
| `/api/device/query/subscribe/alarm` | GET | `/api/device/query/subscribe/alarm` | ✅ |
| `/api/device/query/subscribe/mobile-position` | GET | `/api/device/query/subscribe/mobile-position` | ✅ |
| `/api/device/query/statistics/register` | GET | `/api/device/query/statistics/register` | ✅ |
| `/api/device/query/statistics/keepalive` | GET | `/api/device/query/statistics/keepalive` | ✅ |
| `/api/device/query/tree/{deviceId}` | GET | `/api/device/query/tree/:device_id` | ✅ |
| `/api/device/query/tree/channel/{deviceId}` | GET | `/api/device/query/tree/channel/:device_id` | ✅ |
| `/api/device/query/channel/audio` | GET | `/api/device/query/channel/audio` | ✅ |
| `/api/device/query/channel/one` | GET | `/api/device/query/channel/one` | ✅ |
| `/api/device/query/channel/stream/identification/update/` | POST | `/api/device/query/channel/stream/identification/update/` | ✅ |
| `/api/device/query/info/{deviceId}` | GET | `/api/device/query/info/:device_id` | ✅（已实装 15s SIP 等待响应） |
| `/api/device/query/status/{deviceId}` | GET | `/api/device/query/status/:device_id` | ✅（已实装 15s SIP 等待响应） |

### 来自 [DeepWiki REST API Controllers](https://deepwiki.com/648540858/wvp-GB28181-pro/9.2-rest-api-controllers) ServerController 真实证据

| WVP-PRO 端点 | HTTP | GBServer 路由 | 状态 |
|------|------|------|------|
| `/api/server/media_server/list` | GET | `/api/server/media_server/list` | ✅ |
| `/api/server/media_server/online/list` | GET | `/api/server/media_server/online/list` | ✅ |
| `/api/server/media_server/one/{id}` | GET | `/api/server/media_server/one/:id` | ✅ |
| `/api/server/media_server/save` | POST | `/api/server/media_server/save` | ✅ |
| `/api/server/media_server/delete` | DELETE | `/api/server/media_server/delete` | ✅ |
| `/api/server/media_server/check` | GET | `/api/server/media_server/check` | ✅ |
| `/api/server/media_server/media_info` | GET | `/api/server/media_server/media_info` | ✅ |
| `/api/server/media_server/load` | GET | `/api/server/media_server/load` | ✅ |
| `/api/server/system/configInfo` | GET | `/api/server/system/configInfo` | ✅ |
| `/api/server/system/info` | GET | `/api/server/system/info` | ✅ |
| `/api/server/config` | GET | `/api/server/config` | ✅ |
| `/api/server/resource/info` | GET | `/api/server/resource/info` | ✅ |
| `/api/server/version` | GET | `/api/server/version` | ✅ |
| `/api/server/info` | GET | `/api/server/info` | ✅ |

### 来自 [PtzController.java gitee 镜像](https://gitee.com/suye222/wvp-pro) 真实证据

20 条 PTZ/Preset/Cruise/Scan/FI/Wiper/Auxiliary 端点，全部已挂载（见 GBServer router.rs 第 119-145 行）。这些是 GBServer 早期 [handlers/front_end.rs](src/handlers/front_end.rs) 已实装的 PTZ 命令发送路径（送 SIP Control 命令给设备）。

### 来自 PlayController / PlaybackController 真实证据（DeepWiki 章节 "Live Stream Playback (PlayController.java86-156)" + "Historical Playback (PlaybackController.java83-143)"）

7 条播放端点 + 6 条回放端点，全部已挂载且实装 SIP INVITE/MESSAGE 流程（见 [handlers/play.rs](src/handlers/play.rs) + [handlers/playback.rs](src/handlers/playback.rs)）。

### 来自 `ApiDeviceController.java`（LiveGBS 兼容 API，路径前缀 `/api/v1/device`）

WVP-PRO 提供 LiveGBS 兼容的 `/api/v1/device/{list,channellist,...}` 端点。**GBServer 未实现**这部分（prefix 是 `/api/v1/device` 而非 `/api/device`）。这是 LiveGBS 第三方集成接口，不是 WVP-PRO 主端点。

### 仍未严格对照 WVP-PRO 的 GBServer 独有扩展

- **`/api/sy/camera/*`** 中亿视图（12 端点）：WVP-PRO 没有，GBServer 独有
- **`/api/jt1078/*`**（28+ 端点）：WVP-PRO 主分支不包含 JT1078，GBServer 独有
- **`/api/system/{info,version,stats,online-users}`**（4 端点）：WVP-PRO 用 `/api/server/*` 提供类似功能，命名不同

### 综合结论（基于真实源码）

✅ **WVP-PRO 主 API（DeviceQuery / Server / Play / Playback / PTZ）**：**所有真实源码可见端点已 1:1 对齐**，包括协议消息实现细节（GB28181 SIP SUBSCRIBE / INVITE / ConfigDownload / DeviceControl / Message 等）。

⚠️ **GBServer 独有扩展**（SY 视图、JT1078、live_gbs 兼容接口）属于范围外，未与 WVP-PRO 对齐。

⚠️ **JT1078 协议操作层 10 端点仍是"已受理"响应**——需要真实 GB/T 808/1078 终端 session 联调才能真下发。这是协议层实现，不是 HTTP API 缺口。

⚠️ **P1 端点 await 路径只有单元测试，无真实 GB28181 设备 e2e**——实际 GB/T 28181 设备在线、SIP 响应符合预期，需要真实摄像头（或 SIP 信令模拟器）做联调才能完整验证。

⚠️ **前端业务页已全部迁移完成**（2026-08-23），当前无 UI 平替阻塞项；仅剩少量体验类收尾（见 P4 节）。

⚠️ **CI 已恢复**（2026-09-11），但 `fmt` / `clippy` 仍为非门禁 —— 基线未清零前不设为硬约束。
