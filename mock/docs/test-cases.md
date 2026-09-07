# GBServer 详细测试用例

> 与 [TEST_PLAN.md](../TEST_PLAN.md) 配套，按模块列出每个用例的：目的、输入、预期、模拟工具、验收标准。

---

## 附录 A：已知占位 / Stub 端点

以下端点当前在 `src/handlers/stub.rs` 与 `device_stub.rs` 中实现，**仅返回兼容性空响应**。测试只验证响应**结构**与**HTTP 200**，不验证业务正确性。

### A.1 `device_stub.rs`（设备操作占位）

| 端点 | 当前行为 | 测试期望 |
|------|----------|----------|
| `GET /api/device/query/sync_status` | 返回 `{code:0, data:{...}}` | 结构正确 |
| `DELETE /api/device/query/devices/:id/delete` | 不删除 | 返回 200 + `data: {...}` |
| `GET /api/device/query/devices/:id/sync` | 不触发 | 返回 200 |
| `POST /api/device/query/transport/:id/:mode` | 不切换 | 返回 200 |
| `GET /api/device/query/subscribe/mobile-position` | 不订阅 | 返回 200 |
| `GET /api/device/query/channel/one` | 返回空 | `code:0, data:null` 或 `[]` |
| `GET /api/device/query/streams` | 返回空 | `code:0, data:[]` |
| `GET /api/device/query/subscribe/alarm` | 不订阅 | 返回 200 |
| `GET /api/device/control/record` | 不触发 | 返回 200 |
| `GET /api/device/query/sub_channels/:d/:p/channels` | 返回空 | `code:0, data:[]` |
| `GET /api/device/query/tree/channel/:d` | 返回空 | `code:0, data:[]` |
| `POST /api/device/query/channel/audio` | 不操作 | 返回 200 |
| `POST /api/device/query/channel/stream/identification/update/` | 不操作 | 返回 200 |
| `POST /api/device/query/device/update` | 不更新 | 返回 200 |
| `POST /api/device/query/device/add` | 不添加 | 返回 200 |
| `GET /api/device/query/devices/:id` | 返回空 | `code:0, data:null` |
| `GET /api/device/query/tree/:id` | 返回空 | `code:0, data:[]` |

### A.2 `stub.rs`（其他域占位）

| 端点 | 当前行为 | 测试期望 |
|------|----------|----------|
| `GET /api/role/all` | 返回空 | `code:0, data:[]` |
| `GET /api/common/channel/list` | 返回空 | `code:0, data:[]` |
| `GET /api/region/tree/list` | 返回空 | `code:0, data:[]` |
| `POST /api/region/add` | 不添加 | 返回 200 |
| `POST /api/region/update` | 不更新 | 返回 200 |
| `DELETE /api/region/delete` | 不删除 | 返回 200 |
| `GET /api/region/path` | 返回空 | `code:0, data:[]` |
| `GET /api/region/tree/query` | 返回空 | `code:0, data:[]` |
| `GET /api/region/description` | 返回空 | `code:0, data:[]` |
| `GET /api/region/addByCivilCode` | 不添加 | 返回 200 |
| `GET /api/region/queryChildListInBase` | 返回空 | `code:0, data:[]` |
| `GET /api/region/base/child/list` | 返回空 | `code:0, data:[]` |
| `GET /api/group/tree/list` | 返回空 | `code:0, data:[]` |
| `POST /api/group/add` | 不添加 | 返回 200 |
| `POST /api/group/update` | 不更新 | 返回 200 |
| `DELETE /api/group/delete` | 不删除 | 返回 200 |
| `GET /api/group/path` | 返回空 | `code:0, data:[]` |
| `GET /api/group/tree/query` | 返回空 | `code:0, data:[]` |
| `GET /api/log/list` | 返回空 | `code:0, data:[]` |
| `GET /api/log/file/:name` | 返回 404 | `code != 0` |
| `GET /api/userApiKey/*` | 返回空 / 200 | 结构正确 |
| `GET /api/cloud/record/*` | 返回空 / 200 | 结构正确 |
| `GET /api/record/plan/*` | 返回空 / 200 | 结构正确 |
| `GET /api/position/history/:id` | 返回空 | `code:0, data:[]` |

---

## A. 鉴权（JWT / API Key）

### A.1 登录

