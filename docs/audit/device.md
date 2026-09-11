# device.ts 契约审计

范围：`web/src/api/device.ts`（15 个 API 函数）对后端 `src/router.rs` / `src/handlers/*.rs` 的路径、method、请求字段、响应键名。15 个 URL 全部在 `src/router.rs` 中注册且 HTTP method 一致（逐条见文末"核对说明"），因此下列条目均为**字段级**不一致。

## 1. response-field GET /api/device/query/devices
- 前端：`web/src/api/device.ts:159` `online?: number | boolean`（同接口类型还声明 `:163` `status?: string`、`:154-155` `heartBeatInterval?: number` / `heartBeatCount?: number`）
- 前端消费：`web/src/views/device/index.vue:138-140` `return row.online === true || row.online === 1 || row.status === 'ON'`；`:50-53` 用 `isOnline(row)` 渲染"在线/离线"标签；`:61-63` 用 `isOnline(row)` 决定"撤防/布防"按钮
- 后端：`src/db/device.rs:16-17` `#[derive(Debug, Clone, Default, Serialize, FromRow)] #[serde(rename_all = "camelCase")] pub struct Device`，`:27` `pub on_line: Option<bool>,` → 序列化键为 `onLine`，且 Device 无 `status` 字段；`src/handlers/device.rs:37-45` 直接序列化 `Device`（`DevicePage { total, list, page, size }`）
- 影响：设备列表每行的 `row.online` 与 `row.status` 恒为 `undefined`，`isOnline()` 恒返回 `false` → "在线"列恒显示"离线"（在线设备也显示离线），操作列按钮恒显示"布防"并调用 `setGuard()`，在线设备的"撤防"入口不可达。WVP-PRO 的 `Device.onLine`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/gb28181/bean/Device.java:86`）序列化键同为 `onLine`，前端应按 `onLine` 读取。另：`Device` 结构体与 `src/db/device.rs:55` 的 `DEVICE_SELECT_COLUMNS` 都不含 `heart_beat_interval` / `heart_beat_count`（列存在，见 `database/init-sqlite-2.7.4.sql:47-48`），故 `DeviceRecord` 声明的 `heartBeatInterval` / `heartBeatCount` 永远读不到值。

## 2. request-field POST /api/device/query/device/add
- 前端：`web/src/api/device.ts:94-100` `add(data: Partial<DeviceRecord>)` → `POST /device/query/device/add`；提交对象为 `web/src/views/device/EditDialog.vue:80-94` 的 `form`，表单包含 `ip`（`:16-18`）、`port`（`:19-21`）、`heartBeatInterval`（`:36-38`）、`heartBeatCount`（`:39-41`）、`expires`（`:42-44`）、`password`（`:45-47`），提交于 `:133` `await add(form)`
- 后端：`src/handlers/device_stub.rs:783-797` `DeviceAddBody` 只有 `device_id / name / manufacturer / model / transport / stream_mode / media_server_id / custom_name`（无 `deny_unknown_fields`）；`src/handlers/device_stub.rs:858-871` 只透传这些字段给 `src/db/device.rs:1112-1123` 的 `insert_device(...)`，其 INSERT 列表（`:1127`）不含 `ip / port / password / heart_beat_interval / heart_beat_count / expires`
- 影响：新增设备时填写的 IP、端口、心跳间隔、心跳次数、注册有效期、密码被静默丢弃（HTTP 200 + `code:0`），界面提示"新增成功"但库中这些列仍为空；WVP-PRO 的 `/device/add`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/gb28181/controller/DeviceQuery.java:235`）接收完整 Device 字段。

