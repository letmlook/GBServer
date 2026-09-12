# cloudRecord.ts 契约审计

> **状态：已全部修复（2026-09-12 第二十九轮）**。修复中还发现 5 个审计没覆盖到的
> 深层缺陷（ZLM 文件列表 API 名大小写错、缺 vhost、响应结构想当然、
> `deleteRecord` 端点在新版 ZLM 不存在、容器化部署下按本机路径找文件）。
> 详见 `docs/WVP_PARITY.md` 第二十九轮小节。

审计对象：`web/src/api/cloudRecord.ts`（14 个导出函数）× `src/router.rs` × `src/handlers/stub.rs` × `src/handlers/cloud_record_extra.rs` × 唯一消费方 `web/src/views/cloudRecord/index.vue`。

真值来源：WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/vmanager/cloudRecord/CloudRecordController.java`（下文简称 WVP 控制器）。

已逐个核对并**通过**的部分（不列入下方问题）：

- 路径存在性：`cloudRecord.ts` 的 14 个 url 在 `src/router.rs` 中**全部注册**（438-471、930-935），无 404。
- `/cloud/record/collect/delete`：前端 `cloudRecord.ts:134-140` 用 GET + `id`，`src/router.rs:466-469` 注册的是 `delete(...)`，但同一 path 又在 `src/router.rs:930` 注册了 `get(cloud_record_extra::collect_delete)`，两个 `.route()` 同处一条链（460-940 之间无 `Router::new`/`.merge`），axum 按 method 合并，GET 由 `cloud_record_extra.rs:40-53` 处理，其 `id: Option<i64>`（`cloud_record_extra.rs:15-18`）与前端 `{ id }` 一致，**通过**。
- `/cloud/record/task/list`：前端 `{page,count}` → 后端 `{total,list}`（`stub.rs:1413`），**通过**。
- `/cloud/record/collect/list`：前端只读 `list` → 后端 `{total,list}`（`stub.rs:1666`），**通过**。
- `/cloud/record/list` 的响应信封：前端 `{total,list}`（`cloudRecord.ts:32`）→ 后端 `{total,list}`（`stub.rs:1551-1554`），**通过**。

## 1. http-method GET /api/cloud/record/delete

- 前端：`web/src/api/cloudRecord.ts:88-92` `method: 'get', url: '/cloud/record/delete', params: { id }`
- 后端：`src/router.rs:456-459` `.route("/api/cloud/record/delete", delete(stub::cloud_record_delete))`（该 path 只注册了 `delete`，无 `get`；WVP 控制器 `:355` 同样是 `@DeleteMapping("/delete")`）
- 影响：唯一调用方 `web/src/views/cloudRecord/index.vue:176` `await deleteCloudRecord(row.id ?? 0)`；GET 命中只注册了 DELETE 的 path，axum 返回 405，axios 错误拦截器（`web/src/utils/request.ts:45-69`）弹出错误提示，`await` 抛出导致 `index.vue:177` 的「已删除」永远不会执行 —— 用户点「删除」录像删不掉。

## 2. request-field DELETE /api/cloud/record/delete

- 前端：`web/src/api/cloudRecord.ts:91` `params: { id }`（单数 `id`，且放在 query 上）
- 后端：`src/handlers/stub.rs:1418` `Json(body): Json<CloudRecordDeleteBody>`，`stub.rs:1420` `let ids = body.ids.unwrap_or_default()`，DTO 为 `stub.rs:1043-1045` `pub ids: Option<Vec<String>>`（WVP 控制器 `:358` 是 `@RequestBody BatchRemoveParam ids`，body 形如 `{"ids":[...]}`）
- 影响：即使把 method 改成 DELETE，前端也**不发送请求体**且字段名是 `id` 而非 `ids`，`Json` 提取器因缺少 `Content-Type: application/json`（415）或空 body（400）直接失败，删除依然不生效；此缺陷与第 1 条相互独立，需同时修。

## 3. request-field GET /api/cloud/record/play/path

- 前端：`web/src/api/cloudRecord.ts:59` `params: { id }`；调用方 `web/src/views/cloudRecord/index.vue:145` `getCloudRecordPlayPath(row.id ?? 0)`
- 后端：`src/handlers/stub.rs:1113` `let record_id = q.record_id.or(q.cloud_record_id).unwrap_or_default()`；DTO 只认 `#[serde(alias = "recordId")] record_id`（`stub.rs:1016-1019`）与 `cloudRecordId`，**没有 `id` 字段**；WVP 控制器 `:249` 要求的也是 `@RequestParam(required = true) Integer recordId`
- 影响：`id` 无法绑定 → `record_id` 为空串 → `stub.rs:1114` 解析失败 → 直接返回空 `{playPath:"",httpPath:"",httpsPath:""}`（`stub.rs:1115-1119`）；`index.vue:147` 取到的 url 为空，用户点「播放」永远只看到「该录像无可播放路径，请确认 ZLM 录像已生成」，云端录像无法播放。

