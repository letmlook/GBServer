# GBServer 遗留问题与进度（Open Issues）

> 生成时间：2026-09-13（本地 08:3x）
> **最近更新：2026-10-25（第十一轮深度代码审查，HEAD `14292cf`）**
> —— 本轮做了一次**深度代码审查**：聚焦**多方言运行期改造的爆炸半径**与
> **十轮以来几乎没被正面覆盖的协议层**（`sip/`、`jt1078/`、`zlm/`、
> `scheduler/`、`cascade/`），并派两个只读侦察 Agent 并行覆盖非 DB 层。
> 共 **13 条**发现，**已全部修完并逐条变异验证**（B32~B44）：其中 **2 条 P0**
> （SIP Keepalive 无鉴权可劫持任意设备；JT1078 接收缓冲无上限可致 OOM）、
> **5 条 P1**、**6 条 P2**。
> 门禁：`cargo test --lib` **871 通过 / 0 失败**；真实库专项 **11 通过 / 0 失败**
> （PG 16 / MySQL 8 容器）；`cargo fmt --check` **exit 0**；**每条修复都做了
> 变异验证**（先让测试变红再还原）。
> 上一轮（2026-09-19 真实设备接入核验轮）把第一台**真实国标设备**
> （EasyGBD `34020000001320128497`，TCP）接入平台并端到端实测，
> 据此更新 D1（真机核验）、B1 / B6（真机复现的新证据）、C2（零引用 db 函数重算），
> 并新增 B14 / C8 / C9 / E6 四项。
> 代码基线：2026-09-13~19 的功能提交（延迟列 / 通道播放对话框 / 缩略图落盘 / 直播页 WebRTC /
> 控制台改版 / 用户管理修复 `bfa65f8` / 主界面标题与按钮布局 `8ecec6a`）
> 测试基线（2026-09-19 实测 @ `8ecec6a`）：`cargo test --no-fail-fast` **746 通过 / 0 失败 / 3 忽略**；
> e2e 最近一次运行 35 通过 / 0 失败（筛选运行，非 59 条全量）。
>
> ⚠️ 注意：`743 通过` 等更早的数字**在修复前无法复现** —— 测试目标曾因
> `test_support.rs` / `lib.rs` 的测试构造器缺字段而**编译失败**，同日已修。这是测试代码问题，不涉及运行时行为。
>
> 本文档**只列尚未完成/尚未验证的事项**。已实现能力、设计决策与真机核验结论见
> [`STATUS.md`](STATUS.md)；逐轮修复过程记录已按「只保留最新状态」清理，需要时走 git 历史。
> 更新规则：每轮修完一批就把对应条目移出本文档；新增未完成项必须在这里登记，
> 不允许只写在提交信息里。

## 图例

| 标记 | 含义 |
|---|---|
| 🔴 | 未实现（端点/功能缺失） |
| 🟠 | 已定位的真实缺陷，尚未修复 |
| 🟡 | **部分完成**：已修但未完全复验，或已核验但仍有未覆盖项 |
| 🔵 | 代码债 / 待清理（不影响功能正确性） |
| ⚪ | 明确范围外（记录在案，不计划实现） |
| ❓ | 待复现/待确认，尚不能定性 |

---

## 总览

| # | 类别 | 事项 | 标记 |
|---|---|---|---|
| A1 | 功能端点 | 中亿视图（SY）定制模块 10 条端点已补齐为 23 条 `/api/sy/*`，全部可路由且受鉴权保护 | ✅ |
| A2 | 功能端点 | 管理员诊断及 Redis/ZLM 故障演练已完成；目标部署环境复验仍属 M4 | ✅ |
| A3 | 功能端点 | 外部集成协议 `/api/v1/*` 11 条 + `/auth/login` | ⚪ |
| B1 | 缺陷 | 普通流无效回连已移除，TCP-PASSIVE 参数与媒体等待已修；真实 ZLM + Mock 正反场景通过，真机待复验 | 🟡 |
| B2 | 缺陷 | 录像计划 `startRecord` 与设备推流的竞态 | ✅ 已修并复验 |
| B3 | 缺陷 | 假设备 mock 缺 `connection_lost` → 进程崩溃（"设备在线但无 RTP"） | ✅ 已修并复验 |
| B4 | 环境 | 后台进程"静默退出"= 同组 job 被中止连带杀进程（已定性，非缺陷） | ⚪ |
| B5 | 测试 | `cloudRecord` e2e 已按本轮新 MP4 验收，ZLM + SIP Mock + Chromium 3/3 通过 | ✅ |
| B6 | 缺陷 | `send_session_bye` 会话定位过粗（跨类型/跨代际误停） | ✅ 已修（2026-09-19） |
| B7 | 缺陷 | `/api/play/webrtc` 调 ZLM 的形态错误 → 恒失败（**已修，普通流真实浏览器验证可播**） | ✅ 已修 |
| B10 | 缺陷 | ZLM 流列表反序列化失败（`"fps": 25.0` 浮点 vs `u32`）→ 流列表恒为空 | ✅ 已修 |
| B11 | 缺陷 | `on_server_started` 用 ZLM **内部** HTTP 端口覆盖库里的对外端口（80 覆盖 8080）→ 后端所有 ZLM 调用 502 | ✅ 已修 |
| B12 | 缺陷 | ZLM 对同一路流**按协议各返回一行** → 控制台「重点通道」同一通道重复铺格子、「直播 N」虚高 | ✅ 已修并复验 |
| B13 | 性能 | 控制台首屏"等最慢接口"才统一赋值 + `system/info` 被 3 个组件同时拉 → 卡片空等约 2s | ✅ 已修并复验 |
| B14 | 缺陷 | `DeviceControl` / `DeviceConfig` 应答按设备、SN、类型和目标关联；传输模式、设防/撤防、远程录像、配置下发、重启、预置位、直接 PTZ 与批量 PTZ 停止/重启可区分发送和执行；**远程启动、报警复位、强制关键帧、看守位、拉框、配置下发等其余控制入口已补齐 `sipSent`/`deviceResult` 与 `waitResult`**；前端控制与回放控制兼容入口明确只声明已发送 | 🟡 真机未验 |
| B15 | 缺陷 | 设备保活/注册统计查了**库里不存在的表**（真实表名 `gb_device`）→ 两个统计接口恒返回 0 | ✅ 已修 |
| B17 | 缺陷 | 代码审查发现：录像列表过滤错列、SY collect/delete 入参不一致、SY 通道端点只收数字主键、sy/test 缺管理员校验与吞错、API Key 过期时间非法输入静默当"永不过期"、序列回拨致 id 重复发放、非 public schema 下序列同步静默失效、`MAX(int4)` 解码失败、JT1078 鉴权路径写副作用与鉴权状态残留、录像控制重复实现、`serde_flex` 变体顺序致 20 位数字串溢出 | ✅ 已修并复验（B17 自身的根因有两处写错，见条目内的第二轮补充） |
| B18 | 缺陷 | **第二轮**代码审查（复查 B17 的修复本身）：`serde_flex` 饱和转换静默给错编号、日期选择器仍可自由输入使 400 保护失效、`resolve_channel_pk` 数字优先导致静默假成功、SY collect/delete 契约改坏 + 裸 415 文本、JT1078 鉴权码写库失败仍下发导致终端永久锁死、过期时间按 UTC 解释偏移 8 小时、`sy/test` 的 `database` 标志误判、录像过滤无边界跨设备误命中、取消收藏回假成功、JT1078 超时不置离线、`seq_sync` 白名单静默跳过异名表、4 个死代码警告、PendingGuard 注册窗口、模块环与重复日志、提交信息不实宣称、e2e 空断言、jt1078 时间格式混写 | ✅ 已修并复验 |
| B19 | 缺陷 | **第三轮**代码审查（复查第二轮的 9 个提交）：录像过滤的边界只认 `_` 而仓库写了四种流名形状（`record_info` 的 `{dev}/{ch}`、JT1078 的纯 `{phone}`）→ 被静默过滤掉、`cleanup_once` 在所有入站报文都要拿的 `sessions` 锁内做 DB 写（实测阻塞 5.29s，连接池耗尽可到 30s/终端）→ 能掐住整个 JT1078 信令面、平台删除端点回裸文本 422、`resolve_channel_pk` id 形态歧义静默选错通道、锁修复引入的 TOCTOU（在线终端被写回离线）、鉴权码读失败覆盖运维配置、`phone_number` 无唯一约束致重复行读取报错、`sy/test` issues 重复、`register_time` 基准不同 | ✅ 已修并复验 |
| B20 | 缺陷 | **第四轮**代码审查（复查第三轮的 5 个提交）：「置终端离线」的**条件写是空条件**（`deadline` 取晚 + 秒级精度下 `<=` 恒成立）→ 在线终端仍被写回"离线"、第三轮那条测试还**测反了**（变异测试：只把实现改成正确形态，该测试 25 次红 21 次）、`phone_number` 重复行 → "取哪一行"不确定 → 运维配的鉴权码被静默抹掉、B19 ⑦ 的根因陈述**不成立**（sqlx 0.7.4 的 `fetch_optional` 多行时静默返回首行，根本没有 "expected at most one row" 这个错误串）、删除端点**空 body** 从"422报错"变成"200 但一条都没删"、body 超 2MB 仍是裸文本 413、摘会话后未复查会话是否已复活、注册路径重复读库 | ✅ 已修并复验 |
| B16 | 缺陷 | 跨方言类型/宽度不匹配 —— postgres 主键是 INT4（解成 i64 报错）、`parent_id` 是 VARCHAR 却绑成 i64、`serial` 序列没跟上种子行、`SELECT 1` 字面量是 INT4、审计表少 `elapsed_ms` 列 → 云录像计划调度器、报警列表/详情、录像计划通道列表、**通道编辑**、**新建用户/角色**、审计耗时在 postgres 上整个不可用 | ✅ 已修并复验 |
| B9 | 配置 | `rtc.externIP` 平台未下发 → 容器部署下浏览器 ICE 永远连不上 | ✅ 已修 |
| A4 | 前端 | 直播页 WebRTC 播放入口（`postWebrtcPlay` 原先无调用方） | ✅ 已接 (2026-09-19) |
| C1 | 代码债 | `handle_packet` 23 个参数 | 🔵 |
| C2 | 代码债 | 29 个无引用的 `db::` 函数（逐条判定删除/接上；上一版记为 36 个） | 🔵 |
| C3 | 缺陷/代码债 | JT1078 注册应答写死 `"GBServer"`、0x0102 被误当属性上报 → 已改为终端鉴权（0x8102 应答），鉴权码从库取/生成入库，强制/宽松两档 | ✅ 真机未验 |
| C4 | 代码债 | API Key 过期记录不清理 —— 已接后台清理任务，过期记录按小时清除 | ✅ |
| C5 | 代码债 | ~~`/api/user/users` 的 `UsersQuery` 无 `query` 字段~~ | ✅ 已修 (`bfa65f8`) |
| C6 | 缺陷 | 用户管理权限缺口：9 个管理端点无角色校验（普通用户可造管理员级角色） | ✅ 已修 (`bfa65f8`) |
| C7 | 缺陷 | 删角色不校验引用 → 悬空 `role_id` 用户被 INNER JOIN 静默吞掉 | ✅ 已修 (`bfa65f8`) |
| C8 | 缺陷 | 直播响应的 `hasAudio` 优先取 ZLM 当前流的 ready 音频 track，无 track 时才回退到目录字段，并用 `audioSource`（`media`/`catalog`）标明来源；目录字段在库里以 NULL 表示"设备没说"，与"设备说没有"可区分 | 🟡 真机未验 |
| C9 | 契约核对 | GET/DELETE 登录后 HTTP 与两套存储状态、失败回滚均已验证；PostgreSQL/MySQL 隔离库往返通过 | ✅ 已关闭 |
| D1 | 真机核验 | GB28181 真实设备（TCP 被动 / 401 鉴权 / SDP 端口差异 / 目录分页） | 🟡 已接入并核验主链路，剩余项见 D1 |
| D2 | 真机核验 | JT1078 真实终端（0x0802 变体 / 双向对讲）；**0x8100 鉴权码的平台侧已按标准实现**（见 C3），待真机核对实际报文 | 🔴 |
| D3 | 真机核验 | 对讲音频互通（G.711A 时间戳/回声/抖动） | 🔴 |
| E1 | 环境 | 冒烟脚本 2 项"预期为真"项（CSV 非 JSON、dummy 代理 404） | ⚪ |
| E2 | 环境 | 本 ZLM 版本缺 7 个 hook 事件键（已如实告警） | ⚪ |
| E3 | 环境 | 长驻服务需单独 job 启动（同组 job 中止会连带杀掉） | ⚪ |
| E4 | 环境 | CI 自动触发按用户要求保持关闭（`workflow_dispatch`） | ⚪ |
| E5 | 部署 | ZLM 改 host 网络 + 配置文件只读管理 + mac 叠加文件 | ✅ 已改 |
| E6 | 环境 | 后端访问 `192.168.3.88:8080`（ZLM）偶发 `error sending request`。**2026-10-20 复核**：该地址是 `config/application.toml` 里写死的开发机 LAN IP；本机 LAN IP 已变为 `192.168.3.149`，用旧值时 ZLM 节点连不上、WebRTC ICE 候选不可达（`iceConnectionState` 停在 `checking`）。用 `GBSERVER_ZLM_PRIMARY_IP` / `GBSERVER_ZLM_PRIMARY_RTC_EXTERN_IP` 覆盖后恢复（见 `e2e/README.md`） | ⚪ 需按本机 IP 覆盖 |
| E7 | 部署验收 | PostgreSQL 与 SQLite 后端容器 + 真实 ZLM + SIP Mock 的 FLV 点播/停止、MP4 下载及 Chromium WebRTC 已通过；SQLite 全新卷可自动建库，时区对齐后完整 Chromium E2E 72/72；真机 LAN 与目标 Linux Compose 部署待验收 | 🟡 |
| B32 | 缺陷 | **第十一轮**：SIP Keepalive（MESSAGE，**无鉴权**）可改写**任意已注册设备**的信令地址 → 设备被劫持 + 平台变成出站跳板（19 处出站信令全改发攻击者） | ✅ 已修并复验 |
| B33 | 缺陷 | **第十一轮**：JT1078 会话接收缓冲无上限 + `last_heartbeat` 在解析前刷新 → 未认证 UDP 可把服务端内存撑爆（OOM） | ✅ 已修并复验 |
| B34 | 缺陷 | **第十一轮**：JT808 校验码按**转义后**字节计算 → 正文含**奇数个** `0x7E`/`0x7D` 时差 `0x01`，终端拒收（既有往返测试用偶数个，把洞掩盖了） | ✅ 已修并复验 |
| B35 | 缺陷 | **第十一轮**：SIP TCP 分帧要求整块缓冲是合法 UTF-8 → GB2312 正文里的一个字节让**整条连接**永久卡死 + 缓冲无上限增长 | ✅ 已修并复验 |
| B36 | 缺陷 | **第十一轮**：级联「keepalive 超时检测」判的是**距上次 REGISTER 多久**（`note_liveness` 生产调用点为 0）→ 健康上级每 3~4 分钟被误判 `Failed` | ✅ 已修并复验 |
| B37 | 缺陷 | **第十一轮**：全局方言入口未收口（14 处调用点手上有 pool 却读进程级全局）→ 审计日志保留策略在 **PG/MySQL 上 100% 失效** | ✅ 已修并复验 |
| B38 | 缺陷 | **第十一轮**：JT1078 缺失序号统计因 u16 回绕构造出 **65533** 条 → hook 收 400KB、UDP 重传必超包长 | ✅ 已修并复验 |
| B39 | 缺陷 | **第十一轮**：入站 INVITE 无重传去重（服务端事务层是死代码）→ 每次重传新开一个 RTP 端口，设备仍推流到孤儿端口，且端口泄漏 | ✅ 已修并复验 |
| B40 | 缺陷 | **第十一轮**：级联注册不读服务端在 200 OK 里授予的 `Expires` → 上游侧已过期而本级自认在线 | ✅ 已修并复验 |
| B41 | 缺陷 | **第十一轮**：级联 `server_port` 用 `as u16` 静默回绕（70000 → 4464），入库无范围校验 | ✅ 已修并复验 |
| B42 | 缺陷 | **第十一轮**：录像调度器「本次不录像」但仍跑满 20s 重试 → tick 超时后 `interval` Burst 退化成连续紧循环 | ✅ 已修并复验 |
| B43 | 缺陷 | **第十一轮**：ZLM 节点流计数是**事件计数**而非实际流数 → 漂移永久偏高，多节点选路长期偏斜且无告警 | ✅ 已修并复验 |
| B44 | 缺陷 | **第十一轮**：`has_schema` 把所有错误吞成 `false` → ZLM 不可达时静默摘掉 `hlsUrl`，现场无法区分 | ✅ 已修并复验 |

---

## 核心闭环健康度

结论：**没有发现导致全流程跑不起来的阻塞问题**。9 条核心闭环逐条实测
（2026-09-13 模拟设备基线 + 2026-09-19 真机补充）：

| # | 闭环 | 结果 | 证据 |
|---|---|---|---|
| 1 | 设备接入（注册/心跳/目录） | ✅ | 2026-09-13：在线设备 2、通道 4。**2026-09-19 真机**：`34020000001320128497` 401 摘要注册成功、Keepalive 30s、目录 1 通道入库、注销后重注册 |
| 2 | 实时点播 → ZLM → FLV | ✅ | 2026-09-13：FLV 实拉 **2,408,800 字节**；停止时发 BYE。**2026-09-19 真机**：FLV 实拉 **3,291,903 字节 / 6s**，`ffprobe` 识别 H264 1080×1920 + pcm_alaw |
| 3 | 历史回放 + 回放控制 | ✅ | pause/speed/seek 全 code=0，停回放正常（未在真机验证） |
| 4 | 对讲（含音频上行） | ✅ | 假设备收到 **150 个 RTP 包**，BYE 校验通过（未在真机验证） |
| 5 | 语音广播 | ✅ | start/stop code=0（未在真机验证） |
| 6 | 设备控制（PTZ/布防/录像/远程启动/配置查询） | ✅ | 5 项全 code=0（模拟设备）；**真机**仅验证传输模式切换，且应答未被解析（B14） |
| 7 | JT1078（终端/通道） | ✅ | 终端列表 total=4、通道列表 code=0（未在真终端验证） |
| 8 | 级联平台 | ✅ | 平台记录在册（推流/点播已在此前轮次端到端验证） |
| 9 | 云录像（计划→录制→落库→播放→删除） | ✅ | `cloudRecord` spec **3/3 通过**，落库 1.0MB MP4 |

另有：`cargo test --no-fail-fast` **746 通过 / 0 失败 / 3 忽略**（2026-09-19 实测 @ `8ecec6a`）、
e2e 最近一次运行 35 通过 / 0 失败（筛选运行）、三方言冒烟仅剩 2 项已记录预期项（E1）。

### 阻塞/非阻塞判定

| 事项 | 是否影响核心闭环 | 判定 |
|---|---|---|
| A1 中亿视图 10 端点 | 否（第三方定制集成模块） | ✅ 已补齐并复验 |
| A2 `/api/test/*` | 否（运维诊断） | 🟢 可发布，后续迭代 |
| A3 `/api/v1/*` 外部集成协议 | 否（另一套协议/鉴权） | 🟢 范围外 |
| B1 端口不一致时的 `connectRtpServer` | 普通流无效回连已移除；真实 ZLM + Mock 证实 TCP-PASSIVE 成功流可拉 FLV、无媒体时清理资源 | 🟡 真机待复验 |
| B2 录像计划竞态 | 曾影响（已修已复验） | ✅ 已关闭 |
| B3 mock 崩溃 | 否（测试基建） | ✅ 已关闭 |
| B4 进程"静默退出" | 否（会话操作副作用） | ✅ 已定性 |
| B5 cloudRecord 用例时序 | 否（测试用例脆弱点） | 🟢 可发布，后续加固 |
| B6 会话 BYE 定位过粗 | 曾导致跨类型/跨代际误停 | ✅ 已修：按业务类型或 ZLM 流 ID 精确定位，终止会话不再参与选择 |
| B14 命令应答真机复验 | Mock 的设备执行结果已关联并区分；真机与其它控制入口仍缺证据 | 🟡 待验收 |
| C1 `handle_packet` 23 参数 | 否 | 🟢 可发布 |
| C2 29 个无引用 db 函数 | 否 | 🟢 可发布（清理） |
| C3 JT1078 鉴权码只存不用 | **仅真终端**有风险（平台自身不校验，mock 不校验） | ✅ 已修（0x0102 按标准作终端鉴权，鉴权码从库取，强制/宽松两档）；真机未验 |
| C4 API Key 过期记录不清理 | 否（鉴权已正确拒绝） | ✅ 已关闭 |
| C8 `hasAudio` 与实际流不符 | 否（仅元数据/前端图标不准） | 🟢 来源语义已分清（`audioSource`），真机未验 |
| C9 收藏删除方法核对 | GET/DELETE 登录后 HTTP 与两套存储状态、失败回滚通过；PostgreSQL/MySQL 运行复验通过。云录像页面当前没有收藏入口 | ✅ 已关闭 |
| D1 真机剩余项 | 未知风险（模拟器覆盖不到的行为） | 🟡 主链路已通，其余项真机联调前不建议对外承诺 |
| D2–D3 真机核验缺口 | 未知风险（模拟器覆盖不到的行为） | 🟡 真机联调前不建议对外承诺 |

---

## A. 未实现的功能端点

### A1 ✅ 中亿视图（SY）定制模块 10 条端点（已补齐并复验）

来源：第三方地图视图（中亿 SY）定制对接需求。原先 `/api/sy/*` 只挂了 13 条，
契约清单里的 10 条要么没有实现，要么只有**非 SY 前缀**的等价实现 —— 对接方按
契约请求 `/api/sy/...` 会拿到 404。

现已补齐为 **23 条 `/api/sy/*`**，10 条契约端点全部可路由，且按
「复用已有实现、只加前缀别名，不重写逻辑」的要求实现
（`src/handlers/sy_contract.rs`）：

| 端点 | 语义 | 复用的实现 |
|---|---|---|
| `GET /api/sy/camera/one` | 单通道详情 | `/api/common/channel/one` |
| `GET /api/sy/camera/update` | 更新通道 | `/api/common/channel/update`（同一份 DTO 与落库逻辑） |
| `GET /api/sy/push/play` | 推送播放（含校验） | `/api/play/start` + 平台侧前置校验 |
| `GET /api/sy/push/play-without-check` | 推送播放（跳过校验） | `/api/play/start` |
| `GET /api/sy/record/collect/add` | 录像收藏/加入收藏夹 | `/api/cloud/record/collect/add` |
| `GET` + `DELETE /api/sy/record/collect/delete` | 取消收藏 | `/api/cloud/record/collect/delete` |
| `GET /api/sy/record/zip` | 录像打包下载（ZIP） | `/api/cloud/record/zip` |
| `GET /api/sy/record/list-url` | 录像列表（仅 URL 形态） | `/api/cloud/record/list-url` |
| `GET /api/sy/forceClose` | 强制关闭流 | `/api/push/forceClose` |
| `GET /api/sy/test` | 对接自检 | 平台就绪度（DB / 设备 / 媒体节点 / SIP） |

两点语义说明：

- `push/play` 与 `push/play-without-check` 的区别是**平台侧前置校验**：
  前者先确认设备已登记、通道属于该设备且设备在线，不满足就返回业务错误。
  但要说准确：点播链路（`play::play_start`）自己也会做同样的三项检查，而且
  都在发起 INVITE **之前** —— 所以**两个端点都不会对着未知目标发 SIP**，
  真正的差别只是错误文案（SY 口径 vs 平台口径）与检查顺序。
- `sy/test` 只回报就绪度（版本、计数、布尔、问题列表），**不**回报节点地址、
  密钥或完整 Hook 载荷。

全部挂在受保护路由表：未登录返回 401，而不是 404。
（`/api/cloud/record/collect/delete` 的 GET 与 DELETE 路由均已注册；登录后 HTTP
业务往返已按 C9 验证。当前云录像页面没有收藏入口。）

### A2 ✅ 管理员诊断端点（本机故障演练已完成）

`GET /api/test/hook/list` 只返回已鉴权的 `on_play` / `on_publish` 最近一条
白名单摘要，保留范围由 `capturedTypes` 明示；其它 Hook 尚未建立可信来源验证，
不纳入快照。`GET /api/test/redis` 区分未配置、连通和不可用，PING 最多等待
1 秒，故障原因使用固定码。两条路由调用 `authz::require_admin`；自动化已覆盖
401/403/200、拒绝 Hook 不入快照、secret 脱敏及 Redis 超时。

本机 Docker 环境已对 Redis、ZLM 分别执行停止与恢复演练，响应使用脱敏错误码；
目标部署环境的复验归入 M4。

### A3 ⚪ 外部集成协议 `/api/v1/*`（范围外）

`/api/v1/{login,getserverinfo,userinfo,device/list,device/channellist,device/fetchpreset,stream/{start,stop,touch},control/{ptz,preset}}`
共 11 条 + `/auth/login`。

**为何范围外**：这套接口使用**第三方签名鉴权**（`sign` 参数，不是本平台的
JWT/API-Key），是另一套独立的集成协议；且需要与其返回结构逐字段对齐才有意义。
若将来要支持，应作为独立里程碑（新增鉴权过滤器 + 独立 DTO 层），不要在现有
handler 上打补丁。

---

## B. 已定位的缺陷

### B1 🟡 普通流与 TCP-PASSIVE 模拟场景已修，真机待复验

**2026-10-18 当前进展**：普通流在 ZLM 报告媒体到达后只记录 SDP 端口差异，
不再调用 `connectRtpServer`。另修正了 `on_rtp_server_started` 把“端口已打开”
误判为“媒体已到达”的问题。真实 ZLM + SIP Mock 运行验证：只回 SIP 200、
不推 RTP 时，点播约 15.0 秒后返回媒体超时并向 Mock 发 BYE；开启 ffmpeg
RTP/PS 后约 5.3 秒点播成功，FLV 可读取 `FLV` 文件头，停止接口成功。
**2026-10-18 TCP-PASSIVE 进展**：真实 ZLM 证明 `openRtpServer` 必须传
`tcp_mode=2`，而 `connectRtpServer` 的 `dst_url` 只能传主机/IP，端口另传。
修正后，无设备媒体时 HTTP 等约 15.0 秒返回媒体超时，ZLM RTP Server
无残留，Mock 收到匹配会话的 BYE；在本机 TCP relay 发送 44 个 RTP/PS 包时，
约 5.73 秒返回成功，FLV 实取 `FLV` 文件头，停止接口成功。
真机端口差异与 TCP-PASSIVE 行为仍需复验。以下保留历史复现与旧候选方案。

**现象**（2026-09-13 模拟设备，2026-09-19 真机两次复现）：

```
# 2026-09-13 08:28（模拟设备）
start_live_stream: connectRtpServer rtp://127.0.0.1:10000 failed:
  ZLM error: 仅支持tcp主动模式

# 2026-09-19 13:49:30 / 14:56:11（真机 EasyGBD，TCP-PASSIVE）
Device 200 OK m=20002 != ZLM RTP server port 30066. Switching to ZLM connectRtpServer to device:20002
connectRtpServer to device:20002 failed: ZLM error: 仅支持tcp主动模式
Device 200 OK m=20014 != ZLM RTP server port 30038. Switching to ZLM connectRtpServer to device:20014
connectRtpServer to device:20014 failed: ZLM error: 仅支持tcp主动模式
```

**背景**：设备在 INVITE 的 200 OK SDP 里宣告**自己的**收流端口（真机每次都不同：
20002 / 20010 / 20014…），此时平台需要调 ZLM `connectRtpServer` 让 ZLM 主动去连
（TCP 主动模式）；但 ZLM 对 `rtp_type=0`（UDP）拒绝该调用。

**代码位置**：`src/handlers/play.rs:344-375`。端口不一致只打 `tracing::warn!`，
`connect_rtp_server` 失败只打 `tracing::error!`（362-364）并**不返回错误**，继续走
下面的 `ApiResult::success`；对照 `play.rs:295-307` 的 TCP-PASSIVE 分支是
**会返回错误**的（304）。

**真机带来的新认识（改变了影响评估）**：这台真机**仍然把 RTP 推到了 INVITE 指定的
ZLM 端口**（`on_publish from 192.168.65.1` → 流正常注册 → FLV 实拉 3.29MB）。
也就是说：**该回退路径失败并没有挡住实时流**，只是平台白跑了一次必然失败的调用 +
打了一条 ERROR 日志。真正受影响的只有"确实要求 ZLM 主动回连"的设备类。