| 用例 | 输入 | 预期 |
|------|------|------|
| 正确密码 | `POST /api/user/login {username:admin, password:21232f297a57a5a743894a0e4a801fc3}` | 200 + `code:0` + JWT |
| 错误密码 | 同上但 password 错误 | 200 + `code != 0` |
| 空 username | `{username:"", password:""}` | 200 + `code != 0` |
| 用户名不存在 | `{username:nouser, password:...}` | 200 + `code != 0` |

**fixture**：[http-requests/login.json](../fixtures/http-requests/login.json)
**响应**：[http-responses/login.json](../fixtures/http-responses/login.json)

### A.2 JWT 鉴权

| 用例 | 请求头 | 预期 |
|------|--------|------|
| 正确 JWT | `access-token: <有效 JWT>` | 200 |
| 过期 JWT | 同上但 expired | 401 |
| 篡改 JWT | 修改 payload | 401 |
| 缺失 | （无 token） | 401 |
| 通过 Authorization | `Authorization: Bearer <JWT>` | 200 |

### A.3 API Key

| 用例 | 请求头 | 预期 |
|------|--------|------|
| 正确 Key | `X-API-Key: <有效>` | 200 |
| `apiKey` query 参数 | `?apiKey=<值>` | 200 |
| 无效 Key | 错误 key | 401 |
| 禁用 Key | 已 disable | 401 |

### A.4 登出

| 用例 | 预期 |
|------|------|
| `GET /api/user/logout` | 200 + token 失效（审计日志记录） |

---

## B. 设备

### B.1 注册与注销

| 用例 | 模拟工具 | 预期 |
|------|----------|------|
| SIP mock 自动注册到 GBServer | `sip_device_mock.py` | GBServer 日志显示 "设备已注册"，`/api/device/query/devices` 出现该设备 |
| 注册后 401 触发 Digest 重试 | `sip_device_mock.py` | mock 端日志显示 "401 Unauthorized" → "REGISTER (with Digest) sent" → "200 OK" |
| 注销（Expires: 0） | `sip_device_mock.py --auto-register` 启动后改 mock 行为 | GBServer 端设备标记 offline |
| 重复注册（相同 deviceId） | 两个 mock 进程用相同 deviceId | GBServer 后注册覆盖前者 |

### B.2 心跳与超时

| 用例 | 模拟工具 | 预期 |
|------|----------|------|
| 正常心跳 | `--auto-keepalive 30` | GBServer `keepaliveTime` 持续更新 |
| 心跳停止后超时 | 手动 kill mock 进程，等 90 秒 | 设备置为 offline |
| 心跳间隔配置 | 改 config `[sip] heartbeat.timeout_multiplier` | 超时阈值 = 间隔 × 倍数 |

### B.3 目录查询

| 用例 | 模拟工具 | 预期 |
|------|----------|------|
| 单包 Catalog | `--channels 4` | GBServer 解析出 4 个通道 |
| 多包 Catalog（8 个/包） | `--channels 32` | GBServer 解析出 32 个通道；中间不丢包 |
| 大数量 Catalog（500） | `--channels 500` | 500 个通道全部入库 |

### B.4 设备信息查询

| 用例 | 模拟工具 | 预期 |
|------|----------|------|
| DeviceInfo 应答 | 默认 | mock 响应 DeviceInfo XML |
| DeviceStatus 应答 | 默认 | mock 响应 DeviceStatus XML |

### B.5 设备 CRUD（HTTP）

| 用例 | 请求 | 预期 |
|------|------|------|
| 设备分页查询 | `POST /api/device/query/devices {page:1, count:10}` | 200 + `code:0` + 设备列表 |
| 设备通道查询 | `GET /api/device/query/devices/:id/channels` | 200 + 通道列表 |
| 保活统计 | `GET /api/device/query/statistics/keepalive` | 200 + 统计 |
| 注册统计 | `GET /api/device/query/statistics/register` | 200 + 统计 |

### B.6 设备控制

| 用例 | 请求 | 预期 |
|------|------|------|
| PTZ 控制 | `GET /api/device/control/ptz?deviceId=...&command=...` | 200 + SIP INFO 发送到 mock（mock 日志显示 "PTZ INFO 收到"） |
| 预置位 | `GET /api/device/control/preset?deviceId=...&presetCmd=...` | 200 |
| 重启 | `GET /api/device/control/reboot?deviceId=...` | 200 |
| 看守 | `GET /api/device/control/guard?deviceId=...&cmd=...` | 200 |
| 配置查询 | `GET /api/device/config/query?deviceId=...` | 200 |
| 配置更新 | `POST /api/device/config/update` | 200 |
| 目录订阅 | `GET /api/device/query/subscribe/catalog?deviceId=...` | 200 + mock 收到 SUBSCRIBE |

