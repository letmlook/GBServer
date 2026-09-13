# GBServer 遗留问题与进度（Open Issues）

> 生成时间：2026-09-13（本地 08:3x）
> 代码基线：第五十八轮（详见 `WVP_PARITY.md`）+ 本轮 `startRecord` 重试与 mock 加固
> 测试基线：`cargo test` **743 通过 / 0 失败**、`npx playwright test` **66 通过**、
> 三方言冒烟仅剩 2 项已记录的"预期为真"项（E1）
>
> 本文档**只列尚未完成/尚未验证的事项**，已完成的历史证据见
> [`WVP_PARITY.md`](WVP_PARITY.md)（逐轮记录）与 [`audit/`](audit/)（模块审计）。
> 更新规则：每轮修完一批就把对应条目移出本文档；新增未完成项必须在这里登记，
> 不允许只写在提交信息里。

## 图例

| 标记 | 含义 |
|---|---|
| 🔴 | 未实现（端点/功能缺失） |
| 🟠 | 已定位的真实缺陷，尚未修复 |
| 🟡 | 已修复，**尚未完成端到端复验/提交** |
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
| B1 | 缺陷 | UDP 模式下 `connectRtpServer` 不可用（ZLM 报"仅支持tcp主动模式"） | 🟠 |
| B2 | 缺陷 | 录像计划 `startRecord` 与设备推流的竞态 | ✅ 已修并复验 |
| B3 | 缺陷 | 假设备 mock 缺 `connection_lost` → 进程崩溃（"设备在线但无 RTP"） | ✅ 已修并复验 |
| B4 | 环境 | 后台进程"静默退出"= 同组 job 被中止连带杀进程（已定性，非缺陷） | ⚪ |
| B5 | 测试 | `cloudRecord` e2e 仍有时序耦合（本轮 3/3 通过） | 🔵 |
| B6 | 缺陷 | `send_session_bye` 只按 设备+通道 定位会话 →**跨类型误停**（停直播会停掉同通道回放） | 🟠 |
| B7 | 缺陷 | `/api/play/webrtc` 调 ZLM 的形态错误 → 恒失败（**已修，普通流真实浏览器验证可播**） | ✅ 已修 |
| B8 | 缺陷 | **GB28181 流走 WebRTC 解不出帧**（收 358KB RTP 但 `framesReceived=0`；普通流正常） | 🟠 |
| B9 | 配置 | `rtc.externIP` 平台未下发 → 容器部署下浏览器 ICE 永远连不上 | 🟠 |
| A4 | 前端 | 直播页没有 WebRTC 播放入口（`postWebrtcPlay` 无调用方） | 🔴 |
| C1 | 代码债 | `handle_packet` 23 个参数 | 🔵 |
| C2 | 代码债 | 32 个无引用的 `db::` 函数（逐条判定删除/接上） | 🔵 |
| C3 | 缺陷/代码债 | JT1078 鉴权码只存不用 + 注册应答写死 `"GBServer"` + 0x0102 语义存疑 | 🟠 |
| C4 | 代码债 | API Key 过期记录不清理（鉴权已判过期，仅表数据堆积） | 🔵 |
| D1 | 真机核验 | GB28181 真实设备（TCP 被动 / 401 鉴权 / SDP 端口差异 / 目录分页） | 🔴 |
| D2 | 真机核验 | JT1078 真实终端（0x0802 变体 / 0x8100 鉴权 / 双向对讲） | 🔴 |
| D3 | 真机核验 | 对讲音频互通（G.711A 时间戳/回声/抖动） | 🔴 |
| E1 | 环境 | 冒烟脚本 2 项"预期为真"项（CSV 非 JSON、dummy 代理 404） | ⚪ |
| E2 | 环境 | 本 ZLM 版本缺 7 个 hook 事件键（已如实告警） | ⚪ |
| E3 | 环境 | 长驻服务需单独 job 启动（同组 job 中止会连带杀掉） | ⚪ |
| E4 | 环境 | CI 自动触发按用户要求保持关闭（`workflow_dispatch`） | ⚪ |

