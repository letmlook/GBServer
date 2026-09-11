# live.ts 契约审计

审计对象：`web/src/api/live.ts`（10 个导出函数）对后端 Axum 路由 / handler DTO / 响应键名。

审计方法：读 `web/src/api/live.ts` → 在 `src/router.rs` 核对 path 与 method → 读 `src/handlers/*.rs` 的 Query/Json DTO 与 `json!({...})` 键名 → `grep -rn` 找页面调用方确认影响 → 用 WVP-PRO Java 源码（`/tmp/wvpsrc/wvp-GB28181-pro-master`）交叉验证真值。

URL 拼接口径：dev 用 `VITE_APP_BASE_API='/dev-api'`，`web/vite.config.ts:64` 把 `/dev-api` 重写为 `/api`；prod 用 `/api`。因此 live.ts 里写的 `/play/start/...` 最终是 `/api/play/start/...`。

已核对一致、不列入问题的函数：`startPlay`、`stopPlay`、`playSnap`、`getSsrc`、`startBroadcast`、`stopBroadcast`、`getPlayUrl`（path/method 均存在；响应键 `playUrl/flvUrl/wsUrl/ws_flv/hls/webrtc/ssrc/snapUrl/url/streamId` 与前端声明相符）。其中 `getPlayUrl`、`getSsrc`、`startBroadcast`、`stopBroadcast`、`getWebrtcPlay` 当前无调用方。

---

## 1. http-method GET /api/play/webrtc

- 前端：`web/src/api/live.ts:78-83`

  ```ts
  export function getWebrtcPlay(params: { deviceId: string; channelId: string }) {
    return request<WvpResult<{ url: string }>>({
      method: 'get',
      url: '/play/webrtc',
      params
    })
  }
  ```

- 后端：`src/router.rs:355-358`

  ```rust
  .route(
      "/api/play/webrtc",
      post(webrtc::webrtc_play),
  )
  ```

  `src/handlers/webrtc.rs:21-24` 只接受 JSON body，不读 query：

  ```rust
  pub async fn webrtc_play(
      State(state): State<AppState>,
      Json(req): Json<WebRtcOfferRequest>,
  ) -> Json<WVPResult<serde_json::Value>> {
  ```

- 影响：同一 path 只注册了 POST，前端发 GET 会被 axum 的 MethodRouter 判为 405 Method Not Allowed，axios 走错误分支，用户看到请求失败；即便把前端改成 POST，参数也仍在 query string 里、进不了 `Json<WebRtcOfferRequest>` body，`deviceId/channelId` 全为 None，`stream` 拼成空串。当前无调用方（`grep -rn "getWebrtcPlay" web/src` 只命中 live.ts 定义本身），属未引爆的定时炸弹。

## 2. response-field GET /api/play/webrtc

- 前端：`web/src/api/live.ts:79` — 期望 `data.url`

  ```ts
  return request<WvpResult<{ url: string }>>({
  ```

- 后端：`src/handlers/webrtc.rs:59-64` — 返回键里没有 `url`

  ```rust
  return Json(WVPResult::success(serde_json::json!({
      "sdp": answer_sdp,
      "type": "answer",
      "app": app,
      "stream": stream
  })));
  ```

- 影响：调用方读 `res.data.url` 恒为 `undefined`（后端给的是 SDP answer）。当前无调用方（同第 1 条），一旦按第 1 条修好 method，会立刻表现为"拿不到播放地址"。

## 3. request-field GET /api/front-end/ptz/{deviceId}/{channelId}

- 前端：`web/src/api/live.ts:104-107` — 发的是 `cmd`

  ```ts
  return request<WvpResult>({
    method: 'get',
    url: `/front-end/ptz/${params.deviceId}/${params.channelId}`,
    params: { cmd: params.cmd, speed: params.speed ?? 50 }
  })
  ```

  页面调用：`web/src/views/live/index.vue:430`

  ```ts
  await sendPtzApi({ deviceId: channel.deviceId, channelId: channel.channelId, cmd })
  ```

- 后端：`src/handlers/front_end.rs:13-14` 的 `PtzQuery.command` 没有任何 `cmd` 别名（其它字段有 camelCase 别名，`command` 没有）：

  ```rust
  pub struct PtzQuery {
      pub command: Option<String>,
  ```

  `src/handlers/front_end.rs:180` 只从 `command` 取值：

  ```rust
  let command = q.command.clone().unwrap_or_default();
  ```

  空命令落到 `src/handlers/front_end.rs:79` 的默认分支（无方向的 PTZ 字节）：

  ```rust
  _ => format!("050100000000{:02X}FF", h_speed),
  ```

  交叉验证：WVP-PRO `PtzController.java:78,83` 的查询参数名是 `command`（`String command`），WVP 前端 `web/src/api/frontEnd.js:190` 也是 `command: command` —— 后端是对的，`live.ts` 的 `cmd` 是错的。

- 影响：`command` 永远为 `""`，后端给设备下发的是 `05010000000001FF`（`0501` + 方向位全 0，即"无动作"）。直播间 `web/src/views/live/index.vue:136-144` 的 上/下/左/右/放大/缩小/停止 7 个按钮都会弹 "PTZ XXX 已下发"（后端返回 `code:0`），但摄像机一动不动 —— 用户看到的是"成功但无效"。