## 3. request-field POST /api/device/query/device/update
- 前端：`web/src/api/device.ts:102-108` `update(data: Partial<DeviceRecord>)` → `POST /device/query/device/update`；编辑时 `web/src/views/device/EditDialog.vue:102-104` `Object.assign(form, props.device, { password: '' })` 把列表行的 `ip / port / expires / heartBeatInterval / heartBeatCount` 一起带入表单，提交于 `:130` `await update(form)`
- 后端：`src/handlers/device_stub.rs:799-813` `DeviceUpdateBody` 字段与 `DeviceAddBody` 相同；`:829-841` 调用 `src/db/device.rs:1152-1163` 的 `update_device(...)`，其 `UPDATE gb_device SET ...`（`:1166-1168`）只更新 `name / manufacturer / model / transport / stream_mode / media_server_id / custom_name`
- 影响：编辑设备（改 IP、端口、心跳、注册有效期、密码）后提示"已保存"，但除名称/厂家/型号/信令传输/流传输模式/媒体服务器/自定义名之外的修改一律不落库，重新打开仍是旧值。

## 4. response-field GET /api/device/query/sync_status
- 前端：`web/src/api/device.ts:40-46` `request<WvpResult<{ total: number; current: number; errorMsg?: string }>>({ method: 'get', url: '/device/query/sync_status', params: { deviceId } })`
- 后端：`src/handlers/device_stub.rs:47-51` `SyncStatusQuery { #[serde(alias = "deviceId")] device_id }`（入参一致）；`:80-87`（及 `:89-95` 分支）响应 `{"deviceId", "status", "activeSubscriptions", "online", "streamMode", "message"}`，没有 `total` / `current` / `errorMsg`
- 影响：当前 `web/src` 内无调用方（`grep -rn "syncStatus" web/src` 只命中 `web/src/api/device.ts:40` 的定义本身），无用户可见影响；但按该类型接入后 `total / current / errorMsg` 恒为 `undefined`，同步进度无法渲染。WVP-PRO 该接口返回 `SyncStatus`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/gb28181/bean/SyncStatus.java:14-25`，即 total/current/errorMsg/syncIng/time），其消费方 `web-legacy-vue2/src/views/dialog/SyncChannelProgress.vue:83-90` 正是用 `data.total`、`data.current`、`data.errorMsg` 计算百分比。

## 5. response-field GET /api/device/query/devices/{deviceId}
- 前端：`web/src/api/device.ts:19-24` `queryDeviceOne(deviceId)` 声明返回 `WVPResult<DeviceRecord>`
- 后端：`src/handlers/device_stub.rs:889-903` 只返回 `deviceId / name / manufacturer / model / transport / streamMode / onLine / ip / port / createTime / updateTime / mediaServerId / customName`
- 影响：当前无调用方（`grep -rn "queryDeviceOne" web/src` 仅命中定义）。键名不一致同上（后端 `onLine` vs 前端 `online`），且 `DeviceRecord`（`web/src/api/device.ts:142-167`）声明的 `id / firmware / expires / heartBeatInterval / heartBeatCount / registerTime / channelCount / sdpIp / status / gbId / gbDeviceId / treePath` 全部缺失。潜在影响：编辑弹窗用 `props.device?.id` 判断新增还是编辑（`web/src/views/device/EditDialog.vue:76`），若改用该接口取详情，`id` 缺失会让"编辑"退化成"新增"。

## 6. query-param GET /api/device/query/devices/{deviceId}/channels
- 前端：`web/src/api/device.ts:110-116` `queryChannels(deviceId, params: DeviceQueryParams & { online?: boolean; channelType?: number })`，其中 `DeviceQueryParams`（`:4-9`）还含 `query`
- 后端：`src/handlers/device.rs:56-60` `ChannelsQuery { page: Option<u32>, count: Option<u32> }`，`:63-85` 的 `query_channels` 只按 `device_id` 分页，从不读取 `query / online / channelType`
- 影响：当前唯一调用方 `web/src/views/device/index.vue:172` 只传 `{ page: 1, count: 500 }`，故暂无实际影响；但按该类型传入通道关键字、在线状态、通道类型过滤会被静默忽略（返回该设备全部通道）。WVP-PRO 同接口支持这三个参数（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/gb28181/controller/DeviceQuery.java:98-113`）。

---

## 核对说明（一致项与无影响项）

以下已逐条核对，**未发现不一致**：

