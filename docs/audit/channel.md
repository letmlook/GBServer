# channel.ts 契约审计

审计对象：`web/src/api/channel.ts` 的 11 个导出函数（GET/POST/DELETE 方法、query 参数名、body 字段名、响应字段名）。
已核对一致、不列入问题清单的：`deleteChannel`（DELETE `?id=` ↔ `ChannelDeleteQ.id`）、`resetChannel`（只需 `id`，`Partial<Channel>` 里的 `id` 能绑定）、`updateStreamIdentification`（`deviceDbId`/`streamIdentification` 后端已有 `#[serde(alias)]`，`src/handlers/device_stub.rs:739-750`）。
真值参考：WVP-PRO Java `ChannelController.java` 与 WVP/legacy 前端 API 模块（`/tmp/wvpsrc/wvp-GB28181-pro-master/...`、`web-legacy-vue2/src/api/commonChannel.js`）。

## 1. request-field POST /common/channel/add

- 前端：`web/src/api/channel.ts:51-56` `addChannel(data: Partial<Channel>)` → `method: 'post', url: '/common/channel/add', data`；实际发送体由 `web/src/views/channel/EditDialog.vue:69-79` 定义（`channelId` / `deviceId` / `civilCode` / `manufacturer` / `streamIdentification` / `channelType` / `address`），`web/src/views/channel/EditDialog.vue:120-125` 组装 `payload` 并 `await addChannel(payload)`。
- 后端：`src/router.rs:528` `.route("/api/common/channel/add", post(common_channel::channel_add))`；`src/handlers/common_channel.rs:387-397` `struct ChannelAddBody { pub device_id: Option<String>, ..., pub channel_id: Option<String> }`——全部 snake_case 且**没有任何 `#[serde(alias)]`**；`src/handlers/common_channel.rs:403-409` `let device_id = body.device_id.as_deref().unwrap_or("") ... if device_id.is_empty() || channel_id.is_empty() { return Err(AppError::business(ErrorCode::Error400, "deviceId 和 channelId 必填")) }`。
- 影响：前端发的是 `channelId`/`deviceId`，serde 只认 `channel_id`/`device_id`，两个字段永远是 `None` → 请求体校验直接失败。用户在通道页点「新增通道」→ 填表 → 保存，必然收到 400「deviceId 和 channelId 必填」，新增通道 100% 不可用。（对照组：`src/handlers/jt1078.rs:264-275` 同类 DTO 写了 `#[serde(alias = "channelId")]` 并有测试 `channel_add_body_accepts_frontend_names`（`src/handlers/jt1078.rs:1952`），本文件没有。）

## 2. request-field POST /common/channel/update

- 前端：`web/src/api/channel.ts:71-77` `updateChannel(data: Partial<Channel>)` → `post '/common/channel/update'`；`web/src/views/channel/EditDialog.vue:120-122` `const payload = { ...form, channelType: ... }; await updateChannel(payload)`，`form` 的可编辑字段为 `name` / `civilCode` / `manufacturer` / `streamIdentification` / `channelType` / `address`（`web/src/views/channel/EditDialog.vue:69-79`）。
- 后端：`src/router.rs:521-522` `post(common_channel::channel_update)`；`src/handlers/common_channel.rs:336-346` `struct ChannelUpdateBody { id, name, channel_id, civil_code, parent_id, business_group, ptz_type, custom_name }`（无 alias，且**没有** `manufacturer` / `stream_identification` / `channel_type` / `address`）；`src/handlers/common_channel.rs:353-366` 把这些 `Option` 原样传给 `common_channel::update`，后者用 `src/db/common_channel.rs:46-53` `name = COALESCE(?, name), gb_device_id = COALESCE(?, gb_device_id), civil_code = COALESCE(?, civil_code), ...` 落库——`None` 表示「保持原值」。
- 影响：只有 `name` 和 `id` 能绑上，`civilCode`（行政区划）/`channelType`（类型）/`manufacturer`（行业）/`streamIdentification`（网络标识）/`address`（安装地址）要么字段名对不上、要么后端 DTO 里根本不存在，全部按 `None` 静默处理；接口仍返回 `code:0`，前端提示「已保存」。用户改了这些字段后刷新页面会发现只有名称生效，其余修改静默丢失。

## 3. http-method POST /common/channel/play

- 前端：`web/src/api/channel.ts:87-93` `changeAudio(channelId: string, audio: boolean)` → `method: 'post', url: '/common/channel/play', params: { channelId, audio }`。
- 后端：`src/router.rs:587-588` `.route("/api/common/channel/play", get(common_channel::channel_play))`——该方法**只注册了 GET**，`src/handlers/common_channel.rs:737-744` 的入参是 `Query<ChannelIdQuery>`，`ChannelIdQuery.channel_id` 为 `Option<i64>`（`src/handlers/common_channel.rs:164-168`），全函数没有 `audio` 参数。
- 影响：一旦被调用就是 405 Method Not Allowed；**当前无调用方**（`web/src` 内除 `web/src/api/channel.ts` 外无任何 `changeAudio` 引用）。另外该函数形参是国标 ID 字符串，而后端 `channel_id` 是 `i64` 主键，即使改成 GET 也会因 `i64` 解析失败报 400。真值参考：WVP `ChannelController.java:306` 为 `@GetMapping("/play")`，WVP/legacy 前端 `playChannel` 也是 GET `params: { channelId }`（`web-legacy-vue2/src/api/commonChannel.js:254-261`）。