## 4. request-field GET /api/cloud/record/list（deviceId / channelId）

- 前端：`web/src/api/cloudRecord.ts:27-28` 声明 `deviceId?: string; channelId?: string`；`web/src/views/cloudRecord/index.vue:113-114` 实际传 `deviceId: query.deviceId, channelId: query.channelId`（表单输入框在 `index.vue:17、20`）
- 后端：`src/handlers/stub.rs:1013-1040` 的 `CloudRecordQuery` **没有** `device_id`/`channel_id` 字段，也没有任何 `#[serde(alias = "deviceId")]`（对比同一 DTO 给 `mediaServerId`/`callId`/`startTime` 都加了 alias）；`stub.rs:1456-1555` 的 handler 全程未引用设备/通道；WVP 控制器 `:120-129` 的 `/list` 同样只收 query/app/stream/mediaServerId/callId
- 影响：serde 默认忽略未知字段，这两个过滤条件被**静默丢弃**。用户在「设备」「国标通道ID」输入框里填了值点查询，返回的仍是全部录像，筛选看起来无效且没有任何错误提示。

## 5. request-field GET /api/cloud/record/list（startTime / endTime 取值格式）

- 前端：`web/src/api/cloudRecord.ts:25-26` 声明 `startTime?: string; endTime?: string`；`web/src/views/cloudRecord/index.vue:117-118` 用 `query.startTime?.toISOString()` 传值，即 `2024-05-01T03:00:00.000Z` 形式
- 后端：`src/handlers/stub.rs:1474-1475` 对两者调 `normalize_record_time_ms`，该函数（`stub.rs:30-44`）只接受 `%Y-%m-%d %H:%M:%S`、`%Y-%m-%dT%H:%M:%S` 两种格式，失败时 `unwrap_or_default()` 返回 **0**；WVP 控制器 `:115-116` 的 `@Parameter` 文档要求的是 `yyyy-MM-dd HH:mm:ss`
- 影响：带毫秒和 `Z` 的 ISO 串两种格式都解析失败 → 过滤值变成 0 → `stub.rs:1491-1493` 的 `if start_time > filter_end { continue; }` 对任何真实录像（start_time 均为 13 位毫秒时间戳）都成立，**全部记录被丢弃**。用户一旦填写「结束」时间点查询，表格必然为空；只填「开始」时间（start_filter=0）则过滤完全不生效。已用最小 chrono 程序实测确认：`normalize_record_time_ms("2024-05-01T03:00:00.000Z") == 0`，而 `"2024-05-01 03:00:00"` 正常返回 `1714532400000`。

## 6. request-field GET /api/cloud/record/download/zip

- 前端：`web/src/api/cloudRecord.ts:115` `params: { ids: ids.join(',') }`，入参来自 `web/src/views/cloudRecord/index.vue:160、182` 的 `row.id` / `selection.map(r => r.id)`；而 `row` 来自 `getCloudRecordList`，其 `id` 由后端拼成复合串
- 后端：`src/handlers/stub.rs:1503` `build_cloud_record_id(...)`、`stub.rs:1512` `obj.insert("id", record_id)`，该 id 形如 `zlm1::record::record::xxx.mp4`（`stub.rs:52-53` `format!("{media_server_id}::{app}::{stream}::{file_name}")`）；而 `/download/zip` 侧 `src/handlers/cloud_record_extra.rs:243` 用 `parse_zip_ids`，该函数（`cloud_record_extra.rs:95-100`）按 `,` 切分后 `parse::<i64>()`，非数字被 `filter_map` 丢弃
- 影响：复合串解析不出任何 i64 → `ids` 为空 → `cloud_record_extra.rs:244-246` 返回 `WVPResult::error("missing ids")`，axios 因 `code !== 0` 直接 reject（`web/src/utils/request.ts:34-36`）。用户点「下载」或勾选后「打包下载」只会看到「missing ids」错误提示，ZIP 永远打不出来（注意 `/list-url` 返回的才是数字 DB id：`cloud_record_extra.rs:75` `"id": r.id`，两个列表的 id 语义不同）。

## 7. http-method POST /api/cloud/record/task/add