**候选方案**（择一，需要决策）：
1. 检测到设备 200 OK 端口与 INVITE 端口不一致时，**记录一条明确的业务提示**
   （设备画像：可能需要 TCP 主动模式），而不是打 ERROR 日志后静默继续；
2. 对 UDP 设备改用"关掉原 RTP server → 在新端口重开 → 让设备按新端口推流"的方式；
3. 强制这类设备走 TCP-PASSIVE（配置层约束）；
4. 若确认设备实际按 INVITE 端口推流（如本机真机），可把这次回连**降级为 INFO**
   并在若干次成功推流后**跳过**该回退，减少无效调用与噪声。

### B2 ✅ 录像计划 `startRecord` 竞态（已修复并复验）

**根因**：`RecordPlanScheduler` 拉起设备流后**立即**调 ZLM `startRecord`，
而 ZLM 注册流是异步的（`on_stream_changed` 之后才可见），于是 `startRecord`
回 `can not find the stream`；此前只调用一次就放弃 → **"录像计划时有时无"**。

**修复**（工作树，未提交）：新增 `start_record_with_retry`（25 × 800ms ≈ 20s 有界重试，
只在"流还没到"时重试）、"已经在录"视为幂等成功。

**已验证**：手动触发后 `started MP4 recording` 一次成功；停止录制后
`gb_cloud_record` 落库 `2026-09-13-08-17-51-0.mp4 (1000165 bytes)`。

**复验（2026-09-13，B3/E3 环境问题排除后）**：`cloudRecord` spec
**3/3 全通过**（列表 57.8s、播放 3.8s、删除 3.9s）；此前多次 skip/fail
均由 B3（mock 崩溃）与 E3（同组 job 被杀）造成。

### B3 ✅ 假设备 mock 崩溃（已修复并复验）

**现象**：mock 进程抛
`AttributeError: 'SipDeviceMock' object has no attribute 'connection_lost'` 后**退出**；
而平台内存里设备仍是在线（keepalive 未超时），表现为
**"设备在线，但一帧 RTP 都不推"**，极易误判成平台缺陷（本轮就误判过一次，
导致 `cloudRecord` 三条用例全 skip）。

**修复**：给 `SipDeviceMock` 补 `connection_lost` / `error_received`
（已复验：修复后 mock 连续跑满整轮 e2e 不再退出）。

### B4 ✅ 后台进程"静默退出"：已定性为**会话操作副作用**（不是产品缺陷）

**现象**：2026-09-13 08:28:36，18080 后端与 SIP 假设备**同一秒**消失；
后端无 panic / 无 `shutdown` 日志，mock 侧留下 `Shutting down...`（它自己的
信号处理）。`~/Library/Logs/DiagnosticReports/` 里没有对应崩溃报告。

**结论**：两者是被**同一个信号**杀掉的 —— 原因是把 `nohup` 启动的服务和
一个前台任务写进了**同一条命令/同一个 job**，随后该 job 被中止（本会话确实
中止过一个正在跑 e2e 的后台 job），信号连带杀掉了同组的 mock 与后端。
**不是产品缺陷**，但会伪装成"设备在线却不推流 / 接口连不上"，
已在本轮排查中浪费过一次时间。

**处置**：长驻服务必须**单独**用 `run_in_background: true` 启动（或 `setsid`），
不要和会被中止的任务同组；启动命令已追加 `BACKEND EXIT CODE=$?` 便于下次定性。

### B5 ✅ `cloudRecord` e2e 已按本轮新 MP4 验收（Mock + 真实 ZLM）

**现象**：同一 spec 在不同轮次出现 `66 passed` / `65 passed + 1 skipped` /
`3 skipped` / `2 failed` 等多种结果。

**已定性的部分**：`3 skipped`、`2 failed` 的直接原因是 B3（mock 崩溃）+ B4
（后端消失）导致 `ensureRecording` 拿不到通道/JSON 解析失败 —— **环境问题，非用例缺陷**。

**2026-10-17 验证**：用例先同步托管 SIP Mock 的目录，建立唯一录像计划并保持
20 秒录制窗口，再停录并轮询本轮新增、对应通道的非空 MP4 数据库记录。
Docker Desktop 上的真实 ZLM、ffmpeg SIP Mock、临时 SQLite 和 Chromium 完整跑通
3/3；第二轮从空通道表开始，得到计划 3、记录 2、125446 字节 MP4，随后播放、
下载和删除均通过。后端/ZLM/Mock 进程异常有独立健康诊断。此项只证明 Mock
设备链路；真机录像仍属 M4 验收。

### B6 ✅ `send_session_bye` 会话定位过粗（已修，2026-09-19）

修复将停止入口改为按 `StreamType` 选择最新未终止的业务会话；ZLM 的
`on_stream_none_reader` 与 RTP 超时 hook 则按已知 `stream_id` 精确定位。ZLM 流标识
也改为按 INVITE 的 Call-ID 回填，不会覆盖同通道的其他会话。新增回归测试覆盖同通道
直播、回放、下载会话的隔离与已终止旧会话的排除；`cargo test --lib` 在允许本地端口绑定
的环境中 691 通过、0 失败。

**证据（2026-09-13 实测）**：只开了一路**回放**（`playback_34020000001320000001_1789260493987`），
随后调用 `/api/play/stop`（本意是停**实时**流）：

```
GET /api/play/stop/34020000001320000001/34020000001320000001
→ {"code":0,"data":{"callId":"playback_34020000001320000001_1789260493987"}}   ← 停掉的是回放
```

**根因**：`SipServer::send_session_bye(device_id, channel_id)` 内部用
`InviteSessionManager::get_by_device_channel`，只匹配 `(device_id, channel_id)`
且 `status != Terminated`，**不看 `StreamType`**。同一个通道可能同时存在
`play_` / `playback_` / `download_` 三条会话，于是：

* `play_stop` 可能挑到回放会话、`playback_stop` 可能挑到直播会话；
* 同一类型有多条（同一通道重复点播）时挑到哪条是**不确定**的（HashMap 顺序）；
* ZLM 的 `on_stream_none_reader` / RTP 超时路径同样按 `(device, channel)` 调它 ——
  一路流的观看者走光，可能把**另一路**流的会话 BYE 掉。

**次要症状**：设备收到"没有对话"的重复 BYE（mock 侧统计
`valid=7 / invalid=4`，100% 是 `unknown-dialog`；后端日志同一 call_id 出现两次
`Sent session BYE`），真实设备会回 481/400 并可能记录协议告警。

**真机复现（2026-09-19，EasyGBD `34020000001320128497`）**：14:21 起了一路直播，
14:44:02 因 RTP 超时自动 BYE（`Sent session BYE … call_id=play_…_1789798876484`）。
14:56:10 重新点播（新 call_id `play_…_1789800970170`），14:56:33 调
`/api/play/stop/34020000001320128497/34020000001310000001`：

```
{"code":0,"data":{"callId":"play_34020000001320128497_1789798876484"}}   ← 停掉的是 14:21 那条已终止会话
14:56:33.538 Sent session BYE to device 34020000001320128497 channel 34020000001310000001
              call_id=play_34020000001320128497_1789798876484
14:56:33.554 Skip duplicate ACK for call_id=play_…_1789798876484 (already sent)
```

即：**当前这条会话（`1789800970170`）从未收到 BYE**（会话泄漏），平台把 BYE 发给了
一条 34 分钟前就已终止的对话；真机对未知对话的 BYE 回了 200 OK（宽容），
但严格实现的设备会回 481/400 并记协议告警。
由此又暴露一层：**RTP 超时 BYE 之后会话没有被标记为 `Terminated` 或从管理器移除**，
否则不会被 `get_by_device_channel` 选中。

**已实施的修复**：
1. `send_session_bye` 增加 `stream_type`（或 `stream_id`）参数：`play_stop` 只找
   `Play`、`playback_stop` 只找 `Playback`、下载只找 `Download`；
2. hook 侧（`on_stream_none_reader`、RTP 超时）**已有 `stream_id`**，
   应按 stream_id 精确定位会话，而不是退回 `(device, channel)`；
3. 同一类型多条时按 `created_at` 最新的一条停（或全部停并逐个发 BYE）；
4. **会话终止后必须从管理器移除或置为 `Terminated`**（超时 BYE、收到设备 BYE、
   481 之后都要做），否则下次 stop 仍会挑到尸体会话。

**调用面（2026-09-19 复核）**：`send_session_bye` 定义在 `src/sip/server.rs:5792`，
签名仍是 `(device_id, channel_id)`；`get_by_device_channel` 实现在
`src/sip/gb28181/invite_session.rs:455-461`，过滤条件只有
`device_id` / `channel_id` / `status != Terminated`（`InviteSession.stream_type`
字段存在但从未参与筛选）。**12 处调用**全部是 2 参数：
`zlm/hook.rs:949,1601`；`handlers/play.rs:255,280,302,412,451`；
`handlers/common_channel.rs:890,2090`；`handlers/playback.rs:338,550,999`。

**影响评估**：不阻塞主流程（每个流程单独跑都通），
但"直播 + 同通道回放/下载并发"、"无人观看自动关流"以及**同一通道反复点播**
（真机实测）场景会**停错流或漏停流**。

**2.0 前端补充**：通道播放弹窗此前吞掉 `/api/play/stop` 的业务错误，
仍提示“已停止”；关闭弹窗又经 `@close`、属性监听和卸载路径重复发停止请求。
现失败时保留可重试状态，成功后清理本地会话，重复关闭不再重发。
Chromium 使用停止失败/成功模拟响应验证页面反馈和请求次数；真实设备停流仍待 M4 复验。

**关闭与起流的竞态**：停止命令只在存在本地会话时才发，而起流响应可能晚于
关闭动作到达。此时后端那条 SIP 会话和 ZLM 收流端口已建好却没人接管，流一直挂着。
现于起流响应返回后检查“关闭是否已发生”，是则立刻停流并给出未确认提示。
浏览器回归分别覆盖“关闭在起流之后被观察到”与“关闭在起流之前被观察到”两种顺序；
后者修复前停止请求数为 0，修复后为 1。

### B7 ✅ `/api/play/webrtc` 调 ZLM 的形态错误（已修并验证）

**原实现**：把 `{secret, app, stream, type, sdp}` 整体当 **JSON body** POST 给
ZLM `/index/api/webrtc` → ZLM 一律回
`Required parameter missed: "type"` —— **该接口从来没有成功过一次**。

**根因**：ZLM 只从 **URL 查询参数**（或表单）里取 `app/stream/type`，
SDP 必须放在 **请求体**（`Content-Type: application/sdp`）。实测三种形态：

| 调用形态 | ZLM 响应 |
|---|---|
| query 参数 + SDP 作为 body | ✅ 进入 SDP 校验（`Assertion failed: … 只支持 group BUNDLE 模式` —— 说明参数已接受） |
| 全 JSON body（原实现） | ❌ `Required parameter missed: "type"` |
| `POST /index/api/whep`（query + body） | ✅ 返回纯 SDP 文本 |

**修复**（`src/handlers/webrtc.rs`）：改为 `reqwest::Url` + `query_pairs` 拼参数、
SDP 作为 body；兼容 ZLM 回 JSON（`{code, sdp, id}`）与回纯 SDP 文本两种形态；
补 `stream`/`sdp` 缺失与 `type` 取值（play/push）校验。

**验证（真实 Chromium + 真实 ZLM，2026-09-13）**：

```
浏览器（bundled Chromium）: H264 支持 = true
POST /api/play/webrtc {app:'rtp', stream:<GB流>, type:'offer', sdp:<真实 offer>}
  → code=0，answer 2881 字节，候选 127.0.0.1:8000
  → connectionState=connected  iceConnectionState=connected

对照：普通流 recon/p1（非 GB 源）
  → videoWidth=320 videoHeight=240 currentTime=10.27s
  → framesReceived=281 framesDecoded=281 framesDropped=0 pli=0   ✅ 真的在播
```

### B9 ✅ `rtc.externIP` 未下发（已修复：可配置 + 三处自动下发）

实测：ZLM 的 answer 候选地址默认是 **`172.18.0.2:8000`**（容器网段），
宿主浏览器不可达 → `iceConnectionState` 永远停在 `checking`、0 字节；
用 ZLM `setServerConfig?rtc.externIP=127.0.0.1` 后候选变成 `127.0.0.1:8000`，
**ICE 立刻 connected 并开始收流**。

**已修复（2026-09-13）**：新增可配置项 `[[zlm.servers]] rtc_extern_ip`
（也写入 `gb_media_server.rtc_extern_ip`，媒体节点页保存即可设置），并在**三处**
用 `set_server_config_verified` 下发 + 回读校验：

1. 节点上线（`zlm/health_checker.rs`）；
2. **ZLM 每次启动**（`on_server_started` hook）—— ZLM 重启后运行期配置会丢，
   这一处是自愈的关键；
3. 媒体节点保存接口（改完立即生效，不用等下一轮健康检查）。

实测：ZLM 重启后 `rtc.externIP` 从空自动恢复为 `127.0.0.1`（日志
`ZLM rtc.externIP set to 127.0.0.1 for server zlmediakit-1`），浏览器
ICE 立即 `connected`。**host 网络部署无需配置该项**（见「ZLM 网络模式」一节）。

### A4 ✅ 直播页 WebRTC 播放入口（2026-09-19 已接）

原先 `web/src/api/live.ts::postWebrtcPlay` 已按后端契约写好（POST + JSON body）
但**全仓库没有调用方**。2026-09-19（commit `73e2175` "Live 视图支持 WebRTC"）已接上：

- `web/src/views/live/index.vue:173,408` —— 直播页协议切换里的 WebRTC 分支
- `web/src/components/ChannelPlayDialog/index.vue:424,774` —— 通道播放对话框同样支持

`RTCPeerConnection` + `<video>`、断开时回收会话均已实现。
**现状**：GB28181 流的 WebRTC 播放已由用户在当前部署环境验证正常（2026-09-19）；
此前的无帧现象不再作为开放缺陷跟踪。B9（`rtc.externIP` 下发）已修。

### E5 ✅ ZLM 网络模式改为 host + 配置文件管理（2026-09-13）

**改动**：

| 文件 | 内容 |
|---|---|
| `docker-compose.yml` | ZLM 改 `network_mode: host`，**删除全部端口映射**与 `extra_hosts`；gbserver 侧加 `extra_hosts`（用 `host.docker.internal` 访问宿主上的 ZLM），并通过 env 指定 `ZLM__SERVERS__0__IP/HOOK_URL/RTC_EXTERN_IP`；config.ini **只读**挂载 |
| `docker-compose.mac.yml`（新） | Docker Desktop（macOS/Windows）叠加文件：ZLM 切回桥接 + 端口映射 + `extra_hosts`，并下发 `rtc_extern_ip=127.0.0.1`。用法 `docker compose -f docker-compose.yml -f docker-compose.mac.yml up -d` |
| `docker/zlm/config.ini` | `[rtc] externIP` 保持**留空**并加注释说明；`[rtp_proxy] port_range=30000-30100`；`[http] port` 由镜像默认 80 改为 **8080**（内外一致，见 B11） |
| `docker-compose*.yml` | 新增命名卷 `zlmrecord` / `zlmsnap`：录像与截图**持久化**。默认写在容器文件系统里，`--force-recreate` 或镜像升级会把 MP4 全删掉，而 `gb_cloud_record` 里的记录还在 —— 现象是"列表里有录像、点开 404/500"（本轮实测踩到过一次） |
| `src/config.rs` / `db/media_server.rs` / `handlers/server.rs` / `zlm/health_checker.rs` / `zlm/hook.rs` | 新增 `rtc_extern_ip` 配置项、DB 列（含旧库迁移）、保存接口字段与三处下发逻辑 |

**为什么改用 host 网络**：收流端口池（`rtp_proxy.port_range`）与 WebRTC ICE 候选
都要求 ZLM 直接使用宿主地址；桥接模式必须逐口映射整段 UDP（Docker Desktop 对上
百个端口映射极慢，实测会卡住 daemon），且候选地址会是容器内网 IP。

**踩坑记录（重要）**：**ZLM 收到 `setServerConfig` 会把整份 config.ini 重写**
—— 实测把仓库里 700+ 行中文注释全部抹掉，并把 hook URL（`host.docker.internal`）、
`mediaServerId`、`rtc.externIP` 写进文件。因此 config.ini **必须只读挂载**，
运行期配置全部由平台下发（节点上线 / `on_server_started` / 保存接口）。

**验证（本机 macOS，用 mac 叠加文件）**：ZLM 重建后
`rtc.externIP=127.0.0.1`、`rtp_proxy.port_range=30000-30100`、
`mediaServerId=zlmediakit-1` 自动恢复；hook 13/20 生效；
实时点播 FLV 拉流 237KB；WebRTC `iceConnectionState=connected`；
录像计划→MP4→落库（69395 字节）正常；`cargo test` 743 通过、`playwright` 66 通过。

**未在本机验证的部分**：host 网络本身 —— Docker Desktop **不支持** host 网络
（实测容器端口在宿主不可达），只能在 Linux 服务器上验证；compose 两种形态均已
通过 `docker compose config` 校验。**上线前请在 Linux 上确认**：
① 30000-30100/udp 与 8000/udp 已放行；② 后端（容器）能通过
`host.docker.internal` 访问宿主上的 8080；③ hook URL 用 `127.0.0.1` 还是
宿主内网 IP（取决于后端是否也在 host 网络）。

### B10 ✅ ZLM 流列表因 `fps` 浮点解析失败（已修）

**现象**：真实 ZLM 对国标流返回 `"fps": 25.0`（JSON 带小数点），而
`zlm::types::TrackInfo.fps` 声明为 `Option<u32>` → **整个 `getMediaList`
反序列化失败**：`/api/device/query/streams` 恒为空、`/api/server/stream/all` 报错，
日志只有 `invalid type: floating point 25.0, expected u32`。

**修复**：`fps` 改为 `Option<f64>`（整数与浮点都能解析），并补回归测试
（`25.0` / `25` / 缺省三种形态）。

### B11 ✅ `on_server_started` 覆盖对外的 HTTP 端口（已修）

**现象**：媒体节点检测返回
`媒体节点 127.0.0.1:80 检测失败: HTTP error: 502 Bad Gateway`，随后所有
ZLM 相关功能（流列表、录像删除、截图…）一起 500 —— 因为
`on_server_started` 把 ZLM **容器内部**的 http 端口（80）写进了
`gb_media_server.http_port`，而后端访问 ZLM 用的是**对外映射**端口（8080）。

**修复**：
1. `update_ports` 不再无条件覆盖 `http_port`，只在该列为空/0 时填充
   （`CASE WHEN http_port IS NULL OR http_port = 0 THEN ? ELSE http_port END`）——
   这个字段是"后端访问 ZLM 的地址"，必须由配置决定，不能被节点自报覆盖；
2. `docker/zlm/config.ini` 的 `[http] port` 固定为 **8080**（原来镜像默认 80，
   桥接映射 8080:80 时内外不一致），`docker-compose.mac.yml` 相应改为
   `8080:8080`。这样一来 host 网络与桥接两种模式下内外端口都一致。

**实测**：ZLM 重建后 DB 里 `http_port` 保持 8080，节点检测
`{"code":0,"reachable":true,"httpPort":8080}`。

### B12 ✅ ZLM 每个协议一行 → 控制台「重点通道」同一通道重复铺格子（已修并复验）

**现象**（用户报「控制台下面的重要通道里面相同的通道视频会出现多个」）：
一路正在点播的通道在控制台「重点通道」面板里占了 3~5 个格子，标题都是同一个流名；
卡片「活跃通道」右侧的「直播 N」也随之虚高（本机实测 1 路流显示成 `直播 5`）。

**根因**：ZLM `getMediaList` 对**同一路流按协议各返回一行** —— 实测 ZLM master 上
一路 `rtp/34020000001320128497_34020000001310000001` 返回
`hls` / `rtsp` / `ts` / `rtmp` / `fmp4` **五行**（读者数只挂在被真正播放的那个协议行上）。
`/api/device/query/streams` 原样透出这五行，而控制台
`web/src/views/dashboard/index.vue::rebuildChannels` 直接 `streams.slice(0, 6)` 铺格子
→ 同一个通道占满面板。同一通道还可能同时存在实时流与回放/下载流
（`设备ID_通道ID_开始_结束`），同样会变成两个格子。

**修复**：新增 `web/src/utils/mediaStream.ts`
（`dedupeMediaStreams` 按 (mediaServerId, app, stream) 合并协议行并取最大读者数；
`extractKeyChannels` 再按 `设备ID_通道ID` 归并通道、实时流优先、读者多的优先、
顺序稳定），控制台改用它派生面板，并把「直播 N」改成去重后的**路数**；
面板无流时改显示空态（此前会一直留着上一次的旧格子）。只有回放/下载流的通道标 `REC`。

**复验**：新增回归用例 `e2e/tests/dashboard.spec.ts`（桩数据 11 行 → 必须只渲染 2 个通道格，
另有一条对真实 ZLM 数据的"通道不重复"断言）；修复前该用例在真实环境实测
`5 个格子 = 1 个通道` 失败，修复后 2/2 通过；`npm run build`（含 vue-tsc）通过、
`smoke.spec.ts` 20/20 通过。

**后端计数补修（2026-10-18）**：`zlm/hook.rs` 的活跃流指标与 flow report 落库统计、
`zlm/client.rs::get_active_stream_count` 的选路负载不再使用 `getMediaList().len()`，
统一按 `(vhost, app, stream)` 去重。同一流的不同协议行算一路，不同虚拟主机保留为
不同流。单测覆盖 5 行协议记录计为 1 路；本机真实 ZLM 临时推流得到 4 行协议记录、
1 路媒体。目标部署统计仍待验收。

### B13 ✅ 控制台首屏要等约 2s 才出数据（已修并复验）

**现象**（用户报「控制台打开后立马查询数据，不要等 2s 再开始查询」）：
打开控制台后卡片/面板先空着，约 1.5~2.5s 才一起出数。

**根因（两个，叠加）**：
1. **等最慢的接口**：`loadAll()` 是 `await Promise.allSettled([5 个接口])` **之后**才逐项
   赋值 —— `/api/server/system/info` 在服务端要真采一次 CPU（60ms）与网络速率
   （2×100ms）再读磁盘，本机单次实测 0.79s、浏览器里 1.2~2.5s，于是 0.5s 就回来的
   设备数/流列表/告警/媒体节点也要陪着它等；节点流量（`media_server/load`）还被串在
   它后面，再晚一截。
2. **同一接口被 3 个组件同时拉**：控制台、侧边栏「存储」、导航栏「平台信息」都在
   `onMounted` 里各发一次 `system/info`（实测浏览器里 3 个并发响应分别 1.2s / 1.5s /
   2.3s），首屏最慢的那块因此被拖到 2.5s。

**修复**：
1. `dashboard/index.vue::loadAll` 改为**各面板各自落地**：每个接口 resolve 后立刻写自己
   的 ref（抽了 `applyInfoDerived()` / `applyNodeRows()` / `loadNodeTraffic()`，让
   `system/info` 与流列表任一先到都能刷新界面），重点通道由流列表那一支自己重建，
   节点流量只跟在节点列表后面查；
2. 新增 `web/src/utils/systemInfo.ts::loadSystemInfo()`：**并发合并在途请求**
   （不落缓存、不返回旧值），控制台/侧边栏/导航栏共用一次网络往返。

**复验**：新增回归用例 `e2e/tests/dashboard.spec.ts` 的
「system/info 再慢也不能拖住其它面板」（把该接口永久挂起，断言重点通道仍渲染 ——
旧实现下会一直等到超时失败）。实测首屏出数时刻：设备数/媒体节点 ~0.65s，
重点通道 ~1.0s（此前要等 system/info），CPU/内存 ~1.2~1.4s（此前 2.5s）；
`system/info` 4s 内请求数从 3 个并发降到 1 个挂载请求 + 1 次轮询。
`npm run build`（含 vue-tsc）通过，`smoke.spec.ts` 20/20、`dashboard.spec.ts` 4/4 通过。

### B14 🟡 设备命令应答闭环（真机复验待做）

**现象**（2026-09-19 13:48:43，真机 EasyGBD）：

```
Transport mode change: device=34020000001320128497, mode=TCP-PASSIVE      ← 平台下发
MESSAGE from 34020000001320128497 - CmdType: Some("DeviceControl")        ← 设备应答
Unhandled MESSAGE body: <?xml version="1.0" encoding="GB2312"?>
  <Response><CmdType>DeviceControl</CmdType><SN>1789796923</SN>
  <DeviceID>34020000001320128497</DeviceID><Result>OK</Result></Response>
```

**当前代码**：`ResponseRouter` 已映射 `DeviceControl`、`ConfigDownload` 与其
`DeviceConfig` 应答别名；以设备、SN、命令类型及控制目标匹配 pending。
对可等待的控制/配置请求，缺失目标 ID 或 `Result` 的应答不会完成 pending。
传输模式、设防/撤防、远程录像、配置下发、远程重启、预置位及直接 PTZ 可分别呈现 `confirmed`、
`rejected`、`unconfirmed`，并将 SIP 发送状态与设备结果分开；默认即时发送
的离散操作保持 `sent` 语义。`/api/front-end/*` 的即时控制响应也统一携带
`sipSent=true` 与 `deviceResult.status=sent`，不再把发送成功称为设备操作成功。
`/api/common/channel/front-end/*` 的旧 `code/msg` 响应也补上相同发送字段；
巡航、扫描、看守位和拉框控制的“成功”文案改为“命令已发送”。
真实后端 + SIP Mock 复跑聚焦、预置位、看守位的 HTTP/SIP 路径通过，
OpenAPI 示例与实际响应一致；这些兼容入口仍未等待设备执行回执。
回放暂停、恢复、跳转、倍速两组路由原先在 SIP 发送前更新本地会话，
`/api/playback/*` 还可能在 SIP 缺失时返回成功；现先解析目标并发送，失败不更改会话，
发送成功只标记 `deviceResult.status=sent`。控制入口现要求已登记回放流及其
准确 Call-ID，并验证活动回放 SIP 对话和双方 tag；无对话时拒绝，不再发送可能被
设备丢弃的独立 INFO，也不会误用同通道直播。隔离 SQLite 后端 + 真实 ZLM +
SIP Mock 已验证暂停 INFO 属于当前回放对话；真机执行仍待验收。
并发控制的 INFO 和回放停止 BYE 已改为同一活动对话内按顺序分配 CSeq；
本机 Mock 并发控制收到 `2,3,4,5,6`，随后 BYE 通过对话与递增序号校验。
批量控制保留原有 `success/failed` 发送计数，逐设备补充 `sipSent`、`deviceResult`；
`ptzStop` 和 `reboot` 支持 `waitResult=true`，查询类命令不提供执行等待；
批量请求最多并行处理 16 台设备并按请求顺序返回，避免每台无回执设备串行等待 5 秒。

**2026-10-20 补齐"其它控制入口"**：远程启动（`teleboot`）、报警复位（`reset_alarm`）、
强制关键帧（`i_frame`）、看守位（`home_position`）、拉框放大/缩小（`drag_zoom`）、
基本配置与视频参数下发（`config/set/*`）此前只回 `result: "command sent"` ——
既没有 `sipSent`/`deviceResult`，也无法等待设备回执，等于把"SIP 发出去了"
当成"设备执行了"。现这 7 个入口统一返回 `sipSent` 与 `deviceResult`，并支持
`waitResult=true`（最长 5 秒）区分 `confirmed`/`rejected`/`unconfirmed`。
与之同时，`/api/device/control/i_frame` 与 `/api/device/control/home_position`
的 OpenAPI 之前**没有声明任何查询参数**（`params(...)` 缺失），现已补上。

**语义收敛到一处**：此前"发送 → 等回执 → 解释 `Result`"这段逻辑在 4 个文件里
各写一份（device_control / device_stub / device_batch 等 7 处），容易悄悄漂移。
现抽到 `src/handlers/device_result.rs`（`send_command` / `send_command_and_wait`），
状态取值（`not_sent|sent|confirmed|rejected|unconfirmed`）也只有一份定义。

**本轮运行证据**：真实后端 + Python SIP Mock 对传输模式接口执行了确认、拒绝、
仅 SIP 200 无设备回执、重复回执、5.5 秒迟到回执及新旧 SN 隔离，结果符合预期。
配置查询 `ConfigDownload` 的 Mock 回执延迟 11 秒，平台仍返回 `source=live`；
此前 pending 默认 10 秒而接口等待 15 秒的预算错位已修复。
2026-10-18 批量 PTZ 停止与重启又覆盖即时发送、确认、拒绝、无回执；
不存在设备返回 `not_sent`，查询类命令要求执行等待时被明确拒绝。
这只能证明 Mock 路径，不代表 2026-09-19 的真机缺陷已在同一设备上复验。