---

## 核心闭环健康度（2026-09-13 实测）

结论：**没有发现导致全流程跑不起来的阻塞问题**。9 条核心闭环逐条实测：

| # | 闭环 | 结果 | 证据 |
|---|---|---|---|
| 1 | 设备接入（注册/心跳/目录） | ✅ | 在线设备 2、通道 4 |
| 2 | 实时点播 → ZLM → FLV | ✅ | FLV 实拉 **2,408,800 字节**；停止时发 BYE |
| 3 | 历史回放 + 回放控制 | ✅ | pause/speed/seek 全 code=0，停回放正常 |
| 4 | 对讲（含音频上行） | ✅ | 假设备收到 **150 个 RTP 包**，BYE 校验通过 |
| 5 | 语音广播 | ✅ | start/stop code=0 |
| 6 | 设备控制（PTZ/布防/录像/远程启动/配置查询） | ✅ | 5 项全 code=0 |
| 7 | JT1078（终端/通道） | ✅ | 终端列表 total=4、通道列表 code=0 |
| 8 | 级联平台 | ✅ | 平台记录在册（推流/点播已在此前轮次端到端验证） |
| 9 | 云录像（计划→录制→落库→播放→删除） | ✅ | `cloudRecord` spec **3/3 通过**，落库 1.0MB MP4 |

另有：`cargo test` **743 通过 / 0 失败**、`npx playwright test` **66 通过**、
三方言冒烟仅剩 2 项已记录预期项（E1）。

### 阻塞/非阻塞判定

| 事项 | 是否影响核心闭环 | 判定 |
|---|---|---|
| A1 中亿视图 10 端点 | 否（第三方定制集成模块） | 🟢 可发布，后续迭代 |
| A2 `/api/test/*` | 否（运维诊断） | 🟢 可发布，后续迭代 |
| A3 LiveGBS `/api/v1/*` | 否（另一套协议/鉴权） | 🟢 范围外 |
| B1 UDP `connectRtpServer` | **特定设备类**才有影响（标准设备按 INVITE 端口推流，正常） | 🟡 需真机确认设备画像 |
| B2 录像计划竞态 | 曾影响（已修已复验） | ✅ 已关闭 |
| B3 mock 崩溃 | 否（测试基建） | ✅ 已关闭 |
| B4 进程"静默退出" | 否（会话操作副作用） | ✅ 已定性 |
| B5 cloudRecord 用例时序 | 否（测试用例脆弱点） | 🟢 可发布，后续加固 |
| **B6 BYE 跨类型误停** | **条件性影响**：同一通道"直播+回放/下载"并发、或无人观看自动关流时可能停错流 | 🟠 **建议下一个迭代优先修** |
| C1 `handle_packet` 23 参数 | 否 | 🟢 可发布 |
| C2 32 个无引用 db 函数 | 否 | 🟢 可发布（清理） |
| C3 JT1078 鉴权码只存不用 | **仅真终端**有风险（平台自身不校验，mock 不校验） | 🟡 需真机确认 |
| C4 API Key 过期记录不清理 | 否（鉴权已正确拒绝） | 🟢 可发布 |
| D1–D3 真机核验缺口 | 未知风险（模拟器覆盖不到的行为） | 🟡 真机联调前不建议对外承诺 |

---

## A. 未实现的功能端点

### A1 🔴 中亿视图（SY）定制模块 10 条端点

来源：WVP-PRO `web/custom/CameraChannelController.java`（对照脚本见 `WVP_PARITY.md`
第五十七/五十八轮的方法：抽 `@*Mapping` + 归一化比对 `router.rs`）。

缺失清单（本仓库 `/api/sy/*` 已实现 list / list-with-child / cont-with-child /
box / circle / polygon / address / meeting / control/{play,stop,ptz} 等）：

