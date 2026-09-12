# streamProxy.ts 契约审计

> **状态：已修复（2026-09-12 第三十三轮）**。10 条全部落地，且用真实 ZLM 验证过：
> `POST /api/proxy/add`（旧字段 `url`/`enabled` 与 WVP 字段 `srcUrl`/`enable` 都收）
> → 落库 → `GET /api/proxy/list?query=…&pulling=…` 真实过滤 → `GET /api/proxy/start`
> 让 ZLM 真的拉起代理流（ZLM `getMediaList` 能看到 `originUrl` 指向源地址）
> → `GET /api/proxy/stop` 关流并清 `pulling` → `DELETE /api/proxy/del?app=&stream=`。
> 修复过程中还发现两个连带缺陷：
> 1. `update` 的 SQL 只会写 5 列（`type`/`timeout`/`ffmpeg_cmd_key`/`rtsp_type`/`enable*` 全丢），
>    新增更是把 `enable` 写死 false、`type` 根本不写；
> 2. `ZlmClient::add_stream_proxy` 发的音频开关参数名是 **`enable_aac`**，
>    而 ZLM 只认 **`enable_audio`**（对未知参数静默忽略）→ "开启音频"从未生效过。
> 附带修正：`query` 过滤（第 10 条）在 `gb_stream_push` 上同样被忽略、且 postgres 下
> `pushing = ?` 绑 `Int` 会 `boolean = integer` 500 —— 为此给 `dyn_where::BindValue`
> 补了 `Bool` 变体。

审计范围：`web/src/api/streamProxy.ts` 的 8 个函数（`/api/proxy/list`、`/one`、`/add`、`/update`、`/save`、`/start`、`/stop`、`/delete`）。

路由与方法先做一次核对：`src/router.rs:286`（list，get）、`:291`（add，post）、`:292`（update，post）、`:293`（save，post）、`:294`（start，get）、`:295`（stop，get）、`:296`（delete，delete）、`:916`（one，get）——与前端 `web/src/api/streamProxy.ts:24-86` 声明的 method 全部一致，**不存在路径缺失或 HTTP method 不一致**。以下问题全部出在字段名绑定上。

真值参考：WVP-PRO Java 实体字段名为 `srcUrl`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/streamProxy/bean/StreamProxy.java:43`），旧版前端同样使用 `srcUrl`（`web-legacy-vue2/src/views/streamProxy/edit.vue:29`）。

## 1. request-field POST /api/proxy/add
- 前端：`web/src/views/streamProxy/EditDialog.vue:21` `<el-input v-model="form.url" placeholder="rtsp://... 或 rtmp://..." />`，`:74` `url: [{ required: true, ... }]`，`:103` `await addStreamProxy(form)`；`web/src/api/streamProxy.ts:40-46` 把 `data`（含键 `url`）原样 POST
- 后端：`src/handlers/stream.rs:512-513` `#[serde(alias = "srcUrl")] pub src_url: Option<String>`（DTO 中只有 `src_url`/`srcUrl`，没有 `url`），`src/handlers/stream.rs:529` `let src_url = body.src_url.unwrap_or_default();`，`src/handlers/stream.rs:533-535` 为空即 `WVPResult::error("Stream and src_url are required")`
- 影响：点“新增代理”必失败，页面弹出红色提示 `Stream and src_url are required`（`src/response.rs:28-34` code=-1，`web/src/utils/request.ts:34-37` 会 reject），**无法新增任何拉流代理**。

## 2. request-field POST /api/proxy/update
- 前端：`web/src/views/streamProxy/EditDialog.vue:21` 同一个 `form.url`，`:100` `await updateStreamProxy(form)`；`web/src/api/streamProxy.ts:48-54` 原样提交
- 后端：`src/handlers/stream.rs:562-563` `#[serde(alias = "srcUrl")] pub src_url: Option<String>`，`src/handlers/stream.rs:580-589` 以 `body.src_url.as_deref()` 调 `stream_proxy::update`，落库语句为 `src_url = COALESCE(?, src_url)`（`src/db/stream_proxy.rs:207-208`）
- 影响：编辑弹窗里改“源 URL”后提示“已保存”，但 `src_url` 为 NULL → COALESCE 保留旧值，**源地址静默改不动**，用户以为已修改。

