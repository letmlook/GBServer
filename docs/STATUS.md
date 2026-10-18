# GBServer 当前状态

## 2.0.0 当前验证增量（2026-10-18，`v2.0`）

此节仅记录本轮实际证据；下方 2026-09-19 的规模和真机数据是历史快照。

| 项目 | 本轮结果 | 证据范围 |
|---|---|---|
| Rust 测试 | `cargo test --no-fail-fast --locked`：806 通过、0 失败、3 个文档测试忽略 | SQLite 默认特性；本机自动化，ZLM + Mock 的实际运行证据见下文；真机仍待复验 |
| M4 Compose ZLM 配置加载 | Compose 的 `GBSERVER__ZLM__SERVERS__0__*` 会让配置库把节点数组解析成 map，启动时报 `invalid type: map, expected a sequence`；改用独立标量环境变量覆盖首个 ZLM 节点。SQLite Compose 同时对齐 ZLM `8080` 端口、只读配置文件及录像/截图持久卷 | 定向测试先复现旧变量导致配置加载失败，新变量的节点 IP、HTTP 端口、Hook 地址、RTC 地址及空 RTC 值清除均通过；主、mac 叠加、SQLite 三套 Compose 展开值核对通过。2026-10-18 PostgreSQL arm64 镜像重新构建，用隔离库和 Compose 同款标量变量启动容器，健康检查 healthy，库中节点 `192.0.2.10:18082` 与 RTC 地址一致；全量 Rust 测试 806 通过。SQLite ZLM 实际媒体闭环与目标部署仍待验收 |
| M4 容器 SIP 与媒体可达地址 | Compose 要求填写设备可达的 `GBSERVER_SIP_ADVERTISE_IP`，后端用它生成 SIP Via/Contact/SDP；`sip.bind_ip=0.0.0.0` 让容器内 UDP/TCP 仍监听所有网卡。主 Compose 和 SQLite Compose 均发布 `5060/udp` 与 `5060/tcp`；ZLM 节点地址采用浏览器可达的宿主 IP，桥接部署的 RTC 候选也采用该地址 | `docker compose config` 核对主、SQLite、mac 叠加配置均输出正确 IP 与双协议 5060；缺少通告 IP 时 Compose 明确拒绝。SIP 配置解析定向测试通过，`cargo test --no-fail-fast --locked` 805 通过、0 失败、3 忽略，`web/npm run build` 与 `cargo build --locked` 通过。本机隔离 SQLite 后端设置通告 `127.0.0.1`、绑定 `0.0.0.0`，实际 UDP/TCP socket 均监听 `*:5060`，系统信息分别返回两地址和版本 `2.0.0`。2026-10-18 在本机代理下 `docker compose build gbserver` 成功生成 arm64 镜像；用隔离 PostgreSQL 库和 Redis DB 15 启动容器，健康检查为 healthy，容器内 SIP UDP/TCP 均监听 `0.0.0.0:5060`，同源登录页、静态资源、登录及 HTTP 版本接口正常，OpenAPI 与受保护接口返回 `2.0.0`；容器端口 `18081` 的 Chromium 冒烟 20/20 通过。此次禁用 ZLM 节点并未发布 SIP 宿主端口，设备 LAN 可达性、容器媒体和目标服务器部署仍待验收 |
| M4 回放收流与停止复核 | 回放等待先确认设备 INVITE 200，再等 ZLM 媒体；486 拒绝及无媒体超时按本次 Call-ID 清理。回放媒体 URL 使用 ZLM 实际 `rtp` 应用名，停止核对设备、通道与会话 Call-ID，重复停止返回错误。普通回放、通道回放、下载邀请先登记 ACK 上下文再发包；三处在发包前校验 RTP 端口，发包失败关闭收流端口 | 本机隔离 SQLite + 真实 ZLM + SIP Mock：486 拒绝后无 RTP 残留；200 但无 RTP 时 15 秒超时；正常回放的 FLV 地址可读出 `FLV`，停止后 ZLM 无流且 Mock 收到有效对话内 BYE，错误通道和重复停止被拒绝。通道回放与下载的新增端口检查、真机、目标 Docker 部署仍待运行复验 |
| M4 录像下载结果与落盘 | 下载 INVITE 等设备 SIP 200 和 ZLM RTP 媒体就绪后才调用 `startRecord`；录制启动失败返回业务错误并按本次 Call-ID 发 BYE、关闭 RTP。停止下载先让 ZLM 收尾录制，再对本次 Call-ID 发 BYE，并校验设备和通道；MP4 回调已完成时不回退成 `finalizing` | 本机隔离 SQLite 后端 + 真实 ZLM + SIP Mock：最终代码的 486 拒绝、200 无 RTP 超时与正常推流三场景均通过；无 RTP 约 15 秒报超时并发送有效 BYE；正常停止得到 58,877 字节 MP4、有效 BYE，下载接口 Range 实拉 MP4 文件头，错误通道停止被拒绝；最终后端日志无 `stopRecord` 找不到流。`cargo test --no-fail-fast --locked` 804 通过、0 失败、3 忽略，`cargo build --locked` 通过；`verify_download_runtime.py` 可复跑。真机、目标 Docker 部署仍待验收 |
| M4 点播故障定位与拒绝清理 | 点播时 ZLM 未配置返回业务错误；RTP 开端口失败、设备拒绝及媒体超时均在响应中标出节点地址、流 ID 和请求入口。ZLM HTTP 错误不回显带 Secret 的查询 URL；486 拒绝的 INVITE 不再发送无效 BYE，等待媒体的 Call-ID 与实际 INVITE 一致，失败时按本次会话清理 | 本机隔离 SQLite 后端 + 真实 ZLM + SIP Mock：将 ZLM HTTP 端口指向无监听的 `18081`，点播返回连接错误且无 Secret；Mock 返回 486 时立即失败，ZLM `getRtpInfo` 确认无残留，Mock 未收到 BYE；Mock 回 200 但不发 RTP 时 15 秒超时，RTP 已清理且收到有效对话内 BYE。恢复正常推流后 `verify_play_stop_bye_runtime.py` 通过，停止后 11 秒未重连。定向脱敏测试及全量 Rust 测试通过；真机、目标部署与实际 RTP 端口映射错误仍待验收 |
| M4 RTP 端口范围错误 | 点播在发送 INVITE 前校验 ZLM 分配端口与节点声明的 RTP 范围；端口为 0、范围无效或越界时关闭刚开启的 RTP Server，响应给出节点、流、分配端口与范围 | 本机隔离 SQLite 后端 + 真实 ZLM + SIP Mock：只把隔离库节点范围改为 `40000-40100`，ZLM 分配 `30034`，接口立即拒绝；Mock 未收到 INVITE，`getRtpInfo` 确认无残留。正常范围下 `verify_play_stop_bye_runtime.py` 通过。定向范围测试与全量 Rust 测试通过；实际容器端口映射和目标部署仍待验收 |
| 版本一致性 | SIP `User-Agent` 统一从 Cargo 包版本生成，覆盖设备侧 SIP 和级联请求；HTTP `/api/server/version`、`/api/system/info` 与 OpenAPI 均返回 `2.0.0` | `cargo fmt --check`、Rust 全量测试与 `cargo build --locked` 通过；本机后端启动后对三个 HTTP 出口实测均为 `2.0.0`；级联 REGISTER/INVITE 的 SIP 报文构造测试通过，真机报文待复验 |
| 前端构建 | `web/npm run build` 通过 | Vue 类型检查与 Vite 打包，不代表浏览器播放通过 |
| M1 播放收流 | 普通点播和级联点播在媒体 ready 后不再二次回连。`on_rtp_server_started` 仅表示端口已打开，不误报媒体到达。TCP-PASSIVE 使用 ZLM `tcp_mode=2`、纯主机 `dst_url` 主动连接，并等真实流注册后才返回成功；失败清理 RTP Server 和 Play 会话 | 回归测试覆盖媒体判定、被动模式等待及取消；本机真实 ZLM + SIP Mock：普通模式无 RTP 时 15.0 秒超时、有 RTP 时 FLV 可读；TCP-PASSIVE 无设备媒体时 15.0 秒失败，ZLM RTP Server 0 残留且 Mock 收到 BYE；使用本机 TCP RTP/PS relay 时约 5.73 秒成功、FLV 头为 `FLV`、停止成功。真机仍待复验 |
| M1 录像重试 | 25 次、800ms 的生产预算不变；新增缺流重试和不可恢复错误次数测试 | `start_record_retry_tests` 4/4；本轮 ZLM + SIP Mock 生成新 MP4，详见下行 |
| M1 云录像验收 | `cloudRecord` 用例只接受本轮新建计划后、所选通道新增的非空 MP4 记录，播放与删除复用其 ID；录制等待中检查后端、ZLM 和托管 SIP Mock | Docker Desktop 真实 ZLM + ffmpeg SIP Mock + 临时 SQLite + Chromium：干净目录同步后 3/3 通过；本轮计划 3、流 `34020000001320000001_34020000001320000001`、记录 2、MP4 125446 字节，播放下载和删除均通过；健康检查单测 2/2。真机未覆盖 |
| M4 浏览器媒体与录像 | 新增可显式启用的 WebRTC 真实媒体用例，分别断言 SDP answer、ICE 连通和视频帧解码；FLV 用例结束时主动停止点播 | 本机真实 ZLM + ffmpeg SIP Mock + 临时 SQLite + Chromium：`E2E_REQUIRE_WEBRTC=1 npx playwright test tests/webrtcMedia.spec.ts` 1/1，ICE=`connected`、解码 3 帧、接收 6159 字节；`playUrls`、`cloudRecord`、`deviceControl` 关键用例 5/5，本轮云录像计划 1、新 MP4 记录 1、177512 字节，播放 Range 与删除通过；`web/npm run build` 通过。浏览器证据仍是 Mock 流，真机与生产拓扑待验收 |
| M4 同源静态部署冒烟 | GBServer 直接托管构建后的 `web/dist`，浏览器业务请求和静态资源经同一后端端口；版本出口保持 `2.0.0` | 基线 `6c4344d`、macOS、本机 debug 后端 + 临时 SQLite + ZLM `master/4b07053`（镜像 `sha256:d1c2ee83…`）；无 Vite，`E2E_BASE_URL=http://127.0.0.1:18080 npx playwright test tests/smoke.spec.ts --project=chromium` 20/20（健康、登录、17 个业务页）；`/login` 返回 200 且引用构建静态资源，`/api/server/version`、`/api/system/info`、OpenAPI 都为 `2.0.0`。这是本机同源拓扑冒烟，尚未验证目标服务器 Docker 部署 |
| M4 同源媒体与录像复验 | Playwright 媒体用例通过 `E2E_API_PREFIX=/api` 访问 GBServer，默认仍用 Vite 的 `/dev-api`；浏览器与业务 API 同源，无 Vite | 基线 `7533e13` 的后端 + 本机真实 ZLM + ffmpeg SIP Mock + 临时 SQLite + Chromium：`smoke`/`deviceControl` 21/21；`playUrls` 1/1（FLV 头有效、停止成功），WebRTC 1/1（ICE connected、解码 3 帧、6159 字节），`cloudRecord` 3/3（新计划 1、记录 1、MP4 125651 字节，Range 与删除通过）。曾因 Mock 关闭心跳而被判离线，恢复持续心跳后 WebRTC 通过。仍不是目标服务器 Docker 或真机证据 |
| M4 同源运维与控制复验 | `frontEnd`、`mediaServer`、`operations`、`syCamera` 用例也可选择 `/api`；媒体节点编辑时空 Secret 表示保留现有密钥，切换节点时清空未保存的密钥输入，新增节点仍要求输入 Secret | 本机后端 + ZLM + SIP Mock + 临时 SQLite + Chromium：初次合跑 9/11，媒体节点编辑因列表脱敏但表单强制 Secret 而无法发送保存请求，另 1 项串行跳过；修复后 11/11，媒体节点停用/启用落库，留空编辑后 ZLM 再探测可达；最终密钥输入清空回归 3/3。`web/npm run build` 通过。仅本机 Mock 环境，目标部署待验收 |
| M4 全量浏览器回归 | 直播页验收改为当前的通道点播、播放状态、抓图和停止；停止或切换通道时令旧 WebRTC 协商及播放器回调失效，避免已停止画面被迟到回调改成“播放失败” | 本机 GBServer 同源静态部署 + 真实 ZLM + ffmpeg SIP Mock（持续心跳、每 5 秒上报告警）+ 临时 SQLite：`E2E_BASE_URL=http://127.0.0.1:18080 E2E_API_PREFIX=/api E2E_REQUIRE_WEBRTC=1 E2E_SIP_MOCK_DEVICE_ID=34020000001320000001 npx playwright test --project=chromium` **72/72 通过、0 跳过**（3.2 分钟）；本轮新 MP4 117397 字节，WebRTC ICE connected、解码 3 帧。首轮 5 项失败中，4 项是旧直播 UI 断言和停止时的迟到 WebRTC 状态问题，1 项恰逢 Docker Desktop 重启导致 ZLM 连接中断；稳定环境复跑后均通过。`web/npm run build` 通过。真机与目标服务器仍待验收 |
| M4 Docker 方言构建契约 | 主 Compose 显式传 `postgres`，SQLite Compose 显式传 `sqlite`；Dockerfile 以对应 Cargo 特性构建，并在运行镜像安装健康检查所用的 `curl`；部署手册标明 `2.0.0` 与三方言镜像构建方式 | `docker compose config` 对 PostgreSQL/SQLite 的 build args 与数据库 URL 配对校验通过；本机 `cargo build --no-default-features --features postgres --locked` 通过。2026-10-18 主 Compose 的 PostgreSQL arm64 镜像构建成功，并用隔离 PostgreSQL 库启动验证；SQLite 镜像构建及目标服务器部署仍待验收 |
| M4 容器健康检查 | 运行镜像健康检查改用已安装的 `curl` 请求 `/api/health`，避免依赖未安装的 `wget` | Dockerfile 与 Compose 配置静态核对；2026-10-18 本机 arm64 容器启动后 `docker inspect` 报告 healthy，宿主映射端口 `/api/health` 返回 200；目标服务器部署仍待验收 |
| M4 PostgreSQL 同源运行冒烟 | 使用 PostgreSQL 特性构建的本机后端、隔离 PostgreSQL 数据库、Redis 和真实 ZLM，直接托管 `web/dist`，无 Vite | 隔离库由 `init-postgresql-2.7.4.sql` 初始化；Chromium `smoke.spec.ts` 20/20（健康、登录及业务页面）；管理员诊断 `/api/test/redis` 返回 `connected`，媒体节点列表 1 个在线，版本出口均为 `2.0.0`；测试后停止后端并删除隔离库。这是本机原生后端接容器依赖的拓扑，仍不等于 Docker 后端容器或目标服务器验收 |
| C9 收藏删除 | GET、DELETE 两种方法均进入鉴权，未登录返回 401；登录后 HTTP 对两种删除方法验证了收藏表与 `gb_cloud_record.collect` 一致清除。添加收藏也同时设置两套存储；数字 JSON `id` 可用，添加/删除中途失败均回滚 | SQLite 路由鉴权、登录后往返及两项事务失败注入测试通过；PostgreSQL 16、MySQL 8 初始化隔离库分别完成登录、添加、列表、GET 删除、再次添加、DELETE 数字 JSON 删除，最终两套存储均清空；测试库/容器已清理。当前云录像页面无收藏入口，浏览器页面验收不适用 |
| M2 命令应答路由 | `DeviceControl` 和 `DeviceConfig` 响应已映射；设备、CmdType、SN 不匹配或重复应答不完成 pending；回执携带的错误通道 ID 也被拒绝。命令与查询共用进程内唯一 SN，配置查询 pending 覆盖完整 15 秒等待，发送失败和超时后清理 | 定向测试与全量 Rust 测试通过；SQLite 后端 + SIP Mock 的 `ConfigDownload` 即时返回 `source=live`，延迟 11 秒仍在第 15 秒前收到实时回执（实测 11.01 秒）；真机闭环待验证 |
| M2 不完整回执防护 | 可等待的 `DeviceControl`、`DeviceConfig` 请求要求设备 `<Response>` 同时含有效目标 `DeviceID` 和非空 `Result`；缺字段应答不完成 pending，等待到期保持 `unconfirmed` | 定向测试先复现缺 `DeviceID` 即完成的错误，修复后覆盖两种命令、缺/空目标和缺/空结果，`pending_request` 测试 29/29；`cargo test --no-fail-fast --locked` 791 通过、0 失败、3 个文档测试忽略。本机后端 + SIP Mock 的传输、配置、重启、设防、录像 OK/ERROR/无回执以及重复、迟到场景仍通过；真机异常报文未覆盖 |
| M2 传输模式结果 | 新增进程内唯一 SN、发送失败清理 pending、传输模式 `waitResult=true` 的设备确认/拒绝/未确认状态；设备列表按状态提示 | 本机真实后端 + Python SIP Mock 的接口运行验证：确认、拒绝、仅 SIP 200 无设备回执、重复回执、5.5 秒迟到回执和新旧 SN 隔离均符合预期；其它控制入口和真机回执待做 |
| M2 设防/撤防结果 | 设防接口支持 `waitResult=true`，只接受 `SetGuard`/`ResetGuard`，返回 `sipSent` 与设备确认/拒绝/未确认状态；设备列表提供独立的设防、撤防动作，不再把在线状态当作已设防状态 | Rust 参数校验测试、前端类型检查与构建通过；真实后端 + SIP Mock HTTP/SIP 往返中，设防的确认、拒绝、无设备回执分别返回 `confirmed`、`rejected`、`unconfirmed`，且与传输模式请求 SN 不同。Chromium Playwright 设备列表用例 1/1 通过，覆盖在线/离线按钮及三种提示；真机与其它控制入口仍待验证 |
| M2 远程录像控制结果 | 远程录像命令等待设备执行回执；只有 `Result=OK` 才更新 StateStore 的录像状态并广播 `record_state`。拒绝或无回执的 HTTP 响应 `recording=null`，保留明确的 `deviceResult` | 真实后端 + SIP Mock HTTP/SIP 往返中，确认、拒绝、无回执分别返回 `confirmed`、`rejected`、`unconfirmed`，`StopRecord` 确认后返回 `recording=false`，SN 与其它命令不同；前端构建与全量 Rust 测试 781/781 通过。真机未验证 |
| M2 配置下发 XML | `BasicParam` 的文本值以及自定义 `ConfigType`、`ConfigData` 经过 XML 转义；保留原字段与默认值，避免 `&`、`<` 或带标签的 JSON 破坏 SIP 报文结构 | 两条回归测试先因缺少构造函数失败，修复后通过；`cargo test --no-fail-fast --locked` 789 通过、0 失败、3 个文档测试忽略。未对真实设备下发配置 |
| M2 配置下发结果 | `POST /api/device/config/update` 保持默认即时发送，新增 `waitResult=true` 在 5 秒内等待同设备、SN、`DeviceConfig` 的执行回执；成功响应增加 `sipSent` 和 `deviceResult`，保留原 `result` 字段；错误目标 ID 的配置应答不再完成 pending | 目标 ID 回归测试先失败后通过；本机临时 SQLite 后端 + SIP Mock：默认 `sent`，等待后的 OK=`confirmed`、ERROR=`rejected`、仅 SIP 200=`unconfirmed`，另覆盖重复及 5.5 秒迟到回执；`cargo test --no-fail-fast --locked` 790 通过、0 失败、3 个文档测试忽略。真机配置下发未验证 |
| M2 远程重启结果 | `GET /api/device/control/reboot` 保持默认即时返回，新增 `waitResult=true` 等待同设备、SN 的 `DeviceControl` 执行回执；响应增加 `sipSent`、`deviceResult`，保留原 `result` 字段 | 本机临时 SQLite 后端 + SIP Mock：测试先复现缺少结果字段；修复后默认 `sent`，等待后的 OK=`confirmed`、ERROR=`rejected`、仅 SIP 200=`unconfirmed`，重复及 5.5 秒迟到回执场景通过。真机重启未验证 |
| M2 预置位结果 | `GET /api/device/control/preset` 要求通道 ID、合法预置位动作和 1–255 编号，拒绝把未知动作兜底成 STOP；默认即时返回 `sent`，`waitResult=true` 等待同设备、通道、SN 的执行回执，保留原 `result` 字段 | 参数校验测试先复现缺通道仍进入发送路径，修复后通过；本机后端 + SIP Mock 对预置位确认、拒绝、仅 SIP 200、默认即时返回均通过；`cargo test --no-fail-fast --locked` 792 通过、0 失败、3 个文档测试忽略。真机预置位未验证 |
| M2 PTZ 与中亿视图预置位 | `/api/device/control/ptz` 在发送前校验动作和预置位编号；未知动作不再兜底成 STOP，`/api/sy/camera/control/ptz` 传入的预置位编号正确进入 PTZ XML；即时响应标明 SIP 已发送和设备结果仍为 `sent` | 参数回归测试先失败后通过；本机后端 + SIP Mock 实测中亿视图 `GOTO_PRESET&preset=3` 发出 `PTZCmd=A50F01820003003A`，控制回执完整场景通过；全量 Rust 测试 793 通过、0 失败、3 个文档测试忽略。真机 PTZ 未验证 |
| M2 通用通道 PTZ 参数 | `/api/common/channel/front-end/ptz` 仅接受已定义的 PTZ 动作，拒绝负数及超过 255 的速度；STOP 仍允许速度 0。无效参数在查库与发送 SIP 前返回错误，不会被兜底为 STOP 或截断成 `u8` | 参数定向测试及全量 Rust 测试 794 通过、0 失败、3 个文档测试忽略；尚未在真机验证该兼容入口 |
| M2 通用通道镜头与预置位 | 聚焦、光圈仅接受明确开关动作和 1–255 速度，实际编码使用请求速度；预置位设置、调用、删除要求 1–255 编号，响应只声明命令已发送，避免将未收到设备回执写成操作成功 | 指令字节和参数定向测试通过；全量 Rust 测试 796 通过、0 失败、3 个文档测试忽略。本机后端 + SIP Mock 先同步目录再用通道数据库 ID 发聚焦速度 40、调用预置位 3，Mock 分别收到 `A50F01422800001F`、`A50F01820003003A`；编号 256 被拒绝，既有控制回执正反场景全部通过。通用通道兼容入口尚未在真机验证 |
| M4 Catalog 多包同步 | 同步按 `SumNum` 声明的目录条目总数和唯一通道 ID 收齐，重复/乱序包不提前完成；收包数与目录条目数分开返回，未知总包数显示 `null`；旧 SN 响应不推进新会话或覆盖通道 | 定向测试先复现按包数统计导致 3 条目录在 2 包后不完成，修复后通过；本机后端 + SIP Mock 17 通道分 3 包，HTTP 返回 `done`、17/17 条目、3 包、无错误，原有控制回执场景同时通过。真实设备的分页/乱序/迟到报文仍待复验 |
| M4 心跳超时与恢复 | 隔离后端设 `GBSERVER__SIP__KEEPALIVE_TIMEOUT=3`、`GBSERVER__SIP__HEARTBEAT__TIMEOUT_MULTIPLIER=1`、`GBSERVER__SIP__HEARTBEAT__CHECK_INTERVAL_SECS=1`；无自动心跳的 SIP Mock 注册后超时置离线，离线控制请求拒绝，重新注册恢复在线 | `mock/tools/sip-device/verify_heartbeat_timeout_runtime.py` 对本机后端 + 临时 SQLite 实际运行通过：在线 → 超时离线 → PTZ 拒绝 → 重注册在线。仅是 Mock 与缩短的超时配置；真实设备及生产默认心跳时序待验证 |
| M4 主动停止点播 | `/api/play/stop` 在无活动会话或 BYE 发送失败时返回业务错误；主动停止会清理重连登记并短暂抑制旧播放器、缩略图的缺流 Hook，显式重新点播立即解除抑制；已终止会话不接受迟到响应或状态更新重新激活 | 本机真实 ZLM + SIP Mock：无会话停止返回错误，正常点播停止收到有效对话内 BYE；等待 11 秒覆盖缩略图重试窗口后再次停止返回无活动会话，未发第二次 BYE。`verify_play_stop_bye_runtime.py` 通过；仍需真机和目标部署复验 |
| C8 直播音频标志 | 点播响应优先以当前 ZLM 流的 ready 音频 track 判定 `hasAudio`，无 track 时回退目录字段，并以 `audioSource` 标明来源 | 音频 track 单测、前端构建及 `cargo test --no-fail-fast --locked`（774 通过、3 文档测试忽略）通过；本轮 ZLM + Mock 点播返回 `audioSource=media` 且 `hasAudio=true`，ZLM 同流 ready 音频 track；真机待复验 |
| M4 多方言运行 | PostgreSQL 与 MySQL 特性分别 `cargo build --no-default-features --features <dialect> --locked` 通过；独立测试库按对应初始化 SQL 建表，后端启动、登录、媒体节点/设备列表查询、收藏 GET 和 DELETE 往返均通过 | PostgreSQL 16 容器的隔离库与 MySQL 8 独立临时容器；各 31 张初始化表，测试后隔离库/容器已清理。证明本轮相关路径可运行，不等于全部业务及目标部署验收 |
| M4 平台通道跨方言更新 | MySQL 自定义名称/地址更新缺少 `UPDATE ... SET`，隔离库测试先复现 1064 错误，修复后通过；PostgreSQL 改用参数绑定；SQLite 共享分支的往返测试通过 | MySQL 8 与 PostgreSQL 16 初始化隔离库分别运行 `mysql_platform_channel`、`postgres_platform_channel` 的忽略测试；两列同时/分别更新及含单引号值通过。默认 SQLite 全量测试包含 `sqlite_custom_fields_roundtrip` |
| M3 管理员诊断 | `on_play`/`on_publish` 各保留最近一条白名单元数据；管理员可查 Hook 和 Redis/StateStore 状态。媒体节点列表和检测响应不返回保存的 ZLM secret；ZLM 代理要求管理员并拒绝原始配置读取，配置下发失败摘要不回显配置值 | 定向诊断及媒体节点测试通过，完整 `cargo test --no-fail-fast --locked` 777 通过、3 文档测试忽略；本机真实 Redis 容器停止时返回 `ping_failed`，恢复后 `connected`；真实 ZLM 容器停止时节点检测返回脱敏错误，恢复后 `reachable=true`。代理 GET 当前返回 405，POST 权限和配置拦截已有回归测试 |

