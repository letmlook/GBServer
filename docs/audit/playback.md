# playback.ts 契约审计

调用方：仅 `web/src/views/playback/index.vue:92-99`（路由 `/playback`，见 `web/src/router/index.ts:52-54`）。
路径与方法已逐个核对：`web/src/api/playback.ts` 的 6 条 `/api/playback/*` 与 `/api/gb_record/query/*`，
后端全部注册为 `get`（`src/router.rs:398-423`），路径参数个数、名称顺序一致；`PlaybackQuery` /
`RecordQuery` 已带 `#[serde(alias = "startTime"/"endTime")]`（`src/handlers/playback.rs:174-179`、`515-522`），
前端 camelCase 查询参数可以绑定，**无 snake_case/camelCase 绑定错误**。以下仅列真实存在的响应字段不一致。

## 1. response-field GET /api/gb_record/query/:device_id/:channel_id

- 前端：`web/src/api/playback.ts:55-64` `export interface RecordItem { deviceId: string; channelId: string; name: string; ... }`；消费方 `web/src/views/playback/index.vue:47` `<el-table-column prop="name" label="名称" min-width="120" show-overflow-tooltip />`
- 后端：ZLM MP4 兜底分支 `src/handlers/playback.rs:600-607` `serde_json::json!({ "fileName": f.name, "filePath": f.path, "fileSize": f.size, "startTime": f.create_time, "endTime": f.create_time, "downloadUrl": format!("/record/{}", f.name) })`；GB28181 正常分支 `src/handlers/playback.rs:557-566` 返回 `deviceId/name/filePath/startTime/endTime/address/secrecy/type`
- 影响：当设备不在线、RecordInfo 为空而 ZLM 上存在 MP4 文件时（`src/handlers/playback.rs:592-618` 被判为 `Ok(files)` 且 `total>0`），列表行的 `name` 恒为 `undefined`，"名称"列整列为空白（前端另两列 `startTime/endTime` 因字段名相同而正常）；且两个分支都不返回 `channelId`（ZLM 分支连 `deviceId` 也没有），RecordItem 里声明的这两个必填字段恒为 `undefined`。

## 2. response-field GET /api/playback/start/:device_id/:channel_id

- 前端：`web/src/api/playback.ts:5-9` `request<WvpResult<{ streamId: string; playUrl: string }>>`；消费方 `web/src/views/playback/index.vue:146-148` `const data = res.data ?? { streamId: '', playUrl: '' }` → `playUrl.value = data.playUrl`
- 后端：兜底分支 `src/handlers/playback.rs:298-310` `Json(WVPResult::success(serde_json::json!({ "streamId": ..., "deviceId": ..., "channelId": ..., "app": ..., "stream": ..., "startTime": ..., "endTime": ..., "currentTime": ..., "speed": 1.0, "source": ..., "msg": "Playback session created" })))` —— 键集合中没有 `playUrl`（也无 `flvUrl`/`hls`）
- 影响：SIP 未启用、未配置 ZLM、或 GB28181 Playback INVITE 失败（`src/handlers/playback.rs:201-297` 的两个 `if let` 都未命中/失败）时，接口仍返回 `code: 0`，但 `data.playUrl` 为 `undefined`，`web/src/views/playback/index.vue:71-72` 的 `v-if="playUrl"` 判false，播放区回落到 `web/src/views/playback/index.vue:81` 的 `<el-empty description="从左侧选择录像片段开始回放" />`：用户点了录像片段却看不到任何失败提示（失败只落在后端日志），且 `currentStreamId` 已被赋值，后续暂停/停止按钮可用却对空会话说谎。

## 3. response-field GET /api/playback/start/:device_id/:channel_id

- 前端：`web/src/api/playback.ts:5` 类型只声明 `{ streamId: string; playUrl: string }`；`web/src/views/playback/index.vue:72` `<video :src="playUrl" controls autoplay class="video" />`
- 后端：成功分支 `src/handlers/playback.rs:228` `let play_url = format!("rtsp://{}:554/{}/{}", media_ip, app, stream_id);` → `src/handlers/playback.rs:254` `"playUrl": play_url`；同一响应 `src/handlers/playback.rs:255-256` 还返回 `"flvUrl": flv_url`（`http://…/{app}/{stream}.flv`）与 `"hls": hls_url`（`http://…/{app}/{stream}/hls.m3u8`）
- 影响：成功路径下 `playUrl` 恒为 `rtsp://` 地址，被直接绑到原生 `<video src>`，浏览器不支持 RTSP 协议，回放画面始终不播放；后端已经算好并返回的 `flvUrl`/`hls`（前端类型未声明、页面未使用）才是可播放地址——同项目 `web/src/views/live/index.vue:281` 就是 `const url = data.hls || data.flvUrl || data.playUrl || ''` 并用 hls.js/flv.js 播放，回放页没有沿用该约定。