---

## C. 流媒体服务器

### C.1 列表与查询

| 用例 | 请求 | 预期 |
|------|------|------|
| 列表 | `GET /api/server/media_server/list` | 200 + 包含 ZLM mock（id=zlmediakit-mock-1） |
| 在线列表 | `GET /api/server/media_server/online/list` | 200 + 在线节点 |
| 单节点 | `GET /api/server/media_server/one/zlmediakit-mock-1` | 200 + 节点详情 |
| 健康检查 | `GET /api/server/media_server/check` | 200 |
| 录像检查 | `GET /api/server/media_server/record/check` | 200 |
| 系统配置 | `GET /api/server/system/configInfo` | 200 |
| 系统信息 | `GET /api/server/system/info` | 200 |
| 地图配置 | `GET /api/server/map/config` | 200 |
| 服务器信息 | `GET /api/server/info` | 200 |
| 资源信息 | `GET /api/server/resource/info` | 200 |
| 全流列表 | `GET /api/server/stream/all` | 200 |

### C.2 配置管理

| 用例 | 请求 | 预期 |
|------|------|------|
| 保存 | `POST /api/server/media_server/save {id, ip, port, secret, ...}` | 200 + 数据库新增 / 更新 |
| 删除 | `DELETE /api/server/media_server/delete?id=...` | 200 |
| 节点负载 | `GET /api/server/media_server/load` | 200 |

---

## D. 推流 / 拉流代理

### D.1 推流

| 用例 | 请求 | 预期 |
|------|------|------|
| 列表 | `GET /api/push/list` | 200 + 推流列表 |
| 添加 | `POST /api/push/add {app, stream, ...}` | 200 |
| 更新 | `POST /api/push/update` | 200 |
| 启动 | `GET /api/push/start?id=...` | 200 |
| 停止 | `GET /api/push/remove?id=...` | 200 |
| 批量删除 | `DELETE /api/push/batchRemove` | 200 |
| GB28181 关联 | `POST /api/push/save_to_gb` | 200 |
| 取消关联 | `DELETE /api/push/remove_form_gb` | 200 |

### D.2 拉流代理

| 用例 | 请求 | 预期 |
|------|------|------|
| 列表 | `GET /api/proxy/list` | 200 |
| FFmpeg 命令列表 | `GET /api/proxy/ffmpeg_cmd/list` | 200 |
| 添加 | `POST /api/proxy/add {url, stream, ...}` | 200 + ZLM mock 收到 `addStreamProxy` |
| 更新 | `POST /api/proxy/update` | 200 |
| 保存 | `POST /api/proxy/save` | 200 |
| 启动 | `GET /api/proxy/start?id=...` | 200 |
| 停止 | `GET /api/proxy/stop?id=...` | 200 |
| 删除 | `DELETE /api/proxy/delete?id=...` | 200 |

**验证**：调用 `curl -s 'http://127.0.0.1:8080/index/api/getMediaList?secret=demo'` 应能看到新增的代理。

---

## E. 级联平台

### E.1 CRUD

| 用例 | 请求 | 预期 |
|------|------|------|
| 添加 | `POST /api/platform/add {serverGBId, serverIp, serverPort, ...}` | 200 + 数据库新增 |
| 列表 | `GET /api/platform/query` | 200 |
| 单个 | `GET /api/platform/info/:id` | 200 |
| 服务配置 | `GET /api/platform/server_config` | 200 |
| 更新 | `POST /api/platform/update` | 200 |
| 删除 | `DELETE /api/platform/delete?id=...` | 200 |
| 退出 | `GET /api/platform/exit/:device_gb_id` | 200 |

### E.2 通道管理

| 用例 | 请求 | 预期 |
|------|------|------|
| 列表 | `GET /api/platform/channel/list` | 200 |
| 推送 | `GET /api/platform/channel/push` | 200 |
| 添加 | `POST /api/platform/channel/add` | 200 |
| 设备添加 | `POST /api/platform/channel/device/add` | 200 |
| 设备移除 | `POST /api/platform/channel/device/remove` | 200 |
| 通道移除 | `DELETE /api/platform/channel/remove` | 200 |
| 自定义更新 | `POST /api/platform/channel/custom/update` | 200 |

