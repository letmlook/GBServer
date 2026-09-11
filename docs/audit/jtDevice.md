# jtDevice.ts 契约审计

审计对象：`web/src/api/jtDevice.ts`（21 个导出函数）。
后端路由：`src/router.rs`（`api_protected` 链，JT1078 段从 `src/router.rs:818` 起）。
真值交叉验证：WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master`。

调用方普查（`grep -rn "<函数名>" web/src`）：

- **有调用方**：`getJtTerminalList`(`web/src/views/jtDevice/index.vue:192`)、`deleteJtTerminal`(`:273`)、`getJtChannelList`(`:243`、`:125`)、`getJtAreaCircleList`(`:204`)、`addJtAreaCircle`(`:282`)、`deleteJtAreaCircle`(`:288`)、`getJtAreaPolygonList`(`:213`)、`setJtAreaPolygon`(`:297`)、`deleteJtAreaPolygon`(`:304`)、`getJtRouteList`(`:222`)、`setJtRoute`(`:313`)、`deleteJtRoute`(`:320`)、`addJtTerminal`/`updateJtTerminal`(`web/src/views/jtDevice/TerminalEditDialog.vue:103`/`:100`)
- **当前无调用方**：`getJtTerminalOne`、`addJtChannel`、`updateJtChannel`、`deleteJtChannel`、`getJtAreaRectangleList`、`addJtAreaRectangle`、`deleteJtAreaRectangle`

WVP 真值要点（仅用于判定哪一侧写错）：JT1078 全系列接口的设备标识字段都叫 `phoneNumber`（`src/main/java/com/genersoft/iot/vmp/jt1078/controller/bean/SetAreaParam.java:16`、`.../JT1078TerminalController.java:62,81`）；删除终端为 `DELETE` + `phoneNumber`（`web/src/api/jtDevice.js:44-52`）；通道新增用 `terminalDbId`（`.../JT1078TerminalController.java:107-110`）；通道名字段是 `name` 不是 `channelName`（`.../jt1078/bean/JTChannel.java:22`）；终端在线状态是布尔（`.../jt1078/bean/JTDevice.java:61`）。注意区域/路线的响应封装（GBServer 的 `{count, items}` vs WVP 的 `data: []`）是 GBServer 自定契约，本审计只判断「前端模块 ↔ GBServer 后端」是否自洽。

---

## 1. http-method GET /api/jt1078/terminal/delete

- 前端：`web/src/api/jtDevice.ts:59-65` `export function deleteJtTerminal(id: number | string) { return request<WvpResult>({ method: 'get', url: '/jt1078/terminal/delete', params: { id } }) }`（`method: 'get'` 在第 61 行，`url` 在第 62 行，`params: { id }` 在第 63 行）
- 后端：`src/router.rs:826-829` `.route("/api/jt1078/terminal/delete", delete(jt1078::terminal_delete))` —— 该路径只挂 `delete(...)`；handler 注释亦写 `/// DELETE /api/jt1078/terminal/delete`（`src/handlers/jt1078.rs:429`）。`grep -rn "terminal/delete" src/` 只有 `src/router.rs:827` 一处注册，不存在 GET 或 `any()` 别名。WVP 真值同为 DELETE（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/jtDevice.js:46` `method: 'delete'`；`.../jt1078/controller/JT1078TerminalController.java:61` `@DeleteMapping("/delete")`）
- 影响：`web/src/views/jtDevice/index.vue:273` `await deleteJtTerminal(row.id ?? 0)` 点「删除」后，路径已注册但方法不匹配，Axum 返回 **405 Method Not Allowed**，axios 拦截器抛错，终端永远删不掉（`:274` 的 `ElMessage.success('已删除')` 不会执行，用户只看到错误提示）。

## 2. request-field DELETE /api/jt1078/terminal/delete