本轮尚未接入真实 GB28181 设备；ZLM + Mock 及本机三方言的证据不能替代真机与目标部署拓扑验收。

> 📌 **最近更新：2026-09-19（真实设备接入核验轮，HEAD `8ecec6a`）**
> 本轮把第一台**真实国标设备**（EasyGBD，`34020000001320128497`，TCP）接进平台并做了端到端实测，
> 同时把 §1 的全部计数、§2 的能力矩阵、§6 的验证证据按当前代码/运行态重新实测。
> 逐轮修复过程记录（阶段性快照、修复叙述）已按「只保留最新状态」的要求清理，需要时走 git 历史。
>
> **补充（同日）**：项目转入**独立演进**，全仓清除外部实现（WVP / LiveGBS 等）的命名与引用，
> 并完成响应信封类型、收藏表名等实体改名（详见 §3 首条）。
>
> 本文档与 [`OPEN_ISSUES.md`](OPEN_ISSUES.md) 的分工：
> **本文档 = 已实现什么、为什么这么设计、已验证过什么**；**OPEN_ISSUES = 还没做什么**。

## 1. 规模基线（2026-09-19 实测 @ `8ecec6a`）

| 维度 | 数值 | 复现命令 |
|------|------|----------|
| 后端代码量 | 87,662 行 Rust | `find src -name '*.rs' \| xargs wc -l` |
| 已注册路由 | 430 条唯一 `/api/...` 路径（`router.rs` 425 处 `.route(`） | `grep -oE '"/api/[^"]*"' src/router.rs \| sort -u \| wc -l` |
| Handler 模块 | 31 个（`stub.rs` / `device_stub.rs` **是真实实现**，非兼容 shim，见 §3） | `grep -c 'pub mod' src/handlers/mod.rs` |
| 后端测试 | **750 通过 / 0 失败 / 3 忽略**（15 个测试二进制） | `cargo test --no-fail-fast` |
| 编译 | `cargo check` 0 error；clippy 272 条告警（2026-09-19 早测，本轮未重测） | `cargo check` / `cargo clippy --all-targets` |
| 前端 | 18 个业务视图目录（另有 `404.vue` / `redirect.vue` 两个顶层文件）、17 个 API 模块、13 个通用组件、42 个 SVG 图标、**174** 处 `url:` 字面量 | `find web/src/views -mindepth 1 -maxdepth 1 -type d \| wc -l`；`grep -oE "url[[:space:]]*:[[:space:]]*[\`'\"]" web/src/api/*.ts \| wc -l` |
| e2e | 17 个 spec（59 条 `test()` / 9 条 `test.describe()`）；最近一次运行 **35 通过 / 0 失败 / 0 跳过** | `ls e2e/tests/*.spec.ts \| wc -l`；`e2e/test-results/.last-run.json` |
| 旧 Vue 2 前端 | 已于 2026-09-19 删除，仅存 git 历史 | `git log --oneline -- web-legacy-vue2` |

