# GBServer 遗留问题与进度（Open Issues）

> 生成时间：2026-09-13（本地 08:3x）
> **最近更新：2026-09-19（真实设备接入核验轮，HEAD `8ecec6a`）**
> —— 本轮把第一台**真实国标设备**（EasyGBD `34020000001320128497`，TCP）接入平台并端到端实测，
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

# 方言冒烟（pg:18081 / mysql:18082 需用各自 feature 构建并独立 target 目录）
python3 scripts/dialect_smoke.py --base http://127.0.0.1:18080/api --token "$TOKEN"

# UI e2e（需 18080 后端 + 9528 前端 dev）
cd e2e && npx playwright test
```

**环境注意事项**（踩过的坑，写下来避免重复排查）：
* `target/debug/gbserver` 是**最后一次构建的 feature**；用 pg/mysql 构建时务必加
  `CARGO_TARGET_DIR=/tmp/gbt-pg`（或 `-my`），否则会覆盖 sqlite 二进制（"陈旧二进制陷阱"）。
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