**剩余**：真机传输模式或控制命令回执。Mock 之外的设备结果证据仍缺，
已覆盖的命令入口在真机上尚未逐一复验；真机若回 `Result != OK`，
需有可定位日志。

### B16 ✅ 跨方言类型/宽度不匹配：postgres 上多个功能整个不可用（2026-10-20 已修）

这一类的根因是**三种方言的类型宽度/类型本身不一样**，而 sqlx-postgres 对类型是
**严格**校验的（sqlx-mysql / sqlx-sqlite 宽松），于是同一个洞**只在 postgres 上炸** ——
sqlite 的单测与 mysql 的冒烟全绿，问题一路留到 postgres 才暴露。

2026-10-20 用**真实 PostgreSQL** 跑后端 + 全量 Playwright 时抓到两类：

**(1) INT4 主键解成 `i64`**
`postgres` 的 `serial` 主键是 **INT4**，而 mysql 的 `bigint auto_increment` 与
sqlite 的 `INTEGER` 都是 64 位。Rust 侧按 `i64` 解 INT4 列 → sqlx 直接报
`decoding column "id": mismatched types; Rust type i64 (as SQL type INT8) is not
compatible with SQL type INT4`。命中 3 处：

| 位置 | 影响 |
|---|---|
| `src/scheduler/record_plan.rs` 录像计划调度器 | **云录像计划每一轮都失败** —— 计划拉不起流、不落盘 |
| `src/handlers/stub.rs` 录像计划通道列表 | 整个接口 500 |
| `src/handlers/alarm.rs` 报警列表 / 详情 | 两个接口 500 |

**(2) VARCHAR 列绑成 `i64`**
`gb_device_channel.parent_id` 是 **VARCHAR**（国标父通道编号是字符串），但
`ChannelWriteFields.parent_id` 声明成 `Option<i64>` → postgres 报
`COALESCE types bigint and character varying cannot be matched`。
注意这是**解析期**就失败：**哪怕请求根本没传 `parentId`**（绑的是 NULL，类型照样是
int8），**通道编辑在 postgres 上也是整个 500**。

**(3) `serial` 序列没跟上表内容**
初始化 SQL 用**显式 id=1** 播了 admin 用户与 admin 角色，而 `serial` 序列停在
初始值且 `is_called=false` —— **下一次 `nextval` 仍然返回 1**，于是**第一次**往
这两张表插数据就撞主键：`duplicate key value violates unique constraint
"gb_user_pkey"`。sqlite 没有这个问题（`INTEGER PRIMARY KEY AUTOINCREMENT`
自己算下一个 id），mysql 也没有（`AUTO_INCREMENT` 会扫表），**只有 postgres 会炸**。
症状是"新建用户/角色报重复数据"，真因是序列没同步。

**(4) `SELECT 1` 的字面量类型**
postgres 里 `SELECT 1` 的字面量是 **INT4**，按 `i64` 解 → `decoding column 0:
mismatched types`。`db::user::role_exists` 就是这么写的，于是**新建用户**在
postgres 上必 500。

**(5) 审计日志表少一列**
`db::audit_log::ensure_table` 只有 sqlite 分支建了 `elapsed_ms`，postgres 与
mysql 没有；而 `insert_with_metrics` 写的是带 `elapsed_ms` 的扩展列 ——
每个被审计的请求都先失败一次再回退（postgres 日志里**每请求一条 ERROR**），
而且请求耗时根本没落库。审计轨迹在主生产库上一直是缺的。

**修复**：

* 新增 `dyn_where::bigint_col`：postgres 侧把整数列显式转成 `::bigint`，
  其它方言原样返回。与既有的 `dialect_sql`（占位符）、`BindValue::Bool`（布尔列）
  是同一类问题的同一种处理方式 —— **在 SQL 侧统一**，不要指望解码端兼容。
* `parent_id` 改回**字符串**：`ChannelWriteFields.parent_id`、`ChannelUpdateBody`、
  `ChannelAddBody`、`SyChannelUpdateQuery` 全部改成 `Option<String>`。
  这与列类型（VARCHAR）、前端本来就发的东西（`parentId?: string`）以及代码里
  既有的 `parent_id = '0'` 查询一致。
* 新增 `db::seq_sync::sync_serial_sequences`：**每次启动**把所有 `serial` 主键的
  序列推进到 `MAX(id)`。放在启动时而不是建表时，是因为**已装的库同样中招** ——
  序列一旦停在 1，除非有人手动 `setval`，否则永远是坏的。
  找序列用 `pg_depend` 的自动依赖（`deptype='a'`），**不要**用
  `pg_get_serial_sequence(table,'id')` —— 它在列不存在时是**抛错**而不是返回 NULL，
  对 `pg_class` 里的系统视图逐行调用会直接报
  `column "id" of relation "pg_statistic" does not exist`。
* `db::user::role_exists` 改用 `SELECT EXISTS(...)`（与 `db::device::device_exists`
  同一套按方言分支的写法）：postgres 返回 `bool`、mysql/sqlite 返回整数。
* `db::audit_log::ensure_table` 的 postgres / mysql 分支补上 `elapsed_ms BIGINT`，
  并对**已存在的表**补 `ALTER TABLE ... ADD COLUMN`（postgres 用 `IF NOT EXISTS`，
  mysql 忽略重复列错误）—— `CREATE TABLE IF NOT EXISTS` 不会给老表补列。

**验证**（真实 PostgreSQL 16 + MySQL 8 容器）：

* `scripts/dialect_smoke.py`（仓库自带的方言运行期冒烟）**两个方言都只剩 2 项
  "预期失败"**（CSV 导出不是 JSON、proxy/start 指向不存在的 RTSP 源）——
  修前 postgres 上报警/录像计划通道列表/计划相关多条是失败项。
* **冒烟脚本已补上 `role/add` 与 `user/add`**：这两条只有管理员能调的写路径
  之前没被覆盖，正是 (3)(4) 两个坑长期没人看见的原因。
* 复现 (3)：把 `gb_user_id_seq` / `gb_user_role_id_seq` 复位成
  `setval(seq, 1, false)`（模拟全新安装），重启后端 → 序列自动被推到
  `MAX(id)`，随后 `role/add` 与 `user/add` 都成功建出 id=2 的新行。
* 复现 (5)：修复后 postgres 日志不再新增 `column "elapsed_ms" ... does not exist`
  的 ERROR，审计行的 `elapsed_ms` 也有值了。
* 全量 Playwright 在 postgres 上从 **72/79 → 79/79**（修复前 3 个失败里，2 个是
  上述类型错误，第 3 个是通道编辑 500）。
* 逐个打**全部 270 个安全 GET 端点**，修复后 **postgres 上再无任何
  `mismatched types` / `operator does not exist` / 语法错误**。
* 通道编辑实测：只改名称 → 名称已改，`manufacturer`/`channelType` 未被清空。
* 全量 Rust 单测 778 通过 / 0 失败；`cargo fmt --check` 通过。

**教训**：`cargo test`（默认 sqlite）**永远发现不了这一类问题**。方言冒烟必须
对真实 pg/mysql 实例跑 —— 这正是 `scripts/dialect_smoke.py` 存在的理由。

### B17 ✅ 代码审查发现的问题（2026-10-24 修复）

对 v2.0 分支的改动做了一轮代码审查（两路独立审查 + 自查），修掉下列**确凿**问题。
多数只在**真实 postgres** 上才暴露 —— sqlite 单测与 mysql 冒烟都是绿的。

**① 录像列表的设备/通道过滤落在 `app` 列上 → 永远查不到录像**
`cloud_record_extra::list_url` 把 `device_id`/`channel_id` 塞进了
`db::cloud_record::list_paged` 的 **`app`** 参数，而该函数的第 2 个形参就是
`app`（SQL 是 `WHERE app = ?`）。本平台的录像流名是 `{deviceId}_{channelId}`，
设备/通道号在 **`stream`** 列里，`app` 列存的是 ZLM 应用名（'rtp'/'record'）。
结果：`GET /api/cloud/record/list-url?deviceId=…` 恒 0 行、`total` 恒 0 ——
"按设备/通道查录像"从来就没工作过。同一平台的 `/api/cloud/record/list` 走
`CloudRecordFilter`（按 `stream` 前缀匹配），是对的 —— 两条列表接口因此结果不一致。
现改为统一走 `CloudRecordFilter` + `list_filtered`，并给 `count` 加上限（1000）。

**② SY 的 `record/collect/delete`（DELETE）入参与平台不一致 → 收藏根本没删掉**
平台 DELETE 收 **JSON body**（`CloudRecordCollectQuery`，数字主键与组合串都收），
而 SY 别名的 DELETE 转调的是 GET 版实现（只认 query 的数字主键）—— 对接方照平台
写法发 body 会被 axum 忽略 → `missing id`，收藏还在。现两个方法都走同一套
`clear_collect`，入参与返回形状（`{recordId,status}`）与平台完全一致。

**③ SY 的 `camera/one` / `camera/update` 只收数字主键 → 国标号返回裸 HTTP 400**
平台 DTO 的 `id` 是 `i64`，20 位国标通道号装不下 → axum 的 QueryRejection 直接回
**纯文本 400**，不是 `{code,msg,data}` 信封；而同一 SY 模块的 `push/play` 用的恰恰是
国标号 —— 两套 ID 口径并存。现 SY 侧改收字符串：数字主键与国标通道号都能用，
查不到时按平台口径回 `data=null`。

**④ `/api/sy/test` 没做管理员校验 → 部署拓扑泄露给普通用户**
它是本轮**新增**的诊断端点（不是别名），会回报设备/通道/媒体节点数量。
按仓库规约（AGENTS.md「新增任何管理类端点都必须调用 `authz::require_admin`」）与
平台同类诊断口（`/api/test/hook/list`、`/api/test/redis`）一致 → 已加管理员校验。

**⑤ `/api/sy/test` 的计数 `unwrap_or(0)` 吞错 → 可能报出 `ok:true` 的自相矛盾结论**
某个计数 SQL 失败时会报 0，而 `database` 恰好成功 → 报告变成
`{"database":true,"device_count":0,"ok":true}`，对接方据此判定"平台就绪"就错了。
现逐项保留 `Result`，失败记进 `issues` 并让 `ok=false`。

**⑥ API Key 的过期时间写错 → 静默变成"永不过期"**
`parse_expired_at` 对无法解析的非空值返回 `None`，接口照样回 `code: 0` ——
运维本想给密钥设过期，手滑打错日期（`2026-13-45 99:99:99`、`2026-02-30`），
凭证就**永久有效**了。现改为业务错误（400）；前端也从文本框换成 `el-date-picker`。

> **第二轮补充（这条初版写得过乐观，已纠正）**：初版写的是"让非法日期根本输不出来"——
> **不成立**。`el-date-picker` 的输入框**默认可编辑**，且它的用户输入走 dayjs 的
> **非严格**解析：解析不出来 → 不 emit → `v-model` 仍是 `null` → 提交时变成"永不过期"；
> 解析得出来但日期非法 → dayjs **归一化**（`2026-02-30` → `2026-03-02`、
> `2026-13-45 99:99:99` → `2027-02-18 04:40:39`）。**两条路都让 400 保护根本触发不到**，
> 也就是说本条 bug 换了个形式原样残留。现已加 `:editable="false"`（只能从面板选）
> 并补一条格式校验兜底。详见 B18 ②。

**⑦ `seq_sync` 会把序列往回拨 → id 被重复发放**
postgres 的 `nextval` **不是事务性的**：回滚的 INSERT、或删掉最大 id 的行，都会让
序列**停在 `MAX(id)` 之上** —— 这是常态。原实现无条件 `setval(seq, MAX(id), true)`，
等于把序列拉回来，接下来几次插入会**复用已发放过的 id**。而本 schema **没有任何外键**
（`init-postgresql-2.7.4.sql` 里没有 `REFERENCES`），引用完整性全靠应用层
（`gb_device_channel.data_device_id`→`gb_device.id`、
`gb_user_api_key.user_id`→`gb_user.id`），id 一旦复用，残留引用会**静默挂到另一行**。
现改为**只前进不后退**：只有"下一个要发放的 id 已被占用"时才推进。

**⑧ `seq_sync` 硬编码 `public` schema → 非 public 部署下静默失效**
表名/序列名都没带 schema 限定；`search_path` 一改，整条 `setval` 语句直接报错，
它要修的 duplicate key 依旧存在。现从 `pg_class`/`pg_namespace` 取出**表自己的
schema** 并全程限定。

**⑨ `seq_sync` 的 `MAX(int4)` 按 `Option<i64>` 解 → 推进步骤整条报错**
postgres 里 `max(int4)` 的返回值仍是 **int4**（只有 `max(int8)` 才是 int8），
按 i64 解会报 "mismatched types"。这与本仓库其它地方的 INT4 列处理是同一个坑。
现显式 `::bigint`。判据（"要不要推进"）已抽成纯函数并补了单测。

**⑩ JT1078 鉴权路径调用了带写副作用的 `resolve_auth_code`**
鉴权时用"读取（没有就生成并写库）"的函数：库里没配置时会**现生成一个随机码**，
再拿它跟终端回送的码比对 —— 必然对不上，终端从此再也鉴权不过（而库里还多了一条
随机码）。现拆成纯读的 `stored_auth_code` 与有副作用的 `issue_auth_code`，
**只有注册应答**才签发。

**⑪ JT1078 会话超时不清鉴权状态**
`cleanup_once` 移除超时会话时没清"已鉴权"集合 → 终端会话没了却仍算已鉴权。
现在一并清除。

**⑬ `serde_flex::AnyScalar` 的变体顺序让"字符串原样返回"的承诺落空**
> **第二轮补充（这条初版的根因写错了，已纠正）**：初版说是 untagged 枚举"按声明顺序
> 逐个尝试"、字符串会先被 `Int(i64)` 吃掉并溢出报错。**实测不成立** ——
> serde 的 `ContentDeserializer::deserialize_integer` 对 `Content::String` 直接返回
> `invalid_type`，**不会**把字符串当数字解析；实测两种变体顺序在 JSON 与查询串
> 两条路径上输出**完全相同**（20 组用例逐条比对）。真正的报错来自
> `de_opt_i64` 自己对字符串做 `parse::<i64>()`，20 位国标号会 `PosOverflow`
> （"number too large to fit in target type"）。所以 ③ 的修复点是**改 DTO 的类型**
> （收字符串），**不是**调变体顺序 —— 调顺序无效。
>
> 不过第二轮顺带查出了这一块**真正的 P1**：`Float` 分支用 `f as i64` 做**饱和转换**，
> 超过 `i64::MAX` 的 JSON 整数会静默变成 `9223372036854775807`（一个根本不存在的编号），
> 而不报错。详见 B18 ①。

**⑫ `device_stub` 的录像控制仍留着自己的一份"发送→等回执→解释 Result"**
"收敛到一处"此前只是宣称，代码里还有第二份（连状态字面量都是各写各的）。
现已改用共享实现。

**本轮未改（先例问题，记录在案）**：`/api/cloud/record/zip` 的响应里带服务端
**绝对路径** `filePath`（如 `/data/record/...`）。这是平台既有行为，SY 别名照搬
同样形状，没有扩大暴露面（该端点本就要求登录）。改它会影响既有调用方，
且它不是"改动引入的回归"，故本轮不动 —— 若要收紧，应作为独立改动统一处理
平台版与 SY 版。

**验证**：定向 Rust 单测 22 个（seq_sync 4 + sy_contract 13 + serde_flex 5）全部通过；全量
`cargo test --no-fail-fast` **863 通过 / 0 失败**（比上轮 +12）；三方言构建**零告警**通过；`scripts/dialect_smoke.py`
在 pg / mysql / sqlite 三方言上都只剩 2 项"预期失败"；全量 Playwright 79/79。

### B18 ✅ 第二轮代码审查（复查 B17 的修复本身）

第一轮（B17）修完 12 个问题后，对**这一批改动本身**又做了一轮审查：三路独立
verifier（分别审 `sy_contract`+`cloud_record_extra`、`seq_sync`+`jt1078`、
`device_result`+API Key 前端）+ 自查。共查出 **17 个问题**：6 个 P1、5 个 P2、6 个 P3 —— 其中 **16 个已修**，
1 个（⑨）经自查**推翻**、维持原状（详见该条）。

**① P1 `serde_flex` 的 `Float` 分支用饱和转换 → 静默给出错编号**
`AnyScalar::into_string` 对整数值浮点做 `f as i64`，而 Rust 的 `as` 对超范围是
**饱和**的：超过 `i64::MAX` 的 JSON 整数（20 位国标号就是 3.4e19）会静默变成
`9223372036854775807` —— 一个**根本不存在的编号**，而且不报错。
可复现路径：`DELETE /api/cloud/record/delete` 的 body `{"ids":[34020000001320000001]}`
（第三方不带引号发数字）→ id 被改写成 `i64::MAX` → 删不到目标，或**删错**。
现新增 `U64` 变体保住 `u64` 范围内的精度；超出 `u64::MAX` 的**报错**（f64 已丢失原始
字面量，无法还原，只能让调用方改用字符串）而不是给错值。`de_opt_i64` / `de_opt_bool`
/ `de_opt_i64_vec` 同步收紧。补 4 组回归测试。

**② P1 日期选择器仍可自由输入 → 后端新加的 400 保护在 UI 上根本触发不到**
见 B17 ⑥ 的第二轮补充。`el-date-picker` 的输入框默认可编辑，用户输入走 dayjs
**非严格**解析：要么被**归一化**成另一个合法日期，要么被**静默丢弃**成"永不过期"。
现加 `:editable="false"`（只能从面板选）+ 格式校验兜底。

**③ P1 `resolve_channel_pk`「能 parse 成 i64 就当主键」→ 静默假成功**
20 位国标号超 i64 会正确落到国标查表分支，但**19 位及以下**的数字串（包括对接方
去掉前导 0 的 20 位号）**能** parse 成 i64 → 被当成内部主键。实测：
`camera/one?id=12345` 明明有通道却回 `data=null`；`camera/update?id=999999&name=X`
回 `code:0`（"改名成功"）而实际**什么都没改**。
现改为**国标号优先 + 两条分支都校验存在性**，查不到统一回 0 → 两种 id 形态的
"通道不存在"语义一致（此前 GB 号回 400、数字主键回成功，是相反的两种结论）。

**④ P1 SY 的 collect/delete 契约被改坏 + 无 body 时回裸 415 文本**
本轮把 DELETE 的入参从 Query 改成 JSON body，但 **utoipa 注解没跟着改**，
`params(全限定路径::CollectQuery)` 让 utoipa 退化成"必填 path 参数"，
而路由里根本没有 `{id}` 占位符（实测 OpenAPI：`{"name":"id","in":"path","required":true}`）。
按文档生成的客户端会把 id 拼进 URL → 404；而照文档发 `DELETE ...?id=1`（无 body、
无 Content-Type）会被 axum 的 `Json` extractor 拒成 **415 + `text/plain` 裸文本**，
破坏契约承诺的 `{code,msg,data}` 信封。
现：注解改用**未限定的** `params(CloudRecordCollectQuery)` + `request_body`；
实现改成 `Option<Json<..>>`（axum 在缺 Content-Type 时给 `None` 而不是 415）
+ 查询串双通道兜底。

**⑤ P1 JT1078 鉴权码写库失败时仍下发 → 终端永久锁死**
`issue_auth_code` 在写库失败（终端行不存在 / DB 抖动）时**照样返回**那个码，
而校验路径只认**库里存的**码 —— 平台下发的码永远校验不过，终端**再也自愈不了**。
实测两种变体（UPDATE 被 trigger 拦掉 / 终端行不存在）都被拒。
现：`issue_auth_code` 改返回 `Result`，写不进去就**不下发**，注册应答回
`result=1`（告诉终端"没成功"），终端重试时数据库若已恢复就能正常注册 ——
失败因此**可恢复**，而不是一次性的永久锁死。

**⑥ P1 过期时间按 UTC 解释墙钟 → 整体偏移 8 小时**
`parse_expired_at` 用 `and_utc()`，把前端传来的**本地墙钟**当 UTC。TZ=+8 时密钥
比设定值**多活 8 小时**，列表回显的时间和用户选的日历时间也对不上。
现按服务器本地时区解释（`chrono::Local`，含夏令时三态处理），并补契约单测。

**⑦ P2 `sy/test` 的 `database` 标志误判 + issues 互斥结论**
`database` 是从"5 个计数里有没有失败"反推的，而那 5 个计数查的是 5 张**不同**的表：
只 DROP `gb_media_server` 时，报告同时给出"媒体节点数统计失败"和"数据库不可读"
两条**互相矛盾**的结论。现 `database` 只由核心表（设备/通道）决定；媒体节点统计
失败时**不再**断言"未配置媒体节点"（查不到 ≠ 没有）。
另外失败计数不再回落 0，改给 `null`（"未知" ≠ "一个都没有"）。

**⑧ P2 录像过滤无分隔符边界 → 截断的设备号跨设备误命中**
设备过滤是纯前缀匹配，19 位的截断设备号会同时命中**不同设备**的录像；
通道过滤是纯后缀匹配，同理会命中以它结尾的别的通道。
现两端都加"紧邻分隔符"的边界校验。

> **第三轮补充（这条初版的前提写错了）**：初版写"流名规范形状是 `{deviceId}_{channelId}`"
> —— **不成立**，仓库实际写了**四种**形状（`_` / `/` / `$` / 无分隔符），边界只认 `_`
> 会把其中三种**静默过滤掉**。边界校验本身是对的（要加），但分隔符集合取窄了。
> 详见 B19 ①。

**⑨ 取消收藏对未收藏的录像也回 "deleted" → 审查误报，维持原状**
> **第二轮自查推翻（这条没有改）**：审查把"取消收藏一条本来就没收藏的录像也回成功"
> 判成假成功。改前全量单测挂了一条 —— `router::tests::
> authenticated_collect_roundtrip_updates_business_storage` **明确断言重复删除要回
> `code:0`**（与 add 侧的 `already_collected` 断言配对）。也就是说 **DELETE 幂等是
> 仓库既有的有意契约**（REST 语义：删一个已经删掉的东西，答案是"已达成"），
> 不是漏判。改动已回退，对应单测也一并撤掉。
> **教训**：审查判"假成功"之前，得先看**仓库自己的测试有没有在断言这个行为** ——
> 有断言的，多半是有意契约。

**⑩ P2 JT1078 会话超时不置离线 → 超时的终端仍在线且可被下发**
`gb_jt_terminal.status` 全仓库只有一处会写且参数恒为 `true`（"粘 true"），
心跳还会把地址重新登记回来 —— "超时=终端不在了"这条语义没真正落地：
超时的终端仍被当作在线、平台照样能给它下发命令，而它其实从未重新鉴权。
现清理时同时置 `status=false`，闭环成立。

**⑪ P2 `seq_sync` 的标识符白名单 → 异名表被**静默跳过**
白名单只放行 `[a-z0-9_]+`，于是带引号的大写表名（`"GB_Mixed"`）与非 ASCII 表名
（`"终端表"`）被**无声排除**在同步之外 —— 而这些表恰恰最可能带显式种子 id，
跳过就等于本模块要修的 duplicate key **原样复发**，且日志里一行提示都没有。
现改为**标识符转义**（`"..."` + 内嵌 `"` 翻倍），不再过滤任何表。

**⑫ P3 4 个死代码警告**：`seq_sync` 的 `SerialColumn` / `next_value_of` /
`needs_advance` / `is_safe_ident` 在非 postgres 构建下未被使用（`#[allow(dead_code)]`
把其中一个盖住了）；`build_playback_control_xml` 是被 MANSRTSP 取代的遗留 MANSCDP
构造器，只被测试引用却以 `pub(crate)` 进入生产构建。现按 feature / `cfg(test)` 正确分门。

**⑬ P3 `PendingGuard` 覆盖不到"已登记、守卫未建立"的窗口**
守卫在调用方建立，而登记发生在 `send_message_to_device` 的 await 之前 ——
两者之间隔着一次 await，客户端此刻断开就会留下一条 pending。
现把守卫**下沉到 SIP 层**（`PendingDropGuard`），覆盖登记→发送这段窗口。

**⑭ P3 `device_result` ↔ `device_stub` 互为依赖（模块环）+ 重复错误日志**
纯解析函数 `parse_device_control_result` 移到 `device_result.rs`（共享实现所在处），
断开环；同一次失败的两条错误日志删掉一条。

**⑮ P3 提交信息与代码注释里的不实宣称**
"守卫让后续同设备请求误判为『已有请求在飞』"—— 仓库里**没有**这样的生产判断
（`has_pending_for_device` / `has_pending_for` 只被测试引用），真实收益只是
"提前回收一条 pending"。代码注释已改写。

**⑯ P3 OpenAPI 文档没列出等价参数名 + e2e 的浮层断言可能是空断言**
`toBeHidden()` 在元素**不存在**时也算通过 —— 浮层从未打开（交互写坏）时用例照样绿。
现先断言浮层 `toBeVisible()` 再 `toBeHidden()`；并新增一条"未来过期时间仍可用"的用例。

另：utoipa 不支持 serde 的 `alias`，所以 OpenAPI 里只显示 `id`，而
`id` / `channelId` / `channel_id`（以及收藏侧的 `id` / `recordId` / `cloudRecordId`）
**实际等价**。utoipa 不会把手工写在字段说明里的文字当成参数名，所以这几句
只进 `description`、不进 `parameters` —— 文档消费者仍只会看到 `id` 一个名字，
但 `id` 本来就是能用的那个，不影响对接。

**⑰ P3 jt1078 模块的时间写入混了三种格式**
`update_time`/`create_time` 同一列混着 UTC 墙钟纯格式、RFC3339 与本地墙钟，
字典序排序错乱。现统一成仓库主约定的 `%Y-%m-%d %H:%M:%S`（协议来源的
`register_time` 保持 RFC3339 不动 —— 那是终端自己报的时刻，语义不同）。

**验证**：定向 Rust 单测 34 个（seq_sync 5 + sy_contract 15 + serde_flex 9 +
api_key_expiry 4 + cloud_record 过滤 1）全部通过；全量
`cargo test --no-fail-fast` 通过；三方言构建通过；`scripts/dialect_smoke.py`
在 pg / mysql / sqlite 三方言上都只剩 2 项"预期失败"；全量 Playwright 81/81。

**仍未改（沿用 B17 的先例判断）**：`/api/cloud/record/zip` 的响应里带服务端绝对路径
`filePath`。理由同上：平台既有行为、SY 别名照搬同样形状、端点本就要求登录，
改它会影响既有调用方，应作为独立改动统一处理。

### B19 ✅ 第三轮代码审查（复查第二轮的 9 个提交）

第二轮（B18）修完 16 个问题后，对**那一批改动本身**又做了一轮审查：两路独立
verifier（分别审 `serde_flex`+`sy_contract`+`cloud_record`+`stub`、以及
`jt1078`+`sip`）+ 自查。共查出 **6 个问题**：2 个 P1、2 个 P2、2 个 P3，**全部已修**。

**① P1 录像过滤的边界校验只认 `_` → 把仓库自己写的另外两种流名形状整段过滤掉**
（第二轮引入的**功能回归**）
第二轮给录像过滤加"分隔符边界"时，前提写的是"流名的规范形状是
`{deviceId}_{channelId}`"（依据是 `handlers/play.rs`）。**这个前提只覆盖了一种形状**。
仓库实际往 `gb_cloud_record.stream` 写的有**四种**
（见 `sip/gb28181/stream_reconnect.rs` 的 `parse_stream_id` —— 那是仓库自称
"全局唯一"的流名解析器，它认的分隔符集合正是 `_` / `$` / `/`）：
- `{device}_{channel}` —— `handlers/play.rs` 的 `openRtpServer` 规范写法；
- `{device}/{channel}` —— `db/cloud_record.rs::insert_records` 落 `record_info` 时用的
  （设备通过 GB28181 RecordInfo 上报的历史录像，**活路径**：
  `sip/server.rs` 应答 RecordInfo 查询时调用它）；
- `{device}${channel}` —— 部分早期实现 / 上级平台下发的写法；
- `{phone}` —— JT1078 通道号为 0 时**没有分隔符**（`handlers/jt1078.rs`）。

边界只认 `_` 的话，后两种被**静默整段过滤掉**：平台 UI 的"设备/通道"输入框和
第三方对接方的 list-url 都会"少查数据但总数看着正常"，**全程无报错**。
三方言实测一致（不是方言问题，是语义问题）。现把分隔符放宽为
`IN ('_','$','/')`，并给设备过滤补 `OR stream = ?`（纯号码流）。

**教训**：改过滤/匹配逻辑时，**先枚举"写入端到底写过哪些形状"**，别只看一个调用点。