### E.3 目录管理

| 用例 | 请求 | 预期 |
|------|------|------|
| 添加目录 | `POST /api/platform/catalog/add` | 200 |
| 编辑目录 | `POST /api/platform/catalog/edit` | 200 |

**验证**：添加平台后，GBServer 会向 mock 级联平台发送 REGISTER；mock 端日志显示已收到。

---

## F. 实时播放

### F.1 播放控制

| 用例 | 请求 | 预期 |
|------|------|------|
| 启动 | `GET /api/play/start/:device_id/:channel_id` | 200 + 返回 streamId + playUrl + SIP INVITE 发送到 mock |
| 停止 | `GET /api/play/stop/:device_id/:channel_id` | 200 + BYE |
| 广播启动 | `GET /api/play/broadcast/:device_id/:channel_id` | 200 |
| 广播停止 | `GET /api/play/broadcast/stop/:device_id/:channel_id` | 200 |
| WebRTC 播放 | `POST /api/play/webrtc` | 200 |

**验证**：`curl -s http://127.0.0.1:8080/index/api/getMediaList?secret=demo` 应能看到播放流。

### F.2 分享

| 用例 | 请求 | 预期 |
|------|------|------|
| 创建分享 | `GET /api/play/share?...` | 200 + token |
| 分享信息 | `GET /api/play/share/info?token=...` | 200 |
| 启动分享 | `GET /api/play/share/start?token=...` | 200 |

---

## G. 回放 / 录像

### G.1 回放控制

| 用例 | 请求 | 预期 |
|------|------|------|
| 启动 | `GET /api/playback/start/:d/:c?start=...&end=...` | 200 |
| 暂停 | `GET /api/playback/pause/:stream_id` | 200 |
| 恢复 | `GET /api/playback/resume/:stream_id` | 200 |
| 拖动 | `GET /api/playback/seek/:stream_id/:seek_time` | 200 |
| 倍速 | `GET /api/playback/speed/:stream_id/:speed` | 200 |
| 停止 | `GET /api/playback/stop/:d/:c/:stream_id` | 200 |

### G.2 录像查询

| 用例 | 请求 | 预期 |
|------|------|------|
| 录像列表 | `GET /api/gb_record/query/:d/:c?start=...&end=...` | 200 + 录像段列表（依赖 mock RecordInfo 响应） |
| 下载启动 | `GET /api/gb_record/download/start/:d/:c?start=...&end=...` | 200 |
| 下载停止 | `GET /api/gb_record/download/stop/:d/:c/:stream_id` | 200 |
| 下载进度 | `GET /api/gb_record/download/progress/:d/:c/:stream_id` | 200 |

---

## H. 云录像

| 用例 | 请求 | 预期 |
|------|------|------|
| 播放路径 | `GET /api/cloud/record/play/path?...` | 200 |
| 日期列表 | `GET /api/cloud/record/date/list` | 200 |
| 加载录像 | `GET /api/cloud/record/loadRecord` | 200 |
| 拖动 | `GET /api/cloud/record/seek` | 200 |
| 倍速 | `GET /api/cloud/record/speed` | 200 |
| 任务添加 | `GET /api/cloud/record/task/add` | 200 |
| 任务列表 | `GET /api/cloud/record/task/list` | 200 |
| 删除 | `DELETE /api/cloud/record/delete` | 200 |
| 列表 | `GET /api/cloud/record/list` | 200 |
| 收藏 | `GET /api/cloud/record/collect/{add,delete,list}` | 200 |
| 公开 | `GET /api/cloud/record/download/zip` | 200 |
| ZIP 下载 | `GET /api/cloud/record/zip` | 200 |
| 列表 URL | `GET /api/cloud/record/list-url` | 200 |

**验证**：触发 ZLM mock `on_record_mp4` 后，GBServer 应处理 Webhook；查询 `/api/cloud/record/list` 应出现对应录像。

---

## I. 前端控制（PTZ / 预置位 / 巡航 / 扫描）