## 2. 已实现能力矩阵

「真机」列 = 该模块**是否已被真实设备（EasyGBD `34020000001320128497`）实测覆盖**；
`—` 表示尚未用真机验证（不等于不可用）。注意 `stub.rs` / `device_stub.rs` 承载的条目
是**真实实现**（见 §3）。

| 模块 | 状态 | 真机 | 说明 |
|------|------|------|------|
| 用户 / 认证 | ✅ 完整 | — | JWT（`access-token` / `Authorization: Bearer`）+ API Key（`X-API-Key` / `apiKey`），审计日志异步落库 |
| API Key | ✅ 完整 | — | add / delete / enable / disable / remark / reset / list（前端**无管理界面**，见 [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) §4） |
| 角色 | ✅ 完整 | — | all / add / delete |
| 设备 CRUD / 接入 | ✅ 完整 | ✅ | 含统计、tree、status、channels；真机：401 摘要鉴权注册、DeviceInfo、Keepalive、目录同步、注销+重注册、传输模式切换 |
| 设备控制 | ✅ 完整 | ⚠️ 部分 | PTZ / 预置位 / 布防 / 录像 / 重启 / 批量；含扫描、巡航、雨刷、光圈、聚焦、拉框缩放。真机仅测了传输模式切换（设备回 `Result=OK`，但**平台未解析 DeviceControl 应答**，见 B14） |
| 设备配置查询 / 更新 | ✅ 完整 | — | 走 SIP 并**等待设备响应**（15s 超时，`handlers/device_control.rs:339`、`device_stub.rs:417`），非 fire-and-forget |
| 通道 | ✅ 完整 | ✅ | 含 civilCode、parent、地图瓦片、行业编码、网络标识；真机目录 1 通道入库（单包，未覆盖分页/乱序） |
| 直播 | ✅ 完整 | ✅ | start / stop / snap / ssrc / share / broadcast / webrtc。真机点播+FLV 实拉成功（见 §6） |
| 回放 | ✅ 完整 | — | start / stop / pause / resume / seek / speed |
| 云端录像 | ✅ 完整 | — | 计划 → 录制 → 落库 → 播放 → 删除全链路 |
| 推流 / 拉流代理 | ✅ 完整 | — | push + proxy + ffmpeg_cmd 启停 |
| 上级平台（级联） | ✅ 完整 | — | REGISTER 保活、目录/通道同步、级联推流、上级点播本级 |
| 媒体节点（ZLM） | ✅ 完整 | — | list / one / save / online / check / load / media_info / record_check；多节点最少负载选择 |
| 录像计划 | ✅ 完整 | — | 分钟 / ISO 星期口径、到点自动拉流录制 |
| 区域 / 业务分组 | ✅ 完整 | — | tree / path / addByCivilCode / sync；前端界面已补 |
| 日志 | ✅ 完整 | — | 列表 + 文件下载（三方言均可用） |
| 报警 | ✅ 完整 | — | list / before / detail / clear / handle / snap / device / batch / delete |
| 对讲 | ✅ 完整 | — | invite / start / stop / ack / bye / status / list；音频上行端到端验证过（假设备） |
| 语音广播 | ✅ 完整 | — | start / stop |
| RTP / PS | ✅ 完整 | — | send / receive + getTestPort |
| 移动位置 | ✅ 完整 | — | 订阅 + history 查询，打通两张位置表 |
| WebRTC | ✅ 可用 | ⚠️ | 直播页与通道播放对话框均已接入；GB28181 流已由用户在当前部署环境验证可正常播放。不同设备及网络拓扑仍建议在交付验收时覆盖。 |
| JT1078 车载终端 | ✅ 路由齐全 | — | 终端 / 通道 / 围栏（圆/多边形/矩形）/ 路线 / 位置 / 多媒体检索 / 录像下载；**GBServer 独有扩展**；真终端未测（D2） |
| 中亿视图（SY） | ✅ 完整 | — | 已实现 13 条 `/api/sy/*`（list / list/ids / list-with-child / list-for-mobile / cont-with-child / box / circle / polygon / address / meeting/list / control/{play,stop,ptz}）；第三方对接所需的另 10 条未实现（A1） |
| 系统 / 监控 | ✅ 完整 | — | `/api/system/info`、`/api/server/system/info`、`/metrics`（12 个指标族）、`/api/health`、`/api/ready` |