## 3. request-field POST /api/proxy/update（enabled）
- 前端：`web/src/views/streamProxy/EditDialog.vue:27` `<el-switch v-model="form.enabled" />`，`:68` `enabled: true`，`:100` 随 `form` 提交
- 后端：`src/handlers/stream.rs:557-567` `ProxyUpdateBody` 只有 `id/app/stream/src_url/media_server_id/name`，**没有 `enable`/`enabled` 字段也没有 alias**；`proxy_update` 未调用已存在的 `src/db/stream_proxy.rs:559-579 update_enable_status`；新建时 `src/db/stream_proxy.rs:149-150` 固定写入 `enable = false`
- 影响：编辑弹窗里的“启用”开关保存后不落库，`enable` 仍是新建时的 false，列表里也看不到启用状态（与第 6 条叠加）。

## 4. request-field POST /api/proxy/add、/api/proxy/update（type / destUrl）
- 前端：`web/src/views/streamProxy/EditDialog.vue:8-12` 协议下拉 `v-model="form.type"`（rtsp/rtmp/hls），`:24` `<el-input v-model="form.destUrl" placeholder="(可选) 国标转发目标" />`
- 后端：`src/handlers/stream.rs:508-521`（`ProxyAddBody`）与 `src/handlers/stream.rs:557-567`（`ProxyUpdateBody`）都没有 `type`/`destUrl` 字段（serde 默认忽略未知键）；`src/db/stream_proxy.rs:138-190 add` 的 INSERT 列里也没有 `type`，表定义 `database/init-sqlite-2.7.4.sql:250` `type VARCHAR(50)` 无默认值
- 影响：选择的协议类型不会被保存，列表“类型”列恒为空；填写的“目标 URL”被静默丢弃，无任何效果。

## 5. response-field GET /api/proxy/list（src_url vs url）
- 前端：`web/src/views/streamProxy/index.vue:23-25` `prop="url"` 且 `{{ row.url }}`；类型声明 `web/src/api/streamProxy.ts:16` `url?: string`
- 后端：`src/db/stream_proxy.rs:18` `pub src_url: Option<String>`（该结构体仅 `type` 有 `#[serde(rename = "type")]`，见 `src/db/stream_proxy.rs:11-37`），`src/handlers/stream.rs:467-472` 直接把 `Vec<StreamProxy>` 放进 `ProxyListPage.list`
- 影响：列表“源 URL”列**永远空白**，用户无法确认代理指向的地址；前端读到的键实际是 `src_url`。

## 6. response-field GET /api/proxy/list（enable vs enabled）
- 前端：`web/src/views/streamProxy/index.vue:26-30` `<el-switch v-model="row.enabled" @change="onToggle(row)" />`，`:98` `if (row.enabled) await startStreamProxy(...)`
- 后端：`src/db/stream_proxy.rs:26` `pub enable: Option<bool>` → JSON 键为 `enable`
- 影响：“启用”列开关初始渲染恒为关闭，即使该代理已在拉流；用户看到的状态与库中 `enable` 不一致。

## 7. response-field GET /api/proxy/list（pulling / stream_status vs status）
- 前端：`web/src/views/streamProxy/index.vue:33` `row.status === 1 ? '运行中' : '停止'`，`:40-41` `:disabled="row.status === 1"` / `:disabled="row.status !== 1"`
- 后端：`src/db/stream_proxy.rs:25` `pub pulling: Option<bool>`、`src/db/stream_proxy.rs:36` `pub stream_status: Option<String>`，序列化键为 `pulling`(bool) 与 `stream_status`("ready"/"active")，**没有 `status` 键**
- 影响：状态列**永远显示“停止”**，并且“启动”按钮永不置灰、“停止”按钮永远置灰——对正在拉流的代理无法从界面停止。

## 8. request-field POST /api/proxy/save
- 前端：`web/src/api/streamProxy.ts:56-62` `saveStreamProxy` 原样提交 `Partial<StreamProxy>`（键为 `url`/`enabled`/`destUrl`）；**当前无调用方**（`grep -rn "saveStreamProxy" web/src` 只命中定义处 `web/src/api/streamProxy.ts:56`）
- 后端：`src/handlers/stream.rs:603-635` `proxy_save` 复用 `ProxyAddBody`（`src/handlers/stream.rs:512-513` 只认 `srcUrl`），`:613-615` 为空同样返回错误
- 影响：当前无调用方，用户暂不可见；一旦被调用，与第 1 条同因必失败。

