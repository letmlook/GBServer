# alarm.ts 契约审计

审计对象：`web/src/api/alarm.ts`（8 个导出函数）与 `src/router.rs` / `src/handlers/alarm.rs` / `src/handlers/parity_extras.rs`。
真值交叉验证：WVP-PRO Java `src/main/java/com/genersoft/iot/vmp/vmanager/alarm/AlarmController.java`。

结论摘要：8 个前端函数中 3 个 HTTP method 与后端注册不一致（直接 405），2 个请求体/参数传递方式与后端 DTO 不匹配，1 个请求参数字段名后端不存在，2 个已接收的筛选参数在 SQL 中被丢弃，1 个响应键名前后端不一致。

---

## 1. http-method GET /api/alarm/clear

- 前端：`web/src/api/alarm.ts:56` `export function clearAlarm(id: number | string) {` → `:58` `method: 'get',` / `:59` `url: '/alarm/clear',`
- 后端：`src/router.rs:936` `.route("/api/alarm/clear", delete(parity_extras::alarm_clear))`
- 影响：`web/src/views/alarm/index.vue:141` 的每行「清除」按钮走 `clearAlarm(row.id)`，实际发出 `GET /api/alarm/clear`，Axum 对该路径只注册了 DELETE，直接返回 405 Method Not Allowed；`request.ts` 响应拦截器把非 401 错误弹出 `ElMessage.error`，紧随其后的 `ElMessage.success('已清除')` 永远不会执行。即告警列表页的「清除」功能 100% 失败。

## 2. request-field DELETE /api/alarm/clear

- 前端：`web/src/api/alarm.ts:59` `url: '/alarm/clear',` → `:60` `params: { id }`（期望「清除指定 id 的告警」）
- 后端：`src/handlers/parity_extras.rs:25` `pub async fn alarm_clear(` → `:27` `) -> Json<WVPResult<serde_json::Value>> {` → `:28` `match db::alarm::delete_all(&state.pool).await`；`src/db/alarm.rs:327` `pub async fn delete_all(pool: &Pool)` → `:333` `sqlx::query("DELETE FROM gb_device_alarm")`（无 WHERE，清空整表）
- 影响：后端 handler 不接收任何参数，`id` 被完全忽略且执行的是全表删除。即便第 1 条的方法问题被修复，用户在单行点「清除」也会把**所有设备**的告警记录一并删掉，且响应 `{"cleared": n}` 不含前端期望的语义。WVP-PRO 的 `DELETE /api/alarm/clear`（`AlarmController.java:59-67`）同样按下拉筛选条件（alarmType/beginTime/endTime）清空，从不接收单个 id。

## 3. http-method POST /api/alarm/batch

- 前端：`web/src/api/alarm.ts:79` `export function batchAlarm(data: { ids: (number | string)[]; action: 'delete' | 'clear' | 'handle' }) {` → `:81` `method: 'post',` / `:82` `url: '/alarm/batch',`
- 后端：`src/router.rs:904` `.route("/api/alarm/batch", delete(alarm::alarm_batch_delete))`
- 影响：`web/src/views/alarm/index.vue:155` `await batchAlarm({ ids: ..., action: 'clear' })` 发出的 `POST /api/alarm/batch` 返回 405，页面顶部「批量清除」按钮在确认弹窗后必然报错，选中的告警一条也不会被处理。WVP-PRO 对应接口是 `DELETE /api/alarm/delete` + `@RequestBody List<Long> ids`（`AlarmController.java:51-56`），也不是 POST。

## 4. request-field DELETE /api/alarm/batch

- 前端：`web/src/api/alarm.ts:79` `data: { ids: (number | string)[]; action: 'delete' | 'clear' | 'handle' }`，调用点 `web/src/views/alarm/index.vue:155` 传 `action: 'clear'`
- 后端：`src/handlers/alarm.rs:366` `pub struct AlarmBatchDelete {` → `:367` `pub ids: Vec<i64>,`（无 `action` 字段，serde 默认忽略未知字段）
- 影响：后端唯一实现是 `src/handlers/alarm.rs:387` `DELETE FROM gb_device_alarm WHERE id = ?` 的循环删除，`action` 无任何分支。若仅修复第 3 条的方法不一致，用户点「批量清除」的后果将是**永久删除**这些告警，而不是「清除（标记/归档）」；`action: 'handle'` 同样会被当成删除。

## 5. request-field POST /api/alarm/handle

- 前端：`web/src/api/alarm.ts:71` `export function handleAlarm(data: { id: number | string; result: string }) {` → `:75` `params: data`（作为 URL query string 发送，请求体为空）
- 后端：`src/handlers/alarm.rs:312` `Json(body): Json<AlarmHandleBody>,`（要求 `Content-Type: application/json` 的请求体）
- 影响：`web/src/views/alarm/index.vue:135` `await handleAlarm({ id: row.id ?? 0, result: value })` 只发 query 参数、不带 JSON body，Axum 的 `Json` 提取器会以 415 Unsupported Media Type 拒绝，弹窗里输入的处理结果无法提交，「处理」按钮必然失败。