**② P1 `cleanup_once` 在持有 `self.sessions` 锁的情况下做 DB 往返**
（第二轮引入）
`self.sessions` 是**所有**入站 JT1078 报文的必经锁（`process_jt_message` 一进来
就 `sessions.lock().await`）。Rust 的 `MutexGuard` 析构作用域是**所在语句块**，
不是"最后一次使用点" —— 第二轮把"置终端离线"的 DB 写加在了锁的持有期内。
实测该 DB 写被占锁时阻塞 **5.29 s**（仓库自己的 `busy_timeout(5s)`），
连接池耗尽时按 sqlx 默认 `acquire_timeout` 可到 **30 s/终端** ——
一个**后台清理任务**因此能掐住**整个 JT1078 信令面**（注册、心跳、0x0200 位置汇报、
指令应答）。无死锁（锁序恒为 `sessions → terminal_addrs → authenticated_terminals`），
是**队头阻塞**。
现把三段拆成独立作用域：锁内只做内存操作，**DB 写全部放到锁外**。

**③ P2 平台删除端点回裸文本 422**（第二轮引入）
`DELETE /api/cloud/record/delete` 用 `Json<T>` 提取器，解析失败由 axum 直接回
**422 + `text/plain` 裸文本** —— 破坏契约承诺的 `{code,msg,data}` 信封
（第三方按信封解析会直接炸），而且一颗坏元素让**整批** ids 都没删。
现改成自己收原始字节再解析，失败时回 `AppError::business(Error400, …)`
（HTTP 400 + `code:400`，与同族的 `/api/userApiKey/add` 口径一致）。

**④ P2 `resolve_channel_pk` 在 id 形态冲突时静默选错通道**（第二轮引入）
第二轮把解析改成"国标号优先"，但没有处理**两个口径都命中且指向不同通道**的情况。
实测造一条 `gb_device_id='999999'` 的通道后，`?id=999999` 拿到的是**那条通道**
（而不是主键 999999 的通道），随后按它改名 —— **改错了行还回成功**。
现两个口径都命中且 pk 不同 → 回 400「id 形态有歧义」。

**⑤ P2 锁修复引入的 TOCTOU：在线终端可能被写回"离线"**（修 ② 时新引入）
②把清理拆成"锁内摘会话"与"锁外写库"两段，两者之间没有原子性：终端若在这期间
发过心跳，`persist_terminal_online` 会把 `status` 写回 `true`，随后清理的
`status=false` 才落地 —— **活着的终端在库里被标成离线**，而平台其实仍能给它下发命令。
第三轮改为**条件写**：`UPDATE ... SET status=0 WHERE phone_number=? AND (update_time IS NULL
OR update_time <= :deadline)`，`deadline` 是清理开始的时刻。期间有更新的行就跳过
（终端下次心跳本来也会把它置回在线）。

> **第四轮补充（这条初版写得过乐观，已纠正）**：第三轮那版条件写**一行都没挡住** ——
> `deadline` 取在 DB 写之前（比"摘会话"晚），期间到达的心跳其 `update_time` 落在
> deadline **之前** → 条件成立；加上秒级精度下同秒心跳 `update_time == deadline` 也成立。
> 行为上**完全等价于无条件写**。第四轮把 `deadline` 前移到**摘掉会话的那一刻**、
> 比较改成**严格小于**，才真正生效。详见 B20 ①。

**⑥ P3 鉴权码"读失败"被当成"没配置" → 覆盖运维手工配置的码**
`issue_auth_code` 读失败时只 warn，随后生成随机码并**写库覆盖** ——
一次瞬时的**读**失败就把管理页配好的 `auth_code` 覆盖掉，那些终端全部鉴权失败，
而平台完全看不出发生过覆盖。现读失败直接返回 `Err`（注册失败、可重试），
只有 `Ok(None)`（确实没配）才生成新码。

**⑦ P3 `phone_number` 没有唯一约束 → 重复行让「取哪一行」不确定**
三方言的唯一索引都是 `unique (id, phone_number)`（`id` 是主键），所以
`phone_number` **没有任何唯一性**。重复行存在时的真实危害**不是**"读取报错"：

> **第四轮纠正**：本条初版写的是"重复行会让 `fetch_optional` 报
> `expected at most one row`"—— **不成立**。sqlx 0.7.4 的 `fetch_optional` 语义是
> "returns at most one row"，多行时**静默返回第一行**，整个 sqlx 0.7.4 里根本
> 没有这个错误串。这条不成立的断言会**主动误导**后来人以为"重复行已被兜底"。

真实路径是：读侧**取哪一行不确定**（没有 `ORDER BY` 时 postgres 返回物理扫描
顺序的第一行，而心跳每次都会重写这些行 → 取到的行会翻转）→ 恰好取到一条
`auth_code` 为空的行 → 判定成"没配置" → 生成随机码 → 而写侧
`UPDATE ... WHERE phone_number = ?` 会更新**所有**重复行 → **运维配的码被静默抹掉**，
终端随后全部鉴权失败、平台看不出发生过覆盖（实测复现）。

现读侧固定为「**优先取真配了码的那条**，再按 `id` 取最早的」，写侧保持
"更新所有行"——两者对齐：读到的规范行一定被写覆盖。
**表级唯一约束需要先做历史去重，属独立的数据迁移，本轮只做读取侧兜底。**

**⑧ P3 `sy/test` 的 issues 把同一件事说两遍**
核心表统计失败时，issues 里既有"设备数统计失败"又有"设备/通道统计失败：数据库核心表
不可读"。现去掉后者（前面的分项说明已经把原因讲清）。

**⑨ P3 `register_time` 仍是 RFC3339，与同表其它时间列格式与基准都不同**
`update_last_position` 的 `register_time` 是**协议来源**（设备自报的本地墙钟，
带 `+08:00` 偏移），本轮统一时间格式时**有意保留**它 —— 设备自报时刻的偏移是
它自己的时区，转成 UTC 需要知道设备时区，转换反而会引入错误。
但它与 `update_time`（UTC 纯格式）确实不同基准，跨列比较会错位。
现明确记在案：跨列排序/比较时不要把这两列放在一起。

**验证**：定向 Rust 单测 38 个（seq_sync 5 + sy_contract 15 + serde_flex 9 +
api_key_expiry 4 + cloud_record 过滤 1 + jt1078 鉴权 2 + jt1078 清理 2）全部通过；
全量 `cargo test --no-fail-fast` 通过；三方言构建**零告警**通过；
`scripts/dialect_smoke.py` 在 pg / mysql / sqlite 三方言上都只剩 2 项"预期失败"；
全量 Playwright 通过。
**在真实环境验过的**：`seq_sync` 的标识符转义在**真实 postgres** 上把
`"MixedSchema"."GB_Mixed"`（带引号大写名 + 非 public schema）成功发现并推进，
插入不再撞主键（此前被白名单静默跳过）；录像过滤的新边界 SQL 在**真实 postgres
与 mysql** 上逐条比对（完整设备号命中 1 / 截断设备号 0 / 完整通道号命中 1 /
截断通道号 0）；JT1078 用**仓库自带模拟器**实测了"写库失败 → 注册回 result=1 →
终端自动重试 → 数据库恢复后注册成功并拿到鉴权码"的全链路（把永久锁死变成可恢复失败）。

**教训**：**第三轮仍然挖出了第二轮引入的两个 P1**（过滤回归、锁内 DB 写）。
"上一轮说对了"的假设每轮都要重新检验 —— 而且这轮的 P1 都藏在上一轮
**新写的注释与前提**里（"流名规范形状是 `{deviceId}_{channelId}`"、
"把 DB 写放进 cleanup 里"），不在代码的显眼处。

### B20 ✅ 第四轮代码审查（复查第三轮的 5 个提交）

第三轮（B19）修完 6 个问题后，对**那一批改动本身**又做了一轮审查：两路独立
verifier（分别审 `cloud_record`+`stub`+`sy_contract`+`record_plan`、以及
`jt1078`+`db/jt1078`）+ 自查。共查出 **5 个问题**（1 个 P1 由本轮自查先发现、
2 个 P2、2 个 P3），**全部已修**。

**① P1 条件写是"空条件" → 在线终端仍被写回"离线"**（第三轮引入）
第三轮把"置终端离线"改成条件写（`WHERE update_time <= :deadline`），但两处不对：
① `deadline` 取在**第四段**（DB 写之前），而第一段（摘会话）到第四段之间有心跳
到达时，心跳经 `persist_terminal_online` 把 `update_time` 刷成心跳时刻 —— 那个
时刻**早于** `deadline` → 条件成立 → **活着的终端被置离线**；
② `update_time` 与 `deadline` 都是**秒级**精度，同一秒的心跳 `update_time == deadline`，
`<=` 同样成立。
换句话说，这个"保护"在行为上**完全等价于无条件写**，一行都没挡住。
现：`deadline` 前移到**摘掉会话的那一刻**（在第一段的锁内取，与"会话失效时刻"同基准），
比较改成**严格小于** `update_time < deadline`（同秒心跳落到"不置离线"的安全侧，
代价只是可能晚一个清理周期 —— 终端下次心跳本来也会把自己置回在线）。

**② P1 第三轮的测试"测反了"，把错误实现固化住了**（第三轮引入）
第三轮那条 `session_timeout_marks_terminal_offline_in_db` 的夹具把行的 `update_time`
写成"插入那一刻"，而清理的 `deadline` 是"清理那一刻" —— 两者**同一秒** → `<=` 成立
→ 测试通过。一旦把实现改成正确形态（deadline 前移 + 严格 `<`），`update_time == deadline`
→ 不成立 → 断言 `status == 0` 失败。
**变异测试实测：只把实现改成正确形态、其余不动，这条测试 25 次里红 21 次（84%）** ——
它不是"没测到"，而是**把 bug 当成期望行为固化住了**，任何人按正确方式修这个 bug
CI 都会红，从而把修复推回去。
现：夹具的时间戳改为 `now - 2min`（忠实真实场景：终端最后上报是超时之前），
并补一条**同秒**用例（`update_time == deadline` → 不置离线）。

**③ P2 `phone_number` 重复行 → 「取哪一行」不确定 → 运维配的鉴权码被静默抹掉**
`phone_number` 在三方言里都没有唯一约束（唯一索引是 `(id, phone_number)`，而 id 是主键）。
读侧没有 `ORDER BY` 时，**取到哪一行是不确定的**（postgres 返回物理扫描顺序的第一行，
而心跳每次都会重写这些行 → 取到的行会翻转）；写侧 `UPDATE ... WHERE phone_number = ?`
会更新**所有**重复行。两者叠加：读侧若恰好取到一条 `auth_code` 为空的行 → 判定成
"没配置" → 生成随机码 → 写进所有行 → **运维配的码被抹掉**，终端随后全部鉴权失败，
平台看不出发生过覆盖（实测复现）。
现读侧固定为「**优先取真配了码的那条**，再按 `id` 取最早的」，写侧保持"更新所有行"——
两者对齐：读到的规范行一定被写覆盖。

**④ P2 B19 ⑦ 的根因陈述是错的：`fetch_optional` 对多行不报错**
sqlx 0.7.4 的 `fetch_optional` 语义是"returns at most one row"，多行时**静默返回第一行**，
整个 sqlx 0.7.4 里没有 `"expected at most one row"` 这个错误串。
B19 ⑦ 写"重复行会让读取直接报错"—— **不成立**，那条不成立的断言会**主动误导**
后来人以为"重复行已被兜底"。真实路径见 ③。已改写代码注释与台账。

**⑤ P3 删除端点：空 body 从"422报错"变成"200 但一条都没删"**
第三轮把提取器从 `Json<T>` 改成 `Bytes` + 手动解析，注释写"空 body 等价于 `ids: None`"
—— **不等价**：`Json<T>` 对空 body 回 422，而 `ids: None` 对应的是 `{}` / `{"ids":null}`。
两者混为一谈会让"请求体丢了"的请求回 `code:0` + 空删除列表 —— **看起来成功，
实际一条都没删**，而且与 `{}` 的响应完全同形，调用方无法区分。
现：空 body、纯空白 body、缺 ids、`ids: null`、`ids: []` 全部明确回 `code:400`。

**⑥ P3 删除端点：body 超过 2MB 仍是裸文本 413**
`Bytes` **仍受** axum 的 `DefaultBodyLimit`（2MB）保护（超大 body 不会被整个读进内存，
这点是好的），但长度超限的拒绝发生在**提取器内** → 冒泡出去是裸文本 413 ——
第三轮"请求体错误也要回契约信封"只修了一半。现用 `Result<Bytes, BytesRejection>`
接住它，同样回契约信封（`code:400`）。另：错误文案**截断**到 200 字符
（serde 的 `invalid_type` 会把整个输入串带进 `Display`，100KB 的 body 能回出 100KB 的响应）。

**⑦ P3 清理的另外两处窗口**
- 摘会话之后、清地址表之前若有报文把会话**复活**（`process_jt_message` 的
  `map.entry().or_insert_with`），活着的终端会丢掉 `terminal_addrs` 映射 →
  `get_terminal_addr` 返回 None → `is_terminal_online` 为 false → **命令下发被拒**，
  直到下一个心跳。现：摘会话后**在同一把锁内复查**会话是否已复活，已复活的地址映射保留。
- 注册路径原本读两遍库（外层 `stored_auth_code` 吞错 + 内层 `issue_auth_code` 再读）。
  现只留 `issue_auth_code`（它内部已是"读 → 有则用/无则生成并落库"的完整逻辑，
  且**读失败会返回 Err**）；外层那次只会吞错 + 多打一次库。

**验证**：定向 Rust 单测 48 个（seq_sync 5 + sy_contract 16 + serde_flex 9 +
api_key_expiry 4 + cloud_record 过滤 1 + jt1078 鉴权 8 + jt1078 清理 3 + 删除端点 2）全部通过；
全量 `cargo test --no-fail-fast` **872 通过 / 0 失败**；
三方言构建零告警；`scripts/dialect_smoke.py` 三方言都只剩 2 项"预期失败"；
全量 Playwright 通过。
**用了变异测试**验证新测试不是空断言：把"优先取有码的行 + ORDER BY"去掉 →
`duplicate_phone_rows_keep_the_operator_configured_auth_code` 变红（生成随机码
`2d72b4e1…` 而非运维配的码）→ 恢复后变绿。同理把"deadline 前移 + 严格小于"改回去 →
第三轮那条测试 84% 变红。**教训**：新写的回归测试要**自己先变异一遍**，
确认它真的守得住 —— 本轮就有一条测试在写完时是"测反了"的。

**本轮已知但未改（记录在案，避免下轮重复发现）**：
- **平台侧仍有 64 处 `Json<T>` 提取器**，解析失败时回 axum 的裸文本 422/415。
  本轮只修了**本轮动过**的那个端点。**SY 第三方契约面已全覆盖** ——
  `src/handlers/sy_contract.rs` 里只有一处 `Json` 提取器（收藏删除的 DELETE），
  且已用 `Option<Json<..>>`。平台侧的调用方是本仓库自己的前端（axios 对非 2xx
  有统一处理），要全量迁移应作为独立改动（抽一个 `JsonOrEnvelope<T>` 提取器，
  而不是逐端点手写）。
- **`gb_jt_terminal.phone_number` 的表级唯一约束**仍需先做历史去重（独立的数据迁移），
  本轮只做读取侧兜底。
- **`/api/cloud/record/zip` 返回服务端绝对路径** `filePath`：平台既有行为，
  SY 别名照搬同样形状且端点本就要求登录，改它会影响既有调用方，
  作为独立改动统一处理（沿用 B17 的先例判断）。

### B21 ◐ 第五轮代码审查（**只完成自查**，未完成独立复核）

> **状态说明**：本轮原计划同样派两路独立 verifier（审 JT1078 改动 / 核验台账一致性），
> 但两者长时间高 CPU 占用（各自在跑 `cargo test` 全量重编译），**按用户要求中止**。
> 因此本轮**只有自查结论**，**没有独立交叉复核** —— 与 B17~B20 相比，
> 本条的置信度**低一档**。中止前它们已完成的交叉验证见下。

**① P2 「复查会话是否已复活」是恒真的空操作**（第四轮引入）
第四轮把清理的第一段（摘会话）与第二段（清地址表）**合并到同一把 `sessions` 锁**内
（这一步本身是**对的**，它关闭了「摘会话 ↔ 清地址表之间会话被复活」的窗口），
但同时又加了一行：

```rust
removed.retain(|addr| !map.contains_key(addr));   // ← 恒真
```

持着 `sessions` 锁时**谁也进不来**往 `map` 里插会话（`process_jt_message` 的
`map.entry(addr).or_insert_with` 也要拿这把锁），而 `map` 里的那些 addr 刚被我们
`remove` 过 —— 所以这行**永远不删任何东西**。
留着一行「看起来像有保护、实际什么都不做」的代码比没有更糟：后来人会以为
「复活窗口已被复查覆盖」，而真正的保护是**合并作用域**这件事本身。
现删掉该行，注释改写清楚：关闭窗口的是**合并两个作用域**；`deadline` 在
**摘掉会话之后立刻**取（此刻仍持锁，不可能有心跳插进来），它就是「会话失效的时刻」。

**锁序复核**（本轮自查）：`sessions → terminal_addrs` 与既有 `remove()` 一致；
全仓无反向嵌套 —— `is_terminal_online` 的 `terminal_addrs` 锁在 `get_terminal_addr`
**函数内**就释放了（不是调用方的临时量，Rust 2021 的 `if let` 临时量规则不适用），
不成环。

**中止前拿到的交叉验证**（被中止的台账核验 verifier 留下的产物）：
它在多个历史提交上跑了全量测试，与本台账记录的数字**逐一吻合**：

| 提交 | 轮次 | verifier 实测 | 台账记录 | 一致 |
|---|---|---|---|---|
| `f2b2c64` | 第二轮 | 851 通过 / 0 失败 / 3 忽略 | 851 | ✓ |
| `ed37680` | 第三轮 | 863 通过 / 0 失败 / 3 忽略 | 863 | ✓ |
| `f1b2bee` | 第四轮 | 869 通过 / 0 失败 / 3 忽略 | 869 | ✓ |

**验证（自查部分）**：`cargo fmt --check` 通过；全量 `cargo test --no-fail-fast`
**872 通过 / 0 失败**；三方言构建**零告警**；`scripts/dialect_smoke.py` 在
pg / mysql / sqlite 三方言都只剩 2 项「预期失败」；全量 Playwright **81/81**
（真实 WebRTC：ICE=connected、frames=3、bytes=6159）。
删除端点的四个请求体边界（空 body / 缺 ids / 语法错误 / 3MB 超大 body）
与 OpenAPI 契约（`requestBody` 不再标 required、补 400 响应）在真实实例上逐条实测。

**未完成的核验 → 后续已补齐**（第六轮结束时结清）：
- **本轮改动的变异测试** ✓：把那行空操作加回去，96 个 jt1078 测试**仍然全绿**
  → 确认它确实是行为中性的空操作，不是「看起来没影响」而已。
- **台账 B17~B20 的逐条代码对照** ✓：23 条「已修」声明逐条 grep 到代码落点，
  **全部属实**。
- **JT1078 锁序** ✓（穷举证明，非抽查）：全仓只有 `remove` 与 `cleanup_once` 两个
  函数嵌套两把锁，顺序都是 `sessions → terminal_addrs`；其余函数各只取一把锁；
  **全仓无反向嵌套 → 无死锁**。另用一段独立小程序证明 `get_terminal_addr`
  尾表达式里的 guard **在函数返回前就已释放**（`try_lock` 立即成功）——
  所以 `is_terminal_online` 不会同时持有两把锁、不构成反向嵌套。
- **「合并作用域」这条结构性不变量的回归保护** ✓：变异测试发现**没有功能测试
  能覆盖**它（拆回独立作用域 → 96 个测试仍全绿）—— 要在两段之间精确注入一次
  心跳得在生产代码里加钩子，代价大于收益。故新增一条**源码契约测试**守住
  （用缩进定位 `sessions` 作用域的结束点，要求 `terminal_addrs` 的取锁落在其内），
  并做了**双向变异验证**：拆开 → 变红；恢复 → 变绿。

### B22 ◐ 第六轮代码审查（SY 第三方契约面专项）

前五轮修的是「最近几轮改动引入的缺陷」。本轮换了个角度：**把 `/api/sy/*` 这个第三方
对接面当成一个对外 API 来审** —— 逐个端点核对「鉴权、错误形状、参数容忍度、OpenAPI 描述」，
并对 24 个 SY 端点做**运行时**审计（真实 HTTP 请求，逐条检查响应形状）。

**① P1 非法查询串回的是 axum 的裸文本，不是契约信封**
`Query` 提取器在解析失败时**绕过 handler** 直接回 `text/plain` 的 400
（`Failed to deserialize query string: page: invalid digit found in string`）——
handler 里怎么写都救不回来。SY 是**第三方对接面**，对接方按 `{code,msg,data}` 解析，
拿到裸文本会**直接让它的解析器炸**。
实测（修复前）：`GET /api/sy/camera/list?online=1` → **HTTP 422 + text/plain**。
现：`src/error.rs` 新增 `envelope_rejection_errors` 中间件，挂在 SY 子树上，
把「纯文本的 400/415/422」包回信封（`code` 取 HTTP 状态码）；其它响应一律放行。

**② P2 SY 相机列表的布尔参数违反仓库自己的约定**
`online` / `traditional` 用的是 serde 原生 `Option<bool>`，查询串里**只认
`"true"`/`"false"`**；第三方按最常见的方式传 `online=1` 会整个请求 422。
而仓库的 `serde_flex::de_opt_bool` 注释里明确写着"前端把开关值放在查询串里时常常是
`"true"`/`"1"`；只认 `bool` 会 422" —— **SY 端点违反了自己的约定**。
现两处都改用 `de_opt_bool`（接受 `true/false/1/0/on/yes/no`）。
前端传的是 JS 布尔（`online=true`），**向后兼容** ✓。

**③ P3 `/api/sy/camera/list/ids` 的 OpenAPI 缺 401 响应声明**
它声明了 `security`，但 `responses` 里只有 200。已补。

**④ 顺带核出的全仓现象（记录在案，本轮未改）**
全仓有 **99 个端点**「声明了鉴权但没写 401 响应」——OpenAPI 描述与实际行为不符
（实际都会回 401）。这是**文档约定**问题，不是功能缺陷；平台侧的调用方是本仓库前端
（axios 对非 2xx 有统一处理），要全量补齐属于独立的文档改动。
本轮只在**第三方对接面**（`/api/sy/*`）补齐 —— 那里的文档是给外部对接方看的。

**验证**：24 个 SY 端点**运行时**逐条审计 —— 无鉴权全部 401、有鉴权全部返回
`{code,msg,data}` 信封；OpenAPI 层面逐条核对（`security` 齐全、401 齐全、
无误渲染成 `path` 的参数 —— 此前担心的「全限定路径导致 utoipa 退化」经实测**不成立**，
23 个 SY 端点的参数都正确渲染为 `in:query`）。新增 2 条回归测试
（非法查询串必须回 JSON 信封；`online` 必须同时接受 `true/false` 与 `1/0`）。

### B23 ◐ 第七轮（第四轮独立复核的结论 + 据此修的 5 处）

第四轮派了**一路**独立 verifier 复核 JT1078 的清理任务与条件写（B21 遗留的最后一项），
结论 **PASS**：条件写确实有效、锁序无环、合并作用域确实关闭了复活窗口、被删的空操作
确为空操作。但它同时指出 2 个 P2、5 个 P3。**P2 与部分 P3 本轮已修**。

**① P2 条件写**逐行**判定 → 重复 `phone_number` 造成「半离线」**
`phone_number` 没有唯一约束，同一 phone 可能有多行；原来的 `WHERE phone_number = ?
AND update_time < ?` **逐行**求值 → "一行旧、一行新"时旧行被置离线、新行仍在线，
在线统计（`count_online_terminals` / `get_online_terminals`）仍把死终端算在线。
现：取该 phone 的**全部**行、逐行解析，**全部都过期才置离线**（保守方向），
并按 **id** 精确写（不再按 phone 批量改）。

**② P3 历史遗留的 `update_time` 格式让条件写**恒成立**
`gb_jt_terminal.update_time` 这一列**历史上被写过三种形状**：当前是
`%Y-%m-%d %H:%M:%S`（UTC 墙钟），旧版 `update_auth_code` / `update_last_position`
写的是 **RFC3339**，还有一段时间 `manager.rs` 用的是 `chrono::Local::now()`
（**本地**墙钟）。用 SQL 直接比字符串时：RFC3339 的第 10 位是 `'T'`（0x54）而纯格式
是 `' '`（0x20）→ 同一日历日内 **RFC3339 的值恒大于** deadline → 条件恒成立 →
那些终端**永远显示在线**。
现：改在 **Rust 里**解析成 epoch 再比（`db::jt1078::parse_update_time`，
兼容纯格式 / RFC3339 / 带毫秒的纯格式）。
**遗留**：「本地时间」那批与纯格式**同形不同基准**，无法从字符串区分 → 会被读成
"未来 8 小时"因而永远判不出过期。彻底订正需要一次数据订正（不在本轮范围）。
另：**解析不出来**时按「**还活跃**」处理（宁可暂不置离线，也不能把活着的终端误标成离线），
代价是那种行永远不会被自动置离线 —— 同样是数据问题。

**③ P3 `terms.retain` 的 O(T×R) 扫描在 `sessions` 锁内**
`removed.contains(a)` 是 Vec 线性扫描 → 整体 O(T×R)。实测终端表 1 万条 + 同时掉线
1000 台时**锁内约 8ms**，10 万条时约 **56ms** —— 期间**所有**入站 JT1078 报文都要等。
现：`removed` 改用 `HashSet` → O(T+R)。

**④ P3 注释「宁可晚一个清理周期再置离线」不实**
会话被摘掉后该 phone 已不在 `terminal_addrs` → **下一轮清理不会再看到它** ⇒
条件不成立就是**一直不置离线**（不是"晚一轮"）。注释已改准。

**⑤ P3 新增的源码契约测试有残余缺口**
原契约只判「`terminal_addrs` 的**取锁行**在 sessions 作用域结束之前」——
若有人 `drop(map)` 提前释放 sessions 锁（取锁行仍在作用域内）就**抓不住**。
现改为判「`terms.retain(` **调用行**在作用域结束之前」（retain 才是真正清地址表的
动作），并做了双向变异验证。

**⑥ P3 既有测试的夹具绕开了生产形态**
原先把行的 `update_time` **回拨 2 分钟**来避开同秒 —— 生产超时是 60s，
真实形态没被覆盖。现补了一条按**生产量级**（60 秒）的用例。

**验证**：新增 7 条离线标记专项测试（正常超时 / 刚活跃 / 历史 RFC3339 过期 /
历史 RFC3339 刚活跃 / 重复行半离线 / 重复行全过期 / 解析器三种形状），
并做了**变异双向验证**（去掉 RFC3339 解析支 → 2 条测试变红；恢复 → 变绿）。
全量 `cargo test --no-fail-fast` **882 通过 / 0 失败**（lib 817 / 集成 65）；
三方言构建零告警；dialect_smoke 三方言都只剩 2 项「预期失败」；全量 Playwright 81/81。

**第四轮独立复核未能覆盖的部分**（记录在案）：pg/mysql 方言下条件写的实机行为
（本轮只跑 SQLite；SQL 三方言同构，但 pg/mysql 的 collation 对定宽 ASCII 时间戳的
排序只有推理、没有实机证据，schema 里 `update_time` 是 `varchar(50)` 且**未指定
collation**）；真机 JT1078 终端；多实例共享 DB、机器时钟不一致时的行为。

### B24 ◐ 第八轮（API Key 台账的排序、e2e 清理空操作、分页偏移溢出）

本轮起点是第七轮之后残留的一个 e2e 失败：`e2e/tests/apiKey.spec.ts` 的
「创建后可鉴权调用…」与「选了未来的过期时间后…」两条用例查不到自己刚建的那行。
查下来是**三个独立缺陷**叠在一起。

**① P1 API Key 列表按 `id` 升序 → 新建的 Key 落在最后一页**
`db/user_api_key.rs` 的 `list_paged`（三方言分支）与 `list_by_user_id` 都是
`ORDER BY id`（**升序**）。前端 `web/src/views/apiKey/index.vue` 每页 20 条。
库里一旦超过 20 条（共享 e2e 库里实测 23 条，含 `e2e-partner` / `curltest` /
`proxytest` 等历史遗留），**新建的 Key（`id` 最大）就落到第二页** → 管理员建完
Key 回到列表页，页面上根本看不到自己刚建的那条。
这是产品缺陷，不是"数据脏"：凭证台账页的主要用途就是"刚建的那条要能立刻找到、
立刻停用或删掉"。仓库其它列表（设备、审计日志、系统日志、JT1078 终端、录像）
一律"新的在前"，API Key 列表是异类。
现：三方言分支与 `list_by_user_id` 全改 `ORDER BY id DESC`。