## 3. 关键设计决策

保留这些结论是为了避免后人重复论证同一个问题。

- **接口文档：代码优先的 OpenAPI（2026-09-19 新增）**。全部 **416** 条路由都由
  handler 上的 `#[utoipa::path]` 注解驱动：`routes!()` 一次注册同时产出 axum 路由与
  OpenAPI path，**结构上不可能「加了路由忘写文档」**。`/swagger-ui` 交互式文档（右上角
  Authorize 填 JWT / API Key 即可直接调用），`/api/openapi.json` 出 OpenAPI 3.1 规范。
  实现与运维见 [`OPENAPI_GUIDE.md`](OPENAPI_GUIDE.md)。
  - 受保护路由（403 条）挂 `auth_middleware` + 审计；公开路由（13 条：登录、health、
    ready、metrics、ZLM hook、RPC、分享播放、缩略图、WebSocket、对讲音频）不挂鉴权。
  - 覆盖门禁：`test_every_documented_path_is_routable` 遍历 spec 每条路径真实发请求并
    断言非 404 —— 文档与实现漂移、迁移丢路由都会在 CI 暴露。
  - 为此把 axum 从 0.7.9 升到 0.8.9（`utoipa-axum` 要求），84 条路由的路径参数
    语法 `:param` → `{param}`；`tower 0.5` / `tower-http 0.6`（http 1.x）。