| 用例 | 请求 | 预期 |
|------|------|------|
| PTZ 转动 | `GET /api/front-end/ptz/:d/:c?command=...` | 200 + SIP INFO 到 mock |
| 辅助开关 | `GET /api/front-end/auxiliary/:d/:c` | 200 |
| 雨刷 | `GET /api/front-end/wiper/:d/:c` | 200 |
| 光圈 | `GET /api/front-end/fi/iris/:d/:c` | 200 |
| 聚焦 | `GET /api/front-end/fi/focus/:d/:c` | 200 |
| 预置位查询 | `GET /api/front-end/preset/query/:d/:c` | 200 |
| 预置位添加 | `GET /api/front-end/preset/add/:d/:c` | 200 |
| 预置位调用 | `GET /api/front-end/preset/call/:d/:c` | 200 |
| 预置位删除 | `GET /api/front-end/preset/delete/:d/:c` | 200 |
| 巡航点增删 | `GET /api/front-end/cruise/point/{add,delete}/:d/:c` | 200 |
| 巡航速度 | `GET /api/front-end/cruise/speed/:d/:c` | 200 |
| 巡航时间 | `GET /api/front-end/cruise/time/:d/:c` | 200 |
| 巡航启停 | `GET /api/front-end/cruise/{start,stop}/:d/:c` | 200 |
| 扫描设置 | `GET /api/front-end/scan/set/{speed,left,right}/:d/:c` | 200 |
| 扫描启停 | `GET /api/front-end/scan/{start,stop}/:d/:c` | 200 |
| 旧命令 | `POST /api/ptz/front_end_command/:d/:c` | 200 |

同样的命令在 `common_channel/front-end/*` 下也有重复实现，行为应一致。

---

## J. 区域 / 分组 / 用户 / 角色 / API Key

| 用例 | 请求 | 预期 |
|------|------|------|
| 区域查询 | `GET /api/region/one?id=...` | 200 |
| 区域分页 | `GET /api/region/page/list?page=...&count=...` | 200 |
| 区域同步 | `GET /api/region/sync` | 200 |
| 用户列表 | `GET /api/user/users` | 200 |
| 用户添加 | `POST /api/user/add` | 200 |
| 用户删除 | `DELETE /api/user/delete?id=...` | 200 |
| 修改密码 | `POST /api/user/changePassword` | 200 |
| 管理员重置密码 | `POST /api/user/changePasswordForAdmin` | 200 |
| 推送 Key | `POST /api/user/changePushKey` | 200 |
| 角色列表 | `GET /api/role/all` | 200（占位返回空） |
| 角色添加 | `POST /api/role/add` | 200 |
| 角色删除 | `DELETE /api/role/delete` | 200 |
| API Key 添加 | `POST /api/userApiKey/add` | 200 |
| API Key 列表 | `GET /api/userApiKey/userApiKeys` | 200 |
| API Key 启用 | `POST /api/userApiKey/enable` | 200 |
| API Key 禁用 | `POST /api/userApiKey/disable` | 200 |
| API Key 重置 | `POST /api/userApiKey/reset` | 200 |
| API Key 删除 | `DELETE /api/userApiKey/delete` | 200 |
| API Key 备注 | `POST /api/userApiKey/remark` | 200 |

---

## K. 报警

| 用例 | 请求 | 预期 |
|------|------|------|
| 报警列表 | `GET /api/alarm/list?page=1&count=10` | 200 + 报警列表 |
| 报警详情 | `GET /api/alarm/detail/:id` | 200 |
| 报警处理 | `POST /api/alarm/handle` | 200 |
| 报警删除 | `DELETE /api/alarm/delete/:id` | 200 |
| 批量删除 | `DELETE /api/alarm/batch` | 200 |
| 设备维度删除 | `DELETE /api/alarm/device/:device_id` | 200 |
| 时间维度删除 | `DELETE /api/alarm/before/:time` | 200 |
| 清空 | `DELETE /api/alarm/clear` | 200 |
| 抓图 | `GET /api/alarm/snap/:param` | 200 |

**验证**：JT1078 mock 触发报警后，`/api/alarm/list` 应包含该报警。

---

## L. JT1078 部标终端

### L.1 终端管理

| 用例 | 请求 | 预期 |
|------|------|------|
| 终端列表 | `GET /api/jt1078/terminal/list` | 200 |
| 终端查询 | `GET /api/jt1078/terminal/query?phone=...` | 200 |
| 终端添加 | `POST /api/jt1078/terminal/add` | 200 |
| 终端更新 | `POST /api/jt1078/terminal/update` | 200 |
| 终端删除 | `DELETE /api/jt1078/terminal/delete` | 200 |

### L.2 通道管理