- 前端：`web/src/api/jtDevice.ts:63` `params: { id }`，传的是终端数据库主键（调用方 `web/src/views/jtDevice/index.vue:273` `row.id`）
- 后端：`src/handlers/jt1078.rs:434` `Query(q): Query<TerminalQuery>`，DTO `src/handlers/jt1078.rs:27-33` 只有 `device_id`（alias `deviceId`）和 `phone_number`（alias `phoneNumber`），没有 `id`；`src/handlers/jt1078.rs:436-439` 取不到 `phone_number` 即返回 `AppError::business(ErrorCode::Error400, "缺少 phoneNumber")`。WVP 真值也按手机号删（`/tmp/wvpsrc/wvp-GB28181-pro-master/web/src/api/jtDevice.js:48-50` `params: { phoneNumber }`；`.../JT1078TerminalController.java:62` `addDevice(String phoneNumber)`）
- 影响：即使把第 1 条的方法改成 DELETE，参数名 `id` 仍绑不到 `phone_number`，请求返回 **400「缺少 phoneNumber」**，删除依旧失败。

## 3. request-field POST /api/jt1078/terminal/add + /update

- 前端：`web/src/views/jtDevice/TerminalEditDialog.vue:63-71` 表单字段为 `phoneNumber`/`plateNo`/`plateColor`/`model`/`makerId`/`provinceId`/`cityId`，整对象提交（`TerminalEditDialog.vue:100` `await updateJtTerminal(form)`、`:103` `await addJtTerminal(form)`）；字段声明见 `web/src/api/jtDevice.ts:6`（`phoneNumber`）、`:8`（`provinceId`）、`:10`（`cityId`）、`:12`（`makerId`）、`:14`（`plateColor`）、`:15`（`plateNo`）
- 后端：`src/handlers/jt1078.rs:230-240` `TerminalAddBody { phone_number(alias "phoneNumber"), name, device_id, manufacturer, model, sim, vehicle_no }`、`src/handlers/jt1078.rs:242-252` `TerminalUpdateBody` 字段相同 —— 没有 `plateNo`/`plateColor`/`makerId`/`provinceId`/`cityId` 的任何 alias；且 `src/handlers/jt1078.rs:401-405`（add）与 `:421-425`（update）把 `plate_color` 实参写成 `None`、`plate_no` 取自 `vehicle_no`、`maker_id` 取自 `manufacturer`
- 影响：新增/编辑终端时**车牌号、车牌颜色、厂商、省域编码、市域编码填了也不落库**（serde 默认忽略未知字段，静默丢弃），只有 `phoneNumber` 与 `model` 生效；重开对话框这些字段仍是空的/旧值。

## 4. response-field GET /api/jt1078/terminal/list

- 前端：`web/src/api/jtDevice.ts:18` `status?: number`；消费点 `web/src/views/jtDevice/index.vue:27` `<el-tag :type="row.status === 1 ? 'success' : 'info'">{{ row.status === 1 ? '在线' : '离线' }}</el-tag>`
- 后端：`src/handlers/jt1078.rs:342` `"status": t.status`，而 `src/db/jt1078.rs:20` 声明 `pub status: Option<bool>`，JSON 序列化为 `true`/`false`。WVP 真值也是布尔（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/jt1078/bean/JTDevice.java:61` `private boolean status;`）
- 影响：`true === 1` 恒为 false，终端列表的「在线」列**永远显示「离线」**，在线终端也被标成离线。

## 5. response-field GET /api/jt1078/terminal/channel/list

- 前端：`web/src/api/jtDevice.ts:72` `channelName?: string`；消费点 `web/src/views/jtDevice/index.vue:132` `<el-table-column prop="channelName" label="通道名" min-width="160" />`
- 后端：`src/handlers/jt1078.rs:475` `"name": c.name` —— 响应键名是 `name`，从不返回 `channelName`。真值支持后端：WVP `JTChannel` 的字段就是 `name`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/jt1078/bean/JTChannel.java:22` `private String name;`），是前端写错了字段名
- 影响：终端通道表格「通道名」列恒为空白，`channelName` 读到 `undefined`。

## 6. response-field GET /api/jt1078/terminal/channel/list