- **项目自 2026-09-19 起独立演进，不再保留任何外部实现（WVP / LiveGBS 等）的命名与引用**。
  已完成的实体改名（**API 契约不变**，只是内部命名）：
  - 响应信封类型改名为 `ApiResult<T>`（`src/response.rs`，888 处调用点、前端
    `web/src/types/api.ts` 的 `ApiResult<T>`）；JSON 形状仍是 `{code,msg,data}`。
  - 设备配置查询/下发 handler 改为 `config_query_*` / `config_set_*`，DTO 改为
    `ConfigQueryParams`；告警批量删除改为 `alarm_delete_batch`。路由路径未变。
  - 收藏录像表改名为 `gb_record_collect`。**不做旧表兼容迁移**：全仓已无旧表名字面量，
    老部署升级后收藏列表为空（收藏无对外契约承诺）；需要保留数据时手工执行
    `ALTER TABLE <旧表名> RENAME TO gb_record_collect`。
  - Redis 录制态键只认 `gbserver:recording:*`，不再回退读旧命名空间（在途录制态丢失可由
    设备重新上报恢复）。
  - 全仓（代码 / 注释 / 文档 / 测试名）已无任何 WVP / LiveGBS 残留。
- **管理员判定集中在 `src/handlers/authz.rs`**（2026-09-19 新增）：`authority == "0"`
  为管理员、内置角色 `id = 1` 兜底，拒绝返回 `ErrorCode::Error403`。此前 `require_admin`
  内联在 `handlers/user.rs`、硬编码 `role_id == 1`，且只覆盖 4 个端点 —— 导致
  `/api/role/*`、`/api/userApiKey/*`、`/api/user/{users,all}` 任何已登录用户都能调，
  普通用户可自助创建管理员级角色。当前 `authz::require_admin` 有 **17 处调用**
  （`role.rs` 3 / `user.rs` 7 / `stub.rs` 7）。**新增管理类端点必须调用 `authz::require_admin`。**
