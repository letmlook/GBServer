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
| A1 | 功能端点 | 中亿视图（SY）定制模块 10 条端点 | 🔴 |
| A2 | 功能端点 | WVP 自带诊断端点 `/api/test/{hook/list,redis}` | 🔴 |
| A3 | 功能端点 | LiveGBS 兼容 API（`/api/v1/*` 11 条 + `/auth/login`） | ⚪ |
| B1 | 缺陷 | UDP/TCP-PASSIVE 下 `connectRtpServer` 被 ZLM 拒绝（真机已复现） | 🟠 |
| B2 | 缺陷 | 录像计划 `startRecord` 与设备推流的竞态 | ✅ 已修并复验 |
| B3 | 缺陷 | 假设备 mock 缺 `connection_lost` → 进程崩溃（"设备在线但无 RTP"） | ✅ 已修并复验 |
| B4 | 环境 | 后台进程"静默退出"= 同组 job 被中止连带杀进程（已定性，非缺陷） | ⚪ |
| B5 | 测试 | `cloudRecord` e2e 仍有时序耦合（本轮 3/3 通过） | 🔵 |
| B6 | 缺陷 | `send_session_bye` 只按 设备+通道 定位会话 →**跨类型/跨代际误停**（真机已复现） | 🟠 |
| B7 | 缺陷 | `/api/play/webrtc` 调 ZLM 的形态错误 → 恒失败（**已修，普通流真实浏览器验证可播**） | ✅ 已修 |
| B8 | 缺陷 | **GB28181 流走 WebRTC 解不出帧**（收 358KB RTP 但 `framesReceived=0`；普通流正常） | 🟠 |
| B10 | 缺陷 | ZLM 流列表反序列化失败（`"fps": 25.0` 浮点 vs `u32`）→ 流列表恒为空 | ✅ 已修 |
| B11 | 缺陷 | `on_server_started` 用 ZLM **内部** HTTP 端口覆盖库里的对外端口（80 覆盖 8080）→ 后端所有 ZLM 调用 502 | ✅ 已修 |
| B12 | 缺陷 | ZLM 对同一路流**按协议各返回一行** → 控制台「重点通道」同一通道重复铺格子、「直播 N」虚高 | ✅ 已修并复验 |
| B13 | 性能 | 控制台首屏"等最慢接口"才统一赋值 + `system/info` 被 3 个组件同时拉 → 卡片空等约 2s | ✅ 已修并复验 |
| B14 | 缺陷 | 设备 `DeviceControl` / `ConfigDownload` 等**应答未被解析**（真机实测落入 `Unhandled MESSAGE body`） | 🟠 |
| B9 | 配置 | `rtc.externIP` 平台未下发 → 容器部署下浏览器 ICE 永远连不上 | ✅ 已修 |
| A4 | 前端 | 直播页 WebRTC 播放入口（`postWebrtcPlay` 原先无调用方） | ✅ 已接 (2026-09-19) |
| C1 | 代码债 | `handle_packet` 23 个参数 | 🔵 |
| C2 | 代码债 | 29 个无引用的 `db::` 函数（逐条判定删除/接上；上一版记为 36 个） | 🔵 |
| C3 | 缺陷/代码债 | JT1078 鉴权码只存不用 + 注册应答写死 `"GBServer"` + 0x0102 语义存疑 | 🟠 |
| C4 | 代码债 | API Key 过期记录不清理（鉴权已判过期，仅表数据堆积） | 🔵 |
| C5 | 代码债 | ~~`/api/user/users` 的 `UsersQuery` 无 `query` 字段~~ | ✅ 已修 (`bfa65f8`) |
| C6 | 缺陷 | 用户管理权限缺口：9 个管理端点无角色校验（普通用户可造管理员级角色） | ✅ 已修 (`bfa65f8`) |
| C7 | 缺陷 | 删角色不校验引用 → 悬空 `role_id` 用户被 INNER JOIN 静默吞掉 | ✅ 已修 (`bfa65f8`) |
| C8 | 缺陷 | 通道 `hasAudio` 与实际流不符（真机流含 G.711A，接口仍报 `hasAudio=false`） | 🔵 |
| C9 | 缺陷 | 前端用 GET 调 `DELETE /api/cloud/record/collect/delete` → 405 | 🔵 |
| D1 | 真机核验 | GB28181 真实设备（TCP 被动 / 401 鉴权 / SDP 端口差异 / 目录分页） | 🟡 已接入并核验主链路，剩余项见 D1 |
| D2 | 真机核验 | JT1078 真实终端（0x0802 变体 / 0x8100 鉴权 / 双向对讲） | 🔴 |
| D3 | 真机核验 | 对讲音频互通（G.711A 时间戳/回声/抖动） | 🔴 |
| E1 | 环境 | 冒烟脚本 2 项"预期为真"项（CSV 非 JSON、dummy 代理 404） | ⚪ |
| E2 | 环境 | 本 ZLM 版本缺 7 个 hook 事件键（已如实告警） | ⚪ |
| E3 | 环境 | 长驻服务需单独 job 启动（同组 job 中止会连带杀掉） | ⚪ |
| E4 | 环境 | CI 自动触发按用户要求保持关闭（`workflow_dispatch`） | ⚪ |
| E5 | 部署 | ZLM 改 host 网络 + 配置文件只读管理 + mac 叠加文件 | ✅ 已改 |
| E6 | 环境 | 后端访问 `192.168.3.88:8080`（ZLM）偶发 `error sending request`（本机 HTTP 代理同名主机） | ⚪ |

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
| A1 中亿视图 10 端点 | 否（第三方定制集成模块） | 🟢 可发布，后续迭代 |
| A2 `/api/test/*` | 否（运维诊断） | 🟢 可发布，后续迭代 |
| A3 LiveGBS `/api/v1/*` | 否（另一套协议/鉴权） | 🟢 范围外 |
| B1 端口不一致时的 `connectRtpServer` | **真机已复现**，但该设备仍按 INVITE 端口推流，流正常 → 目前只影响"不按 INVITE 端口推流"的设备类 | 🟡 建议补明确错误提示 |
| B2 录像计划竞态 | 曾影响（已修已复验） | ✅ 已关闭 |
| B3 mock 崩溃 | 否（测试基建） | ✅ 已关闭 |
| B4 进程"静默退出" | 否（会话操作副作用） | ✅ 已定性 |
| B5 cloudRecord 用例时序 | 否（测试用例脆弱点） | 🟢 可发布，后续加固 |
| **B6 会话 BYE 定位过粗** | **真机已复现**：停直播时 BYE 发给了已终止的旧会话，当前会话未被 BYE（会话泄漏 + 设备侧收到未知对话的 BYE） | 🟠 **建议下一个迭代优先修** |
| B14 应答未解析 | 条件性：设备已回结果，平台却当未知报文丢弃 → 无法确认传输模式/录像/配置类命令是否生效 | 🟠 建议修 |
| C1 `handle_packet` 23 参数 | 否 | 🟢 可发布 |
| C2 29 个无引用 db 函数 | 否 | 🟢 可发布（清理） |
| C3 JT1078 鉴权码只存不用 | **仅真终端**有风险（平台自身不校验，mock 不校验） | 🟡 需真机确认 |
| C4 API Key 过期记录不清理 | 否（鉴权已正确拒绝） | 🟢 可发布 |
| C8 `hasAudio` 与实际流不符 | 否（仅元数据/前端图标不准） | 🟢 可发布 |
| C9 收藏删除方法不匹配 | 否（该按钮在 UI 未暴露时的调用才会 405） | 🟢 可发布 |
| D1 真机剩余项 | 未知风险（模拟器覆盖不到的行为） | 🟡 主链路已通，其余项真机联调前不建议对外承诺 |
| D2–D3 真机核验缺口 | 未知风险（模拟器覆盖不到的行为） | 🟡 真机联调前不建议对外承诺 |