- 前端：`web/src/api/cloudRecord.ts:96-100` `method: 'post', url: '/cloud/record/task/add', data`（把 `Partial<CloudRecord>` 作为 JSON 请求体发送）
- 后端：`src/router.rs:448-451` `.route("/api/cloud/record/task/add", get(stub::cloud_record_task_add))`，handler 取 `Query(q): Query<CloudRecordQuery>`（`stub.rs:1302-1304`）；该 path 无 `post` 注册；WVP 控制器 `:167` 也是 `@GetMapping("/task/add")`
- 影响：POST 命中只注册 GET 的 path → 405。前端函数**当前无调用方**（`grep -rn "addCloudRecordTask" web/src` 仅命中定义处），故暂无用户可见故障；但一旦接入页面，合并任务下发必然失败。

## 8. request-field GET /api/cloud/record/loadRecord

- 前端：`web/src/api/cloudRecord.ts:63-68` 只发 `{ id, startTime, endTime }`；`CloudRecord` 类型也无 `cloudRecordId` 字段（`cloudRecord.ts:4-17`）
- 后端：`src/handlers/stub.rs:1210` `let record_id = q.cloud_record_id.unwrap_or_default()`，DTO 只认 `cloud_record_id` / `cloudRecordId`（`stub.rs:1018-1019`），**没有 `id`**；WVP 控制器 `:261-263` 要求 `app`、`stream`、`cloudRecordId` 三个必填参数
- 影响：`cloud_record_id` 为空 → `stub.rs:1211` 解析失败 → 直接返回空对象 `{}`（`stub.rs:1212`），播放地址/时长全部拿不到。前端函数**当前无调用方**（`grep -rn "getCloudRecordLoad" web/src` 仅命中定义处）。

## 9. response-field GET /api/cloud/record/loadRecord

- 前端：`web/src/api/cloudRecord.ts:64` 声明返回 `WvpResult<{ records: CloudRecord[] }>`
- 后端：`src/handlers/stub.rs:1211-1256` 返回的是**单个**流内容对象，键为 `id/key/app/stream/mediaServerId/duration/startTime/endTime/filePath/playPath`（`stub.rs:1224-1233`），**不存在 `records` 键**
- 影响：消费方按声明读 `res.data.records` 会得到 `undefined`。前端函数**当前无调用方**（同上），暂无用户可见故障。

## 10. request-field GET /api/cloud/record/seek

- 前端：`web/src/api/cloudRecord.ts:71-76` 发 `params: { streamId, seekTime }`
- 后端：`src/handlers/stub.rs:1263` 只读 `q.record_id.or(q.cloud_record_id)`，`stub.rs:1267` 只读 `q.seek`；DTO 的字段是 `stream`（`stub.rs:1015`，无 `streamId` alias）与 `seek: Option<i64>`（`stub.rs:1038`，无 `seekTime` alias）；WVP 控制器 `:321-325` 要求 `mediaServerId/app/stream/seek`
- 影响：`streamId`、`seekTime` 双双无法绑定 → `record_id` 为空 → `stub.rs:1265` 的 `if !record_id.is_empty()` 不成立，`playback_manager.update_current_time` **从不被调用**，定位请求返回 200 但播放进度不动（返回体里的 `seek` 恒为 0）。前端函数**当前无调用方**（`grep -rn "seekCloudRecord" web/src` 仅命中定义处）。

## 11. request-field GET /api/cloud/record/speed

- 前端：`web/src/api/cloudRecord.ts:79-84` 发 `params: { streamId, speed }`
- 后端：`src/handlers/stub.rs:1285` 读 `q.record_id.or(q.cloud_record_id)`，`stub.rs:1289` 调 `playback_manager.update_speed(&record_id, speed)`；DTO 无 `streamId` 字段（`stub.rs:1015` 只有 `stream`），`speed` 字段本身能正常绑定（`stub.rs:1039`）
- 影响：`speed` 值能收到，但 `record_id` 恒为空 → `stub.rs:1288` 的 `if !record_id.is_empty()` 不成立，倍速**从不生效**，接口却返回成功。前端函数**当前无调用方**（`grep -rn "speedCloudRecord" web/src` 仅命中定义处）。

## 12. response-field GET /api/cloud/record/date/list

- 前端：`web/src/api/cloudRecord.ts:48` 声明返回 `WvpResult<{ list: { date: string; count: number }[] }>`
- 后端：`src/handlers/stub.rs:1167` 返回类型是 `Json<WVPResult<Vec<String>>>`，`stub.rs:1203` `WVPResult::success(result)`，data 是 `["2024-05-01", ...]` 字符串数组（无 `list` 包装、无 `count`）；WVP 控制器 `:73` 同样是 `List<String>`
- 影响：消费方读 `res.data.list` 得到 `undefined`，按 `item.date`/`item.count` 渲染会直接崩或被 `?? []` 吞掉成空日历。前端函数**当前无调用方**（`grep -rn "getCloudRecordDateList" web/src` 仅命中定义处）；此处**前端类型声明是错的一方**，后端与 WVP 一致。