- **用户列表查询用 `LEFT JOIN` 角色表**：`gb_user` 与 `gb_user_role` 之间没有外键约束，
  历史上删角色不校验引用会留下悬空 `role_id`；用 `INNER JOIN` 时这些用户会被 SQL
  静默丢出列表，而 `count_users` 仍计入 —— 表现为「共 N 条只有 N-k 行、翻页也找不回」。
- **`stub.rs` / `device_stub.rs` 是生产实现，不是待退役的兼容层**（2026-09-19 复核更正）：
  这两个文件名为 `stub`，但 64 个 entry **全部是真实实现**（落库 / 下发 SIP / 调 ZLM），
  没有空占位。其中 **48 条是当前 Vue 3 前端正在调用的活跃路径**。它们的自我描述
  「真实实现已迁出、仅挂旧路径」与实际不符 —— 实现从未迁出，函数体就在文件内。
  详见 [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md)。
- **JT1078 是本项目的独有扩展**：国标协议栈之外自行实现的车辆终端协议层；
  协议操作层 12 个端点均为真实下发并等待终端通用应答，已无「已受理」占位响应。
- **状态源统一到 `StateStore`**：`src/cache.rs` 与 `src/sip/gb28181/cascade_service.rs`
  已确认零生产调用并整体删除（后者 751 行）；级联代码现位于 `src/cascade/`、
  `src/sip/gb28181/cascade.rs`、`src/sip/gb28181/cascade_forward.rs`。
  负载均衡回退链为 `StateStore → ZLM 实时计数 → 首个节点`。