---

## A. 未实现的功能端点

### A1 🔴 中亿视图（SY）定制模块 10 条端点

来源：WVP-PRO `web/custom/CameraChannelController.java`（对照方法：抽 `@*Mapping` +
归一化比对 `router.rs`）。

缺失清单（本仓库 `/api/sy/*` 已实现 **13 条**：`camera/list`、`camera/list/ids`、
`camera/list-with-child`、`camera/list-for-mobile`、`camera/cont-with-child`、
`camera/list/{box,circle,polygon,address}`、`camera/meeting/list`、
`camera/control/{play,stop,ptz}`）：

| 端点 | WVP 语义 | 本仓库等价实现 |
|---|---|---|
| `GET /api/sy/camera/one` | 单通道详情 | ❌ 无 |
| `GET /api/sy/camera/update` | 更新通道 | ❌ 无 |
| `GET /api/sy/push/play` | 推送播放（含校验） | ❌ 无 |
| `GET /api/sy/push/play-without-check` | 推送播放（跳过校验） | ❌ 无 |
| `GET /api/sy/record/collect/add` | 录像收藏/加入收藏夹 | ✅ `/api/cloud/record/collect/add`（`router.rs:569`） |
| `GET /api/sy/record/collect/delete` | 取消收藏 | ✅ `/api/cloud/record/collect/delete`（`router.rs:573` / `1082`） |
| `GET /api/sy/record/zip` | 录像打包下载（ZIP） | ✅ `/api/cloud/record/zip`（`router.rs:1087`） |
| `GET /api/sy/record/list-url` | 录像列表（仅 URL 形态） | ✅ `/api/cloud/record/list-url`（`router.rs:1086`） |
| `GET /api/sy/forceClose` | 强制关闭流 | ✅ `/api/push/forceClose`（`router.rs:1069`） |
| `GET /api/sy/test` | 自检 | ❌ 无 |