- 前端：`web/src/api/jtDevice.ts:70` `phoneNumber?: string`、`web/src/api/jtDevice.ts:76` `status?: boolean`；消费点 `web/src/views/jtDevice/index.vue:130` `prop="phoneNumber"`、`:135` `row.status ? '在线' : '离线'`
- 后端：`src/handlers/jt1078.rs:471-480` 组装的每行只有 `id`/`channelId`/`name`/`hasAudio`/`createTime`/`updateTime`，**既无 `phoneNumber` 也无 `status`**
- 影响：终端通道表格「手机号」列恒为空，「状态」列因 `undefined` 恒显示「离线」，与实际通道状态无关。

## 7. query-param GET /api/jt1078/terminal/channel/list

- 前端：`web/src/api/jtDevice.ts:79-85` `getJtChannelList(terminalDbId: number | string)`，`web/src/api/jtDevice.ts:83` `params: { terminalDbId }`；但「终端通道」页的查询按钮把手机号输入框传了进去 —— `web/src/views/jtDevice/index.vue:122` `<el-input v-model="channelPhone" placeholder="筛选 phone" clearable />`、`:125` `@click="loadChannelsFor(channelPhone)"`
- 后端：`src/handlers/jt1078.rs:89-90` `#[serde(alias = "terminalDbId")] pub terminal_db_id: Option<i32>` —— 该查询参数的反序列化目标类型是 `i32`
- 影响：在「终端通道」页输入手机号点「查询」，`terminalDbId=13800138000` 无法反序列化成 `i32`，Axum 在进入 handler 前直接返回 **400 Bad Request**（查询串反序列化失败），查不到通道；只有从终端列表点「通道」按钮（传 `row.id`，`web/src/views/jtDevice/index.vue:243`）这条路径正常。

## 8. request-field POST /api/jt1078/terminal/channel/add