- **数据库三方言用一份 SQL 文本**：统一走 `dyn_where::dialect_sql()` 改写占位符，
  避免「改了这里忘了那里」。注意事项见 [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md)。
- **CI 自动触发保持关闭**：按用户要求只保留 `workflow_dispatch` 手动触发
  （`.github/workflows/ci.yml`），GitHub 层面也已 disable。
- **SIP 的 UDP/TCP 共用同一端口**（默认 5060）：国标设备用「同一端口 + 传输方式」
  接入，`sip.tcp_port` 默认等于 `sip.port`。**别再把它写成 5061** —— 那是旧文档
  遗留的错误值，照抄会让 TCP 设备注册不上。

## 4. 真实设备接入核验（2026-09-19，第一台真机）

| 项 | 值 |
|----|----|
| GB-ID | `34020000001320128497` |
| 名称 / 厂商 / 型号 / 固件 | EasyGBD-128497 / easygbd / `34020000001320128497` / V2.0 |
| 传输 | TCP，注册后由平台切到 `TCP-PASSIVE` |
| 地址 | 192.168.3.121:15060 |
| 通道 | 1 个（`34020000001310000001`，真机流为 **H264 1080×1920@26fps + G.711A(PCMA)**） |

**已核验通过**（逐条证据见 §6）：401 摘要注册与续期、注销（expires=0）后重注册、
Keepalive（30s）、注册后自动 DeviceInfo 查询并落库、目录同步、平台→设备延迟探针（200 OK）、
传输模式切换下发（设备回 `Result=OK`）、实时点播 + FLV 实拉 + 通道缩略图落盘、停止点播。

