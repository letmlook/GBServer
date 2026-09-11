# talk.ts 契约审计

审计对象：`web/src/api/talk.ts`（`startTalk` / `stopTalk` / `listTalk` / `talkAudioWsUrl`）× `src/router.rs` × `src/handlers/talk.rs` × `src/sip/gb28181/talk.rs`。

已逐个核对并**全部通过**的部分（不列入下方问题）：

- 路径与 method：`talk.ts:19-21` `/talk/start/{deviceId}/{channelId}` ↔ `src/router.rs:473-476` `get(talk::talk_start)`；`talk.ts:27-29` `/talk/stop/...` ↔ `src/router.rs:477-480` `get(talk::talk_stop)`；`talk.ts:35-37` `/talk/list` ↔ `src/router.rs:491` `get(talk::talk_list)`；`talk.ts:55-57` `/talk/audio/{deviceId}/{channelId}?token=` ↔ `src/router.rs:1030-1033` `get(talk::talk_audio_ws)`（该路由按设计注册在 `api_protected` 之外，见 `src/router.rs:1024-1029`，handler 内部用 `?token=` 校验，`talk.ts:53-57` 正是这么发的）。路径参数顺序（device 在前、channel 在后）与三个 handler 的 `Path((device_id, channel_id))` 一致（`src/handlers/talk.rs:21、63、214`）。
- 参数绑定：这四个函数没有 query/body 参数（只有 WS 的 `token`，后端 `src/handlers/talk.rs:342` 读的也是 `token`），因此不存在 snake_case/camelCase 别名问题。
- 响应键名：`startTalk` 声明的 `callId` / `status`（`talk.ts:18`）在 `src/handlers/talk.rs:44-50` 的 `json!` 中都存在；`listTalk` 声明的 `{total, list}` 与 `TalkSession` 的 `callId/deviceId/channelId/status/localPort/deviceIp/devicePort/startTime`（`talk.ts:5-14、34`）与 `src/handlers/talk.rs:288-303` 逐键一致；WS 文本帧 `{callId,packets,bytes}` / `{error}`（`src/handlers/talk.rs:440-446、422-425`）与消费方 `web/src/components/TalkPanel/index.vue:139-142` 读取的键一致。
- `listTalk`（`talk.ts:33-38`）**当前无调用方**（`grep -rn "listTalk" web/src` 仅命中定义处），其契约本身无误。

## 1. other GET /api/talk/start/:device_id/:channel_id（返回后立即连 /api/talk/audio/:device_id/:channel_id）

- 前端：`web/src/api/talk.ts:16` 把语义写成「发送 SIP INVITE，**等设备 200 OK 后会话变为 active**」，`talk.ts:17-22` 声明返回 `{ callId, status }`；唯一调用方 `web/src/components/TalkPanel/index.vue:118-123` `await startTalk(...)` 之后**立刻** `new WebSocket(talkAudioWsUrl(...))`，既不看返回的 `status`（`:136-147` 只处理 WS 文本帧），也没有任何轮询/重试 —— `talk.ts` 根本没有封装 `/api/talk/status`，`grep -rn "talk/status" web/src` 无命中。
- 后端：`src/handlers/talk.rs:44-50` 在 INVITE 发出后立即以 HTTP 200 返回 `"status": "inviting"`（`src/sip/server.rs:5057-5067`：`send_request_to(...)` 之后直接 `Ok(call_id)`，**不等待**设备 200 OK）；而紧随其后的 WS 握手 `src/handlers/talk.rs:364-371` 用 `get_by_device_channel(...)` 取会话，`src/sip/gb28181/talk.rs:116-121` 只返回 `is_active()` 的会话，`src/sip/gb28181/talk.rs:73-75` `is_active()` 即 `status == TalkStatus::Active`，取不到就 `404 NOT_FOUND`（`src/handlers/talk.rs:365-371`）；`Active` 只在设备 200 OK 的响应处理里写入（`src/sip/server.rs:3551-3556`）。
- 影响：设备 200 OK 只要晚于 WS 握手（前端在 `startTalk` 返回后同一微任务内就发起握手，而 200 OK 至少要多一个 SIP 往返 + 设备处理），握手就吃 `404`，浏览器触发 `onerror`，`TalkPanel:131-134` 抛出「WebSocket 连接失败（服务端可能还没协商出设备音频地址）」→ `:179-185` 弹「开启对讲失败」并顺手调 `/api/talk/stop` 收尾；此时会话仍是 `Inviting`，`send_talk_bye` 的 `get_by_device_channel` 同样取不到会话必然失败（`src/sip/server.rs:5071-5077`），而 `talk_stop` 把该失败吞成成功（`src/handlers/talk.rs:82-86`），所以清理也静默失败、会话残留在 `TalkManager`（`cleanup_expired` 只清理 `Terminated`，`src/sip/gb28181/talk.rs:131-146`）。用户看到的是「点『对讲』报错/没声音」，只有设备恰好抢在握手前回 200 OK 才会成功。
- 验证边界：本机后端（18080）、ZLM、SIP 设备均未运行，未做在线复现；以上结论全部来自上列代码路径，失败与否取决于 200 OK 与 WS 握手到达服务端的先后（`e2e/tests/live.spec.ts:191-194` 只断言「对讲」按钮存在与计数，从未点击，故该竞态未被任何测试覆盖）。