## 9. response-field GET /api/proxy/one
- 前端：`web/src/api/streamProxy.ts:32-38` 声明返回完整 `StreamProxy`（含 `app`/`stream`/`url` 等）；**当前无调用方**（`grep -rn "getStreamProxyOne" web/src` 只命中定义处 `web/src/api/streamProxy.ts:32`）
- 后端：`src/handlers/stream.rs:927-938` 只返回 `id` / `name`（`format!("proxy-{}", q.id)`）/ `url`（`format!("rtsp://{media_ip}:554/live/proxy{id}")`）三个键，且完全不查库
- 影响：当前无调用方；若页面改用该接口，`app`/`stream`/`src_url` 等字段全为 `undefined`，`url` 也与该代理真实的 `src_url` 无关。

## 10. query-param GET /api/proxy/list（query）
- 前端：`web/src/api/streamProxy.ts:7` `query?: string`；**当前无调用方传入**（`web/src/views/streamProxy/index.vue:68` 只传 `{ page: 1, count: 200 }`）
- 后端：`src/handlers/stream.rs:440` DTO 收下了 `pub query: Option<String>`，但 `src/handlers/stream.rs:450-466` 只把 `mediaServerId`/`pulling` 传给 `count_all`/`list_paged`，`query` 从未被使用；`src/db/stream_proxy.rs:296-302`、`:422-426` 的形参也没有 query
- 影响：当前无调用方，用户暂不可见；对照 WVP 实现会把 query 透传给 service（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/streamProxy/controller/StreamProxyController.java:78`），此处一旦按签名传 query，搜索会被后端忽略、结果等同无过滤。

---

## 修复对照（第三十三轮）

| # | 问题 | 修复 |
|---|------|------|
| 1 | 前端发 `url`，DTO 只认 `src_url`/`srcUrl` | `ProxyBody` 加 `#[serde(alias = "url")]`；前端改用 `srcUrl`（`web/src/api/streamProxy.ts`、`EditDialog.vue`） |
| 2 | 更新时 `src_url` 为 NULL → COALESCE 静默保留旧值 | 同 1；`StreamProxyWrite` 承载全部可写列，`update` 用 COALESCE 写 14 列（含 `type`/`timeout`/`ffmpeg_cmd_key`/`rtsp_type`） |
| 3 | `enabled` 未落库；新增写死 `enable=false` | DTO 收 `enable`（别名 `enabled`），`add` 写真实值；前端改读 `enable` |
| 4 | `type` 不落库；`destUrl` 被静默丢弃 | `add`/`update` 落 `type`；前端删除虚构的"目标 URL"输入，改为 WVP 的 `type`（default/ffmpeg）+ `timeout`/`ffmpegCmdKey`/`rtspType`/`noneReader` |
| 5 | 列表读 `row.url` → 源地址整列空白 | `StreamProxy` 加 `#[serde(rename_all = "camelCase")]`，返回 `srcUrl`；前端改读 `srcUrl` |
| 6 | 列表读 `row.enabled` → 开关恒关 | 前端改用 WVP 的 `enable` 语义化标签（`已启用`/`未启用`） |
| 7 | 列表读 `row.status === 1` → 状态恒"停止"、停止按钮恒禁用 | 前端改读布尔 `pulling`；`proxy_start`/`proxy_stop` 真实维护 `pulling` 与 `stream_status`（active/ready/failed） |
| 8 | `/save` 复用只认 `srcUrl` 的 DTO，一旦被调用必失败 | `proxy_save` 改为 upsert（同 app+stream 存在则更新），与 `add` 共用同一套字段解析 |
| 9 | `/one` 不查库，凭空拼 `proxy-{id}` 与假 URL | `proxy_one` 支持 `?id=` 与 WVP 的 `?app=&stream=`，返回真实行（无则 404） |
| 10 | `query` 被 DTO 收下却从未使用 | `proxy_filter` → `DynWhere`，`list_paged`/`count_all` 共用；前端加搜索框，`pulling`/`mediaServerId` 也真正生效 |

顺带补齐的 WVP 对齐项：
* `DELETE /api/proxy/del?app=&stream=`（WVP 的签名）；
* `GET /api/proxy/ffmpeg_cmd/list` 改为真实读节点 `getServerConfig` 的 `ffmpeg.cmd*`（此前返回 4 条硬编码中文说明，选中的 key 在 ZLM 上根本不存在）；
* `proxy_start` 按 `type` 分流到 `addStreamProxy` / `addFfmpegSource`（带 `rtsp_type`/`timeout_sec`/`enable_audio`/`enable_mp4`/`ffmpeg_cmd_key`），失败时把 `stream_status` 写回 `failed` 而不是显示"运行中"；
* `ProxyListPage` 补 `pageNum`/`pageSize`/`pages`（PageHelper `PageInfo` 兼容）。