**实现时应复用已有实现、只加前缀别名，不要重写逻辑。**
（注意：`/api/cloud/record/collect/delete` 自身还有一个前端方法不匹配的问题，见 C9。）

### A2 🔴 WVP 自带诊断端点

`GET /api/test/hook/list`（列出 ZLM hook 事件与最近一次载荷）、
`GET /api/test/redis`（Redis 连通性）。属运维诊断，优先级低。

### A3 ⚪ LiveGBS 兼容 API（范围外）

`/api/v1/{login,getserverinfo,userinfo,device/list,device/channellist,device/fetchpreset,stream/{start,stop,touch},control/{ptz,preset}}`
共 11 条 + `web/gb28181/AuthController` 的 `/auth/login`。

**为何范围外**：这套接口用 LiveGBS 的 `sign` 签名鉴权（不是本平台的 JWT/API-Key），
是**另一套第三方集成协议**；且需要与 LiveGBS 的返回结构逐字段对齐才有意义。
若将来要支持，应作为独立里程碑（新增鉴权过滤器 + 独立 DTO 层），不要在现有
handler 上打补丁。

---

## B. 已定位的缺陷

### B1 🟠 UDP / TCP-PASSIVE 下 `connectRtpServer` 被 ZLM 拒绝（真机已复现）

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
下面的 `WVPResult::success`；对照 `play.rs:295-307` 的 TCP-PASSIVE 分支是
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

### B5 🔵 `cloudRecord` e2e 仍有时序耦合（当前已稳定）

**现象**：同一 spec 在不同轮次出现 `66 passed` / `65 passed + 1 skipped` /
`3 skipped` / `2 failed` 等多种结果。

**已定性的部分**：`3 skipped`、`2 failed` 的直接原因是 B3（mock 崩溃）+ B4
（后端消失）导致 `ensureRecording` 拿不到通道/JSON 解析失败 —— **环境问题，非用例缺陷**。

**残留的用例脆弱点**：用例依赖"ZLM 关闭 MP4 时才触发 `on_record_mp4` 落库"，
而 `ensureRecording` 在**删除计划之后**才轮询落库结果；当 `stop_record` 与
`start_record` 只相隔几毫秒时（计划被立即删除），ZLM 不产出文件、也不回调。
B2 修好后本轮 3/3 通过，但建议进一步**显式化**：录满 N 秒 → 调 `stop` →
断言 `on_record_mp4` 回调（或断言 ZLM 记录目录出现文件），
避免"计划刚建就删"的极端时序。

### B6 🟠 `send_session_bye` 只按「设备+通道」定位会话 → 跨类型 / 跨代际误停（真机已复现）

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

**修复方向（未做）**：
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

### B8 🟠 GB28181 流走 WebRTC **解不出帧**（未解决）

同一套代码，把 `stream` 换成国标流 `rtp/34020000001320000001_34020000001320000001`：

```
ICE/DTLS：connected
浏览器收到：packetsReceived=466, bytesReceived=358757, packetsLost=0
但：framesReceived=0, framesDecoded=0, keyFramesDecoded=0, pliCount=44（PLI 风暴）
Chromium 的 getStats 里 **没有任何 codec 统计**（codecs: []）
```