- 前端：`web/src/api/jtDevice.ts:67-77` 的 `JtChannel` 用 `terminalDbId?: number`（`web/src/api/jtDevice.ts:69`）标识所属终端，`addJtChannel(data: Partial<JtChannel>)`（`web/src/api/jtDevice.ts:87-93`）把该对象整体作为 body 发出
- 后端：`src/handlers/jt1078.rs:264-275` `ChannelAddBody { device_id(alias "phoneNumber","deviceId"), name(alias "channelName"), channel_id(alias "channelId"), stream_type }` —— **没有 `terminalDbId`**；`src/handlers/jt1078.rs:523` 用 `get_terminal_by_phone(&device_id)` 定位终端，取不到即返回「终端不存在」（`src/handlers/jt1078.rs:529`）。WVP 真值恰恰用 `terminalDbId`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/jt1078/controller/JT1078TerminalController.java:107-110` `assert channel.getTerminalDbId() != 0`）
- 影响：**当前无调用方**（`addJtChannel` 只在 `web/src/api/jtDevice.ts:87` 定义）。一旦接线，按前端接口给出的 `terminalDbId` 语义提交会得到「终端不存在」，通道加不上。

## 9. request-field POST /api/jt1078/area/circle/add

- 前端：`web/src/views/jtDevice/index.vue:282` `await addJtAreaCircle({ phoneNumber: value, centerLat: 0, centerLon: 0, radiusM: 100, label: '未命名' })`（接口字段 `web/src/api/jtDevice.ts:112` `phoneNumber`、`web/src/api/jtDevice.ts:117` `radiusM`）
- 后端：`src/handlers/jt1078_extra.rs:58` `b.get("phone")`、`src/handlers/jt1078_extra.rs:62` `b.get("radius")`；取不到即 `src/handlers/jt1078_extra.rs:63-65` 返回错误「phone / radius 必填且 radius>0」。WVP 真值的手机号字段是 `phoneNumber`（`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/jt1078/controller/bean/SetAreaParam.java:16`），即后端读取的 `phone` 才是偏离项
- 影响：点「圆形区域 → 新增」必然失败并弹「phone / radius 必填且 radius>0」，圆形围栏建不了（`phoneNumber` 绑不到 `phone`，`radiusM` 绑不到 `radius`）。

## 10. request-field POST /api/jt1078/area/polygon/set

- 前端：`web/src/views/jtDevice/index.vue:297` `await setJtAreaPolygon({ phoneNumber: value, pointsJson: '[]', label: '未命名' })`（接口字段 `web/src/api/jtDevice.ts:112` `phoneNumber`、`web/src/api/jtDevice.ts:122` `pointsJson`）
- 后端：`src/handlers/jt1078_extra.rs:150` `b.get("phone")`、`src/handlers/jt1078_extra.rs:152` `b.get("points")`；取不到即 `src/handlers/jt1078_extra.rs:154-156` 返回「phone 必填」
- 影响：点「多边形区域 → 新增」必然失败并弹「phone 必填」，多边形围栏建不了；即便补上 `phone`，`pointsJson` 也永远绑不到后端的 `points`，点位会存成空数组。

## 11. request-field POST /api/jt1078/route/set

- 前端：`web/src/views/jtDevice/index.vue:313` `await setJtRoute({ phoneNumber: value, waypointsJson: '[]', label: '未命名' })`（接口字段 `web/src/api/jtDevice.ts:201` `phoneNumber`、`web/src/api/jtDevice.ts:203` `waypointsJson`）
- 后端：`src/handlers/jt1078_extra.rs:304` `b.get("phone")`、`src/handlers/jt1078_extra.rs:306` `b.get("waypoints")`；取不到即 `src/handlers/jt1078_extra.rs:308-310` 返回「phone 必填」
- 影响：点「路线 → 新增」必然失败并弹「phone 必填」，路线建不了；即便补上 `phone`，`waypointsJson` 也永远绑不到后端的 `waypoints`。

## 12. response-field GET /api/jt1078/area/circle|polygon|rectangle/query + /api/jt1078/route/query

- 前端：`web/src/views/jtDevice/index.vue:54-58` `prop="phoneNumber"`/`"centerLat"`/`"centerLon"`/`"radiusM"`，`web/src/views/jtDevice/index.vue:81-83` `prop="phoneNumber"`/`"pointsJson"`，`web/src/views/jtDevice/index.vue:106-108` `prop="phoneNumber"`/`"waypointsJson"`（接口声明 `web/src/api/jtDevice.ts:112,115-117,122` 与 `web/src/api/jtDevice.ts:201,203`，全为 camelCase）
- 后端：`src/handlers/jt1078_extra.rs:126`（circle）、`:194`（polygon）、`:280`（rectangle）、`:331`（route）把 `items` 原样下发，元素是 `src/db/jt1078.rs:604-614` `JtAreaCircle`、`:616-624` `JtAreaPolygon`、`:626-637` `JtAreaRectangle`、`:639-647` `JtRoute`，字段为 `phone_number`/`center_lat`/`center_lon`/`radius_m`/`points_json`/`waypoints_json`，**没有任何 `#[serde(rename_all = "camelCase")]`**
- 影响：区域/路线查询能返回数据，但表格里「手机号 / 中心纬度 / 中心经度 / 半径(米) / 点位 JSON / 途经点 JSON」列全部空白，只有 `id` 与 `label` 能显示。

## 13. request-field POST /api/jt1078/area/rectangle/add

- 前端：`web/src/api/jtDevice.ts:183-189` `addJtAreaRectangle(data: Partial<JtArea>)`，`JtArea` 的矩形字段是 `ltLat`/`ltLon`/`rbLat`/`rbLon`（`web/src/api/jtDevice.ts:118-121`）加 `phoneNumber`（`web/src/api/jtDevice.ts:112`）
- 后端：`src/handlers/jt1078_extra.rs:209` `b.get("phone")`、`src/handlers/jt1078_extra.rs:211-214` `b.get("leftTopLat")`/`"leftTopLon"`/`"rightBottomLat"`/`"rightBottomLon"`
- 影响：**当前无调用方**（`addJtAreaRectangle` 只在 `web/src/api/jtDevice.ts:183` 定义，`web/src` 内无接线）。一旦接线，因 `phone` 缺失会被 `src/handlers/jt1078_extra.rs:215-217` 判为「phone 必填」，且 `ltLat`/`ltLon`/`rbLat`/`rbLon` 绑不上后端键名，四个角点坐标全部落成 0。