## 13. request-field GET /api/cloud/record/collect/add

- 前端：`web/src/api/cloudRecord.ts:130` `params: { id }`
- 后端：`src/handlers/stub.rs:1583` `let record_id = q.record_id.clone().or(q.cloud_record_id).unwrap_or_default()`，DTO（`stub.rs:1047-1058`）只认 `recordId`/`cloudRecordId`/`deviceId`/`channelId`/`name`，**没有 `id`**；WVP 控制器 `:217` 用 `@RequestParam(required = false) Integer recordId`
- 影响：`id` 无法绑定 → `record_id` 为空 → `stub.rs:1584-1586` 返回 `WVPResult::error("record_id is required")`，axios 直接 reject，收藏必然失败。前端函数**当前无调用方**（`grep -rn "addCloudRecordCollect" web/src` 仅命中定义处）。

## 14. response-field GET /api/cloud/record/list-url

- 前端：`web/src/api/cloudRecord.ts:40` 声明 `WvpResult<{ total: number; list: CloudRecord[] }>`，即列表项按 `CloudRecord`（`cloudRecord.ts:4-17`：`id/app/stream/callId/mediaServerId/startTime/endTime/filePath/folder/size/createTime`）解析
- 后端：`src/handlers/cloud_record_extra.rs:74-84` 每项实际只给 `id/app/stream/fileName/url/startTime/endTime/duration/fileSize`，`callId`、`mediaServerId`、`filePath`、`size`、`createTime`、`folder` **均不存在**（`size` 被命名为 `fileSize`）
- 影响：任何按 `CloudRecord` 声明读取 `row.size`/`row.filePath`/`row.callId` 的消费方会拿到 `undefined`（`CloudRecord` 字段全为可选，故 TS 不报错，属静默失效）。前端函数**当前无调用方**（`grep -rn "getCloudRecordListUrl" web/src` 仅命中定义处）。

---

## 第三十九轮补充修复：设备侧录像删不掉

验证「回放」模块时发现：`/api/cloud/record/list` 会把**设备通过 RecordInfo 上报的
设备侧录像**（`app = "record_info"`，文件名形如「录像片段1」，`file_path` 是设备上的路径）
也列出来，而删除处理器要求"先删掉 ZLM 上的文件"才肯删库记录
（`period` 从 ZLM 的 `<起始>-<结束>.mp4` 文件名解析；这类记录解析不出 period）
→ 这些行**永远删不掉**（返回 `failed`，库记录保留）。

修复：区分"平台侧有没有文件"。

* `app == "record_info"`（设备侧录像）→ 平台/ZLM 侧不存在同名文件 → **只删库记录**；
* 其余（ZLM 录制产物）→ 维持原顺序：先删文件、成功后再删库记录，
  文件删不掉时保留记录并在 `message` 里说明需人工清理。

实测：`DELETE /api/cloud/record/delete {"ids":["27"]}`（record_info 行）
由 `{"deleted":[],"failed":["27"]}` 变为 `{"deleted":["27"],"failed":[]}`。

### 环境限制（本轮无法端到端验证的部分）

本机 Docker Desktop 出现**容器 → 宿主机网络不通**（`host.docker.internal` 解析到
IPv6 ULA `fdc4:f303:9324::254`，`192.168.65.254`、`172.18.0.1`、宿主机 LAN IP 全部
连接失败；`gbserver-redis` 容器同样连不上宿主机的 18080）。后果是 **ZLM 的所有 hook
（`on_publish` / `on_record_mp4` / `on_server_keepalive` …）都到不了后端**：

* 录制计划能拉起流、ZLM 也产出了 MP4 文件，但 `on_record_mp4` 不触发
  → `gb_cloud_record` 里没有新记录；
* 云端录像的 `播放`/`删除` 用例因此**在"没有平台侧真实录像"时显式 skip**，
  而不是拿设备侧上报的记录去断言契约（那样只会得到与契约无关的失败）。

这不是仓库缺陷：`docker-compose.yml` 已补 `extra_hosts: host.docker.internal:host-gateway`
（让该名字稳定解析到 IPv4 网关，避免同类问题），但本机这次是 Docker Desktop 自身的
主机网络故障，需要重启 Docker Desktop 才能恢复。后端侧已做加固：主动探活成功即刷新
`last_keepalive_time`，即使 hook 通路不通，节点也不会被误判离线（见第三十六轮）。