对照普通流有 codec 统计（PT/H264 关联成功）。**即：浏览器收到 RTP，但那些包
与协商出来的 H264（PT 103）关联不上，因此永远组不出帧。**

ZLM 侧该流是健康的：`rtp/<dev>_<ch>` 的 video track 为 H264 352x288@25fps、
`key_frames=527`，FLV/RTSP/HLS 播放都正常（e2e 全绿）。所以问题在
**「PS/RTP 源 → WebRTC」这段的 RTP 封装/PT 改写**，候选原因：
1. ZLM 对 RTP/PS 源流做 WebRTC 时未把负载 PT 改写为协商值（透传了源 PT 96）；
2. PS 解封装后的 H264 未按 RFC 6184 重新打包（缺 `sprop-parameter-sets`/FU-A 分片）；
3. 该 ZLM 构建对 PS 源流的 rtc 支持依赖额外配置（如先经 `addFFmpegSource` 转一路）。

**下一步（择一，需实验）**：
* 抓一次 8000/udp 的包，看实际 PT 与 NAL 头（最直接）；
* 换一个 ZLM tag 试（`zlmediakit/zlmediakit:master` 是最新 master，可能存在回归）；
* 平台侧兜底：把 GB 流先经 ffmpeg 重封装/转码成标准 H264 再交给 WebRTC；
* 或明确"WebRTC 仅用于非国标/代理流"，国标流继续用 FLV/HLS/WS。

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
**仍然遗留**：B8（GB28181 流走 WebRTC 解不出帧）未解决 —— 入口有了，但国标流经
WebRTC 仍收不到帧；B9（`rtc.externIP` 下发）已修。

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

**注意（同一根因的另一处，尚未修）**：后端把 `getMediaList().len()` 当"流数量"用的三处
（`zlm/hook.rs` 的 `set_active_streams` 与 `update_flow_stats`、`zlm/client.rs::get_active_stream_count`
的选路负载）也把 1 路流数成 5 路。前端没有展示 `stream_count`，选路是相对比较，
故未在本次一并改动 —— 需要时同样按 (app, stream) 去重。

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

### B14 🟠 设备 `DeviceControl` 等应答未被解析（真机实测落入 `Unhandled MESSAGE body`）

**现象**（2026-09-19 13:48:43，真机 EasyGBD）：

```
Transport mode change: device=34020000001320128497, mode=TCP-PASSIVE      ← 平台下发
MESSAGE from 34020000001320128497 - CmdType: Some("DeviceControl")        ← 设备应答
Unhandled MESSAGE body: <?xml version="1.0" encoding="GB2312"?>
  <Response><CmdType>DeviceControl</CmdType><SN>1789796923</SN>
  <DeviceID>34020000001320128497</DeviceID><Result>OK</Result></Response>
```

**根因**：`src/sip/server.rs::handle_message` 的 `CmdType` 分支只覆盖
`Keepalive` / `Catalog` / `DeviceInfo` / `DeviceStatus` / `MobilePosition` /
`Alarm` / `RecordInfo`（2081–2288 行），**没有 `DeviceControl`**，其余一律落到
`_ => tracing::debug!("Unhandled MESSAGE body: {}", body)`（2286）。

**影响**：
- 传输模式切换、PTZ、布防、录像、重启等**指令类**下发后，设备回的 `Result=OK/ERROR`
  被当作未知报文丢弃（DEBUG 级日志，生产 `RUST_LOG=info` 下**完全不可见**）；
- 运维无法判断"命令到底生效没有"，只能靠设备行为反推；
- 与 `STATUS.md`「设备配置查询/更新会等待设备响应（15s）」并不矛盾 ——
  那条走的是 `register_device_config_with_receiver` + `await_response`
  （`handlers/device_control.rs:339`、`device_stub.rs:417`），是**查询类**；
  缺失的是**指令类应答**的统一解析与落库/回显。