**② P1 e2e 的 `cleanupLeftovers` 不传 `id` → "清理"一直是空操作**
删除端点是 `DELETE /api/userApiKey/delete?id=`（`IdQuery.id`）。而 e2e 的清理
helper 只把列表 URL 换成了删除 URL、**没带 `id`** → 后端一律回 400「缺少 id」，
**一行都没删掉**。于是 ① 的现象被放大：库里越积越多，新建的 Key 越往后排。
现：删除时带上 `?id=`，并**逐条校验结果**（`!res.ok || body.code !== 0` 就抛错），
让"清理失效"当场失败，而不是留下脏数据、等后面某条断言以另一种面貌炸掉。

**③ P2 分页偏移的 `u32` 乘法溢出**
`(page.saturating_sub(1)) * count` 里的 `page` / `count` 都是 `u32`，而 `page`
直接来自查询串、没有上限。`page = 4294967295`、`count = 100` 时
`4294967294 × 100 = 429 496 729 400` **超出 `u32` 上限**：
  * debug 构建直接 panic（`attempt to multiply with overflow`）—— handler 链路上
    **没有 `CatchPanic` 层**，panic 会打断整条 HTTP 连接；
  * release 构建则**回绕**成 `4 294 967 096` —— 请求第 N 页却拿到了别的页。
同类写法共 **29 处**（`db/` 21 处 + `handlers/` 8 处），涉及平台、通道、推流、
代理、录像计划、日志、审计、用户、API Key、JT1078 终端、公共通道、设备、设备通道、
报警、移动位置、SY 相机等几乎所有列表端点。
现：新增 `src/pagination.rs`，统一用 `i64` / `usize` 算偏移
（`offset` / `offset_usize` / `offset_i64`），29 处全部改用。
其中 4 处 `page` / `count` 本来就是 `i64` 的（`cloud_record` ×2、`alarm`、
`mobile_position`），旧写法 `(page - 1) * count` 在 `page = 0` 时会算出**负的
OFFSET** —— PostgreSQL / MySQL 直接报错、SQLite 当成 0，三种方言行为不一致；
`offset_i64` 把 `page < 1` 一律按第 1 页算。
顺带修掉一个**步长与页长不一致**的隐患：`device` / `jt1078` / `platform` /
`platform_channel` / `stream_proxy` 这几处的 `LIMIT` 用的是 clamp 后的值、
`OFFSET` 用的是 clamp 前的 `count`，`count` 超过上限时翻页会跳过整段数据
（当前所有调用方都先 clamp 到同一上限，所以尚未触发）。

**④ P2 API Key 的写操作在目标不存在时静默「成功」**
`remark` / `enable` / `disable` / `reset` 四个端点都**忽略 `UPDATE` 的
`rows_affected`** —— id 不存在时 `rows_affected == 0`，但那不等于"成功"。
`reset` 最有害：目标不存在时它照样回 200 + 一个**从未被存储**的 32 位十六进制
密钥 → 前端弹出"请立即复制密钥"、管理员把它当回事抄走 → 用它调接口必然 401，
而界面全程显示"成功"。
现：按仓库既有约定（`jt1078` 的终端删除、`platform` 的目录删除都是
"影响 0 行 → 404"）回 **404「API Key 不存在: id=…」**，并给 4 个端点补上
OpenAPI 的 404 响应声明。
**三方言的 `rows_affected` 语义一致**（= 命中行数，与"值有没有变"无关）：
PostgreSQL 统计匹配的；SQLite 的 `sqlite3_changes()` 统计被 UPDATE 处理过的行；
MySQL 侧 sqlx 握手时开了 `CLIENT_FOUND_ROWS`（`sqlx-mysql-0.7.4`
`connection/stream.rs`），所以回的是**匹配**行数而不是**改变**行数 ——
否则"启用一个已经启用的 Key"会被误判成 404。
**不动 `delete`**：DELETE 幂等是仓库明确写下的契约
（`router::tests::authenticated_collect_roundtrip_updates_business_storage`
断言重复删除回 `code:0`），改它属于改契约而不是修缺陷。

**验证**
* 新增 `src/pagination.rs` 单测 4 条（`offset` / `offset_usize` 溢出、正常翻页、
  `offset_i64` 不出负值）；
* 新增 `src/db/user_api_key.rs` 的 `ordering_tests` 4 条（新 Key 必须落在第一页
  第一行、倒序翻页不重不漏、`list_by_user_id` 同口径、超大 `page` 不 panic）；
* 新增 `src/router.rs` 的 `api_key_write_on_missing_row_returns_404` 1 条
  （四个写端点在目标不存在时必须回 404，且**不得回出密钥**）；
* **变异双向验证**（5 组，逐组确认"改回旧实现 → 变红 → 改回 → 变绿"）：
  ① `list_paged` 改回 `ORDER BY id` → 2 条红；
  ② `list_by_user_id` 改回 `ORDER BY id` → 1 条红；
  ③ 偏移改回 `(page.saturating_sub(1)) * count` → 超大页码那条红（panic）；
  ④ `pagination` helper 改回旧的 `u32` 算法 → 2 条红；
  ⑤ 去掉 `require_api_key_affected` 的 404 检查 → 404 那条红（回 `code:0`）。

**本轮未覆盖 / 已知但未改**
* `handlers/sy_camera.rs` 的相机列表是"先取全量再在内存里切页"，
  `total` 语义与分页器的配合此前已单独修过（见 B 系列旧条目），本轮只统一了偏移算法。
* `db/cloud_record.rs` 等 4 处 `i64` 分页的调用方目前都自带 `.max(1)` 兜底，
  `offset_i64` 只是把兜底下沉到 db 层，不改变现有可达行为。
* **前端补了一条**：建完 Key 后把页码重置回第 1 页 —— 列表是"新的在前"，
  若停在第 3 页再刷新，新建的 Key 仍在别的页里，"刚建完却看不到"等于没建。
* `record/zip` 返回服务端绝对路径、`phone_number` 表级唯一约束需先做历史去重
  —— 两项沿用前几轮结论，仍未改。

### B25 ◐ 第九轮（PostgreSQL 上 JT1078「置离线」整条链路失效）

本轮由**三条独立线索**同时指向同一个位置（`db/jt1078.rs` 的
`mark_terminal_offline_if_stale`），最后**两路独立 verifier** 与自查在
**真实 PostgreSQL** 上把它坐实。

**① P0 第七轮引入的 PostgreSQL 方言回归 —— 「超时 ⇒ 终端离线 ⇒ 需重新注册 + 重新鉴权」在 PG 上 100% 失效**

第七轮（`fd13cfe`）把原先**单条原子条件写**改成了「Rust 里 SELECT 判定 → 再按 id UPDATE」，
改写时丢了**方言分支**，留下**两个各自独立**的 PostgreSQL-only 失败点：

* **裸 `?` 占位符**：postgres 的预编译语句协议只认 `$1`。直接发 `?` →
  `ERROR: syntax error at end of input`（`?` 在语句末尾时）或
  `syntax error at or near ","`（后面还有内容时）。
* **`id` 解码成 `i64`，而列是 INT4**：postgres 的 `gb_jt_terminal.id` 是 `serial` →
  **integer(4)**，sqlx-postgres 的类型检查是**严格**的：`i64`（INT8）解 INT4 列直接报
  `mismatched types; Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`。
  仓库自己的 `src/dyn_where.rs` 早就写下了这个坑并提供了 `bigint_col()`，
  本轮那行 SQL **两个既有工具都绕过了**。

**只修占位符仍然失败** —— 实测两步分别触发：
`① 改回裸 ? → syntax error at end of input`；`② 只把 i32 改回 i64（占位符已修）→
mismatched types ... INT8 vs INT4`。两处都必须改。

错误被 `jt1078/manager.rs::cleanup_once` 吞成一条 `tracing::warn!` → **无声失败**：
终端永远显示在线，平台照样给它下发命令，而它其实从未重新鉴权过。
`config/application.toml` 明写 PostgreSQL 是「生产主力」库。

现：占位符走 `crate::dyn_where::dialect_sql`（自动改写成 `$1..$n`），
`id` 解码改成 `i32`（与 `JtTerminal.id` 一致，三方言都通）。

**② P2 第七轮把原子的条件写退化成「先查后写」，引入 TOCTOU**

旧实现是**一条** `UPDATE … WHERE phone_number = ? AND (update_time IS NULL OR update_time < ?)`，
条件在 `WHERE` 里，天然无竞态。新实现的 `UPDATE` 只带 `WHERE id = ?` —— 读（SELECT）与
写（UPDATE）之间的窗口里若到达心跳，`update_time` 被刷新，但**活着的终端仍会被写回离线**。
心跳路径确实会刷新 `update_time`（`manager.rs` → `persist_terminal_online` →
`update_terminal_status`），所以这个竞态在生产里真实存在。

现：`UPDATE … WHERE id = ? AND update_time = ?`，`update_time` 带上**读到的原值** ——
心跳把值改掉了 → 条件不成立 → 影响 0 行 → 终端保持在线。
能走到写这一段的行，其 `update_time` 一定非空且可解析（解析不出的按「还活跃」提前返回了），
所以不存在 `NULL = NULL` 的坑；这些值都是定长数字时间戳，三方言的 `=` 都是字节/数字比较
（MySQL 的 `utf8mb4_general_ci` 大小写不敏感，但值里只有数字与分隔符，不受影响）。
**顺带修掉两处注释与实现脱节**（注释还在描述旧的条件写语义）。

**③ P2 方言占位符必须走统一出口 —— 加一条源码扫描守卫**

2026-09-12 的 postgres 冒烟一次性抓到 23 处「固定 SQL 直接写 `?`」的同类问题
（见 `src/dyn_where.rs` 的说明），修完没有机制防复发；第七轮又新引入 1 处。
现新增 `src/dialect_guard.rs`（`#![cfg(test)]`，生产构建不含本模块）：
扫描 `src/` 下所有 `.rs`，找出**在 postgres 构建里会被编译**、且 SQL 里带**裸 `?`** 的语句，
它们必须被 `dialect_sql(...)` 包裹，或位于 `#[cfg(feature = "mysql" / "sqlite")]` 分支里。
守卫用的是「cfg 分段 + 字面量归属」的静态分析（不是正则），**新增语句即被覆盖**。

**验证**
* 新增 `tests/postgres_jt1078.rs`（`#![cfg(feature = "postgres")]` + `#[ignore]`，
  需 `TEST_POSTGRES_URL` 指向隔离库）：4 条用例（过期置离线 / 活跃不置离线 /
  重复行一半活跃整体不动 / 重复行全过期全置离线）。**在真实 PostgreSQL 上 4/4 通过**。
* 新增 `src/dialect_guard.rs` 守卫 1 条。
* **变异双向验证**（逐组确认「改回旧实现 → 变红 → 改回 → 变绿」）：
  ① 把 `SELECT` 改回第七轮原样（裸 `?` + `i64`）→ postgres 上 **4 条全红**
  （`syntax error at end of input`）；
  ② 只把 `i32` 改回 `i64`（占位符已修）→ **4 条全红**
  （`mismatched types; Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`）；
  ③ 守卫：把 `dialect_sql` 去掉 → 守卫**变红并精确指出行号**。

**本轮未覆盖 / 已知但未改**
* `phone_number` **没有唯一约束**（三方言的 `unique (id, phone_number)` 里 `id` 是主键，
  等价于没有约束），且**没有历史去重脚本** —— 重复行仍会产生。现已做到
  「重复行不再半离线」，但**在线统计仍按行计数**（`count_online_terminals` 是
  `COUNT(*)` 而不是 `COUNT(DISTINCT phone_number)`），1 个物理终端的 2 行仍计 2。
  改统计口径要先做历史去重，另立。
* `handlers/jt1078.rs::terminal_add` **没有存在性检查**，管理员重复提交同一手机号即多一行；
  `insert_terminal` 是裸 INSERT，配合「读不到才插」存在 get→insert 竞态。
  需与上面的去重一起做。
* 「本地墙钟」历史行（旧版用 `chrono::Local::now()` 写过 `update_time`）被按 UTC 解释 →
  读出「未来 8 小时」→ 永远判不出过期 → 永远在线。这是**数据问题**，
  订正需要知道部署时区，不能拍脑袋减 8 小时。沿用 B23 ② 的结论。

### B26 ◐ 第九轮续（7 处写端点「目标不存在仍回 200」的假成功）

第八轮修掉了 API Key 的同类缺陷（`remark` / `enable` / `disable` / `reset`），
但**只修了那 4 个端点**。第九轮把全仓 **242 个写端点**（由 `utoipa::path` 注解驱动，
逐个核对是否在 `src/openapi/routes_*.rs` 注册过）扫了一遍，
查出**同一模式还有 7 处**，其中 1 处连写法都像「已经检查过了」。

| 端点 | 位置 | 现象 |
|---|---|---|
| `POST /api/common/channel/update` | `handlers/common_channel.rs` | 通道不存在 → 回 200「更新成功」，一行都没改 |
| `POST /api/common/channel/reset` | `handlers/common_channel.rs` | 同上，「重置成功」是假的 |
| `POST /api/region/update` | `handlers/stub.rs` | 区域不存在 → 回 200「成功」 |
| `POST /api/group/update` | `handlers/stub.rs` | 分组不存在 → 回 200「成功」 |
| `POST /api/platform/update` | `handlers/platform.rs` | 平台不存在 → 回 200「平台更新成功」；紧接着的 `get_by_id()` 回 `None` → `sync_platform_registration` 被跳过，**级联注册状态从未被触碰** |
| `POST /api/platform/channel/custom/update` | `handlers/platform.rs` | 通道不存在 → 回 200「自定义通道更新成功」 |
| `POST /api/push/update` | `handlers/stream.rs` | `Ok(_)` 把 `u64` 直接丢掉 —— **写法像「已 match 检查过」，实际什么也没比对** |

现：统一走 `handlers/mod.rs::require_affected(affected, 什么, id)` 一个出口，
命中 0 行 → 回 **404**（与仓库既有约定一致：`jt1078` 的终端删除、`platform` 的目录删除、
`device_update`、`channel_audio`、`role` / `proxy` 的删除端点都是「影响 0 行 → 404」）。
5 个端点补上了 OpenAPI 的 404 响应声明。

**`platform/channel/custom/update` 不能用返回值判存在性**：
`db/platform_channel::update` 在 `custom_name` 与 `custom_info` **都为空**时会提前
`return Ok(0)`（「没有要改的字段」），此时通道**存在**也会拿到 0 → 会被误判成 404。
该端点改为先按 id 查存在性，不存在 → 404。

**三方言的 `rows_affected` 语义一致**（= 命中行数，与「值有没有变」无关）：
PostgreSQL 统计匹配的；SQLite 的 `sqlite3_changes()` **实测**统计的是被 UPDATE
处理过的行（即**匹配**行，不是「值变了」的行）；MySQL 侧 sqlx 握手时开了
`CLIENT_FOUND_ROWS`（`sqlx-mysql-0.7.4` 的 `connection/stream.rs`），回的是**匹配**行数。
所以 `affected == 0` ⇔「目标不存在」在三方言上等价。

**不动 `delete`**：DELETE 幂等是仓库写下的契约
（`router::tests::authenticated_collect_roundtrip_updates_business_storage`
断言重复删除回 `code:0`），改它属于改契约而不是修缺陷。
**但仓库内 DELETE 端点存在 200 / 404 两套口径**（`proxy/delete`、`jt1078/terminal/delete`、
`role/delete` 回 404；`push/remove`、`region/delete`、`group/delete`、`record/plan/delete`、
`userApiKey/delete` 回 200）—— 这是**一致性**问题，需单独立项决策，本轮不动。

**已补回归测试守护**（`router::tests::write_endpoints_on_missing_row_return_404`）：
一条用例覆盖上表全部 7 个端点，断言目标不存在时 **HTTP 404 + 信封 `code:404`**。
断言用「收集全部结果再统一报错」而不是逐条 `assert_eq!` —— 后者回归时只会在
第一个坏掉的端点上 panic，看不出其余 6 个是修好了还是同样坏了。
**两组互斥的变异验证**（合起来正好覆盖 7 个端点）：① 去掉 `require_affected`
的 404 检查 → **5 处**变红；② 去掉 `platform/channel/custom/update` 的存在性
查询、把 `push/update` 的 `Ok(0) => Err(404)` 改回 `Ok(_)` → **另 2 处**变红。

### B27 ◐ 第九轮续（JT1078 终端手机号的 BCD 填充方向与国标相反）

`jt1078/command.rs` 的 `phone_to_bcd` / `bcd_to_phone` 是一对**不互为逆运算**的函数：

* 编码是**右补零**（号码靠左放，尾部补 0 到 12 位）；
* 解码是 `trim_end_matches('0')` —— 把**尾随**的 0 全去掉。

尾随 0 分不清是「号码本身的 0」还是「补零的填充」，于是：

* **真实号码以 0 结尾时被截断一位**：`13812345670` → BCD `138123456700` →
  解码成 `1381234567`。平台用它当终端身份（`terminal_addrs` 的 key、
  `gb_jt_terminal.phone_number` 的值、下发给终端的帧头号码），
  管理员看到的却是少一位的号，按真实号码**查不到**这个终端。
* **不同号码塌缩到同一个 key**：`terminal_addrs` 是 `HashMap<String, SocketAddr>`，
  一个 key 只留一个地址 → **A 车的指令发给了 B 车**。
* 对**真实终端**（按国标发报）更糟：标准是**左补零**，终端发来的 BCD 是
  `013912345678`，解码后 `trim_end_matches('0')` 保留前导 0 → 库里存的是
  `013912345678`，而管理员填的是 `13912345678` → `get_terminal_by_phone` 是精确匹配，
  **对不上**。

**国标依据**（多个独立来源一致，JT/T 808-2013 §5.4.1.2 的消息头定义）：
> 起始字节 4　终端手机号　BCD　**手机号不足 12 位，则在前补充数字**，
> 大陆手机号补充数字 0，港澳台则根据其区号进行位数补充。

即 11 位大陆手机号 `13912345678` → `013912345678` → `0x01 0x39 0x12 0x34 0x56 0x78`。

现：编码改**左补零**（号码靠右放，超 12 位保留低 12 位），解码改**去前导 0**，
两者成为**严格逆运算**。仓库自带的 JT1078 模拟器
（`mock/tools/jt1078-terminal/jt1078_terminal_mock.py`）同步改成左补零 ——
两边约定必须一致，否则终端注册必然失败。

另加 `jt1078::command::canonical_phone`：只保留数字、去掉前导 0，
用于管理员手填号码的路径（`handlers/jt1078.rs` 的 `terminal_add` / `terminal_update`），
让「管理员填的形态」与「设备上报的形态」统一，否则同一个终端会在库里留下两行。

**验证**：新增 3 条单测（往返保真（含末位为 0 的号码）/ BCD 字节布局必须是左补零 /
号码规范化）。**数据无需迁移**：现存终端号码是 `13912345678`（模拟器产的），
左补零后模拟器与平台解出的仍是同一个值。

### B28 ◐ 第九轮续（日志保留策略是死配置；系统日志端点缺管理员门禁）

**① P1 日志保留策略**只存在于配置里，没有任何代码消费**

* `config/application.toml` 的 `audit.retention_days = 90` —— 定义了，但
  `grep -rn "retention_days" src/` **除定义处外零命中**；
* `db/log.rs::delete_before` —— 函数在、单测在，**没有任何生产调用点**；
* `db/audit_log.rs` —— **连删除函数都没有**。

结果两张表**无限增长**：实测 e2e 共享库里 `gb_audit_log` **19 万行**、
`gb_log` **6.8 万行**，库文件 **49MB**。长期运行的平台会一直涨。

现：新增 `db::audit_log::delete_before(pool, days)` 与
`db::log::delete_older_than(pool, days)`，并由 `src/lib.rs` 的后台任务按
`log.cleanup_interval_hours`（默认 6 小时）清理；启动时第一次 tick 立即触发，
把历史遗留的老数据一并收掉。配置新增 `[log]` 段
（`system_retention_days` 默认 30、`cleanup_interval_hours` 默认 6），
`audit.retention_days` 的死配置**就此变成真的**。

**截止时间必须在正确的时钟里算**，两张表的时钟不一样：

* `gb_audit_log.create_time` 由**数据库**写入（`CURRENT_TIMESTAMP` / `NOW()`）——
  SQLite 是 **UTC**，MySQL / PostgreSQL 是**会话时区**。在 Rust 里按 UTC 算再拿去比，
  MySQL / PG 上会**整体偏一个时区**（UTC+8 部署下差 8 小时）→ 保留期变成 90 天 ± 8 小时。
  现由 `dyn_where::days_ago_expr(days)` **在数据库自己的时钟里**算
  （sqlite `datetime('now', '-N days')` / mysql `DATE_SUB(NOW(), INTERVAL N DAY)` /
  pg `NOW() - INTERVAL 'N days'`）。
* `gb_log.time` 由**平台自己**写（`logging::CaptureLayer` 用
  `chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")`）→ 截止时间必须在
  **同一个时钟、同一格式**里算，否则字符串比较会错（`%.3f` 的毫秒位）。
  现由 `db::log::delete_older_than` 在 Rust 里用 `Local::now()` 算。

**② P1 系统日志端点没有管理员门禁，且日志里曾有 JWT 签名密钥明文**

* `/api/log/list` 与 `/api/log/file/{name}` 的函数签名里**根本没有 `HeaderMap`** ——
  结构上就做不了鉴权；任何**已登录**用户都能导出 5 万行日志。
* `src/security.rs` 的弱密钥校验失败信息里**回显了密钥值**
  （`starts with 'a7f3c9e2…'`）→ 这条告警进 `gb_log` → 普通用户导出日志就能拿到
  **签名密钥** → 拿它伪造 HS256 token（对称签名，**密钥即身份**）→ **管理员提权**。

现：弱密钥表改成 `(密钥, 说明)` 二元组，报错只说**说明**、**不回显密钥值**（连前缀也不回）；
两个日志端点补 `HeaderMap` 并走 `authz::require_admin`（与 API Key 管理端点同一口径），
OpenAPI 补 403 声明。

**③ P0 容器化部署用的是仓库里公开的 JWT 密钥，且启动只告警不阻断**

`config/application.toml` 的 `secret` 与 `docker-compose*.yml` 的
`${GBSERVER_JWT_SECRET:-a7f3c9e2…}` 是**同一个已进公开 Git 历史的值**。
HS256 是对称签名 —— 拿到仓库的人拿它就能伪造管理员 token，**整个平台的管理面沦陷**。
仓库早就有弱密钥检测（`src/security.rs`）与 `GBSERVER__SECURITY__STRICT=1` 的 fail-fast，
但**默认只 WARN**。

现：`docker-compose.yml` 与 `docker-compose.sqlite.yml` 的
`GBSERVER_JWT_SECRET` 改成**必填**（`${GBSERVER_JWT_SECRET:?…}`，与同文件的
`GBSERVER_SIP_ADVERTISE_IP` 同一口径 —— 没设就拒绝启动），并默认开启
`GBSERVER__SECURITY__STRICT: "1"`（弱密钥 → **启动即失败**）。
本地 `cargo run` 不受影响（仍走配置文件里的值，只告警）。

**本轮未改（已记录，见 B29）**：登录失败回 HTTP 200 且无审计无限流、
`/api/play/share` 无凭据即可铸令牌、ZLM hook 只有 `on_play` / `on_publish` 校验 secret、
`/api/rpc` 在 `[rpc].secret` 未配置时无鉴权、审计中间件层序导致 401 不落库、
API Key 明文入库且支持查询串传递、CORS 全开、DB 原始错误串回客户端、
ZLM 管理 secret 与 ZLM 端口全网暴露。

### B29 ◐ 第九轮续（冒烟脚本的残留数据 + 本轮查出的 P2/P3 清单）

**① P2 `scripts/dialect_smoke.py` 建了 `smoke-user-*` 用户与同名角色，从不清理**

实测 e2e 共享库里已堆了 **9 个** `smoke-user-*` 用户与 9 个同名角色
（`gb_user` 10 行、`gb_user_role` 10 行），postgres / mysql 两套库同样在涨。
每跑一次冒烟就多一行 —— 与第八轮 `cleanupLeftovers` 是同一类问题：
「清理」根本没做，于是**共享库被历史垃圾堆满**，后面的断言被脏数据放大成别的症状
（第八轮就踩过：23 行 API Key 残留把「排序方向」这个缺陷伪装成了「查不到刚建的 Key」）。

现：脚本在建完用户 / 角色后**记下 id**，末尾**先删用户再删角色**（角色删除会拒绝
「仍有用户引用」的角色），并**逐条校验结果** —— 清理失败当场进 `FAIL` 列表，
而不是留脏数据等后面某条断言以另一种面貌炸掉。

**② 本轮查出的 P2 / P3（本轮未改，已记录）**

* **P2 吞错**：`handlers/device_stub.rs::subscribe_mobile_position` 用
  `unwrap_or_default()` 把 DB 写失败吞成 0，响应里却回「位置订阅已发送」。
  同功能的姊妹端点 `handlers/position.rs` 已经做对了（先查存在性再写）。
* **P2 批量端点丢计数**：`common_channel` 的 8 个「批量关联 / 解关联」端点
  （region / group 的 add / delete，以及各自的 device 变体）把 `rows_affected` 全丢掉，
  全部 id 都不存在时仍回 200「成功」。仓库对批量写的既有做法是**回计数**
  （`alarm` 的 `{"deleted": n}`、`stream` 的 `{"removed": n, "errors": []}`）。
* **P2 先查后写的 TOCTOU**：5 个端点（`record/plan/update`、`platform/delete`、
  `media_server/save`、`proxy/save`、`proxy/update`）把存在性判定放在读取侧，
  写入侧的 `rows_affected` 作为权威信号被忽略 → 并发删除的窗口里仍回 200「成功」。
* **P3 计数可观测但口径不一致**：`platform/channel/remove` 回
  `{"removedCount": 0, "message": "通道移除成功"}` —— 调用方能从响应体察觉，
  不算完全静默，但文案误导。
* **P3 解析失败的脏值永久粘住在线**：`parse_update_time` 解析不出时按「还活跃」处理，
  方向安全（宁可漏置离线也不误置），但那种行**永远**不会被自动置离线且没有任何告警。
* **P3 置离线时覆盖了 `update_time`**：写的是「清理那一刻」而不是「最后一次上报那一刻」，
  管理员无法从离线行看出它到底多久没上报了。

**③ 已被独立复核推翻 / 排除的怀疑项（避免下一个人重复踩坑）**

* **持 `sessions` 锁时做 DB IO**（本仓库历史踩过的队头阻塞坑）—— **不成立**：
  `cleanup_once` 的锁作用域内只有内存操作与 `terminal_addrs.lock().await`，
  DB 写在锁外。
* **锁序成环** —— 不成立：唯一同时持两把锁的地方（`cleanup_once` 与 `remove()`）同序。
* **`HashSet` 去重键会不会因 phone 前后空格 / 大小写漏判** —— 不成立：
  去重键是 `SocketAddr`；phone 来自定长数字串，不可能有空格或大小写差异。
  （真正的问题在 B27 的**截断**，不在空格 / 大小写。）
* **JT1078 时间写入是否还有现行代码用本地墙钟** —— 不成立：当前所有写入点都用
  `chrono::Utc::now()`，偏移只存在于 `411daf7` 之前的历史行。
* **静态资源路径穿越**（`/../config/application.toml`）—— 不成立：
  `ServeDir` 拒绝 `..`，随后落到 SPA fallback，响应体是 `index.html`。
* **`/api/log/file/{name}` 目录穿越** —— 已被修且有测试
  （`safe_log_file_name` + `safe_log_file_name_rejects_traversal_and_separators`）。
* **「被 disable 的 API Key 仍然能用」** —— 不成立：鉴权层确实读 `enable` 与 `expired_at`，
  禁用后回 401。
* **「普通用户能改别人的 API Key」** —— 不成立：6 个端点全部先 `require_admin`。
* **「审计表里写进了明文口令」** —— 不成立：`request_body` 列从未被写入，
  `path` 取自 `uri().path()`（不含查询串）。
* **「登录用户名枚举」** —— 不成立：不存在用户与密码错误返回完全相同的 code / msg / 状态码。

**④ 本轮新增的一条排查纪律（差点把环境问题当代码回归去「修」）**

