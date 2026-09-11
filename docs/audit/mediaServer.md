# mediaServer.ts 契约审计

审计对象：`web/src/api/mediaServer.ts`（8 个导出函数）× `src/router.rs` × `src/handlers/server.rs` × `src/db/media_server.rs`。

路由与 method 全部核对通过（`src/router.rs:219-252` 已注册 `online/list`、`list`、`one/:id`、`check`、`save(POST)`、`delete(DELETE)`、`media_info`、`load` 且与前端 method 一致），**未发现 route-missing / http-method 类问题**；`list` / `one` / `save` / `delete` 的响应键名也已用 `#[serde(rename_all = "camelCase")]`（`src/db/media_server.rs:15-16`）对齐前端 `httpPort` / `lastKeepaliveTime` / `status` 等列。

以下 7 条是逐条比对后仍然真实存在的不一致。真值参照 WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java` 与 `/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/server.js`。

## 1. query-param GET /api/server/media_server/check

- 前端：`web/src/api/mediaServer.ts:41-47` `checkMediaServer(id: string)` 只发 `params: { id }`（第 45 行）；唯一调用方 `web/src/views/mediaServer/index.vue:84` `await checkMediaServer(row.id)`。
- 后端：`src/handlers/server.rs:974-981` `MediaServerCheckQuery { ip: Option<String>, #[serde(alias = "httpPort")] port: Option<i32>, secret: Option<String>, #[serde(rename = "type")] type_: Option<String> }`，**没有 `id` 字段**（也无 `deny_unknown_fields`，未知 query 被静默丢弃）；`src/handlers/server.rs:987-990` 取不到参数时兜底 `ip = "127.0.0.1"`、`http_port = 80`、`secret = ""`。
- 交叉验证：WVP 真值 `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java:126` `checkMediaServer(@RequestParam String ip, @RequestParam int port, @RequestParam String secret, @RequestParam String type)`，其前端 `/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/server.js:26-37` 也发 `ip/port/secret/type`。
- 影响：媒体节点页点“检测”永远只探测 `127.0.0.1:80` 且 secret 为空，与所点节点无关 —— 本机没跑 ZLM 时返回的是缺省探测字段（用户会看到“检测失败”），本机恰好有 ZLM 时会把这个错误节点的端口/secret 当成被检测节点的结果。

## 2. response-field GET /api/server/media_server/check

- 前端：`web/src/api/mediaServer.ts:42` 把响应声明成 `WvpResult<{ code: number; msg: string }>`；调用方 `web/src/views/mediaServer/index.vue:85-89` 据此判断 `if ((res.data as any)?.code === 0) ... else ElMessage.error('检测失败: ' + (res.data as any)?.msg ?? '')`。
- 后端：`src/handlers/server.rs:993-1003` 构造 `payload = json!({"ip", "httpPort", "secret", "type", "autoConfig", "rtpEnable", "rtpProxyPort", "rtpPortRange", "sendRtpPortRange"})`，再并入 `media_server_probe_fields()`（`src/handlers/server.rs:1508-1536`：`hookIp/sdpIp/streamIp/httpSSlPort/rtmpPort/rtmpSSlPort/rtspPort/rtspSSLPort/recordAssistPort/rtpEnable`），`src/handlers/server.rs:1034` `WVPResult::success(payload)` —— **data 里没有 `code` / `msg` 键**。
- 前端读取口径：`web/src/utils/request.ts:33-38` 响应拦截器把整个 `{code,msg,data}` body 返回，因此 `res.data` 就是上面的 payload。
- 交叉验证：WVP 该接口返回的是 `MediaServer` 对象（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java:126-128`），也不是 `{code,msg}`。
- 影响：媒体节点页“检测”**无论后端探测成功与否都必然弹红色 “检测失败: ”**（`res.data.code` 为 `undefined`，`undefined === 0` 为假，`res.data.msg` 也为空），用户无法从 UI 得知节点是否连通。

## 3. query-param GET /api/server/media_server/load

- 前端：`web/src/api/mediaServer.ts:49-55` `getMediaLoad(id: string)` 发 `params: { id }`（第 53 行）；调用方 `web/src/views/dashboard/index.vue:189` `msList.slice(0, 6).map((m) => getMediaLoad(m.id ?? ''))`。
- 后端：`src/handlers/server.rs:1333` `pub async fn media_server_load(State(state): State<AppState>)` —— **没有 `Query` 提取器**，query 参数完全被忽略；`src/handlers/server.rs:1353-1377` 遍历 `state.list_zlm_servers()` 返回全部节点的 `[{id, push, proxy, gbReceive, gbSend}, ...]`。
- 交叉验证：WVP `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java:253` `getMediaLoad()` 无入参，`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/server.js:97-102` 也不带 `id`。
- 影响：dashboard 对每个节点都拿到“全部节点”的数组，而 `web/src/views/dashboard/index.vue:194-195` 取 `arr[0]` 与请求的 `msList[i].id` 配对（`arr[0]` 来自 `HashMap::keys()` 的任意首元素，`src/lib.rs:895-896`），于是多节点部署下每张节点卡片的带宽（`gbReceive`/`gbSend`，dashboard:202-204）都显示同一个节点的值，与卡片本身无关。

## 4. response-field GET /api/server/media_server/load

- 前端：`web/src/api/mediaServer.ts:50` 声明 `WvpResult<{ load: number }>`。
- 后端：`src/handlers/server.rs:1363-1373` 每个元素是 `json!({"id", "push", "proxy", "gbReceive", "gbSend"})`，并 `src/handlers/server.rs:1377` `WVPResult::success(serde_json::Value::Array(server_loads))` 直接返回数组。
- 前端实际读取：`web/src/views/dashboard/index.vue:194` 只能写成 `((r.value.data as unknown) as any[]) ?? []` 绕过类型。
- 影响：`data.load` 永远是 `undefined`；任何按声明类型写的调用方都会拿到 `undefined` 而不是负载值，当前调用方是靠强制类型转换绕过的（类型契约是假的）。

## 5. request-field GET /api/server/media_server/media_info

- 前端：`web/src/api/mediaServer.ts:57-63` `getMediaInfo(id: string)` 只发 `params: { id }`（第 61 行），无法表达 `app` / `stream` / `mediaServerId`。
- 后端：`src/handlers/server.rs:1281-1285` `MediaInfoQuery { app: Option<String>, stream: Option<String>, mediaServerId: Option<String> }`；`src/handlers/server.rs:1291-1295` `if app.is_empty() || stream.is_empty() { return Err(AppError::business(ErrorCode::Error400, "缺少 app 或 stream 参数")) }`。
- 交叉验证：WVP `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/server/ServerController.java:178` `getMediaInfo(String app, String stream, String mediaServerId)`，其前端 `/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/server.js:77-88` 发这三个参数。
- 影响：当前 `web/src` 内**无调用方**（`grep -rn "getMediaInfo" web/src` 只命中 `web/src/api/mediaServer.ts:57`）；一旦按现有签名调用，后端恒返回 400 `缺少 app 或 stream 参数`，且 `mediaServerId` 因名字对不上也永远走不到“按节点查”。

## 6. response-field GET /api/server/media_server/media_info

- 前端：`web/src/api/mediaServer.ts:58` 声明 `WvpResult<{ mediaServerId: string; mediaList: MediaInfo[] }>`，`MediaInfo` 接口见 `web/src/api/mediaServer.ts:83-92`。
- 后端：`src/handlers/server.rs:1312-1315` 返回 `serde_json::to_value(info)`，即 `src/zlm/types.rs:29-52` 的单个 `MediaInfo { app, stream, schema, vhost, readerCount, totalReaderCount, originType, originUrl, createStamp, aliveSecond, bytesSpeed, tracks }` —— **既没有 `mediaServerId` 也没有 `mediaList` 键**。
- 交叉验证：WVP `MediaInfo` bean（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/media/bean/MediaInfo.java:22-69`）同样没有 `mediaList`，只有单条流信息字段。
- 影响：当前 `web/src` 内**无调用方**；若调用，`data.mediaList` 恒为 `undefined`，按此类型渲染的流列表会一直为空。

## 7. request-field POST /api/server/media_server/save

- 前端：`web/src/api/mediaServer.ts:72` `MediaServer` 接口声明 `enabled?: boolean`，`web/src/views/mediaServer/EditDialog.vue:23` 有“启用”开关、`:64/:85` 默认 `enabled: true`、`:95` `await saveMediaServer(form)` 整表单提交。
- 后端：`src/handlers/server.rs:1063-1104` `MediaServerSaveBody` 中**没有 `enabled`（也没有任何 alias 指向它）**，serde 默认丢弃未知字段；`src/db/media_server.rs:17-68` 的 `MediaServer` 结构体与三套建表语句（`database/init-sqlite-2.7.4.sql:181`、`database/init-postgresql-2.7.4.sql:322`、`database/init-mysql-2.7.4.sql:178`）的 `gb_media_server` 表都**没有 `enabled` 列**，`list` 响应因此也永远不含 `enabled`。
- 交叉验证：WVP `MediaServer` 模型（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/media/bean/MediaServer.java:12-116`）同样无 `enabled` 字段，WVP 前端表单也不提交它。
- 影响：媒体节点编辑弹窗里的“启用”开关保存后被静默丢弃、刷新后不会持久化 —— 该开关键既不能停用节点，也不反映真实状态（节点在离线与否由 `status` 列/健康检查决定，见 `src/db/media_server.rs:317-340`；列表页 `web/src/views/mediaServer/index.vue:27` 用的也是 `status`）。