## 4. request-field GET /api/front-end/ptz/{deviceId}/{channelId}（speed 被丢弃）

- 前端：`web/src/api/live.ts:98-107` — 声明并始终发送 `speed`（默认 50）

  ```ts
  export function sendPtz(params: {
    deviceId: string
    channelId: string
    cmd: string
    speed?: number
  }) {
    ...
      params: { cmd: params.cmd, speed: params.speed ?? 50 }
  ```

- 后端：`src/handlers/front_end.rs:21` 虽然声明了 `speed`，但 `ptz` handler（`src/handlers/front_end.rs:175-195`）从不读它，只读三个方向速度：

  ```rust
  let h_speed = q.horizon_speed.unwrap_or(1) as u8;   // front_end.rs:181
  let v_speed = q.vertical_speed.unwrap_or(1) as u8;  // front_end.rs:182
  let z_speed = q.zoom_speed.unwrap_or(1) as u8;      // front_end.rs:183
  ```

  （`q.speed` 只在同文件的 `iris`/`focus` 等其它 handler 里被使用，`ptz` 里没有引用。）

  交叉验证：WVP-PRO `PtzController.java:83` 只接收 `command, horizonSpeed, verticalSpeed, zoomSpeed`，`web/src/api/frontEnd.js:189-194` 也只发这四个键 —— `speed` 这个参数名在 WVP 契约里不存在。

- 影响：即使修好第 3 条，速度仍被固定为 1（`unwrap_or(1)`），前端传的 50 完全不生效，云台转动速度与用户设置无关。

## 5. response-field GET /api/device/query/streams

- 前端：`web/src/api/live.ts:86-92` — 列表项声明了 `mediaServerId`（必填）：

  ```ts
  return request<WvpResult<{ total: number; list: { mediaServerId: string; app: string; stream: string; readerCount?: number }[] }>>({
    method: 'get',
    url: '/device/query/streams',
    params
  })
  ```

  调用方 `web/src/views/dashboard/index.vue:321-328` 实际读的是 `deviceId`：

  ```ts
  const liveList = streams.value.slice(0, 6).map((s, i) => ({
    ...
    deviceId: s.deviceId ?? '',
    channelId: s.stream ?? ''
  }))
  ```

- 后端：`src/handlers/device_stub.rs:495-507` — 直接把 ZLM 的流信息透出，键里既没有 `mediaServerId` 也没有 `deviceId`：

  ```rust
  let list: Vec<serde_json::Value> = streams.iter().map(|s| {
      serde_json::json!({
          "schema": s.schema,
          "app": s.app,
          "stream": s.stream,
          "vhost": s.vhost,
          "readerCount": s.reader_count,
          "totalReaderCount": s.total_reader_count,
          "originType": s.origin_type,
          "aliveSecond": s.alive_second,
          "bytesSpeed": s.bytes_speed
      })
  }).collect();
  ```

  交叉验证：WVP-PRO `DeviceQuery.java:118-124` 的 `/streams` 返回 `PageInfo<DeviceChannel>`，而 `DeviceChannel.java:32` 定义了 `private String deviceId`（通道编号）—— 该列表项本就应带通道标识。

- 影响：仪表盘"重点通道"卡片的 `deviceId` 恒为 `''`，`web/src/views/dashboard/index.vue:310-315` 的点击处理会直接命中 `if (!c.deviceId || !c.channelId)`，弹 "该通道暂无可用播放标识" 并 return —— 6 个实时流卡片点不进直播页。另外 `live.ts` 声明的 `mediaServerId` 后端从不返回，类型是假的。

## 6. query-param GET /api/device/query/streams

- 前端：`web/src/api/live.ts:86-92` 发送 `page` / `count` / `query`

  ```ts
  export function queryStreams(params: { page?: number; count?: number; query?: string }) {
    return request<...>({
      method: 'get',
      url: '/device/query/streams',
      params
    })
  }
  ```

- 后端：`src/handlers/device_stub.rs:487-489` — 签名里没有任何 `Query` 提取器，三个参数被静默忽略：

  ```rust
  pub async fn query_streams(
      State(state): State<AppState>,
  ) -> Json<WVPResult<serde_json::Value>> {
  ```

  返回的 `total` 也是全量条数而非分页总数：`src/handlers/device_stub.rs:509-512`

  ```rust
  return Json(WVPResult::success(serde_json::json!({
      "total": list.len(),
      "list": list
  })));
  ```

  交叉验证：WVP-PRO `DeviceQuery.java:120-124` 的 `page` / `count` 是必填分页参数。

- 影响：`page`/`count`/`query` 全部不生效，`total` 恒等于本次返回条数。当前唯一调用方 `web/src/views/dashboard/index.vue:171` 传 `{ page: 1, count: 1000 }` 且不使用 `total` 做分页，所以用户可见影响仅为"参数写了没用、也没有搜索/翻页能力"；一旦有页面用 `page=2`，仍会拿到全量第一页数据。
