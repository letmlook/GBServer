# streamPush.ts 契约审计

> **状态：已修复（2026-09-12 第三十二轮）**。修复中发现 `save_to_gb`/`remove_form_gb`
> 更新的是**不存在的列**（`gb_stream_push.device_id/channel_id`）→ 接口稳定 500。

审计范围：`web/src/api/streamPush.ts` 的 11 个函数（`/api/push/list`、`/add`、`/update`、`/remove`、`/batchRemove`、`/start`、`/stop`、`/upload`、`/save_to_gb`、`/remove_form_gb`、`/forceClose`）。

路由核对：`src/router.rs:271`（list，get）、`:272`（add，post）、`:273`（update，post）、`:274`（start，get）、`:277`（stop，get）、`:278`（remove，**post**）、`:279`（upload，post）、`:280`（batchRemove，delete）、`:281`（save_to_gb，post）、`:282-285`（remove_form_gb，**delete**）、`:917`（forceClose，get）。前端的 method 声明在 `web/src/api/streamPush.ts:18-100`：`getStreamPushList`(get)、`addStreamPush`(post)、`updateStreamPush`(post)、`deleteStreamPush`(**delete**)、`batchDeleteStreamPush`(delete)、`startStreamPush`(get)、`stopStreamPush`(get)、`uploadStreamPush`(post)、`saveToGb`(post)、`removeFromGb`(**get**)、`forceClose`(get)。**存在 2 处 HTTP method 不一致**（第 1、8 条），其余为字段/体型不一致。已确认无误的端点：`GET /api/push/start`、`GET /api/push/stop`、`GET /api/push/forceClose`（前端只传 `id`，后端 `src/handlers/stream.rs:983-986`、`:1060-1062` 同样只收 `id`）。

真值参考：WVP-PRO 的 `/api/push/remove` 为 POST 且 id 走 query（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:54-62`）、`/batchRemove` 为 DELETE 且 `ids` 走 JSON body（同文件 `:72-79`）、`/upload` 为 multipart 且字段名 `file`（`.../streamPush/controller/StreamPushController.java:108-110`）。GBServer 后端与 WVP 一致，偏离的是 GBServer 前端。

## 1. http-method DELETE /api/push/remove
- 前端：`web/src/api/streamPush.ts:42` `method: 'delete',`，`:43` `url: '/push/remove'`，`:44` `params: { id }`
- 后端：`src/router.rs:278` `.route("/api/push/remove", post(stream::push_remove))`；handler 只声明 POST，`src/handlers/stream.rs:156-158` `pub async fn push_remove(... Query(body): Query<PushRemoveBody>)`
- 影响：`web/src/views/streamPush/index.vue:103` “删除”按钮 `await deleteStreamPush(row.id ?? 0)` 发出的 DELETE 打到只注册了 POST 的同一路径 → Axum 返回 405，`web/src/utils/request.ts:63-72` 弹出错误提示，**推流记录删不掉**；若改成 POST（query id）后端即可正常工作。对照 WVP 前端为 `method: 'post'`（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:54-62`），可确认是前端 method 写错。