| 端点 | WVP 语义 |
|---|---|
| `GET /api/sy/camera/one` | 单通道详情 |
| `GET /api/sy/camera/update` | 更新通道 |
| `GET /api/sy/push/play` | 推送播放（含校验） |
| `GET /api/sy/push/play-without-check` | 推送播放（跳过校验） |
| `GET /api/sy/record/collect/add` | 录像收藏/加入收藏夹 |
| `GET /api/sy/record/collect/delete` | 取消收藏 |
| `GET /api/sy/record/zip` | 录像打包下载（ZIP） |
| `GET /api/sy/record/list-url` | 录像列表（仅 URL 形态） |
| `GET /api/sy/forceClose` | 强制关闭流 |
| `GET /api/sy/test` | 自检 |

注意：`collect/add|delete` 与 `zip` 在本仓库已有**同名但不同前缀**的实现
（`/api/gb_record/collect/*`、`/api/cloud/record/zip`），实现时应复用而不是重写。

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

### B1 🟠 UDP 模式下 `connectRtpServer` 不可用

**现象**（实测日志，2026-09-13 08:28）：

```
start_live_stream: connectRtpServer rtp://127.0.0.1:10000 failed:
  ZLM error: 仅支持tcp主动模式
```

**背景**：非标准设备（如 `gbcpp/1.0` 风格）在 INVITE 的 200 OK SDP 里宣告**自己的**
收流端口，此时平台需要调 ZLM `connectRtpServer` 让 ZLM 主动去连（TCP 主动模式）。
但 ZLM 对 `rtp_type=0`（UDP）拒绝该调用。

**影响**：UDP 模式下这类设备**只能**依赖"按 INVITE 的 m= 端口推流"；
若不按，实时流会一直等不到媒体（15s 后超时并回收端口）。当前只打 WARN。

**候选方案**（择一，需要决策）：
1. 检测到设备 200 OK 端口与 INVITE 端口不一致且传输是 UDP 时，**明确报错**并提示
   "该设备要求 TCP 主动模式"，而不是静默等 15s；
2. 对 UDP 设备改用"关掉原 RTP server → 在新端口重开 → 让设备按新端口推流"的方式；
3. 强制这类设备走 TCP-PASSIVE（配置层约束）。

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

### B6 🟠 `send_session_bye` 只按「设备+通道」定位会话 → 跨类型误停

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

**修复方向（未做）**：
1. `send_session_bye` 增加 `stream_type`（或 `stream_id`）参数：`play_stop` 只找
   `Play`、`playback_stop` 只找 `Playback`、下载只找 `Download`；
2. hook 侧（`on_stream_none_reader`、RTP 超时）**已有 `stream_id`**，
   应按 stream_id 精确定位会话，而不是退回 `(device, channel)`；
3. 同一类型多条时按 `created_at` 最新的一条停（或全部停并逐个发 BYE）。

**影响评估**：不阻塞主流程（每个流程单独跑都通，e2e 66 项全绿），
但"直播 + 同通道回放/下载并发"以及"无人观看自动关流"场景会**停错流**。

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

### B9 🟠 `rtc.externIP` 未下发（容器部署下 WebRTC 必失败）

实测：ZLM 的 answer 候选地址默认是 **`172.18.0.2:8000`**（容器网段），
宿主浏览器不可达 → `iceConnectionState` 永远停在 `checking`、0 字节；
用 ZLM `setServerConfig?rtc.externIP=127.0.0.1` 后候选变成 `127.0.0.1:8000`，
**ICE 立刻 connected 并开始收流**。

平台目前在启动时只自动下发 hook 配置与 `rtp_proxy.port_range`
（`zlm/health_checker.rs`），**没有任何 rtc.externIP 的设置**。
**下一步**：加一个可配置项（例如 `zlm.rtc_extern_ip`，缺省用 `sip.stream_ip`
或 `server.public_ip`），在节点上线时用 `set_server_config_verified` 下发并回读校验；
同时文档里写清"域名/公网 IP + `rtc.port`(8000/udp) 必须在防火墙放行"。