全量 Playwright 首跑 **76 通过 / 5 失败**，失败的 5 条全是**媒体类**用例
（live 播放器 ×3、`playUrls` 的 FLV、WebRTC）。本轮改动完全没碰 SIP / SDP / SSRC /
RTP 这条链路 —— 但「没碰」不等于「不是回归」，所以必须查清。查下来是
**环境配置**：重启 SIP 终端模拟器时漏了 `--auto-bye-secs 0`（默认值 3 秒 = 设备端
收到 INVITE 就主动 BYE），于是设备端在平台还在等流的时候就挂断了会话，平台
`/api/play/stop` 随后自然报 `No matching invite session`。补上该参数后单跑 8/8
通过、全量复跑 81/81。

**教训**：`--auto-bye-secs` 的默认值 3 与「测试平台侧 BYE 时的期望值 0」正好相反，
而模拟器的注释里写得很清楚（"0 = 不自动挂断，用于测试平台侧 BYE"）。**重启任何
外部进程前先核对它的启动参数**——尤其是那些**默认值与测试期望相反**的参数；
否则「测试失败」会被误判成「代码回归」，把时间花在不存在的缺陷上。

### B30 🟡 多方言运行期改造后**仍未验证**的事项（2026-10-25）

数据库后端已从「编译期互斥三选一」改为「三驱动同编一个二进制 + 运行期按 `database.url`
选库」，本条目登记**改造完成后仍未拿到证据**的部分 —— 每条都写明复现条件。
方案见 [`DB_RUNTIME_DIALECT_PLAN.md`](DB_RUNTIME_DIALECT_PLAN.md)。

**① pg / mysql 的 9 条真实库专项用例 —— ✅ 已跑通（2026-10-25）**

`tests/postgres_jt1078.rs`(4) / `postgres_device_lookup.rs`(2) / `postgres_platform_channel.rs`(1)
/ `mysql_catalog_channel.rs`(1) / `mysql_platform_channel.rs`(1) 都是 `#[ignore]`，
`cargo test` 里是**跳过**状态 —— 「903 通过」这个数字**不包含**它们。

**2026-10-25 补跑：在真实 PostgreSQL 16 / MySQL 8 上 9/9 通过**，并且
**当场抓出 2 个真实缺陷**（见 B31）—— 此前一直绿是因为它们从没被真正执行过。

```bash
export TEST_POSTGRES_URL="postgres://postgres:postgrespw@127.0.0.1:5432/gbserver_test"
export TEST_MYSQL_URL="mysql://root:rootpw@127.0.0.1:3306/gbserver_test"
# ⚠️ 必须逐个 test target 指定，**不要**用 `cargo test -- --ignored` ——
# 那会把标记为 ignore 的 doc-test 伪代码示例也拉出来跑（它们本来就跑不通）。
cargo test --locked \
  --test postgres_jt1078 --test postgres_device_lookup --test postgres_platform_channel \
  --test mysql_catalog_channel --test mysql_platform_channel \
  -- --ignored
```

**2026-10-25 第十一轮再补 2 条**：`tests/postgres_dialect_judgment.rs` /
`tests/mysql_dialect_judgment.rs`（方言判据必须是 pool 自己的类型，见 B37）——
**合计 11 条真库用例全部通过**。

**这条已经在 CI 里接上**（新的 `dialect` job，用 service container 起
PostgreSQL / MySQL）—— 但**该 job 未在 GitHub Actions 上实跑过**（CI 是
`workflow_dispatch` 手动触发），其底层命令已在本地真实库上验证通过。

**② `cargo build --release` —— ✅ 已跑通（2026-10-25，第十一轮补跑）**

release 关闭了溢出检查，本轮改过的算术表达式（分页偏移等）若仍有 `u32` 乘法，
**release 下不回绕报错而是静默返回别的页** —— 所以这一项必须单独验。

**2026-10-25 第十一轮补跑：`cargo build --release` exit 0**（本仓库零告警，
仅第三方 crate 的 future-incompat 提示：redis 0.25.4 / sqlx-postgres 0.7.4）。

⚠️ **仍未验的是「release 模式下的测试执行」** —— 复现条件：
`cargo test --release -- --ignored`（带 `TEST_*_URL`）。build 通过只证明能编译，
不证明 release 下的行为（溢出检查关闭会改变语义）。

**③ 真机（GB28181 设备 / JT1078 终端）在多方言二进制下未验** —— 本轮的真机证据是
**改造前**在 sqlite 后端上取得的。多方言改造动的是**全部 DB 调用点**，D1 / D2 / D3
三档真机结论**不能直接继承**到 PG / MySQL 后端。复现条件：按 D1 条目接真机 +
`GBSERVER__DATABASE__URL` 指向 PG 或 MySQL。

**④ 目标部署环境（Linux Compose）的三方言实跑未验** —— 本轮三方言冒烟全部在本机
macOS 上跑（pg / mysql 走本机 Docker 容器）。E7 的「部署验收」仍是 🟡，
本轮**没有**改变它的状态。

**⑤ 转换流水线 `scripts/dialect-migrate/run.sh` 从基线整条重跑的可复现性未验** ——
本轮是**增量**改的（基线 `43b178b` → 逐层提交），**没有**执行过「checkout 基线 →
跑整条流水线 → 得到与当前 HEAD 等价的结果」这一闭环。复现条件：

```bash
git checkout 43b178b -- src/
bash scripts/dialect-migrate/run.sh
# 然后比对符号体检与 dialect_guard 守门人
```

> 补记（`eda04f6`）：这条闭环此前**即使跑通也会产出带格式违规的树** —— 流水线最后
> 一步只**打印**「接下来：cargo fmt」而**不执行**，而转换器合并方言分支时产出的
> 超长单行 `.bind(...)` 链本就会触发 rustfmt diff。现已把 `cargo fmt --check`
> 变成流水线**第 7 步门禁**（不合规即 exit 1）。**本条仍未验证**：门禁加上了，
> 但「整条流水线重跑能否得到与当前 HEAD 等价的结果」依旧没跑过。

**⑥ 已知非缺陷（不算缺陷，但别当回归查）** —— 三方言冒烟各有 **5 项**失败，
**失败项集合三方言完全一致**：CSV 导出不是 JSON（E1）、
`proxy/start` / `push/start` / `push/stop` / `media_server/check` 全部是
**ZLM 不可达**（媒体节点仍记着旧 LAN IP `192.168.3.88`，本机已变 `192.168.3.149`，
见 E6）。**三方言同样失败 ⇒ 不是方言回归**。

### B31 ◐ 第十轮（运行期方言改造的连锁问题：判据用错 + CI / 容器构建失效）

多方言改造把编译期方言分支改成了运行期，但**判据选错了对象**，且**下游的可执行
步骤没跟着改**。本条目登记这一串连锁问题。

**① 判据用错：505 处用了进程级全局而不是 pool 自己的类型**

改造前方言分支是 `#[cfg(feature = "postgres")]` —— **编译期**决定，对某个构建
永远正确。改成运行期后，部分站点被写成了 `crate::db::dialect::current()`
（**进程级全局**）：它在「一个进程一个库」的生产场景下与 pool 的方言一致，
但**测试里直接构造 `Pool::Postgres(..)` 时全局仍是默认 sqlite** → 走错分支。
（讽刺的是，同一批文件里有 6 处注释明确写着「方言必须看 pool，不能看进程级全局
`dialect::current()`」—— 代码与自己的注释互相矛盾。）

实测抓到 **2 个只在 PG 上炸的缺陷**：

* **`get_device_by_device_id` 的 `channel_count`** —— 少了 PG 专有的
  `::INTEGER`，而 PG 的 `COUNT(*)` 是 **int8**、Rust 侧按 `Option<i32>` 解 →
  `mismatched types; Rust type Option<i32> (as SQL type INT4) is not
  compatible with SQL type INT8`；
* **`batch_insert_channels`** —— SQL 走了全局的占位符改写，`?` 没被改成
  `$1` → PG 报 **`syntax error at end of input`**。

修法：全仓 **505 处**判据统一改成 **pool 自己的类型**（`pool.dialect()` /
`pool.sql(...)`）。转换器 `scripts/dialect-migrate/fix_pool_dialect.py`，
**编译器当 oracle**（脚本改完用 `cargo check --all-targets` 的报错行号定点
修 pool 变量名 —— handlers 层是 `state.pool`，db 层是 `pool`）。
**真正需要全局的两类保留并注明理由**：`#[cfg(test)]` 里「该造哪个方言的
mock pool」（那一刻**还没有 pool 可问**）、明确的 SQLite 测试直接用字面量。

**② 新增第二条守门人，防止复发**

`src/dialect_guard.rs` 增加 `dialect_branch_must_use_pool_dialect_not_process_global`
—— 扫出**非测试代码**里用进程级全局选方言分支的地方。**变异双向验证**过：
故意在 `src/db/role.rs` 注入 1 处 → 守门人**精确报出行号并变红** → 还原后恢复绿。
（守门人本身要能红才有意义 —— 只会绿的那种等于没写。）

**③ CI 与容器构建曾经都是坏的**

这轮改造让 `--no-default-features --features <单个方言>` **必然编译失败**
（`src/db/mod.rs` 的 `compile_error!` 要求三个 feature 同时启用），而下面
这些地方用的正是这条命令：

| 位置 | 后果 |
|---|---|
| `.github/workflows/ci.yml` 的 `feature-matrix`（三条 leg） | **每次推送 CI 全红** |
| `justfile` 的 `pg` / `mysql` / `build-pg` / `build-mysql` / `feature-check` | 开发者命令全炸 |
| `Dockerfile` 的 `GBSERVER_DB_FEATURE` 建参 | **镜像构建必失败**（部署路径） |
| `mock/scripts/run-all-tests.sh` 的方言分支 | 脚本跑到那一段就报错 |

修法：CI 的 `feature-matrix` 换成 **`dialect` job**（三驱动同编 +
`dialect_guard` 守门人 + **PostgreSQL / MySQL service container 上的真实库
专项用例** —— 这才是新模型下「各方言可用」的等价验收）；`justfile` 的 `pg` /
`mysql` 改成「改 `database.url` 不改编译」，`feature-check` → `dialect-check`；
Dockerfile 去掉方言建参（**一个镜像服务三种方言**），两个 compose 同步去掉
build args。

**④ 文档与注释的旧模型说法**（会让人照着做错事）—— 已逐处改掉：
`CLAUDE.md` / `AGENTS.md` / `database/README.md` / `config/application.toml` /
`Cargo.toml` / `scripts/dialect_smoke.py` / `mock/TEST_PLAN.md` /
`mock/scripts/run-all-tests.sh` / `docs/DB_DIALECT_NOTES.md` /
`docs/DEPLOYMENT_GUIDE.md`。

**教训**：**架构变更不是只改「用到它的那一层」**。方言模型从「编译期」变成
「运行期」时，除了被改的 `src/`，**所有会执行构建命令的地方**（CI、justfile、
Dockerfile、测试脚本、文档里的复现步骤）都依赖旧假设 —— 它们**不会编译报错**，
只会在**执行到那一步时**才炸，而那一步往往在**部署**或**别人 clone 之后**。
判据：**凡是能执行「按方言编译」这个动作的位置，都要逐个复查**。

### B32 ✅ 第十一轮 A：SIP Keepalive 未鉴权 → 一条 MESSAGE 劫持任意设备（P0，已修）

**现象**：任何能连到平台 SIP 端口（UDP/TCP 5060）的人，发一条 **MESSAGE**，
就能把**任意已注册设备**的信令地址改到自己名下。

**根因**：GB28181 的 Keepalive 走 SIP MESSAGE —— 这条路径**没有任何鉴权**
（平台侧也没有；SIP 头 From/Call-ID 在 UDP 上可随意伪造），而设备号取自
**XML 正文**（`<DeviceID>` 可任意填）。修复前 `handle_message` 的 Keepalive
分支无条件把「本次的来源地址」写进两处：

* 库：`update_device_online(..., Some(&ip_str), Some(addr.port()), ...)`
* 内存：`device_manager.update_keepalive(&dev_id, addr)`（写 `ip`/`port`/`addr`）

而信令地址正是平台**全部出站信令**的目标（**19 处** `get_address()` 调用点：
点播/回放/下载的 INVITE、PTZ、目录查询、SUBSCRIBE、BYE……）。于是平台的后续
信令**全部改发给攻击者** —— 既劫持了设备，又把平台变成一个可探测任意
`host:port` 的**出站跳板**（SSRF）。设备号甚至不需要猜：From 头填任意值、
正文填目标设备号即可。

**与 REGISTER 的不对称就是根因**：REGISTER 带 digest 摘要鉴权（401 挑战 →
校验 → 403 拒绝），写的是 REGISTER 的**源地址**；Keepalive 两样都没有，
却享有同样的写权限。

**修法**：

1. **地址只由带鉴权的 REGISTER 写入**。Keepalive 路径不再传 ip/port
   （SQL 是 `ip = COALESCE(?, ip)`，传 `None` 即保持原值）。
2. `DeviceManager::update_keepalive` 不再接收来源地址，只刷新
   `keepalive_time`/`online` —— 内存地址同样不再被心跳改写。
3. 新增来源校验闸门 `keepalive_source_allowed`：来源 **IP** 必须与该设备
   **已登记**的地址一致，否则整条 Keepalive 被忽略并告警。判据用 IP 而非
   `IP:port` —— SIP 头可伪造，只有源地址是网络层事实；而 NAT 可能让同一设备
   的心跳换源端口，只比 IP 可避免误杀正常设备。

**测试**：`keepalive_gate_tests`（陌生 IP 拒 / 同 IP 换端口放行 / 未登记拒）、
`keepalive_refresh_preserves_registered_address`、
`db::device::address_preservation_tests`。**变异验证**：闸门改成永远放行 →
`rejects_foreign_source_ip` 变红；SQL 的 `COALESCE` 去掉 → 库层地址保全测试
变红（`left: None, right: Some("192.168.1.7")`）。
**提交**：`0616826`。**证据强度**：P0（远程、无需凭据）。

### B33 ✅ 第十一轮 B：JT1078 接收缓冲无上限 → 未认证 UDP 可 OOM（P0，已修）

**现象**：任何能连到 JT1078 UDP/TCP 端口的对端（**无需鉴权**）都能让服务端
内存按线速增长直至 OOM，且日志里看不到任何异常。

**根因**：`Jt1078Session::feed_bytes` 对 `self.buffer` 无条件
`extend_from_slice`，既没有上限，也没有「声明长度永远凑不齐」的保护。旧格式
（4 字节大端长度前缀）可以声明 `0xFFFFFFFF` 字节的正文 —— 帧永远解析不出来，
缓冲只增不减。更糟的是 `manager::feed_bytes` 在解析**之前**就刷新了
`last_heartbeat`，于是未认证的对端持续发垃圾就能让会话**永不过期**，清理任务
也回收不掉它；再叠加「每个新源地址新建一个会话」，还能横向放大。

**修法**：`feed_bytes` 入口给缓冲封顶 256KB（JT808 单帧上限 64KB，余量充足）；
超限时告警并清空缓冲、流失步后重建，不返回任何帧。合法大帧不受影响。

**测试**：`session_buffer_bound_tests`（封顶生效 + 合法旧格式帧仍能解析）。
**变异验证**：把上限改成 `usize::MAX` → 封顶测试变红、对照测试仍绿。
**提交**：`a34f223`。**证据强度**：P0（远程、无需鉴权）。

### B34 ✅ 第十一轮 C：JT808 校验码按「转义后」字节计算（P1，已修）

**现象**：正文里出现**奇数个** `0x7E`/`0x7D` 时，平台下发的帧校验码差 `0x01`
—— 平台自己 `split_jt808` 判坏帧丢弃；真实终端按 JT/T 808 算出的期望值也对不
上 → 直接丢帧不回 ACK，平台侧命令等待器超时。典型触发：
`POST /api/jt1078/text-message` 的正文里带**一个** `~` 或 `}`。

**根因**：`build_jt808_frame` 是「先转义正文、再对**转义后**的字节做 XOR」。
转义把 1 个字节变成 2 个，且 `0x7D^0x02 = 0x7F`（原字节 `0x7E`）、
`0x7D^0x01 = 0x7C`（原字节 `0x7D`）—— **每转义一个字节就多异或一次 `0x01`**。
于是转义字节数为偶数时两边碰巧相等、为奇数时恰好差 `0x01`。而 JT/T 808-2013
§5.3 规定校验码算在**未转义**的内容上，平台自己的解析端
（`frame.rs`：先去转义再算 XOR）也是这个口径 —— **两端不一致**。

**为什么一直没被发现**：既有往返测试 `roundtrip_with_escaped_bytes` 的正文
恰好含 **4** 个需转义字节（偶数），把这个洞完整地掩盖了。

**修法**：按标准顺序重建帧 —— 先拼未转义内容并算校验码，再把整帧（含校验码）
统一转义。

**测试**：新增 `roundtrip_with_odd_number_of_escaped_bytes`（1/1/2/2/1 个转义
字节），修复前复现失败（`正文 [126] 的帧被判为坏帧`），修复后转绿。
**提交**：`cb3129a`。**证据强度**：P1。

### B35 ✅ 第十一轮 D：SIP TCP 分帧要求整块缓冲是合法 UTF-8（P1，已修）

**现象**：真实国标设备的报文正文常见
`<?xml version="1.0" encoding="GB2312"?>`，其中的中文按 GB2312 编码 —— 对
UTF-8 就是非法字节。走 TCP 的设备此后**所有**报文（ACK/BYE/目录/心跳）都堵在
缓冲里不再被处理，表现为「设备在线但平台毫无反应」，且内存单调增长。

**根因**：`TcpReader::try_parse_message` 对**整个接收缓冲**做严格
`String::from_utf8`，失败就 `return Ok(None)` —— 而 `Ok(None)` 的语义是
「消息还没收全」。于是正文里一个非法字节让整块缓冲永远变成 `Err`。
（UDP 路径走 `Parser::parse` 的 `from_utf8_lossy`，所以同一台设备走 UDP 一切
正常 —— **只有 TCP 会被这一个字节废掉**。`sip.tcp_enabled` 默认 `true`。）

**修法**：把分帧抽成自由函数 `take_sip_message`，**只在头部做文本化**（头部恒为
ASCII，正文原样切出，解析交给已是 lossy 的 `Parser::parse`）；并加三道上限：
`Content-Length` 超 1MB 报错 / 头未结束且缓冲超 1MB 报错 / 接收缓冲超 2MB 报错
（调用方据此断开连接）。

**测试**：新增 `sip_framing_tests`（GB2312 正文必须能切出 / 粘包两条都要切出 /
两条超限报错）。**变异验证**：把「整块缓冲严格 UTF-8」注回去 →
`takes_message_with_non_utf8_body` 变红、其余仍绿。
**提交**：`70939b5`。**证据强度**：P1。

### B36 ✅ 第十一轮 E：级联「keepalive 超时检测」其实测的是 REGISTER 间隔（P1，已修）

**现象**：健康的**上级平台**每 3~4 分钟就被误判 `Failed`，日志出现自相矛盾的
告警 `keepalive timeout (181s > 3s * 60)` —— 而此刻上级**一直在正常回应
keepalive**。

**根因**：两个概念被混成了一个字段。
`last_register`（距上次 REGISTER 多久）是**注册刷新节奏**的判据（刷新周期
`expires/2`，默认 1800s）；「上级还活着吗」（距上次**双向通信**多久）才是
**存活判定**的判据（超时阈值 `3×60 = 180s`）。修复前超时检测读的是
`last_register`，而记录双向通信的 `note_liveness` 在**生产路径上从未被调用**
（生产调用点为 0，只有它自己的单测在调）。

**影响链**：① 状态被置 `Failed` → `send_keepalive_all` 只给 `Registered` 发
keepalive → 超时窗口内 keepalive 停发（对端若要求间隔 ≤60s，会把本级判为
离线）；② 注册循环补发一轮**不带 Authorization** 的 REGISTER → 401 → 带摘要
重发 —— 对端可见的注册抖动从 30 分钟一次变成约 4 分钟一次。

**修法**：`RegistrationState` 新增 `last_liveness`，把两个概念拆开；
新增 `resolve_platform_from_keepalive_call_id`（按 `cascade_ka_{platform_id}_`
前缀匹配，`platform_id` 自身含下划线也安全）与 `note_liveness_from_response`；
**补上生产接线** —— SIP 响应路径收到级联 keepalive 的 200 OK 时调用它。

**测试**：`healthy_platform_answering_keepalive_is_not_marked_failed`（回应了
keepalive 的健康上级不得被判 Failed；反向对照「上级不再回应」必须仍判超时）、
`production_response_path_refreshes_cascade_liveness`（接线守卫，文本扫描）。
另修正既有两个 c3 用例 —— 它们此前**手动改写 `last_register` 来模拟超时**，
等于自证。**变异验证**：语义退回 + 拆掉接线 → 4 条用例变红，决定性断言报
`left: [], right: ["plat-live"]`。
**提交**：`e4f7377`。**证据强度**：P1。

### B37 ✅ 第十一轮 F：全局方言入口未收口，判据仍然可能用错（P1，已修）

**现象**：审计日志保留策略在 **postgres / MySQL 上 100% 失效**；报警列表与
通道列表在 postgres 上类型不匹配。

**根因**：第十轮（B31）把方言分支的判据改成「pool 自己的类型」，但**全局方言
的入口函数没被一起收口**：`dyn_where::{bigint_col, days_ago_expr, dialect_sql}`
与 `db::dialect::{dialect_sql, current_sql, bigint_col, days_ago_expr}` 读的仍是
**进程级全局** `dialect::current()` —— 只在「一个进程一个库」时与 pool 一致。
生产代码里有 **14 处**调用点手上明明有 pool（`bigint_col` ×3、
`days_ago_expr` ×1、`DynWhere::sql()` ×12）。在「直接构造 `Pool::Postgres(..)`」
的场景（**仓库自己的真库回归测试就是这么做的**）会按 **sqlite** 生成 SQL：
`datetime('now','-90 days')` → postgres 报
`42883 function datetime(unknown, unknown) does not exist`。

**修法**：把「全局方言」这一整类入口**删除**，让编译器强制每个调用点显式给
方言：① `DynWhere::sql(base, dialect)` —— 方言改成**必填参数**（12 处调用点
全部由编译器强制改写）；② 新增 `Pool::bigint_col` / `Pool::days_ago_expr`；
③ 删除 7 个读全局方言的函数，只留显式传方言的 `*_for` 版本；
④ 守卫收紧：`src/dyn_where.rs` 从守门人豁免名单里移除。

**测试**：新增**真实数据库**回归 `tests/postgres_dialect_judgment.rs` 与
`tests/mysql_dialect_judgment.rs`（sqlite 下 `datetime()` 合法，单测全绿也
证明不了任何事）。**变异验证**：退回读全局后两条真库测试同时变红 —— postgres
报 `42883 function datetime(unknown, unknown) does not exist`。
**提交**：`14292cf`。**证据强度**：P1。

### B38 ✅ 第十一轮 G：JT1078 缺失序号统计因 u16 回绕构造出 65533 条（P2，已修）

**现象**：`missing seqs` 的序号数组长度接近 65533，hook 收包体量约 400KB，
重传 UDP 报文必然超过 65507 字节上限（`send_to` 失败且被 `let _ =` 吞掉）。

**根因**：`collect_timed_out_missing` 用 `while s != max_seq.wrapping_add(1)`
扫描缺口。序号是 u16 且会回绕，于是循环上界不是「差值」而是「到 65536 的
距离」。触发：先收到 seq=5 并被投递（`expected_seq` 变 6），随后收到一个
**过期/重复**的 seq=3 的旧帧 —— pending 的最大键变成 3，循环便绕 65534 圈。

**修法**：用回绕距离判定（距离为 0 = 没有缺失区间；距离大于半个序号空间 =
`max_seq` 落后于 `start`，收到的是过期帧）；再加一道业务上限（缺失区间不可能
比 `pending` 容量还长）。

**测试**：`missing_seq_tests`（回绕场景下数量受限 / 正常缺口 `10,11,12` 仍被
如实报出）。**提交**：`3ec77a6`。

---

### B39 ✅ 第十一轮 H：入站 INVITE 无重传去重 → 每次重传新开一个 RTP 端口（P1，已修）

**现象**：设备重发 INVITE（没收到 200 OK 时的**正常**行为）或攻击者重放同一条
INVITE 时，平台**每次都新开一个 ZLM RTP 收流端口**，200 OK 里通告的 `m=` 端口
随之改变 —— 而设备仍按**第一次** 200 OK 的端口推流，媒体落到已被覆盖掉的孤儿
端口。表现正是台账里那类「设备在线但一帧 RTP 都没有」；同时每次重传泄漏一个
收流端口。

**根因**：RFC 3261 §17.2.1 要求服务端对**重传的同一请求重发同一份响应**。
本平台没有服务端事务层 —— `sip/core/transaction.rs:449` 的
`TransactionManager::handle_request` 是**死代码**（全仓库无调用点）。
`handle_invite`（`src/sip/server.rs`）取出 `call_id`/`cseq`/`via` 后**不做任何
去重**，直接 `open_rtp_server` → `invite_session_manager.create`（`insert`
**覆盖**同 Call-ID 的旧会话，旧会话的 `zlm_stream_id` 随之丢失）。

**修法**：

1. 新增 `src/sip/gb28181/invite_txn.rs` —— 入站 INVITE 的重传事务缓存。
   判据用 **Call-ID + Via + CSeq**（RFC 3261 §17.2.1 规定重传三者完全相同）；
   **不能**用 `From` —— 它在 UDP 上可随意伪造。命中即**原样重发同一份响应**
   （含同一个 SDP、同一个收流端口）。边界：只缓存成功应答（200 OK）；
   TTL 64s（覆盖 RFC 的 32s 重传窗口）；容量上限 1024 条（防伪造 Call-ID 撑爆内存）。
2. 覆盖旧会话前**先回收**它的 ZLM 收流端口。
3. 接上 `InviteSessionManager::cleanup_expired` —— 此前**没有任何调用点**
   （BYE 只把状态置成 `Terminated`，会话对象在内存里持续累积）。

**测试**：`sip::gb28181::invite_txn::tests` —— 重传命中同一条缓存且不新增条目 /
Call-ID·Via·CSeq 任一不同则不命中 / 过期条目不得重发且被清掉 / 容量上限生效 /
**接线守卫**（文本扫描，守住「生产路径真的查了缓存、记了响应、回收了旧端口、
接上了清理」）。**变异验证**：把去重查询改成恒 `None` → 接线守卫变红。
**提交**：`6837326`。

### B40 ✅ 第十一轮 I：级联注册不读取服务端在 200 OK 里授予的 Expires（P2，已修）

**现象**：本级在上游侧显示「已过期/注册过期」，而本级日志无任何告警、
`RegistrationStatus` 仍是 `Registered`。

**根因**：SIP 的注册有效期以**服务端授予的 `Expires`**（200 OK 的 `Expires` 头
或 `Contact` 的 `expires` 参数）为准，服务端有权把它压到比请求值更短。
`cascade/register.rs` 的刷新定时器只用「**我们请求的** expires」的一半；
200 OK 分支只调 `mark_registered`，`Expires` / `Contact` 的 expires 参数被
**完全丢弃**。

**影响**：上游 300s 过期、平台 1800s 才刷新 → 中间 1500s 里本级在上游侧是
「已过期」状态（上游停止向本级发 INVITE、拒绝目录订阅），而本级自认为在线。

**修法**：新增纯函数 `parse_granted_expires(contact, expires)`（Contact 的
`expires` 参数优先于 `Expires` 头，参数名大小写不敏感；解析不出返回 `None`）；
`RegistrationState` 新增 `granted_expires_secs`；刷新判据抽成纯函数
`needs_register_refresh(...)`，按**授予值**的一半算（无授予值时回退请求值），
下限 1s。判据必须无锁（注册循环的筛选器正持有 DashMap shard 读锁）。

**测试**：`granted_expires_is_parsed_from_the_200_ok` /
`refresh_interval_follows_the_granted_expires`（**盯住刷新决策本身**）。
**变异验证**：把判据退回「只用请求值」→ 刷新决策用例变红。**提交**：`e49a8b5`。

### B41 ✅ 第十一轮 J：级联 `server_port` 用 `as u16` 静默截断（P2，已修）

**现象**：管理页回显 `serverPort: 70000`，但抓包看到 REGISTER 发往 **4464**
（`70000 as u16 == 4464`；`-1 as u16 == 65535`），日志只出现「无响应」，
查不到根因。

**根因**：`cascade/register.rs` 三处 `as u16` 静默回绕；
`handlers/platform.rs` 的 `server_port: Option<i32>` 入库路径**没有
1..=65535 的范围校验**。`serverPort=0` 时 `SocketAddr::new(ip, 0)` 更没有意义，
注册循环里 `Err(_) => continue` **一次都不试**。

**修法**：入库前 `validate_server_port` 拒绝 1..=65535 之外的值（回 400）；
发包处改用 `server_port_u16`，越界返回 `None` → **跳过该平台并告警**，
而不是把信令打到错端口。