## 6. request-field POST /api/alarm/handle

- 前端：`web/src/api/alarm.ts:71` 字段 `result: string`（语义为「处理结果」文本；`Alarm` 接口在 `:28` 也声明了 `handleResult?: string`）
- 后端：`src/handlers/alarm.rs:302-307` DTO 仅含 `id` / `handle_user`(alias `handleUser`) / `handled`，无 `result`；落库时 `src/handlers/alarm.rs:324` `crate::db::alarm::set_handled(&state.pool, id, handle_user, &now)` 只写 `handle_user`（`src/db/alarm.rs:452` `UPDATE gb_device_alarm SET handled = 1, handle_user = ?, handle_time = ?`）
- 影响：即使按第 5 条改成 JSON body 提交，`result` 仍会被 serde 丢弃，用户填写的处理结论不落库、任何接口也不返回 `handleResult`，前端 `Alarm.handleResult` 永远是 `undefined`。

## 7. http-method GET /api/alarm/before/:time

- 前端：`web/src/api/alarm.ts:41` `export function getAlarmBefore(params: { time: string; page?: number; count?: number }) {` → `:43` `method: 'get',` / `:44` `url: \`/alarm/before/${params.time}\`,`
- 后端：`src/router.rs:906` `.route("/api/alarm/before/:time", delete(alarm::alarm_delete_before_time))`；handler `src/handlers/alarm.rs:421` 返回 `{"deleted": n}`（`:439`）
- 影响：GET 与后端唯一注册的 DELETE 冲突，调用即 405。另外前端声明返回 `{ total, list }`，而后端返回 `{ deleted }`，即便方法对齐也读不到数据。当前无调用方（`grep -rn getAlarmBefore web/src` 仅命中 `alarm.ts:41` 定义本身，无任何页面 import），暂无用户可见影响。

## 8. query-param GET /api/alarm/list

- 前端：`web/src/api/alarm.ts:7` `query?: string`（关键字），调用点 `web/src/views/alarm/index.vue:109` `query: query.query,`
- 后端：`src/handlers/alarm.rs:12-29` `pub struct AlarmQuery {` 只有 `page` / `count` / `device_id`(alias `deviceId`) / `channel_id`(alias `channelId`) / `alarm_method` / `alarm_type` / `start_time` / `end_time` / `handled`，无 `query` 字段
- 影响：`web/src/views/alarm/index.vue:17` 的「关键字（设备ID / 描述）」输入框发出的 `query=xxx` 被 serde 静默忽略，后端不会按设备 ID 或描述过滤，用户输入关键字后列表结果与不填时完全一样。WVP-PRO 的 `/api/alarm/list`（`AlarmController.java:42-46`）也不存在 `query` 参数（只有 alarmType/beginTime/endTime）。

## 9. query-param GET /api/alarm/list

- 前端：`web/src/api/alarm.ts:8-9` `startTime?: string` / `endTime?: string`，由 `web/src/views/alarm/index.vue:95-96` 从日期选择器赋值、`:110-111` 提交
- 后端：`src/handlers/alarm.rs:24-27` 声明了 `#[serde(alias = "startTime")] pub start_time` / `#[serde(alias = "endTime")] pub end_time`，但 SQL 的 WHERE 只用到 device_id / channel_id / alarm_type / alarm_method（`:128-131`，postgres 分支 `:47-50`），COUNT 语句同理（`:181-184`、`:100-103`），`q.start_time` / `q.end_time` 从未被使用
- 影响：`web/src/views/alarm/index.vue:20` 的时间范围选择器完全无效——选择任意时间段后后端仍返回全部告警（分页 total 也不变）。注意真值侧 WVP-PRO 用的是 `beginTime`/`endTime`（`AlarmController.java:40-41`），前端连参数名都与上游惯例不同。

## 10. response-field GET /api/alarm/list + /api/alarm/detail/:id

- 前端：`web/src/api/alarm.ts:18` `alarmLevel?: string`；消费点 `web/src/views/alarm/index.vue:40` `<el-table-column prop="alarmLevel" label="级别" width="100" />`、`web/src/views/alarm/index.vue:126` `级别: ${row.alarmLevel}`、`web/src/views/dashboard/index.vue:67` `a.alarmLevel === '警告'`、`:124` `toneLevel(a.alarmLevel)`、`:129` `{{ a.alarmLevel ?? '信息' }}`
- 后端：`src/handlers/alarm.rs:83` `"alarmPriority": alarm_priority,`（list，postgres 分支 `:164` 同）与 `src/handlers/alarm.rs:238` `"alarmPriority": alarm_priority,`（detail，postgres 分支 `:283` 同）——响应键为 `alarmPriority`，从不返回 `alarmLevel`
- 影响：告警列表页「级别」列、查看弹窗的「级别」行全部显示为空；仪表盘最近告警的级别标签恒为兜底值「信息」，`:67` 的「弱信号」计数恒为 0（`undefined === '警告'` 为 false）。这是 `Alarm` 接口与后端响应键名不一致导致的读 `undefined`，与后端 snake_case/camelCase 无关（后端此处已手工输出 camelCase，只是键名选错）。