### A4 🔴 直播页没有 WebRTC 播放入口

`web/src/api/live.ts::postWebrtcPlay` 已按后端契约写好（POST + JSON body），
但**全仓库没有任何调用方**（注释里也写明"当前无页面调用方，直播页用 flv/hls/ws"）。
要真正"用 WebRTC 看画面"，还需要：播放器组件增加 `webrtc` 分支
（`RTCPeerConnection` + `<video>`，断开时调 ZLM `delete_webrtc`）、
直播页协议切换（FLV/HLS/WS/WebRTC），以及 B8/B9 先修好。

---

## C. 代码债与清理

### C1 🔵 `handle_packet` 23 个参数

`src/sip/server.rs::handle_packet` 参数已达 23 个（历史登记项，见
`WVP_PARITY.md`「仍未解决」第 7 条）。应抽 `SipPacketContext` 结构体。
**风险**：纯重构，但触及所有 SIP 入口，建议单独一轮 + 全量测试。

### C2 🔵 32 个无引用的 `db::` 函数

用脚本扫出（全仓库仅出现一次定义、零引用）——**需要逐条判定"删除"还是"接上"**：

```
db/alarm.rs          : batch_delete_alarms, count_alarms, delete_alarm, list_alarms_paged
db/cloud_record.rs   : delete_by_app_stream, get_collect_records, query_by_device_channel
db/device.rs         : batch_insert_channels, batch_update_channel_status,
                       batch_upsert_channels, count_alive_devices, count_channels,
                       count_registered_devices, delete_channels_by_device
db/jt1078.rs         : count_online_terminals, get_auth_code_by_phone, update_auth_code
db/media_server.rs   : add_white_list_cidr, remove_white_list_cidr, mark_offline_if_expired
db/platform.rs       : update_enable
db/platform_channel.rs: batch_delete_channels, get_by_platform_and_channel, list_by_platform_id
db/role.rs           : get_by_name
db/stream_proxy.rs   : list_by_media_server, update_enable_status, update_pulling_status
db/user.rs           : find_by_username_password, update_user_role, update_username
db/user_api_key.rs   : delete_expired_keys
```

初步分类：
* **删除候选（功能已由别的实现覆盖 / WVP 也没有）**：
  `delete_by_app_stream`（第五十四轮已改为 `delete_by_app_stream_period`）、
  `update_user_role` / `update_username`（WVP `UserController` 没有对应端点，
  已核对）、`add_white_list_cidr` / `remove_white_list_cidr`（WVP 无白名单功能）、
  `update_enable`（平台启停走 `/api/platform/update`）、`get_by_name`。
* **需要接上（可能是缺失的功能）**：`delete_expired_keys`（见 C4）、
  `get_auth_code_by_phone` / `update_auth_code`（见 C3）。
* `mark_offline_if_expired`：已被 `media_server::mark_offline_if_miss_count_exceeded`
  （健康检查按**连续丢失次数**判下线，`zlm/media_node.rs` 调用）取代 → **删除候选**。
* **其余**（alarm/device/platform_channel/stream_proxy 的批量与计数函数）：
  要么被"合并查询"取代（`count_channels` vs `count_all_channels`），
  要么是早期分层遗留 —— 逐条确认后删除。

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

---

## D. 需真实硬件核验

### D1 🔴 GB28181 真实设备

目前所有 GB28181 交互都是对着
`mock/tools/sip-device/sip_device_mock.py`（可选 `--send-rtp` 真实推流）
与真实 ZLMediaKit 验证的，**没有跑过真实摄像机/NVR**。真机上才暴露的差异：