**真机上暴露、已登记为待办的问题**（详见 OPEN_ISSUES）：
B1（设备 200 OK 宣告的端口 ≠ ZLM RTP 端口时 `connectRtpServer` 被 ZLM 拒绝）、
B6（`play_stop` 把 BYE 发给了**已终止的旧会话**）、
B14（设备 `DeviceControl` 应答未被解析）、
C8（通道 `hasAudio` 与实际流不符：流里有 G.711A，接口却报 `hasAudio=false`）。

**真机尚未覆盖**：PTZ / 布防 / 录像 / 重启 / 配置查询、回放与下载、对讲与语音广播、
目录分页（本机 `SumNum=1` 单包）、设备主动 BYE、心跳超时下线、JT1078 真终端。

## 5. 前端 ↔ 后端契约审计结论

2026-09-12 对 **16 个 API 模块 / 130 条契约**做了逐条审计（前端 `web/src/api/` 的调用
形态与本仓库 router / DTO / 响应键名逐字段比对），逐模块结论如下。审计原文已归档至 git 历史。

| 模块 | 条目 | 结论 |
|------|------|------|
| `channel.ts` | 8 | ✅ 已修 |
| `alarm.ts` | 10 | ✅ 已修 |
| `live.ts` | 6 | ✅ 已修 |
| `user.ts` | 6 | ✅ 已修（`query` 搜索 / 权限缺口 / 悬空角色，见 OPEN_ISSUES C5–C7） |
| `cloudRecord.ts` | 14 | ✅ 已修（连带修掉 5 个审计未覆盖的深层缺陷） |
| `device.ts` | 6 | ✅ 已修 |
| `jtDevice.ts` | 13 | ✅ 已修（连带修掉 TEXT 列声明成 `Option<i32>` 的 500） |
| `log.ts` | 7 | ✅ 已修（postgres `serial` vs `i64` 的 500） |
| `mediaServer.ts` | 7 | ✅ 已修（连带修掉心跳时间戳格式导致的「节点全部误判离线」） |
| `platform.ts` | 11 | ✅ 已修（`serverGBDomain` 曾错填成 `sip.realm`） |
| `playback.ts` | 3 | ✅ 已修 |
| `region.ts` | 8 | ✅ 已修（并补上整个模块缺失的前端界面） |
| `streamProxy.ts` | 10 | ✅ 已修 |
| `streamPush.ts` | 12 | ✅ 已修 |
| `syCamera.ts` | 6 | ✅ 已修 |
| `talk.ts` | 1 | ✅ 已修 |

## 6. 已验证过什么（证据类型）

说明这些能力**有实测证据**，不是「路由挂载即认为可用」：

- **真机闭环（2026-09-19，第一台真实国标设备）**：
  - 注册链路：`REGISTER → 401 Challenge → 200`（expires 3600）、注销（expires=0）后 25s 内重注册；
  - 注册后**自动 DeviceInfo 查询**并落库（name/manufacturer/model/firmware 均有值）；
  - 目录同步：`SumNum=1`、1 包、1 通道入库；
  - 延迟探针：每 15s 一条 MESSAGE，设备回 200 OK 结算 RTT；
  - 实时点播：`INVITE → 100 → 200 OK → ACK`，ZLM 出流后**FLV 实拉 3,291,903 字节 / 6s**，
    `ffprobe` 识别为 **H264 1080×1920 + pcm_alaw**；缩略图落盘
    `data/snapshots/34020000001320128497_34020000001310000001.jpg`（98,327 字节）；
  - 停止点播：ZLM 资源与流列表均清空。
- **核心闭环（模拟设备 + 真实 ZLM）**：历史回放 + 回放控制、对讲（假设备收到 RTP 包）、
  语音广播、设备控制 5 项、JT1078 终端/通道、级联平台、云录像全链路。
- **三方言运行时验证**：PostgreSQL 与 MySQL 都真机跑过全量冒烟，抓到并修掉
  数十处只在非 SQLite 上炸的缺陷（占位符语法、列类型、`INSERT ... RETURNING`、
  `CAST` 等）。
- **真实 ZLM 集成**：hook 全链路、流列表、录制文件落盘/删除、多节点负载均衡。
- **测试基线（2026-09-19 实测）**：`cargo test --no-fail-fast` **750 通过 / 0 失败 / 3 忽略**。
- **e2e 体系**：Playwright 17 个 spec / 59 条 `test()`，覆盖设备/通道/直播/回放/云录像/
  推流/代理/平台/区域/媒体节点/终端/日志/报警/控制台等页面；最近一次运行
  **35 通过 / 0 失败**（`e2e/test-results/.last-run.json` 为 `status: passed`）。

> ⚠️ e2e 最近一次运行是**筛选后的 35 条**，不是 59 条全量；**三方言冒烟与全量 e2e
> 在本轮未重跑**。上表中非真机部分为历史验证结论。

## 7. 相关文档

| 文档 | 用途 |
|------|------|
| [`ROADMAP_2.0.md`](ROADMAP_2.0.md) | 2.0.0 迭代顺序、范围与验收条件 |
| [`OPEN_ISSUES.md`](OPEN_ISSUES.md) | **未完成 / 未验证事项**（唯一待办表） |
| [`DEPLOYMENT_GUIDE.md`](DEPLOYMENT_GUIDE.md) | 构建、运行、部署分级、配置、监控、灾备、升级 |
| [`DB_DIALECT_NOTES.md`](DB_DIALECT_NOTES.md) | 写多方言 SQL 的注意事项 |
| [`STUB_COMPAT_PLAN.md`](STUB_COMPAT_PLAN.md) | `stub.rs` / `device_stub.rs` 的真实定位与清退评估（当前不建议清退） |
| [`../web/README.md`](../web/README.md) | 前端说明 |
| [`../e2e/README.md`](../e2e/README.md) | 端到端测试说明 |
| [`../mock/README.md`](../mock/README.md) | 模拟测试资源 |