**测试**：级联侧越界不被回绕 / handler 侧入库前校验 / **端点级**（越界端口
必须被拒且**不得入库**，合法端口仍能新增）。
**变异验证**：级联侧退回 `as u16`、handler 侧去掉校验 → 3 条用例全变红。
**提交**：`9462bba`。

### B42 ✅ 第十一轮 K：录像调度器「本次不录像」但仍跑满 20 秒重试（P2，已修）

**现象**：日志写着「拉起设备流失败 …（本次不录像）」，紧接着却是约 20 秒的
`startRecord` 重试；通道数一多，轮次间隔明显小于 60 秒且首尾相接。

**根因**：`scheduler/record_plan.rs` 的失败分支**没有 `continue`**，代码继续
往下走 `start_record_with_retry(..., 25, 800ms)` —— 对着一路并不存在的
流做 **25 × 800ms ≈ 20s**的重试。每个「设备离线」的通道还要占住 15s
（INVITE 超时），N 个通道串行累加；一轮 tick 超过 60s 间隔后，
`tokio::time::interval` 默认的 **Burst** 行为会把错过的 tick 立即补发 →
调度器从「每分钟一轮」退化成**连续紧循环**，持续对 ZLM 发起 startRecord
（每通道 25 次/轮）。

**修法**：抽出 `LiveStreamOutcome` 与 `should_attempt_recording`（流没拉起来 ⇒
**不调用** `startRecord`，日志与行为终于一致）；抽出 `record_channel(...)`
（`start_record` 可注入）；同一 tick 内的通道**互不依赖** → 改用
`futures_util::stream::iter(...).for_each_concurrent(8, ...)` 并发处理；
`interval.set_missed_tick_behavior(MissedTickBehavior::Skip)` —— 错过的 tick
**不补发**。

**测试**：`recording_gate_tests` —— 流没拉起来时**一次都不该调用** `startRecord` /
流已就绪时必须正常调用一次（对照）/「流还没在 ZLM 注册」**仍要重试**
（B2 修过的竞态不得被误伤）。**变异验证**：把 `should_attempt_recording`
改成恒真 → 首条用例变红。**提交**：`c855c83`。

### B43 ✅ 第十一轮 L：ZLM 节点流计数是「事件计数」而非实际流数（P2，已修）

**现象**：多节点部署下，某些节点被长期跳过（负载均衡长期选错节点），且**没有
任何告警**。

**根因**：`zlm/hook.rs` 的增减逻辑是**增量计数**：`regist=true` 无条件 +1
（**不判断该流是否已计入**），配对的 `regist=false` 只 -1，差额**永久留在
计数里**；减法又饱和在 0，多余的注销会**吞掉后续其它流的减法**，让计数偏高。
唯一的绝对值校正路径 `sync_media_server_stream_count` 需要**有连接断开事件**
才会发生 —— 长期没有播放器的节点永远得不到校正。
`state_store.rs` 的选路直接按这个计数取最小。

**修法**：与 `on_flow_report` 路径**同一口径** —— 直接问 ZLM 要绝对流数
（`getMediaList` + `count_distinct_media_streams`，后者已按 B12 修好按
`(vhost, app, stream)` 去重），拿到就**覆盖**；问不到就**保持原值**，
绝不用猜的增量去覆盖。抽出 `apply_absolute_stream_count` 以便直接断言语义。

**测试**：`stream_count_tests` —— 重复 5 次 register 事件后计数**仍是 2**
（增量路径会变成 7）/ 问不到时保持原值 / 节点首次出现时正确落绝对值。
**变异验证**：把覆盖改成 `+= count` → 幂等用例变红。**提交**：`2e8e3f1`。

### B44 ✅ 第十一轮 M：`has_schema` 把所有错误吞成 `false`（P2，已修）

**现象**：ZLM 不可达（节点重启、网络抖动、secret 轮换导致 401）时，播放地址里
的 `hlsUrl` 被**静默摘掉**，前端降级到 flv/playUrl；而整条链路上**没有一行
错误日志**，现场无法区分「设备没推 hls」与「ZLM 挂了」。

**根因**：`zlm/client.rs` 的 `has_schema` 返回 `bool`，而 `Err(_) => false` 把
**所有**失败（ZLM 不可达 / 响应结构异常）都塌成「这一路流没有这个 schema 的源」。

**修法**：抽出纯函数 `media_list_has_schema(...) -> Result<bool, String>`
（`Ok(false)` = 确实没有；`Err` = **查不到**）；`has_schema` 改为返回
`Result<bool, String>`；新增 `has_schema_lossy` —— 按「没有」处理但**留下 warn**
（写明 app/stream/schema 与失败原因），5 个调用点改用它：行为不变，但 ZLM
不可达时日志里能看见。

**测试**：`media_list_has_schema_distinguishes_absent_from_unreadable`（正常响应
缺该源 → `Ok(false)`；结构异常 → 必须 `Err`）/ wiremock
`zlm_unreachable_is_reported_not_swallowed`（ZLM 500 时严格版必须 `Err`、
宽松版按「没有」处理且不 panic）。**变异验证**：把请求错误退回 `Ok(false)`
（旧行为）→ wiremock 用例变红。**提交**：`0801614`。

---

### B15 ✅ 设备统计查了不存在的表（已修，2026-09-19）

**现象**：`/api/device/query/statistics/keepalive` 与 `.../register` 恒返回
`online=0 / total=0`，即使库里有在线设备。

**根因**：`src/handlers/device.rs` 的 5 条统计 SQL 查了一张**库里不存在的表**（沿用了
早期外部实现的表名）—— 三方言的初始化脚本里都没有它（本项目真实表名是 `gb_device`）。
查询报错后被 `.unwrap_or(0)` 吞掉，于是恒为 0。

**修复**：SQL 表名改为 `gb_device`（`src/handlers/device.rs:154/161/183/190/197`）。
不改任何响应字段名。**注意**：`DATE(create_time) = CURRENT_DATE` 这一口径在三方言下
行为不同，如需严格一致应改用各方言的日期函数（暂无影响，未改）。

---

## C. 代码债与清理

### C1 🔵 `handle_packet` 23 个参数

`src/sip/server.rs::handle_packet` 参数已达 23 个（历史登记项）。应抽
`SipPacketContext` 结构体。
**风险**：纯重构，但触及所有 SIP 入口，建议单独一轮 + 全量测试。

### C2 🔵 29 个无引用的 `db::` 函数

2026-09-19 复核（@ `8ecec6a`）：用「标识符在 `src/` 全仓仅出现 1 次（即只有定义处）」判定，
`src/db/` 下 232 个 `pub (async) fn` 中有 **29 个零引用**。
**需要逐条判定"删除"还是"接上"**：

```
db/device.rs (7)          : batch_insert_channels, batch_update_channel_status, batch_upsert_channels,
                            count_alive_devices, count_channels, count_registered_devices,
                            delete_channels_by_device
db/alarm.rs (4)           : batch_delete_alarms, count_alarms, delete_alarm, list_alarms_paged
db/stream_proxy.rs (3)    : list_by_media_server, update_enable_status, update_pulling_status
db/platform_channel.rs (3): batch_delete_channels, get_by_platform_and_channel, list_by_platform_id
db/media_server.rs (3)    : add_white_list_cidr, mark_offline_if_expired, remove_white_list_cidr
db/jt1078.rs (3)          : count_online_terminals, get_auth_code_by_phone, update_auth_code
db/cloud_record.rs (3)    : delete_by_app_stream, get_collect_records, query_by_device_channel
db/user.rs (1)            : find_by_username_password
db/user_api_key.rs (1)    : delete_expired_keys
db/platform.rs (1)        : update_enable
```

初步分类：
* **删除候选（功能已由别的实现覆盖 / 本就无对应功能入口）**：
  `delete_by_app_stream`（已改为 `delete_by_app_stream_period`）、
  `add_white_list_cidr` / `remove_white_list_cidr`（本平台无白名单功能）、
  `update_enable`（平台启停走 `/api/platform/update`）、
  `platform::add`（新增平台走别的路径）、
  `stream_proxy::update_pulling_status`（已被 `update_pulling_status_by_app_stream` 取代）、
  `mark_offline_if_expired`（已被 `media_server::mark_offline_if_miss_count_exceeded`，
  健康检查按**连续丢失次数**判下线，`zlm/media_node.rs` 调用，取代）。
* **需要接上（可能是缺失的功能）**：无。
  （`delete_expired_keys` 已于 2026-10-20 接上后台清理任务，见 C4；
  `get_auth_code_by_phone` / `update_auth_code` 同日接上终端鉴权，见 C3。）
* **其余**（alarm/device/platform_channel 的批量与计数函数）：
  要么被"合并查询"取代（`count_channels` vs `count_all_channels`），
  要么是早期分层遗留 —— 逐条确认后删除。

**与上一版（36 个）的差异，以及本判定口径的两个坑**：

| 上一版列出的函数 | 现状 | 原因 |
|---|---|---|
| `db/role.rs::get_by_name` | 已**接上** | `handlers/role.rs:38` 现在会调它做重名校验（`bfa65f8` 引入） |
| `db/user.rs::update_user_role` / `update_username` | 已**删除** | 函数体已移除，只在注释里作为"零调用死函数"的教训被提及 |
| `db/user.rs::find_by_username_password` | 仍是零引用 | — |

> ⚠️ 口径坑 1：判定依据是「标识符零出现」。**不同模块下的同名函数会互相"救活"** ——
> 若 `db/a.rs` 与 `db/b.rs` 都有 `get_by_name`，即使两个都没被调用，计数也 ≥2 而漏报。
> 复核时务必按 `模块::函数` 精确 grep。
> ⚠️ 口径坑 2：若某函数通过 `db::module::*` 通配后以短名调用，同样会被误判为"零引用"。

### C3 ✅ JT1078 鉴权码：注册应答写死 `"GBServer"` + 0x0102 语义错（2026-10-20 已修）

核对结果（2026-09-13）：

* `src/jt1078/server.rs` 收到 0x0100 终端注册后回的是
  `command::build_register_response(&msg.phone, msg.serial, 0, "GBServer")`
  —— **硬编码字面量 `"GBServer"`**，而不是该终端在 `gb_jt_terminal.auth_code`
  里配置/入库的值；
* `db::jt1078::get_auth_code_by_phone` / `update_auth_code` **零引用**；
* `gb_jt_terminal.auth_code` 这一列因此是"只写不读"；
* `src/jt1078/session.rs` 把 **0x0102 当作「终端属性上报」**解析（字段是终端类型/
  厂商/型号/ICCID/版本），而 JT/T 808-2011 中 **0x0102 是「终端鉴权」**（正文为
  鉴权码），两者报文结构完全不同 —— **msg_id 语义需按标准核对**，
  并据此确认平台到底有没有做鉴权码校验。

**影响**：真终端若配置了厂商私有鉴权码，会收到"请用 GBServer"的应答，
后续鉴权（若终端校验）必然不一致。mock 终端不校验，所以一直没暴露。
**修复（2026-10-20）**：

1. **0x0102 的语义**：JT/T 808 里 0x0102 是**终端鉴权**（正文即鉴权码），
   不是"终端属性上报"。仓库自己的模拟器注释也写着「0x9101 = 平台下发的实时
   音视频传输请求（此前写 0x0102，那是终端鉴权）」，而 `mock/docs/jt1078-message-types.md`
   把它写成"实时音视频"是错的。原实现按属性上报解析（正文结构完全不同），
   鉴权码因此**一直收不到**，解析出的字段也无人使用（`AttributeReport` 解析后
   直接被丢弃）。现改为按鉴权解析，并回 **0x8102**（应答流水号 + 结果）。
2. **注册应答从 DB 取 `auth_code`**：0x8100 的正文本来就是「应答流水号 + 结果 +
   鉴权码」，代码也已支持传参，只是调用处写死了 `"GBServer"`。现优先用
   `gb_jt_terminal.auth_code`；没配置就**生成一个并入库**，保证"平台下发的"与
   "库里存的"是同一个值 —— 否则终端回送的 0x0102 永远校验不过。
   `db::jt1078::get_auth_code_by_phone` / `update_auth_code` 不再是零引用。
3. **强制 / 宽松两档**（`jt1078.auth_required`，默认 `false` 宽松）：
   * 宽松：终端不鉴权也能注册、能在线 —— 大量真实终端与本仓库模拟器都不回 0x0102，
     强制会让它们直接不可用；
   * 强制：终端必须先通过 0x0102 鉴权才置为在线（注册时只入库元数据）。
     也可用环境变量 `GBSERVER__JT1078__AUTH_REQUIRED=true` 打开。

**验证**：定向 Rust 单测 5/5（注册应答下发的是库里配置的鉴权码而不是 `"GBServer"`；
未配置时平台生成并落库、且下发值与库存值一致；0x0102 鉴权码对 → 0x8102 结果 0
并记为已鉴权、对不上 → 结果 1 且不记为已鉴权；强制档未鉴权不在线、鉴权后才在线；
宽松档不鉴权也能在线）+ 解析/构帧单测 6/6。
**未验证**：真终端（JT1078 真机）仍未接入，0x0102 的实际报文以真机为准。

### C4 ✅ API Key 过期记录不清理（2026-10-20 已修）

核对结果：**鉴权路径已经检查过期**（`src/auth.rs`：`enable == false`，或
`expired_at > 0 && now > expired_at`，都直接 401），所以这不是安全问题；
只是 `db::user_api_key::delete_expired_keys` 零引用，过期记录会**一直留在表里**
（长期运行堆积，列表页也会显示已过期条目）。

**修复**：接上后台清理任务 —— 进程启动 30 秒后跑第一次，之后**每小时**一次，
按 `expired_at < now` 删除过期记录并记录删掉的条数（`src/lib.rs`）。
选择"删除记录"而不是"列表里过滤"，是因为过滤只让界面干净、表里照样堆积；
`delete_expired_keys` 本来就在 `db` 层写好了（三方言），接上即可。

**验证**：定向 Rust 单测 2/2 —— 只删过期行，未过期与"永不过期"（`expired_at` 为
NULL）都保留；没有过期行时删 0 条且不报错。全量 `cargo test --no-fail-fast` 通过。

**副作用（有意为之）**：过期记录最多还会存在约 1 小时。期间列表里能看到它并标成
"已过期"，管理员能查到"是哪个对接方的哪把 Key 过期了"，而不是凭空消失。

### C5 ✅ 用户列表关键字搜索（2026-09-19 已修，commit `bfa65f8`）

原先 `UsersQuery` 只有 `page` / `count`，前端 `UserQueryParams` 却声明了
`query?: string` —— 输入的关键字被 serde 静默丢弃，页面看起来"搜了但没过滤"。

现已补齐：`UsersQuery` 增加 `query`，`db::get_users_paged` / `count_users`
接受同一条件走 `username LIKE`（三方言），前端列表加搜索框。
**关键点**：`count_users` 必须与列表用同一过滤条件，否则「共 N 条」与行数会对不上。

### C6 ✅ 用户管理权限缺口（2026-09-19 已修，commit `bfa65f8`）

**问题**：`require_admin` 只被 4 个端点调用，且实现是硬编码 `role_id == 1`；
其余 9 个管理端点只挂了 JWT 中间件。实测（`viewer` 走正常登录）：

| 请求 | 修复前 | 修复后 |
|---|---|---|
| `POST /api/role/add` `{"authority":"0"}` | **200 建成管理员级角色** | 403 |
| `POST /api/userApiKey/add` | **200 自造凭据** | 403 |
| `GET /api/user/users` | **200 拿到全部用户 + pushKey** | 403 |
| `GET /api/user/all` | 200 | 403 |
| `GET /api/role/all` | 200 | 403 |
| `DELETE /api/role/delete` | 200 | 403 |

**修复**：新增 `src/handlers/authz.rs` 作为唯一判定入口 —— `authority == "0"`
为管理员、内置角色 `id = 1` 兜底（原口径下 `role_id != 1` 的 authority="0" 角色
成员会被误判为非管理员）；拒绝返回 `ErrorCode::Error403`（原为 400）。
7 条 `/api/userApiKey/*` 与 `/api/role/*`、`/api/user/{users,all}` 全部接入。

**新增约定**：任何管理类端点都必须调用 `authz::require_admin`（已写入 `AGENTS.md`）。

### C7 ✅ 悬空 role_id 吞用户（2026-09-19 已修，commit `bfa65f8`）

**问题**：删角色不校验引用；`gb_user` 与 `gb_user_role` 之间没有外键约束，
于是用户 `role_id` 变成悬空值。而列表查询用的是 **INNER JOIN**，这些行被 SQL
直接丢掉，`count_users` 却照旧计入 —— 实测「DB 里 4 个用户、页面只有 3 行，
且翻页也找不回」。

**修复**：
1. `get_users_paged` / `get_all_users` 改 `LEFT JOIN`，角色已删除时显示
   `"(角色已删除)"` 占位；
2. `role_delete` 增加前置检查：内置管理员角色不可删、仍被用户引用不可删
   （返回「该角色仍有 N 个用户在使用」）。

**验证**：手工把某用户 `role_id` 改成不存在的 999 后，列表 `total` 与行数
仍一致（4/4），该用户正常显示。

### C8 🔵 通道 `hasAudio` 与实际流不符（真机实测）

**现象**（2026-09-19 真机）：点播返回 `"hasAudio": false`，但同一路流在 ZLM 里
的 track 明确带音频：

```
GET /api/play/start/34020000001320128497/34020000001310000001
→ {"code":0,"data":{...,"hasAudio":false,...}}

ZLM getMediaList → H264 1080x1920 fps 26.0 (ready) + PCMA (ready)
本地 FLV 实拉 6s 后 ffprobe → Stream #0: h264 1080x1920 / Stream #1: pcm_alaw
```

`gb_device_channel.has_audio` 该行为空/0，平台没有从**实际流**（ZLM track 或
PS 解封装结果）回填该字段，只沿用目录/入库时的值。

**影响**：前端音频图标、`hasAudio` 相关的默认行为对真机不准；不影响播放本身
（FLV 里音频照常下发）。
**下一步**：在 `on_stream_changed` / 点播成功后按 ZLM track 列表回填
`gb_device_channel.has_audio`（或至少在 play 响应里就本次流实时判定）。

**2026-10-17 代码进展**：`/api/play/start` 与 `/api/common/channel/play` 的本次
直播响应已查询 ZLM track；有 track 时 `hasAudio` 表示 ready 音频，查不到时
回退目录值，`audioSource` 标记 `media`/`catalog`。该值不会回填目录字段，
因为目录描述通道能力、本次流描述当前媒体。真机返回仍未复验，C8 保持开放。

**2026-10-20 复核（静态 + 自动化）**：ROADMAP M2 要求的"以真实流媒体信息和设备
目录字段的语义划清边界"在代码层已成立：

* 来源分离：`resolve_live_audio` 先问 ZLM 当前流的 track，问得到就用它并标
  `audioSource="media"`；问不到才回退目录字段并标 `audioSource="catalog"`。
  两个取值在 API 上是显式的，调用方不必猜 `hasAudio` 说的是"这条流"还是
  "这台设备"。
* "没说"与"说没有"可区分：目录里的 `<HasAudio>` 是**厂商扩展**（国标目录
  Item 里没有这个元素），设备不发时解析出 `None`，入库时按 NULL 写 ——
  与"设备明确说有音频/没音频"是两种状态，不会被压成 `false`。
* 自动化覆盖：`live_audio_tests` 断言 ready 音频 track 覆盖目录标志、
  无 track 返回 `None`、只有未就绪音轨返回 `Some(false)`。

**仍未验证**：真机返回。目录语义（设备到底在什么条件下发 `<HasAudio>`）需要
真机采样才能确认，这一项无法在 Mock 上定论。C8 保持 🟡（真机未验）。

### C9 ✅ 收藏删除 GET/DELETE 事务一致性已修并跨方言复验

前端 `web/src/api/cloudRecord.ts` 的 GET 调用对应
`src/handlers/cloud_record_extra.rs::collect_delete`；另有兼容的 DELETE handler。
2026-10-17 新增路由测试 `cloud_record_collect_delete_accepts_both_methods`：
两种方法在未登录状态都到达鉴权中间件并返回 401，旧记录中的“GET 必然 405”不成立。

**2026-10-18 进展**：登录后 HTTP 测试已覆盖添加、列表、GET 取消、重新添加、
DELETE JSON 数字 `id` 取消；两种删除都检查 `gb_record_collect` 与
`gb_cloud_record.collect`，添加也保持两者一致。SQLite 失败注入证明添加、删除
中途出错会回滚。云录像页面当前没有收藏按钮，页面验收不适用。

**2026-10-18 跨方言复验**：PostgreSQL 16、MySQL 8 的隔离库均完成登录后
添加、列表、GET 取消、重新添加、DELETE 数字 JSON `id` 取消；最后查询
`gb_cloud_record.collect=false` 且 `gb_record_collect` 为 0 行。临时库和容器已清理。

---

## D. 需真实硬件核验

### D1 🟡 GB28181 真实设备（2026-09-19 已接入第一台；主链路通过，剩余项待覆盖）

**已接入真机**：EasyGBD `34020000001320128497`（厂商 `easygbd`、型号=GB-ID、固件 V2.0），
TCP 接入 192.168.3.121:15060，平台在注册后把它切到 `TCP-PASSIVE`，
1 个通道（`34020000001310000001`）。真机流实测为 **H264 1080×1920@26fps + G.711A(PCMA)**。

**已核验通过**：

| 项 | 结果 | 证据 |
|---|---|---|
| 401 摘要鉴权注册 / 续期 | ✅ | `REGISTER … Challenge sent` → `Device registered: … (expires: 3600)`，全天多次续期 |
| 注销（expires=0）后重注册 | ✅ | `Device unregistered` → 25s 内再次 `Device registered` |
| 注册后自动 DeviceInfo 查询 | ✅ | `注册后自动 DeviceInfo 查询已下发` → 应答落库 name/manufacturer/model/firmware |
| Keepalive | ✅ | 每 30s 一条 `Keepalive from device` |
| 目录（Catalog）同步 | ✅ | `Catalog RESPONSE … 1 channels (SumNum=Some(1))`，1 包 |
| 平台→设备延迟探针 | ✅ | 每 15s 一条 MESSAGE，设备回 `200 OK - CallID: lat_…` |
| 传输模式切换下发 | ⚠️ 半通过 | 平台发出、设备回 `Result=OK`，但**平台未解析该应答**（B14） |
| 实时点播 + FLV 实拉 | ✅ | FLV **3,291,903 字节 / 6s**；`ffprobe` → h264 + pcm_alaw |
| 通道缩略图落盘 | ✅ | `data/snapshots/34020000001320128497_34020000001310000001.jpg`（98,327 字节） |
| 停止点播 | ✅（有缺陷） | ZLM 流与资源清空，但 BYE 发给了旧会话（B6） |

**仍未覆盖（保持 🔴 级别的未知风险）**：

* 401/407 摘要鉴权的**边界组合**（`qop`/`algorithm` 变体、407 代理鉴权）—— 只跑通了默认组合；
* **目录分页**：本机 `SumNum=1` 单包，`SumNum` 不实、分包乱序、长时间分页未测；
* `TCP-PASSIVE` 下 `connectRtpServer` 的行为 —— 已复现失败（B1），但**未确认**该设备类
  在"不按 INVITE 端口推流"时平台能否出流；
* `ConfigDownload` 各 `ConfigType` 的真实字段差异（当前只在 mock 上验证了
  BasicParam / SnapConfig）；
* 设备控制类：PTZ / 预置位 / 布防 / 录像 / 重启 / 配置查询 —— 真机**一次都没下发过**；
* 回放（Playback）与下载（Download）—— 真机未测；
* 对讲 / 语音广播 —— 真机未测（且真机流是**纯视频+音频推流**，对讲需另验）；
* 设备主动发 BYE、心跳超时下线、注册续期边界；
* WebRTC：GB28181 流已由用户在当前部署环境验证可正常播放；不同设备和网络部署仍建议在交付验收时覆盖。

> 复现环境（本机 macOS）：后端 SQLite 单实例 :18080 + `docker compose` 的
> postgres/redis/zlm（ZLM 用 mac 叠加文件桥接）、真机在 192.168.3.121。
> 详细命令见 §G。

### D2 🔴 JT1078 真实终端

* 0x0802 多媒体检索的 **JT/T 808-2011 与 2019 变体**（字段偏移不同）；
* 0x8100 注册鉴权码的真机核对（平台侧已按标准实现，见 C3；真终端的 0x0102
  报文仍需实测确认）；
* 0x8201 位置查询的应答字段差异（海拔/方向/报警标志）；
* 双向对讲（0x8300 文本下发 / 0x8204 电话回拨）需要真终端配合。

### D3 🔴 对讲音频互通

已验证到"假设备收到 50 个 RTP 包、`y=` 与 SSRC 一致、0 丢序"。真机上还需验证：
G.711A 编解码互通、时间戳/抖动、回声与半双工行为、以及长时间通话的内存占用。

---

## E. 环境与已接受的"预期为真"项

### E1 ⚪ `scripts/dialect_smoke.py` 的 2 项

| 项 | 说明 |
|---|---|
| `GET /log/list?format=csv` 判为 HTML | 该端点**就是**返回 CSV 文本（前端按文本下载），脚本的判据是"必须是 JSON" —— 属脚本预期，不是缺陷 |
| `proxy/start` 返回 500 | 用的是必然失败的 dummy RTSP URL（`rtsp://127.0.0.1:554/nonexistent`），ZLM `DESCRIBE:404` 是**正确行为** |

### E2 ⚪ 本 ZLM 版本缺 7 个 hook 事件键（已如实告警）

启动时健康检查会回读验证 hook 配置，本机 ZLM（`zlmediakit/zlmediakit:master`）
的 `config.ini` 里**没有**这些键，因此相关回调**永远不会到达**：

```
hook.admin_params, hook.on_stream_started, hook.on_rtp_server_started,
hook.on_record_hls, hook.on_rtp_playlist, hook.on_record_progress,
hook.on_send_rtp_progress
```

平台侧代码已**明确列出**不支持的键（不是静默当成成功）。
若将来要支持 `on_record_progress` / `on_rtp_playlist` 这类功能，
需要升级 ZLM 或改用其它事件（例如用 `on_record_mp4` 的时长字段替代录制进度）。

### E3 ⚪ 长驻服务启动方式与**启动顺序**（避免误判）

1. **单独启动**：后端 / SIP mock 等长驻进程必须独立启动（独立
   `run_in_background` job，或 `nohup … & disown`）。把它们和前台任务写进
   同一条命令时，一旦该任务被中止，信号会连带杀掉它们 —— 表现为
   "进程莫名消失、设备仍显示在线"（见 B4）。
2. **先起后端、再起设备（顺序很重要）**：设备的内存注册表在后端进程里，
   mock 若在后端启动**之前**完成 REGISTER，这条注册就丢了；此时
   `/api/device/query/devices` 因为读 DB 仍显示 `onLine=true`，而
   `/api/front-end/*`、`/api/device/control/*` 会因为
   `device_manager().get()` 查不到而回 `Device not online`。
   本轮 e2e 的 2 个 frontEnd 失败就是踩了这个（不是代码缺陷）。
   **判据**：接口回 `Device not online` 但设备列表显示在线 → 重启 mock。

### E4 ⚪ CI 自动触发保持关闭

按用户要求（2026-09-12），`.github/workflows/ci.yml` 只保留 `workflow_dispatch`；
`fmt` / `clippy` 为非门禁。**恢复自动触发前不要改这里。**

### E6 ⚪ 后端访问 `192.168.3.88:8080` 偶发 `error sending request`（本机环境）

2026-09-19 真机联调时观测到一次（`handlers/play.rs`）：

```
查询已存在流的 RTP 信息失败 34020000001320128497_…:
  error sending request for url (http://192.168.3.88:8080/index/api/getRtpInfo?…)
```

`192.168.3.88` 是本机**同时**作为 ZLM 宿主 IP 与公司 HTTP 代理宿主 IP 的地址
（代理 `192.168.3.88:7892`）。这类失败只在**裸跑后端**时出现，且重试即恢复，
不是平台缺陷 —— 与 `AGENTS.md` 记录的 `NO_PROXY` 要求同源。
**做法**：本机裸跑后端时设 `NO_PROXY=localhost,127.0.0.1,::1,192.168.3.88`
（或干脆让后端也只走 127.0.0.1 访问 ZLM）。

### E7 🟡 2.0 Docker 媒体与目标部署待验收