* 401/407 摘要鉴权（`realm`/`nonce`/`qop` 组合、`algorithm=MD5`）；
* `TCP-PASSIVE` 设备的 `connectRtpServer` 行为（与 B1 相关）；
* 200 OK SDP 端口与 INVITE 端口不一致的非标实现；
* 目录（Catalog）分页 `SumNum` 不实、分包顺序乱序；
* 设备主动发 BYE / 心跳超时下线 / 注册续期；
* `ConfigDownload` 各 `ConfigType` 的真实字段差异（当前只在 mock 上验证了
  BasicParam / SnapConfig）。

### D2 🔴 JT1078 真实终端

* 0x0802 多媒体检索的 **JT/T 808-2011 与 2019 变体**（字段偏移不同）；
* 0x8100 注册鉴权码校验（见 C3）；
* 0x8201 位置查询的应答字段差异（海拔/方向/报警标志）；
* 双向对讲（0x8300 文本下发 / 0x8204 电话回拨）需要真终端配合。

### D3 🔴 对讲音频互通

已验证到"假设备收到 50 个 RTP 包、`y=` 与 SSRC 一致、0 丢序"
（见 `WVP_PARITY.md` 第五十四轮）。真机上还需验证：
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

---

## F. 本轮改动清单（随本文档一起提交）

| 文件 | 内容 | 状态 |
|---|---|---|
| `src/zlm/client.rs` | `close_rtp_server_ex`（顶层 `hit`）、`stop_send_rtp_ex` 返回 `bool`、`del_ffmpeg_source`、4 条 wiremock 测试 | ✅ 测试通过 |
| `src/handlers/rtp_control.rs` | `/api/{rtp,ps}/{receive/close,send/stop}` 查询参数版（`stream`/`callId`），`hit=0` 明确报错 | ✅ 实测 |
| `src/handlers/user.rs` | `/api/user/all` | ✅ 实测 |
| `src/handlers/server.rs` | `/api/server/shutdown`（真的退出进程，先回响应） | ✅ 实测（18099 独立实例） |
| `src/handlers/jt1078_extra.rs` | `terminal/channel/{one,delete}?id=` | ✅ 实测 |
| `src/router.rs` | 上述路由注册 | ✅ |
| `src/scheduler/record_plan.rs` | `startRecord` 有界重试 + 2 条分类测试（B2） | ✅ cloudRecord 3/3 |
| `mock/.../sip_device_mock.py` | `connection_lost`/`error_received`（B3）、报警查询应答、DeviceConfig 日志字段补全、`DeviceControl` 结构告警 | ✅ 整轮 e2e 未再退出 |
| `docs/WVP_PARITY.md` | 第五十七/五十八轮记录 | ✅ |

**复验结果（2026-09-13）**：`cloudRecord` **3/3 通过**、
`cargo test` **743 通过 / 0 失败**、`npx playwright test` **66 通过**、
三方言冒烟仅剩 2 项已记录预期项 —— B2 / B3 转入"已修复并复验"。

---

## G. 复现环境

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

---

## 变更记录

| 日期 | 变更 |
|---|---|
| 2026-09-13 | 建立本文档：登记 A1–A3、B1–B5、C1–C4、D1–D3、E1–E4 |
| 2026-09-13 | 新增 B7/B8/B9 与 A4（WebRTC：接口调用形态已修+真实浏览器验证；国标流解不出帧；rtc.externIP 未下发；前端无入口） |
| 2026-09-13 | 新增 B6（`send_session_bye` 跨类型误停，实测证据 + 修复方向）；B5 保持 🔵 |
| 2026-09-13 | 复验并关闭 B2（录像计划 `startRecord` 竞态）、B3（mock `connection_lost` 崩溃）、B4（定性为同组 job 被杀的副作用）；新增 E2（ZLM 缺 7 个 hook 键）、E3（长驻服务启动方式与顺序）；C3 升级为 🟠（鉴权码只存不用 + 注册应答写死 `"GBServer"` + 0x0102 语义存疑） |