| 用例 | 请求 | 预期 |
|------|------|------|
| 通道列表 | `GET /api/jt1078/terminal/channel/list` | 200 |
| 通道单个 | `GET /api/jt1078/terminal/channel/one/:id` | 200 |
| 通道添加 | `POST /api/jt1078/terminal/channel/add` | 200 |
| 通道更新 | `POST /api/jt1078/terminal/channel/update` | 200 |
| 通道删除 | `DELETE /api/jt1078/terminal/channel/delete/:id` | 200 |

### L.3 实时 / 回放 / 下载

| 用例 | 请求 | 预期 |
|------|------|------|
| 实时启动 | `GET /api/jt1078/live/start?phone=...&channel=...` | 200 |
| 实时停止 | `GET /api/jt1078/live/stop` | 200 |
| 回放启动 | `GET /api/jt1078/playback/start?...` | 200 |
| 回放控制 | `GET /api/jt1078/playback/control` | 200 |
| 回放下载 URL | `GET /api/jt1078/playback/downloadUrl` | 200 |
| 回放下载 | `GET /api/jt1078/playback/download` | 200 |
| 录像列表 | `GET /api/jt1078/record/list` | 200 |
| 录像启动 | `GET /api/jt1078/record/start` | 200 |
| 录像停止 | `GET /api/jt1078/record/stop` | 200 |

### L.4 PTZ / 雨刷 / 灯光 / 控制

| 用例 | 请求 | 预期 |
|------|------|------|
| PTZ | `GET /api/jt1078/ptz?phone=...&channel=...&command=...` | 200 |
| 雨刷 | `GET /api/jt1078/wiper` | 200 |
| 补光 | `GET /api/jt1078/fill-light` | 200 |
| 工厂复位 | `POST /api/jt1078/control/factory-reset` | 200 |
| 复位 | `POST /api/jt1078/control/reset` | 200 |
| 控制连接 | `POST /api/jt1078/control/connection` | 200 |
| 车门 | `GET /api/jt1078/control/door` | 200 |

### L.5 配置 / 状态

| 用例 | 请求 | 预期 |
|------|------|------|
| 配置查询 | `GET /api/jt1078/config/get` | 200 |
| 配置设置 | `POST /api/jt1078/config/set` | 200 |
| 终端属性 | `GET /api/jt1078/attribute` | 200 |
| 链路检测 | `GET /api/jt1078/link-detection` | 200 |
| 位置查询 | `GET /api/jt1078/position-info` | 200 |
| 文本下发 | `POST /api/jt1078/text-msg` | 200 + mock 收到 |
| 电话回呼 | `GET /api/jt1078/telephone-callback` | 200 |
| 驾驶员信息 | `GET /api/jt1078/driver-information` | 200 |
| 媒体属性 | `GET /api/jt1078/media/attribute` | 200 |
| 媒体列表 | `POST /api/jt1078/media/list` | 200 |
| 设置电话本 | `POST /api/jt1078/set-phone-book` | 200 |
| 拍照 | `POST /api/jt1078/shooting` | 200 |
| 临时位置追踪 | `GET /api/jt1078/control/temp-position-tracking` | 200 |
| 报警确认 | `POST /api/jt1078/confirmation-alarm-message` | 200 |
| 对讲启动 | `GET /api/jt1078/talk/start` | 200 |
| 对讲停止 | `GET /api/jt1078/talk/stop` | 200 |

### L.6 区域 / 路线

| 用例 | 请求 | 预期 |
|------|------|------|
| 圆形区域添加 | `POST /api/jt1078/area/circle/add` | 200 |
| 圆形区域编辑 | `POST /api/jt1078/area/circle/edit` | 200 |
| 圆形区域删除 | `GET /api/jt1078/area/circle/delete` | 200 |
| 圆形区域查询 | `GET /api/jt1078/area/circle/query` | 200 |
| 圆形区域更新 | `POST /api/jt1078/area/circle/update` | 200 |
| 多边形区域设置 | `POST /api/jt1078/area/polygon/set` | 200 |
| 多边形区域删除 | `GET /api/jt1078/area/polygon/delete` | 200 |
| 多边形区域查询 | `GET /api/jt1078/area/polygon/query` | 200 |
| 矩形区域添加 | `POST /api/jt1078/area/rectangle/add` | 200 |
| 矩形区域编辑 | `POST /api/jt1078/area/rectangle/edit` | 200 |
| 矩形区域删除 | `GET /api/jt1078/area/rectangle/delete` | 200 |
| 矩形区域查询 | `GET /api/jt1078/area/rectangle/query` | 200 |
| 矩形区域更新 | `POST /api/jt1078/area/rectangle/update` | 200 |
| 路线设置 | `POST /api/jt1078/route/set` | 200 |
| 路线查询 | `GET /api/jt1078/route/query` | 200 |
| 路线删除 | `GET /api/jt1078/route/delete` | 200 |