**下一步**：`handle_message` 增加 `DeviceControl` 分支，以及配置下载应答分支
（平台下发 `ConfigDownload`，设备回的 `CmdType` 是 `DeviceConfig`），按 `SN`
关联待确认命令；至少把 `Result != OK` 提升为 WARN。

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
* **删除候选（功能已由别的实现覆盖 / WVP 也没有）**：
  `delete_by_app_stream`（已改为 `delete_by_app_stream_period`）、
  `add_white_list_cidr` / `remove_white_list_cidr`（WVP 无白名单功能）、
  `update_enable`（平台启停走 `/api/platform/update`）、
  `platform::add`（新增平台走别的路径）、
  `stream_proxy::update_pulling_status`（已被 `update_pulling_status_by_app_stream` 取代）、
  `mark_offline_if_expired`（已被 `media_server::mark_offline_if_miss_count_exceeded`，
  健康检查按**连续丢失次数**判下线，`zlm/media_node.rs` 调用，取代）。
* **需要接上（可能是缺失的功能）**：`delete_expired_keys`（见 C4）、
  `get_auth_code_by_phone` / `update_auth_code`（见 C3）。
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

### C3 🟠 JT1078 鉴权码：**只存不用**，注册应答里写死 `"GBServer"`

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
**下一步**：按标准确认 0x0102 / 属性上报的正确 msg_id → 注册应答从 DB 取
`auth_code` → 补鉴权校验（可做成强制/宽松两档）。

### C4 🔵 API Key 过期记录不清理

核对结果：**鉴权路径已经检查过期**（`src/auth.rs`：`enable == false`，或
`expired_at > 0 && now > expired_at`，都直接 401），所以这不是安全问题；
只是 `db::user_api_key::delete_expired_keys` 零引用，过期记录会**一直留在表里**
（长期运行堆积，列表页也会显示已过期条目）。
**下一步**：接一个低频后台清理任务，或删掉该函数并在列表查询里过滤。

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

### C9 🔵 前端用 GET 调一个只注册了 DELETE 的端点

**现象**：`/api/cloud/record/collect/delete` 在 `src/router.rs:573-576` 注册为
`delete(...)`，而前端 `web/src/api/cloudRecord.ts:212`
（`deleteCloudRecordCollect`）用的是 `method: 'get'` → 真实调用会 **405**。
（同时 `router.rs:1082` 另有一条 `cloud_record_extra::collect_delete`，需一并核对口径。）

**影响**：目前该入口在 UI 上未暴露为高频操作，但属真实契约不一致。
**下一步**：统一方法（改前端为 delete，或按 WVP 契约在 GET 上也注册），并补一条契约测试。

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
* WebRTC：真机流建立过 rtc 播放会话（`on_play: rtc/…`），但帧解码未验证（B8）。

> 复现环境（本机 macOS）：后端 SQLite 单实例 :18080 + `docker compose` 的
> postgres/redis/zlm（ZLM 用 mac 叠加文件桥接）、真机在 192.168.3.121。
> 详细命令见 §G。

### D2 🔴 JT1078 真实终端

* 0x0802 多媒体检索的 **JT/T 808-2011 与 2019 变体**（字段偏移不同）；
* 0x8100 注册鉴权码校验（见 C3）；
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
| 2026-09-13 | 新增 B6（`send_session_bye` 跨类型误停，实测证据 + 修复方向）；B5 保持 🔵 |
| 2026-09-13 | 复验并关闭 B2（录像计划 `startRecord` 竞态）、B3（mock `connection_lost` 崩溃）、B4（定性为同组 job 被杀的副作用）；新增 E2（ZLM 缺 7 个 hook 键）、E3（长驻服务启动方式与顺序）；C3 升级为 🟠（鉴权码只存不用 + 注册应答写死 `"GBServer"` + 0x0102 语义存疑） |
| 2026-09-19 | 冻结轮：登记用户管理三项（C5/C6/C7）与 A4；B12/B13 修复并复验 |
| 2026-09-19 | 解冻并修复用户管理（`bfa65f8`）；C5/C6/C7 关闭 |
| 2026-09-19 | **真实设备接入核验轮**：D1 🔴→🟡（真机 EasyGBD 已接入，主链路核验通过，剩余项列明）；B1 补真机复现（并更正影响评估：该设备仍按 INVITE 端口推流，流不受影响）；B6 补真机复现（stop 把 BYE 发给已终止的旧会话）；C2 由 36 重算为 29（`get_by_name` 已接上、`update_user_role`/`update_username` 已删除）；新增 B14（应答未解析）、C8（`hasAudio` 与实际流不符）、C9（收藏删除方法不匹配）、E6（本机代理导致的 ZLM 请求偶发失败）；F/G 段更新为真机联调口径 |