主 Compose、SQLite Compose 和 mac 叠加配置已要求设备可达的 SIP 通告 IP，
容器绑定 `0.0.0.0`，同时发布 SIP UDP/TCP 5060，并让浏览器媒体 URL 与桥接
WebRTC 候选使用可达地址。配置展开、Rust 测试和本机进程监听均已验证。
2026-10-18 发现旧的 `GBSERVER__ZLM__SERVERS__0__*` 环境变量会把数组解析成 map，
导致容器启动失败；三套 Compose 已改用首节点标量覆盖。重建的 PostgreSQL 镜像
在隔离库下用新变量启动成功，健康检查 healthy，数据库媒体节点保存了预期的
IP、HTTP 端口与 RTC 地址。SQLite Compose 已对齐 ZLM 配置文件与端口，
但尚未做该拓扑的实际媒体验收。
2026-10-18 在本机代理下 `docker compose build gbserver` 已构建 arm64 镜像。
用隔离 PostgreSQL 库、Redis DB 15 和禁用 ZLM 节点的临时配置启动容器：
健康检查为 healthy，容器内 SIP UDP/TCP 监听 `0.0.0.0:5060`，同源登录页、
登录、版本接口及 OpenAPI 正常；Chromium 冒烟 20/20 通过。
随后以独立 PostgreSQL 库重建后端镜像，发布 SIP UDP/TCP 5060，接入开发用真实
ZLM 与 SIP Mock。发现 PostgreSQL 设备查询的 `COUNT(*)` 类型不匹配及 Catalog
单条/批量写入的约束错误，修正并用隔离库测试后，容器点播/有效 BYE/停止后不重连、
MP4 下载及 Range 读取、Chromium FLV 实拉均通过。此次是 Docker Desktop 本机拓扑；
容器 WebRTC、真机 LAN 可达性和目标 Linux Compose 部署仍待验收。随后补齐
MySQL/SQLite 以及 PostgreSQL 的批量通道插入必填字段和可选通道类型默认值，
分别用三种数据库的生产初始化 schema 运行单条、批量更新和批量插入定向测试；
数据库路径已验证，MySQL/SQLite 的完整容器媒体拓扑仍未运行。

---

## F. 本轮改动清单（随本文档一起提交）

### F.0 2026-10-25 第十一轮深度代码审查（本轮）

> 本轮分两批：第一批 7 条（B32~B38），第二批把 B39~B44 六条**也全部修完** ——
> 至此第十一轮的 13 条发现**全部落地**。

本轮针对**多方言运行期改造的爆炸半径**与**十轮以来几乎没被正面覆盖的协议层**
做了一次深度审查，并派两个只读侦察 Agent 并行覆盖 `zlm/`+`scheduler/`+`cascade/`
与 `sip/`+`jt1078/`。共 **13 条**发现：**7 条已修并逐条变异验证**（B32~B38），
**6 条已定位未修**（B39~B44）。

| 文件 | 内容 | 状态 |
|---|---|---|
| `src/sip/server.rs`、`src/sip/gb28181/device.rs` | **B32（P0）**：Keepalive（MESSAGE，**无鉴权**）不再改写信令地址；地址只由带摘要鉴权的 REGISTER 写入；新增来源 IP 校验闸门 `keepalive_source_allowed`；`DeviceManager::update_keepalive` 不再接收来源地址 | ✅ 本轮 `0616826` |
| `src/db/device.rs` | **B32**：新增「只刷在线状态时地址原样保留」的库层回归 | ✅ 本轮 `0616826` |
| `src/jt1078/session.rs` | **B33（P0）**：会话接收缓冲封顶 256KB（未认证对端不能把内存撑爆）；**B38**：缺失序号统计改用回绕距离判定，不再构造 65533 条 | ✅ 本轮 `a34f223` / `3ec77a6` |
| `src/jt1078/command.rs`、`src/jt1078/frame.rs` | **B34（P1）**：JT808 校验码改按**未转义**内容计算（JT/T 808-2013 §5.3），与平台自己的解析端口径对齐；新增奇数个转义字节的回归 | ✅ 本轮 `cb3129a` |
| `src/sip/transport/tcp.rs` | **B35（P1）**：TCP 分帧抽成自由函数 `take_sip_message`，**只在头部做文本化**（正文可能是 GB2312）；三道缓冲/长度上限 | ✅ 本轮 `70939b5` |
| `src/cascade/register.rs`、`src/sip/server.rs` | **B36（P1）**：`RegistrationState` 新增 `last_liveness`，把「注册时间」与「存活时间」拆开；新增 `note_liveness_from_response` 并**补上生产接线**（级联 keepalive 的 200 OK） | ✅ 本轮 `e4f7377` |
| `src/db/dialect.rs`、`src/dyn_where.rs`、`src/db/mod.rs`、`src/db/{device,stream_proxy,stream_push,cloud_record,audit_log}.rs`、`src/handlers/{alarm,stub}.rs` | **B37（P1）**：删除全部「读进程级全局方言」的入口（7 个函数），`DynWhere::sql` 的方言改成**必填参数**，新增 `Pool::bigint_col` / `Pool::days_ago_expr` | ✅ 本轮 `14292cf` |
| `src/dialect_guard.rs` | 守门人收紧：`src/dyn_where.rs` 从「进程级全局方言」守门人的豁免名单里移除 | ✅ 本轮 `14292cf` |
| `tests/postgres_dialect_judgment.rs`、`tests/mysql_dialect_judgment.rs` | **B37** 的真实库回归（sqlite 下 `datetime()` 合法，单测全绿证明不了任何事） | ✅ 本轮 `14292cf` |
| `docs/OPEN_ISSUES.md`、`docs/STATUS.md` | 新增 **B32~B44** 共 13 条；总览表补 13 行；本节改动清单 | ✅ 本轮 |
| `src/sip/gb28181/invite_txn.rs`（新增）、`src/sip/server.rs` | **B39（P1）**：入站 INVITE 按 RFC 3261 §17.2.1 做重传去重（Call-ID+Via+CSeq 判据，**不用 From** —— 它可伪造），命中即原样重发同一份 200 OK；覆盖旧会话前回收其 ZLM 收流端口；接上此前无调用点的 `InviteSessionManager::cleanup_expired` | ✅ 本轮 `6837326` |
| `src/cascade/register.rs`、`src/sip/server.rs` | **B40（P2）**：级联注册改用服务端在 200 OK 授予的 `Expires`（Contact 参数优先）驱动刷新节奏；`needs_register_refresh` 抽成无锁纯函数 | ✅ 本轮 `e49a8b5` |
| `src/cascade/register.rs`、`src/handlers/platform.rs` | **B41（P2）**：级联端口入库前校验 1..=65535、发包处不再 `as u16` 回绕（越界→跳过该平台并告警） | ✅ 本轮 `9462bba` |
| `src/scheduler/record_plan.rs` | **B42（P2）**：流没拉起来时**真的不调用** `startRecord`；同一 tick 的通道改并发（`for_each_concurrent(8)`）；`MissedTickBehavior::Skip` 杜绝 Burst 紧循环 | ✅ 本轮 `c855c83` |
| `src/zlm/hook.rs` | **B43（P2）**：节点流计数改用 ZLM 的**绝对**流数覆盖（与 `on_flow_report` 同一口径），问不到则保持原值 | ✅ 本轮 `2e8e3f1` |
| `src/zlm/client.rs`、`src/handlers/{play,stream,common_channel,playback}.rs` | **B44（P2）**：`has_schema` 区分「确实没有该源」与「查不到」；新增 `has_schema_lossy` 留下 warn，5 个调用点改用它 | ✅ 本轮 `0801614` |

**本轮门禁数字**（均为只读判据复跑，可独立复现）：

| 门禁 | 结果 |
|---|---|
| `cargo test --lib` | **871 通过 / 0 失败** |
| 真实库专项（postgres 5 + mysql 3 + 方言判据 2） | **11 通过 / 0 失败**（PG 16 / MySQL 8 容器，隔离库 `gbserver_pg_test` / `gbserver_mysql_test`） |
| `cargo fmt --check` | **exit 0**（0 违规） |
| `cargo clippy --all-targets --all-features` | **exit 0**（0 error / 0 warning） |
| `cargo build --release` | **exit 0**（仅第三方 crate 的 future-incompat 提示：redis 0.25.4 / sqlx-postgres 0.7.4，本仓库零告警） |
| 变异验证 | 每条修复都**先让测试变红再还原**：JT808 奇数转义、JT1078 缓冲封顶、缺包回绕、TCP 非 UTF-8 正文、Keepalive 闸门、库层地址保全、cascade 存活语义与接线、方言判据（PG 报 `42883 function datetime(unknown, unknown) does not exist`） |

**本轮未验证 / 明确不声称已验证**：

* 真机 GB28181 / JT1078 终端在修复后的行为（尤其 B32 的来源 IP 闸门对
  **NAT 换源端口**的容忍度）—— **未验**，需真机复验；
* B32 的修复会让**从未 REGISTER 过**的设备的 Keepalive 一律被拒（查不到已登记
  地址 ⇒ 无法确认身份）—— 这是**有意为之**的取舍，但真机侧是否有设备依赖
  「不注册只发心跳」的非常规流程，**未验**；
* 目标部署环境（Linux Compose）的实跑**未验**。

### F.0.1 2026-10-25 多方言运行期改造（上一轮，保留备查）

| 文件 | 内容 | 状态 |
|---|---|---|
| `Cargo.toml` | `default = ["sqlite", "mysql", "postgres"]` —— 三个驱动**同时**编进一个二进制（少开直接 `compile_error!`） | ✅ 本轮 |
| `src/db/dialect.rs` | 新增：`Dialect` 枚举、URL scheme 推断、`dialect_sql`/`current_sql`/`dialect_sql_for`/`bigint_col`/`days_ago_expr` | ✅ 本轮 |
| `src/db/mod.rs` | `Pool` 改运行期多态枚举（`Sqlite`/`Postgres`/`MySql`）+ 四个分发宏（`dispatch!` / `dispatch_exec!` / `dispatch_dialect!` / `dispatch_exec_dialect!`）+ `ExecResult` | ✅ 本轮 |
| `src/lib.rs` | schema 初始化改运行期 `schema_sql(dialect)`；`src/dyn_where.rs` 委托给 `db/dialect.rs` | ✅ 本轮 |
| `src/db/**`、`src/handlers/**`、`src/sip/**`、`src/zlm/**`、`src/scheduler/**` | 全部 DB 调用点由 `#[cfg(feature=...)]` 编译期分支改**运行期方言分发**（cfg 方言块 28 → 0；`dispatch!` 宏体内裸 `?` 411 → 0） | ✅ 本轮 |
| `src/dialect_guard.rs` | 既有守门人测试扩展为识别 `dialect_sql_for(` / `current_sql(` / `.sql(` / `&format!(`，作为本轮验收 oracle | ✅ 本轮 |
| `scripts/dialect-migrate/` | 转换工具链入库（**33 个脚本** = 31 个 Python 转换/体检 + 2 个 shell 门禁/流水线）：`cfg_runtime.py` 主转换器、`fix_raw_placeholders.py`、`audit_symbols.py` 符号体检、`guard.sh` 膨胀双阈值、`run.sh` 流水线（**第 7 步新增 `cargo fmt --check` 格式门禁**，由 `eda04f6` 补） | ✅ 本轮 |
| `scripts/dialect_smoke_all.sh` | **一个二进制起三个实例**，只改 `GBSERVER__DATABASE__URL` | ✅ 本轮 |
| `docs/DB_RUNTIME_DIALECT_PLAN.md` | 新增：选型证据（为何否决 `Any` 驱动）、四个宏的语义边界、事务方案、验收要求、工具链说明 | ✅ 本轮 |
| `docs/DB_DIALECT_NOTES.md` | 补运行期方言的权威差异清单（占位符统一 `?`、id 列按 i32、CAST 类型名、布尔绑定） | ✅ 本轮 |
| `docs/STATUS.md` | 顶部增量表新增「多方言运行期改造」行；§3 关键设计决策补运行期方言条目；§7 相关文档补方案文档 | ✅ 本轮 |
| `docs/OPEN_ISSUES.md` | 新增 **B30**（本轮仍未验证事项 + 复现条件）；G.2 方言冒烟命令与「陈旧二进制」注意事项按新形态改写；变更记录补本轮 | ✅ 本轮 |
| `src/db/**`、`src/handlers/**`、`src/sip/**`、`src/zlm/**`、`src/lib.rs`（**第二轮**） | 方言判据从**进程级全局** `dialect::current()` 统一改为 **pool 自己的类型**（`pool.dialect()` / `pool.sql(...)`）—— **505 处**。修掉 2 个只在 PG 上炸的真实缺陷（`channel_count` 的 INT8、`batch_insert_channels` 的裸 `?`） | ✅ 本轮 |
| `src/dialect_guard.rs` | 新增第二条守门人 `dialect_branch_must_use_pool_dialect_not_process_global`（非测试代码里用全局选方言分支 → 变红；**变异验证过**） | ✅ 本轮 |
| `scripts/dialect-migrate/fix_pool_dialect.py` | 新增：把进程级全局判据改写成 pool 判据的转换器（**编译器当 oracle** 定点修 pool 变量名） | ✅ 本轮 |
| `.github/workflows/ci.yml` | `feature-matrix`（三条 leg 用已失效的单方言构建）→ 换成 `dialect` job：三驱动同编 + `dialect_guard` 守门人 + **PostgreSQL / MySQL service container 上的真实库专项用例** | ✅ 本轮 |
| `justfile` | `pg` / `mysql` 改成「改 `database.url` 不改编译」；`feature-check` → `dialect-check`；`clippy` 用 `--all-features` 并更正那条已作废的注释；新增 `test-realdb` | ✅ 本轮 |
| `Dockerfile`、`docker-compose.yml`、`docker-compose.sqlite.yml` | 去掉 `GBSERVER_DB_FEATURE` 建参 —— **一个镜像服务三种方言**（此前按单个方言构建在改造后必然失败） | ✅ 本轮 |
| `CLAUDE.md`、`AGENTS.md`、`database/README.md`、`config/application.toml`、`Cargo.toml`、`scripts/dialect_smoke.py`、`mock/TEST_PLAN.md`、`mock/scripts/run-all-tests.sh`、`docs/DB_DIALECT_NOTES.md`、`docs/DEPLOYMENT_GUIDE.md` | 清理「按方言分别编译」的旧说法与命令（其中 `mock/scripts/run-all-tests.sh` 与 Dockerfile 是**可执行**的，照旧会失败） | ✅ 本轮 |

### F.1 2026-09-19 真机核验轮（历史轮次，保留备查）

| 文件 | 内容 | 状态 |
|---|---|---|
| `docs/STATUS.md` | 规模基线全量重测（@ `8ecec6a`）；新增 §4「真实设备接入核验」；能力矩阵加「真机」列；§6 验证证据补真机链路 | ✅ 本轮 |
| `docs/OPEN_ISSUES.md` | D1 改 🟡 并拆「已核验 / 仍未覆盖」；B1、B6 补真机证据；C2 由 36 重算为 29 并列出差异；新增 B14 / C8 / C9 / E6；F 段由 2026-09-13 的改动清单换成本轮清单 | ✅ 本轮 |
| `docs/STUB_COMPAT_PLAN.md` | 行数/entry/路由数按实测刷新（3843/46、1273/18、64 条）；活跃路由 49→48；无调用方清单更正（移除 `/api/group/delete`、补 `/api/device/query/channel/audio`） | ✅ 本轮 |
| `docs/DEPLOYMENT_GUIDE.md` | 修正 SIP TCP 端口（5061→5060）、ZLM 端口与 config.ini 对齐、`vue.config.js`→`vite.config.ts`、删除已不存在的 `cache.rs`/`cascade_service.rs`、指标名与健康检查间隔按 `src/metrics.rs` / `config` 实测重写、`/api/server/system/info` 补入端点表 | ✅ 本轮 |
| `docs/DB_DIALECT_NOTES.md` | 复核四条规则与 `statement_cache_capacity(0)`、`dialect_smoke.py` 路径仍然成立；补「真机联调不涉及方言」的范围说明 | ✅ 本轮 |

**本轮复验结果（2026-09-19 @ `8ecec6a`）**：`cargo test --no-fail-fast`
**746 通过 / 0 失败 / 3 忽略**（15 个测试二进制）；
真机点播 FLV 实拉 **3,291,903 字节 / 6s**（H264 1080×1920 + pcm_alaw）后正常停止；
e2e 最近一次运行 35 通过 / 0 失败。**三方言冒烟与 e2e 全量本轮未重跑。**

---

## G. 复现环境

### G.1 真机联调（2026-09-19 本轮，本机 macOS）

```bash
# 依赖服务：ZLM 用 mac 叠加文件（Docker Desktop 不支持 host 网络）
docker compose -f docker-compose.yml -f docker-compose.mac.yml up -d

# 后端：SQLite 单实例（默认配置即 data/gbserver.db），裸跑需绕开本机 HTTP 代理
cd /Users/letmlook/code/GBServer
NO_PROXY=localhost,127.0.0.1,::1,192.168.3.88 no_proxy=localhost,127.0.0.1,::1,192.168.3.88 \
  RUST_LOG=info,gbserver=debug ./target/debug/gbserver

# 真机接入（设备侧配置）：SIP 服务器 <本机 IP>:5060、传输 TCP、
#   GB-ID 34020000001320128497、密码与 sip.password 一致（默认 admin123）

# 登录取 token（注意：登录参数走 Query，不是 JSON body）
TOKEN=$(curl -s --noproxy '*' \
  'http://127.0.0.1:18080/api/user/login?username=admin&password=admin' \
  | python3 -c 'import json,sys;print(json.load(sys.stdin)["data"]["accessToken"])')

# 在线状态 / 点播 / FLV 实拉 / 停止
curl -s --noproxy '*' 'http://127.0.0.1:18080/api/device/query/devices?page=1&count=20' \
  -H "access-token: $TOKEN"
curl -s --noproxy '*' \
  'http://127.0.0.1:18080/api/play/start/34020000001320128497/34020000001310000001' \
  -H "access-token: $TOKEN"
curl -s --noproxy '*' --max-time 6 -o /tmp/real.flv \
  'http://127.0.0.1:8080/rtp/34020000001320128497_34020000001310000001.live.flv'
ffprobe -v error -show_entries stream=codec_name,width,height -of default=nw=1 /tmp/real.flv
curl -s --noproxy '*' \
  'http://127.0.0.1:18080/api/play/stop/34020000001320128497/34020000001310000001' \
  -H "access-token: $TOKEN"

# 平台侧日志：落库在 gb_log，按需查询比翻 stdout 更可靠
sqlite3 -line data/gbserver.db \
  "SELECT time,logger,message FROM gb_log WHERE message LIKE '%34020000001320128497%' ORDER BY id DESC LIMIT 40;"
```

> 真机排错要点：`/api/play/start` 返回 `code=0` 只代表 SIP INVITE 成功；
> **是否真的出流**要看 `on_publish` / `on_stream_changed register=true` /
> `Media ready` 三条日志，以及 FLV 实际字节数。

### G.2 模拟设备 / 方言 / e2e（历史基线，2026-09-13）

```bash
# sqlite 后端（18080，抓退出码）
cd /Users/letmlook/code/GBServer
(env GBSERVER__DATABASE__URL="sqlite:///tmp/gbe2e/app.db?mode=rwc" \
     GBSERVER__REDIS__URL="redis://127.0.0.1:6379" \
     GBSERVER__JWT__SECRET="test-secret-test-secret-test-secret-1234" \
     GBSERVER__SERVER__PORT=18080 RUST_LOG=info,gbserver=debug \
     ./target/debug/gbserver > /tmp/gbe2e/backend_sqlite.log 2>&1; \
 echo "BACKEND EXIT CODE=$?" >> /tmp/gbe2e/backend_sqlite.log) &

# SIP 假设备（带真实推流 + 报告文件）
env SIP_MOCK_BYE_REPORT=/tmp/gbe2e/bye_report.json \
    SIP_MOCK_TALK_REPORT=/tmp/gbe2e/talk_report.json \
python3 mock/tools/sip-device/sip_device_mock.py --server 127.0.0.1:5060 \
  --device-id 34020000001320000001 --username 34020000001320000001 \
  --password admin123 --auto-bye-secs 0 --auto-alarm-secs 5 --send-rtp &

# 方言冒烟（**一个二进制起三个实例**，只改 database.url）
# 2026-10-25 起不再需要「各自 feature 构建 + 独立 target 目录」：
# 三个驱动已同时编进同一个二进制（default = sqlite + mysql + postgres）。
bash scripts/dialect_smoke_all.sh

# 单方言手动起（登录参数走 Query，不是 JSON body）
python3 scripts/dialect_smoke.py --base http://127.0.0.1:18080/api --token "$TOKEN"

# UI e2e（需 18080 后端 + 9528 前端 dev）
cd e2e && npx playwright test
```

**环境注意事项**（踩过的坑，写下来避免重复排查）：
* ~~`target/debug/gbserver` 是**最后一次构建的 feature**；用 pg/mysql 构建时务必加
  `CARGO_TARGET_DIR=/tmp/gbt-pg`（或 `-my`），否则会覆盖 sqlite 二进制（"陈旧二进制陷阱"）。~~
  —— **2026-10-25 起失效**：三个驱动已同时编进同一个二进制
  （`default = ["sqlite", "mysql", "postgres"]`），`target/debug/gbserver` 换库只需改
  `GBSERVER__DATABASE__URL`，**不再有「陈旧二进制 / 覆盖 sqlite 二进制」问题**。
  仍需注意：改完 `database.url` 后**必须重启进程** —— 方言在**启动时**确定，
  不是每次查询现读配置；只改环境变量不重启，跑的仍是旧方言。
* mock 崩溃后**平台侧仍显示设备在线**（keepalive 未超时）—— 排查"收不到流"时
  先确认 `ps aux | grep sip_device_mock`。
* `gb_cloud_record` 里的行会被 `cloudRecord` 的"删除"用例清掉；排查录像落库时
  先跑手动链路再看表，不要只看 spec 结论。
* 本机 `192.168.3.88` 既是 ZLM 宿主 IP 又是 HTTP 代理宿主 IP → 裸跑后端要对它设
  `NO_PROXY`（见 E6）。
* e2e 需要前端 dev server（:9528）；只起后端跑 e2e 会大面积失败，不等于代码缺陷。

---

## 变更记录

| 日期 | 变更 |
|---|---|
| 2026-09-13 | 建立本文档：登记 A1–A3、B1–B5、C1–C4、D1–D3、E1–E4 |
| 2026-09-13 | 关闭 B10（fps 浮点）、B11（on_server_started 覆盖对外 http 端口）；ZLM `[http] port` 固定 8080 |
| 2026-09-13 | 关闭 B9（rtc_extern_ip 三处自动下发）；新增 E5（ZLM host 网络 / 只读 config.ini / mac 叠加文件；记录 ZLM 会重写 config.ini 的坑） |
| 2026-09-13 | 新增 B7/B8/B9 与 A4（WebRTC：接口调用形态已修+真实浏览器验证；国标流解不出帧；rtc.externIP 未下发；前端无入口） |
| 2026-09-19 | 用户确认当前部署中 GB28181 流可通过 WebRTC 正常播放，关闭 B8；保留历史记录以便后续出现设备或网络兼容性差异时追溯。 |
| 2026-09-13 | 新增 B6（`send_session_bye` 跨类型误停，实测证据 + 修复方向）；B5 保持 🔵 |
| 2026-09-13 | 复验并关闭 B2（录像计划 `startRecord` 竞态）、B3（mock `connection_lost` 崩溃）、B4（定性为同组 job 被杀的副作用）；新增 E2（ZLM 缺 7 个 hook 键）、E3（长驻服务启动方式与顺序）；C3 升级为 🟠（鉴权码只存不用 + 注册应答写死 `"GBServer"` + 0x0102 语义存疑） |
| 2026-09-19 | 冻结轮：登记用户管理三项（C5/C6/C7）与 A4；B12/B13 修复并复验 |
| 2026-09-19 | 解冻并修复用户管理（`bfa65f8`）；C5/C6/C7 关闭 |
| 2026-09-19 | **真实设备接入核验轮**：D1 🔴→🟡（真机 EasyGBD 已接入，主链路核验通过，剩余项列明）；B1 补真机复现（并更正影响评估：该设备仍按 INVITE 端口推流，流不受影响）；B6 补真机复现（stop 把 BYE 发给已终止的旧会话）；C2 由 36 重算为 29（`get_by_name` 已接上、`update_user_role`/`update_username` 已删除）；新增 B14（应答未解析）、C8（`hasAudio` 与实际流不符）、C9（收藏删除方法不匹配）、E6（本机代理导致的 ZLM 请求偶发失败）；F/G 段更新为真机联调口径 |
| 2026-10-25 | **第九轮代码审查**：新增 B25（PostgreSQL 方言回归 + TOCTOU + 方言守卫）、B26（7 处写端点假成功 + 已补回归测试）、B27（JT1078 号码 BCD 方向与国标相反）、B28（日志保留死配置 + 日志端点缺管理员门禁 + 弱密钥回显）、B29（冒烟脚本残留 + 本轮 P2/P3 清单 + 已排除怀疑项 + e2e 首跑失败的环境归因）；全部已修并复验（5 组变异双向验证） |
| 2026-10-25 | **多方言运行期改造**：数据库后端从「编译期互斥三选一」改为「**三驱动同编一个二进制 + 运行期按 `database.url` 选库**」（`default = ["sqlite","mysql","postgres"]`）。否决 sqlx `Any` 驱动（PG `BOOLEAN` / MySQL `TINYINT` / PG `REAL` / SQLite 布尔列四类类型覆盖硬缺口，失败在类型映射层）；改为各方言原生 pool + 运行期枚举分发。修掉三类「编译能过、运行才炸」：cfg 方言块不再互斥（28 → 0）、`dispatch!` 宏体内裸 `?`（411 → 0，用 `dialect_guard` 守门人当 oracle）、三方言占位符数不同（MySQL 24 vs 其余 13）导致 PG `Malformed communication packet`。新增 **B30**（本轮仍未验证：9 条 `#[ignore]` 真实库用例、release 构建、真机、目标部署环境、流水线整条重跑的可复现性）。G.2 的「各自 feature 构建 + 独立 target 目录」与「陈旧二进制陷阱」两条**已失效**，按新形态改写 |
| 2026-10-25 | **更正上一条台账里不可复现的结论 + 堵住复发路径**：`9c7018f` 写了「`cargo fmt` 干净」，实测 `cargo fmt --check` 报 **2 处 diff**（`src/db/common_channel.rs` / `src/db/device.rs`）→ **该结论作废**，`4ffb5ea` 提交正文里同一句「`cargo fmt`：干净」一并作废。根因：转换器把三个方言分支合并成**一条超长单行 `.bind(...)` 链**（基线是三条短链）超出 rustfmt 行宽，而 `scripts/dialect-migrate/run.sh` 的最后一步**只打印**「接下来：cargo fmt」而**不执行** → 流水线每次都交出带格式违规的树。已由 `eda04f6` 修掉这 2 处，并把 `cargo fmt --check` 变成流水线**第 7 步门禁**（不合规即 exit 1），后续同一入口还会跑 `cargo check` + `cargo clippy`。**教训**：门禁数字必须**能被独立复现** —— 写「fmt 干净」用的判据必须是 `cargo fmt --check`（只读），而不是 `cargo fmt`（会重写文件、看不出"当时到底脏不脏"） |
| 2026-10-25 | **多方言改造的连锁修复**：补跑 9 条 `#[ignore]` 真实库专项用例（真实 PG 16 / MySQL 8）→ **当场抓出 2 个真实缺陷**（`get_device_by_device_id` 的 `channel_count` 缺 PG 专有 `::INTEGER` → INT8 vs `Option<i32>`；`batch_insert_channels` 走了全局的占位符改写 → PG `syntax error at end of input`）。根因：改造前方言分支是**编译期** `#[cfg(feature=...)]`（永远正确），改成运行期后 505 处用了**进程级全局** `dialect::current()` 而非 **pool 自己的类型** —— 生产单库时等价，测试里直接构造 `Pool::Postgres(..)` 就走错分支。已全仓改为 `pool.dialect()` / `pool.sql(...)`（转换器 `fix_pool_dialect.py` + 编译器当 oracle），并新增第二条守门人 `dialect_branch_must_use_pool_dialect_not_process_global`（变异验证过：注入 1 处 → 精确报行号并变红）。**同时发现 CI 与容器构建都是坏的**：改造让 `--no-default-features --features <单个方言>` 必然编译失败，而 CI 的 `feature-matrix`、`justfile` 的 6 个 recipe、**Dockerfile 的 `GBSERVER_DB_FEATURE` 建参**用的正是这条命令 → CI 每次推送全红、镜像构建必失败。已重写 CI（`dialect` job：三驱动同编 + 守门人 + service container 上的真实库用例）、`justfile`、`Dockerfile`、两个 compose，并清理 10 个文件里的旧模型说法。详见 **B31** |