### L.7 重传检测

| 用例 | 验证 |
|------|------|
| 启动终端 `--simulate-loss 0.10` | mock 端日志显示 "模拟丢包" |
| GBServer 收到缺失序列号 | GBServer 发出 0x0005 重传请求 |
| mock 收到重传请求 | mock 端日志显示 "重传消息: msg_id=0x... seq=..." |
| 重传 Webhook | `mock/logs/webhook.log` 出现 `jt1078/retransmit` 记录 |

---

## M. 系统

| 用例 | 请求 | 预期 |
|------|------|------|
| 系统信息 | `GET /api/system/info` | 200 + `{serverId, ...}` |
| 系统统计 | `GET /api/system/stats` | 200 |
| 版本 | `GET /api/system/version` | 200 |
| 在线用户 | `GET /api/system/online-users` | 200 |

---

## N. RTP / PS 控制（公开）

| 用例 | 请求 | 预期 |
|------|------|------|
| 打开 RTP 接收 | `POST /api/rtp/receive/open` | 200 |
| 关闭 RTP 接收 | `POST /api/rtp/receive/close/:stream_id` | 200 |
| 启动 RTP 发送 | `POST /api/rtp/send/start` | 200 |
| 停止 RTP 发送 | `POST /api/rtp/send/stop/:stream_id` | 200 |
| 打开 PS 接收 | `POST /api/ps/receive/open` | 200 |
| 关闭 PS 接收 | `POST /api/ps/receive/close/:stream_id` | 200 |
| 启动 PS 发送 | `POST /api/ps/send/start` | 200 |
| 停止 PS 发送 | `POST /api/ps/send/stop/:stream_id` | 200 |
| 获取测试端口 | `GET /api/ps/getTestPort` | 200 |

**验证**：调用 `openRtpServer` 后，ZLM mock 端日志显示 "openRtpServer: port=... stream_id=..."。

---

## O. 公共 / 健康

| 用例 | 请求 | 预期 |
|------|------|------|
| Liveness | `GET /api/health` | 200 + `{status:healthy}` |
| Readiness | `GET /api/ready` | 200 |
| Prometheus 指标 | `GET /metrics` | 200 + Prometheus 文本格式 |
| 登录 | `POST /api/user/login` | 200 + JWT |
| 登出 | `GET /api/user/logout` | 200 |
| RPC 端点 | `POST /api/rpc` | 200 |

---

## P. WebSocket

| 用例 | 验证 |
|------|------|
| 连接 | `ws://127.0.0.1:18080/api/ws` 带 JWT |
| 设备状态推送 | mock 模拟设备注册 / 心跳后，WebSocket 收到 `{type: deviceStatus, ...}` |
| 重连 | 主动断线后自动重连 |

---

## Q. ZLM Hook 触发（端到端）

| 场景 | 验证 |
|------|------|
| 启动 ZLM mock + GBServer + Webhook 接收器 | 三者联通 |
| `curl /trigger/on_server_started` | GBServer 收到 hook，初始化与该节点的会话 |
| `curl /trigger/on_publish` | GBServer 更新流列表（`/api/server/media_server/media_info`） |
| `curl /trigger/on_record_mp4` | GBServer 写入云录像（`/api/cloud/record/list` 出现该文件） |
| `curl /trigger/on_stream_changed` | GBServer 更新流状态 |

完整脚本：
```bash
# 启动
bash mock/scripts/start-mocks.sh zlm webhook-receiver
cd /Users/letmlook/code/GBServer && cargo run

# 触发
for h in on_server_started on_publish on_record_mp4 on_stream_changed; do
  curl -sf "http://127.0.0.1:8080/trigger/$h"
  sleep 1
done

# 验证 Webhook 接收
curl -s http://127.0.0.1:9090/received | jq '.[].path'
# 输出应包含：
# "/hook/on_server_started"
# "/hook/on_publish"
# "/hook/on_record_mp4"
# "/hook/on_stream_changed"
```