- 路径与 method（`src/router.rs`）：`GET /api/device/query/devices`（`:83`）、`GET .../devices/:device_id/channels`（`:85`）、`GET .../query/sync_status`（`:97`）、`DELETE .../devices/:device_id/delete`（`:101`）、`GET .../devices/:device_id/sync`（`:105`）、`POST .../query/transport/:device_id/:stream_mode`（`:109`）、`GET .../control/guard`（`:113`）、`GET .../query/subscribe/catalog`（`:134`）、`GET .../query/subscribe/mobile-position`（`:138`）、`GET .../config/query/:device_id/BasicParam`（`:142`）、`GET .../control/record`（`:152`）、`POST .../query/device/update`（`:172`）、`POST .../query/device/add`（`:176`）、`GET .../query/devices/:device_id`（`:180`）、`GET .../query/tree/:device_id`（`:184`）——15 条与 `web/src/api/device.ts` 的 method 全部一致。
- `GET /device/query/devices` 入参 `page/count/query/status` 与 `DevicesQuery`（`src/handlers/device.rs:15-23`）一致，`status` 的 `ON/OFF` 映射见 `src/handlers/device.rs:32-36`；响应 `total/list`（`src/handlers/device.rs:39-45`）与调用方 `web/src/views/device/index.vue:151-152`、`web/src/views/dashboard/index.vue:176` 的读取方式一致。
- `setGuard` / `resetGuard` 的 `deviceId` + `guardCmd`（`web/src/api/device.ts:55-69`）与 `PtzQuery`（`src/handlers/device_control.rs:8-20`，`device_id` 有 `alias="deviceId"`、`guard_cmd` 有 `alias="guardCmd"`）一致，`device_guard`（`:107-143`）按 `SetGuard` 区分设防/撤防。
- `subscribeCatalog` 的 `id/cycle`（`web/src/api/device.ts:71-77`）与 `SubscribeQuery`（`src/handlers/device_control.rs:145-150`）一致；`subscribeMobilePosition` 的 `id/cycle/interval`（`:79-85`）与 `SubscribePositionQuery`（`src/handlers/device_stub.rs:267-272`）一致。
- `deviceRecord` 的 `deviceId/channelId/recordCmdStr`（`web/src/api/device.ts:134-140`）与 `RecordControlQuery`（`src/handlers/device_stub.rs:531-539`，三个字段均带 camelCase alias）一致（该函数当前无调用方）。
- `add` / `update` 的 `deviceId`、`streamMode`、`mediaServerId`（`web/src/api/device.ts:142-167` 的 camelCase 命名）能绑上后端字段（`src/handlers/device_stub.rs:785/791/793`、`:801/807/809` 的 alias），不一致的只是上文第 2、3 条的额外字段。
- `queryChannels` 响应的 `ChannelRecord`（`web/src/api/device.ts:169-190`）读取键 `id/deviceId/channelId/name/status/manufacturer/model/owner/civilCode/address/parental/longitude/latitude/subCount/hasAudio/channelType/streamIdentification/parentId` 在 `channel_to_json`（`src/handlers/device_stub.rs:616-659`）中均存在，调用方 `web/src/views/device/index.vue:91-105` 用到的列全部有值。
- `queryChannelTree` 与 `queryDeviceTree`（`web/src/api/device.ts:118-132`）URL 相同（均为 `/device/query/tree/{deviceId}`）、返回类型与 `device_tree`（`src/handlers/device_stub.rs:911-922`，返回 `{total, list}`）一致，二者当前均无调用方；`queryBasicParam`（`:87-92`）、`updateDeviceTransport`（`:48-53`）路径与 method 一致且当前无调用方。
- 未计入不一致的类型字段：`ChannelRecord` 的 `audio`（`web/src/api/device.ts:188`）与 `registerStatus`（`:189`）在 `channel_to_json`（`src/handlers/device_stub.rs:616-659`）中不返回，但 `web/src` 内无任何代码读取（`grep -rn "registerStatus" web/src` 仅命中三处类型声明），当前无用户可见影响。
