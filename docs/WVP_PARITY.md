# WVP 平替进度追踪

> 目标：完全平替 WVP-PRO（Java GB28181 平台）的全部功能。
> 本文档作为持续校对的事实基线：每次推进后更新对应条目并记录证据。

## 当前基线（2026-09-11）

> 本节数字为**实测值**，复现命令见每行「验证方式」。上次基线见文末「历史基线」。

| 维度 | 数值 | 验证方式 |
|------|------|----------|
| 总代码量（src/） | 64,914 行 Rust | `find src -name '*.rs' \| xargs wc -l` |
| 已注册 HTTP 路由 | 380 条唯一 `/api/...` 路径 | `grep -oE '"/api/[^"]*"' src/router.rs \| sort -u \| wc -l` |
| Handler 模块 | 29 个（含 `stub.rs` / `device_stub.rs` 两个兼容 shim） | `grep -c 'pub mod' src/handlers/mod.rs` |
| 后端测试 | **478 通过** / 3 忽略 / 0 失败 | `cargo test --no-fail-fast` |
| 编译状态 | `cargo check` 0 error / **0 warning**；clippy 261；**deprecated 0** | `cargo check` / `cargo clippy --all-targets` |
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
3. **对讲/广播的媒体面**：SDP 与信令已可用，但"浏览器音频 → ZLM → RTP → 设备"
   的上行音频管线尚未实现（`TalkSession.zlm_stream_id` 已记录，无消费方）。
4. **`log_file_download`** 仍是文件路径下载；前端 `getLogFile` 定义了但从未调用。
5. **`catalog_sync` 的完成判定依赖设备如实上报 `SumNum`**：若设备声明
   `SumNum=N` 却只发更少的包，会话会一直停在 `Receiving`（`device_sync`
   8 秒后如实返回该状态）。已保留逐包 upsert 兜底，因此不会丢通道，
   但"同步完成"无法判定。
6. **`SsrcManager` 与 `build_ssrc(prefix, id)` 两套 SSRC 机制并存**：
   设备侧 INVITE 用后者（按设备号确定性推导），级联取流用前者（按会话分配）。
   两者都能产出合法 10 位值，但未统一；统一前需确认回放/下载 SSRC
   是否必须可复现（回放控制/停流需要按 SSRC 反查会话）。
7. **TCP 信令**已与 UDP 统一分发，但 `handle_packet` 的参数已达 23 个，
   后续应改为上下文结构体，否则每次新增能力都要再穿一遍全部调用点。

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
| 后端测试 | **395 通过**（lib 348 + 集成 47）/ 2 忽略 / 0 失败 |
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
| 录像计划 (`record_plan/`) | 6 | ✅ 完整 | add/update/delete/query/link/channel_list |
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
- [ ] **JT1078 协议操作层 12 个端点**（live/record/snap/temp_position_tracking/confirmation_alarm/playback_download/media_upload_delete/terminal_channel_*）
  - 现状：保留为"已受理"响应（log + success），需在线终端 + JT/T 808/1078 协议栈才能真下发
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