## 4. request-field GET /common/channel/one

- 前端：`web/src/api/channel.ts:22-27` `getChannelOne(id: string | number)` → `url: '/common/channel/one', params: { id }`。
- 后端：`src/router.rs:507` `.route("/api/common/channel/one", get(common_channel::channel_one))`；`src/handlers/common_channel.rs:164-168` `pub struct ChannelIdQuery { #[serde(alias = "channelId")] pub channel_id: Option<i64> }`——**只认 `channel_id` 或 `channelId`，不认 `id`**；`src/handlers/common_channel.rs:280-283` `let id = q.channel_id.unwrap_or(0); if id <= 0 { return Ok(Json(WVPResult::success(serde_json::Value::Null))); }`。
- 影响：`id` 无法绑定（serde 静默忽略未知 query 键），`channel_id` 恒为 `None` → 恒返回 `{code:0,data:null}`，调用方只会静默拿到空数据而不会报错；**当前无调用方**。真值参考：WVP `ChannelController.java:69-71` `getOne(int id)`，WVP 与 legacy 前端都发 `id`（`web-legacy-vue2/src/api/commonChannel.js:5-13`），即后端偏离了 `id` 这一契约。

## 5. response-field GET /common/channel/industry/list

- 前端：`web/src/api/channel.ts:30-35` 声明 `request<WvpResult<string[]>>`；`web/src/views/channel/index.vue:205` `getIndustryList().then((r) => (industryList.value = (r.data as string[]) ?? []))`；`web/src/views/channel/EditDialog.vue:17-19` `<el-option v-for="x in industryList" :key="x" :label="x" :value="x" />`——把元素当字符串同时用作 label 和 value。
- 后端：`src/handlers/common_channel.rs:293-303` 返回 `Vec<serde_json::Value>`，元素为 `serde_json::json!({"value": "01", "label": "危险化学品"})` 这类**对象**。
- 影响：行业下拉项的 `label`/`value` 绑到的是对象，Element Plus 用 `toDisplayString(currentLabel)` 渲染（`web/node_modules/element-plus/es/components/select/src/option2.mjs:20`，`currentLabel` 见 `useOption.mjs:23-25`），选项文本显示为 `[object Object]`、选中值也是对象。通道页「新增/编辑通道」弹窗的「行业」下拉不可用。

## 6. response-field GET /common/channel/type/list

- 前端：`web/src/api/channel.ts:37-42` 声明 `WvpResult<string[]>`；`web/src/views/channel/index.vue:206` `typeList.value = (r.data as string[]) ?? []`；`web/src/views/channel/EditDialog.vue:26-30` `<el-option v-for="x in typeList" :key="x" :label="x" :value="x" />`。
- 后端：`src/handlers/common_channel.rs:306-322` 返回 `{"value": 1, "label": "摄像机"}` 等对象数组。
- 影响：与第 5 条同因，「类型」下拉渲染为 `[object Object]`，且 `form.channelType` 得到对象后再 `Number(...)` 变成 `NaN`（`web/src/views/channel/EditDialog.vue:120`），提交值错误。

## 7. response-field GET /common/channel/network/identification/list

- 前端：`web/src/api/channel.ts:44-49` 声明 `WvpResult<string[]>`；`web/src/views/channel/index.vue:207` `networkList.value = (r.data as string[]) ?? []`；`web/src/views/channel/EditDialog.vue:21-25` `<el-option v-for="x in networkList" :key="x" :label="x" :value="x" />`。
- 后端：`src/handlers/common_channel.rs:325-333` 返回 `{"value": "IP", "label": "IP"}` 等对象数组。
- 影响：与第 5 条同因，「网络标识（码流）」下拉显示 `[object Object]`，选中后写入 `form.streamIdentification` 的是对象。

## 8. query-param GET /common/channel/list

- 前端：`web/src/api/channel.ts:4-12` `ChannelListParams` 声明了 `catalogUnderDevice?: boolean` 与 `deviceId?: string`（`web/src/api/channel.ts:14-20` 原样透传）。
- 后端：`src/router.rs:216` `.route("/api/common/channel/list", get(stub::common_channel_list))`；`src/handlers/stub.rs:108-121` `CommonChannelListQuery { page, count, query, online, channelType, hasRecordPlan, civilCode, parentDeviceId, plan_id, has_link }`——**没有 `deviceId`，也没有 `catalogUnderDevice`**，未知 query 键被 serde 静默忽略。
- 影响：这两个声明参数发过去会被后端丢弃。当前调用方都没有传它们（`web/src/views/channel/index.vue:134-140` 只传 page/count/query/online/channelType；`web/src/views/map/index.vue:78` 用 `query: node.deviceId` 走关键字检索），所以**当前无实际用户可见影响**，但该类型定义是错的，后续按接口签名传参会静默失效。真值参考：WVP `ChannelController.java:133-152` 的 `queryList` 同样只接受 page/count/query/online/hasRecordPlan/channelType/civilCode/parentDeviceId。