## 2. request-field DELETE /api/push/batchRemove
- 前端：`web/src/api/streamPush.ts:50` `method: 'delete',`，`:52` `params: { ids: ids.join(',') }` → 实际请求为 `?ids=1,2,3`（无 body、无 `Content-Type`）
- 后端：`src/router.rs:280` `delete(stream::push_batch_remove)`；`src/handlers/stream.rs:307` `Json(body): Json<PushBatchRemoveBody>`，`:300-302` `pub ids: Option<Vec<i64>>`
- 影响：`web/src/views/streamPush/index.vue:110` “批量删除” `await batchDeleteStreamPush(...)`。DELETE 请求没有 JSON body，`Json` 提取器直接拒绝（415 Unsupported Media Type），**批量删除必然失败**并弹错；对照 WVP 前端用 `data: { ids: ids }`（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:72-79`），后端 DELETE + `@RequestBody BatchRemoveParam`（`.../StreamPushController.java:248-251`）本就要求 JSON body。

## 3. request-field POST /api/push/add（url / gbId 被丢弃）
- 前端：`web/src/views/streamPush/EditDialog.vue:11` `<el-input v-model="form.url" placeholder="rtsp://... 或 rtmp://..." />`（`:61` 中 `url: [{ required: true, ... }]` 强制必填）、`:17` `<el-input v-model="form.gbId" .../>`，保存时 `EditDialog.vue:81` `await addStreamPush(form)`；`web/src/api/streamPush.ts:24-29` 把 `form` 原样作为 JSON `data` POST
- 后端：`src/handlers/stream.rs:68-73` `PushAddBody { app, stream, #[serde(alias = "mediaServerId")] media_server_id }`，**没有 `url`、也没有 `gbId`/`device_id` 字段**；落库函数 `src/db/stream_push.rs:138-144` 形参也只有 `app/stream/media_server_id/now`；表定义 `database/init-sqlite-2.7.4.sql:276-291` 无 `url` 列（`device_id`/`channel_id` 也不存在）
- 影响：用户必须填写的“源 URL”与“国标ID”被后端静默忽略（serde 丢弃未知键），点“新增成功”后立即丢失——“源 URL”既不落库也读不回来（与第 6 条互为因果）。

## 4. request-field POST /api/push/update（url / gbId 被丢弃）
- 前端：`web/src/views/streamPush/EditDialog.vue:78` `await updateStreamPush(form)`，提交同一个含 `url`/`gbId` 的 `form`；`web/src/api/streamPush.ts:32-38` 原样 JSON 提交
- 后端：`src/handlers/stream.rs:108-114` `PushUpdateBody { id, app, stream, #[serde(alias = "mediaServerId")] media_server_id }`，同样无 `url`/`gbId`；`:128-135` 只把 `app/stream/media_server_id` 传入 `stream_push::update`
- 影响：编辑弹窗里改“源 URL”或填“国标ID”后提示“已保存”，**实际一个字都没写库**，用户以为已修改。

## 5. response-field GET /api/push/list（mediaServerId vs media_server_id）
- 前端：`web/src/api/streamPush.ts:11` `mediaServerId?: string`；`web/src/views/streamPush/index.vue:26` `<el-table-column prop="mediaServerId" label="媒体节点" min-width="140" />`
- 后端：`src/db/stream_push.rs:11-17` `#[derive(Debug, Clone, Serialize, FromRow)] pub struct StreamPush`（**无 `rename_all = "camelCase"`**，仅 `src/db/role.rs:14`、`src/db/device.rs:10` 那类结构体才做了 camelCase），`:17` `pub media_server_id: Option<String>` → JSON 键为 `media_server_id`；列表直接序列化该结构体（`src/handlers/stream.rs:50-55`、`:42-49`）
- 影响：`web/src/views/streamPush/index.vue:72` 载入列表后，“媒体节点”列**永远空白**（前端读到的键实际叫 `media_server_id`）。WVP 前端同样绑 `mediaServerId`（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/views/streamPush/index.vue:108`）。

## 6. response-field GET /api/push/list（url 键不存在）
- 前端：`web/src/api/streamPush.ts:10` `url?: string`；`web/src/views/streamPush/index.vue:23-25` `prop="url"` 且 `{{ row.url }}`
- 后端：`src/db/stream_push.rs:12-32` 的 `StreamPush` 字段只有 `id/app/stream/create_time/media_server_id/server_id/push_time/status/update_time/pushing/self_push/start_offline_push/stream_status`，**没有任何 `url` 字段**；`list_paged` 的 SELECT 列表（如 `src/db/stream_push.rs:366`）同样不含 url 列
- 影响：列表“源 URL”列**永远空白**，用户无法核对推流源地址。

## 7. response-field GET /api/push/list（status 是 bool，前端按数字 1 比较）
- 前端：`web/src/api/streamPush.ts:9` `status?: number`；`web/src/views/streamPush/index.vue:29` `row.status === 1 ? 'success' : 'info'`，`:36` `:disabled="row.status === 1"`，`:37` `:disabled="row.status !== 1"`
- 后端：`src/db/stream_push.rs:20` `pub status: Option<bool>` → JSON 序列化为 `true`/`false`（不是 1/0）；写入侧同样是布尔，`src/db/stream_push.rs:622` `pub async fn update_status(pool: &Pool, id: i64, status: bool)`
- 影响：`row.status === 1` 恒为 `false` → 状态列**永远显示“停止”**，“停止推流”按钮 `:disabled="row.status !== 1"` 恒为禁用（行内无法点“停止”），“启动”按钮也永不置灰。类型真值参考：前端应比较 `row.status === true`。

## 8. http-method GET /api/push/remove_form_gb
- 前端：`web/src/api/streamPush.ts:90` `method: 'get',`，`:91` `url: '/push/remove_form_gb'`，`:92` `params: { id }`；**当前无调用方**（`grep -rn "removeFromGb" web/src` 只命中定义处 `web/src/api/streamPush.ts:88`）
- 后端：`src/router.rs:282-285` `.route("/api/push/remove_form_gb", delete(stream::push_remove_form_gb))`
- 影响：当前无调用方，用户暂不可见；一旦被调用，GET 打到只注册 DELETE 的路径 → 405，与第 1 条同因。WVP 前端此处也是 `method: 'delete'`（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:64-70`）。

## 9. request-field DELETE /api/push/remove_form_gb（JSON body vs query）
- 前端：`web/src/api/streamPush.ts:92` `params: { id }`（只发 query，无 body）；**当前无调用方**（同第 8 条）
- 后端：`src/handlers/stream.rs:397-398` `Json(body): Json<serde_json::Value>`，`:400` `let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);`，`:402-404` `id <= 0` 即返回 `WVPResult::error("缺少必要参数")`
- 影响：当前无调用方；即使把 method 改成 delete，后端只从 JSON body 取 `id`，前端只发 query → 取到 0，永远返回“缺少必要参数”，解绑国标失效。WVP 前端用 `data: data` 传 body（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:64-70`）。

## 10. request-field POST /api/push/save_to_gb
- 前端：`web/src/api/streamPush.ts:82` `method: 'post',`，`:83` `url: '/push/save_to_gb'`，`:84` `params: { id }`（**没有 `data`**）；**当前无调用方**（`grep -rn "saveToGb" web/src` 只命中定义处 `web/src/api/streamPush.ts:80`）
- 后端：`src/handlers/stream.rs:350` `Json(body): Json<serde_json::Value>`，`:352-354` 从 body 取 `id`/`deviceId`/`channelId`，`:356-358` `id <= 0 || device_id.is_empty()` 即返回 `WVPResult::error("缺少必要参数")`
- 影响：当前无调用方；调用时 POST 无 body/无 `Content-Type` → `Json` 提取器拒绝（415）；即便补上 body，前端只传 `id` 也缺 `deviceId` → “缺少必要参数”，推流永远绑不到国标。WVP 前端为 `method: 'post'` + `data: data`（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/streamPush.js:5-11`）。

## 11. request-field POST /api/push/upload（JSON vs multipart）
- 前端：`web/src/api/streamPush.ts:74-76` `method: 'post', url: '/push/upload', data`，签名为 `data: { id: number | string; url: string }`（`:72`），axios 以 `application/json` 提交；**当前无调用方**（`grep -rn "uploadStreamPush" web/src` 只命中定义处 `web/src/api/streamPush.ts:72`）
- 后端：`src/handlers/stream.rs:844-846` `pub async fn push_upload(... mut multipart: Multipart)`，`:860-899` 只识别 `file` / `app` / `stream` 三个表单字段（`:898` 未识别的字段直接丢弃）
- 影响：当前无调用方；一旦被调用，JSON 请求不是 multipart，被 `Multipart` 提取器拒绝（400）而失败；且前端字段名 `url`/`id` 与后端需要的 `file` 不对应，即便换成 multipart 也拿不到文件（`:902-904` 返回“未提供文件或流名称”）。WVP 后端为 `@RequestParam(value = "file") MultipartFile file`（`.../StreamPushController.java:108-110`），GBServer 后端与其一致，偏离的是前端。

## 12. query-param GET /api/push/list（query 被后端忽略）
- 前端：`web/src/api/streamPush.ts:16` `params: { page?: number; count?: number; query?: string }`，`:20` `params` 原样下发；**当前无调用方传入**（`web/src/views/streamPush/index.vue:72` 只传 `{ page: 1, count: 200 }`）
- 后端：`src/handlers/stream.rs:23` DTO 收下了 `pub query: Option<String>`，但 `push_list` 只使用 `page`/`count`/`pushing`/`mediaServerId`（`src/handlers/stream.rs:33-49`），`query` 从未被引用；`src/db/stream_push.rs:353-359`（`list_paged`）与 `:479-483`（`count_all`）的形参也没有查询词
- 影响：当前无调用方，用户暂不可见；一旦页面按签名传 `query` 做关键字过滤，后端会静默忽略并返回全量列表（结果等同无过滤）。
