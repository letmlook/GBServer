# log.ts 契约审计

> **状态：已修复（2026-09-12 第四十三轮补充）**：`gb_log.id` 在 postgres 脚本里是
> `serial`（INT4），而 `LogEntry.id` 是 `i64` → postgres 下 `/api/log/list` 恒定
> 500（`mismatched types; Rust type i64 ... not compatible with SQL type INT4`），
> CSV 导出同样失败。已把该列改为 `bigserial` 并补启动期幂等迁移
> `ensure_pg_column_types()`；`gb_audit_log` 的 `elapsed_ms` 写入也去掉了
> postgres 下必失败的 `?` 版本。详见 [docs/DB_DIALECT_NOTES.md](../DB_DIALECT_NOTES.md)。

## 1. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:41` `cpu?: number`
- 前端消费：`web/src/views/operations/systemInfo.vue:16` `{{ info.cpu ?? 0 }}%`，`:17` `:percentage="info.cpu ?? 0"`
- 后端：`src/handlers/server.rs:857-859` `let cpu_data: Vec<serde_json::Value> = bufs.cpu...map(|(t, v)| json!({"time": t, "data": v}))`；`src/handlers/server.rs:878` `"cpu": cpu_data,`
- 影响：后端返回的是环形缓冲数组 `[{time,data}]`（与 WVP `SystemAllInfo.cpu` 为 `List<Object>`、`/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/common/SystemAllInfo.java:7` 一致），前端按 number 用：CPU 卡片数值渲染成 "[object Object],..." 形式，`el-progress` 的 `percentage` 收到数组（非 number，进度条失效并触发 prop 校验告警）。后端标量 CPU 百分比在 `cpu_usage`（`src/handlers/server.rs:884`）。

## 2. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:46-52` `memory?: { total?: number; used?: number; free?: number; mem?: { data: number; time: string }[] }`
- 前端消费：`web/src/views/operations/systemInfo.vue:24` `{{ formatSize(info.memory?.used) }} / {{ formatSize(info.memory?.total) }}`，`:76-79` `const m = info.value.memory; if (!m?.total || m.used == null) return 0`
- 后端：`src/handlers/server.rs:860-862` 只产出 `mem_data`（`[{time,data}]` 分数），`src/handlers/server.rs:885` 只返回 `"mem_usage": mem_pct`；`src/handlers/server.rs:877-887` 整个响应对象中没有 `memory` 键
- 影响："系统信息"页内存卡片恒为 `0%`，明细行恒显示 `- / -`（后端实际给的是 `mem` 数组 + `mem_usage` 百分比）。

## 3. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:53` `disk?: { total: number; used: number; free: number; path: string }[]`
- 前端消费：`web/src/views/operations/systemInfo.vue:31` `{{ formatSize(info.disk?.[0]?.used) }} / {{ formatSize(info.disk?.[0]?.total) }}`，`:82-84` `const d = info.value.disk?.[0]; if (!d?.total) return 0; Math.round((d.used / d.total) * 100)`
- 后端：`src/handlers/server.rs:833-837` `json!({"path": path, "free": free_gb, "use": used_gb})`
- 影响：元素键是 `use`（已用 GB）且没有 `total`，前端读的 `d.used` / `d.total` 恒为 `undefined` → "系统信息"页磁盘卡片恒为 `0%`，明细恒显示 `- / -`。

## 4. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:55` `network?: { name: string; rx: number; tx: number }[]`
- 后端：`src/handlers/server.rs:863-865` `net_data...json!({"time": t, "out": out, "in": in_})`，`src/handlers/server.rs:881` `"net": net_data,`（无 `network` 键）
- 影响：前端声明的键名（`network`）与元素结构（`name/rx/tx`）和后端（`net` / `time,out,in`）都不一致，按该类型取值恒为 `undefined`。当前 `web/src` 内无任何代码读取 `info.network` / `info.net`（`grep -rn "\.network\b\|info\.net\b" web/src` 仅命中 `web/src/api/log.ts` 自身），故目前无渲染影响，属类型契约错误。

## 5. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:58-59` `version?: string` / `buildTime?: string`
- 前端消费：`web/src/views/operations/systemInfo.vue:6` `{{ info.version ?? '加载中...' }}`，`:57` `{{ info.version ?? '-' }}`，`:58` `{{ info.buildTime ?? '-' }}`
- 后端：`src/handlers/server.rs:877-887` 的响应对象只含 `cpu/mem/disk/net/netTotal/uptime/cpu_usage/mem_usage/disk_usage`，不含 `version`、`buildTime`
- 影响："系统信息"页副标题恒显示"加载中..."，"构建信息"的版本与构建时间两行恒为 `-`（版本/构建名实际由 `src/handlers/server.rs:757-758` 的 `/api/server/system/configInfo` 返回，本页未调用）。

## 6. response-field GET /api/server/system/info
- 前端：`web/src/api/log.ts:60-64` `mediaServerCount?: number` / `deviceOnline?: number` / `deviceTotal?: number` / `channelOnline?: number` / `channelTotal?: number`
- 前端消费：`web/src/views/operations/systemInfo.vue:103-105` `info.value.mediaServerCount ?? 0`、`${info.value.deviceTotal ?? 0} / ${info.value.deviceOnline ?? 0}`、`${info.value.channelTotal ?? 0} / ${info.value.channelOnline ?? 0}`；`web/src/views/dashboard/index.vue:186` `deviceOnline.value = info.value.deviceOnline ?? 0`（展示于 `:16`、`:65`、`:68`，并参与 `:272` 在线率、`:280` 计算）
- 后端：`src/handlers/server.rs:877-887` 不返回上述任一键；设备/通道计数在另一个端点 `src/handlers/server.rs:959-964`（`/api/server/resource/info` 的 `device/channel/push/proxy`）
- 影响："系统信息"页"资源统计"三行恒为 `0`、`0 / 0`；dashboard"在线设备"卡片恒为 `0`、离线数恒等于设备总数、在线率恒为 `0%`。

## 7. query-param GET /api/log/list
- 前端：`web/src/views/operations/historyLog.vue:149` `params.set('format', 'csv')`，`:152-153` `const url = \`/api/log/list?${params.toString()}\`` + `fetch(url, { headers: { 'access-token': token } })`（该导出路径为页面内直接 fetch，未走 `log.ts` 的 `getLogFile`）
- 后端：`src/handlers/stub.rs:656-669` `LogListQuery` 字段只有 `page/count/query/log_type(type)/start_time(startTime)/end_time(endTime)/level`，无 `format`；`src/handlers/stub.rs:700-705` `log_list` 始终返回 `{"total":...,"list":...,"page":...,"count":...}` JSON
- 影响："历史日志"页"导出"按钮拿到的是 `WVPResult` JSON 文本，却被保存成 `gbserver-log-<ts>.csv`（`web/src/views/operations/historyLog.vue:156-162`）；因 HTTP 200 使 `r.ok` 为真，`:165-167` 的失败回退提示不会触发，用户无感知地得到一个内容是 JSON 的 .csv 文件。可用的导出端点是 `src/handlers/stub.rs:736`（`GET /api/log/file/gbserver-log.csv`），前端 `web/src/api/log.ts:32-38` 的 `getLogFile` 当前无调用方。

---

已核对未发现不一致：`GET /api/log/list` 的 `page/count/query/level/startTime/endTime` 与 `LogListQuery`（`src/handlers/stub.rs:656-669`，`start_time`/`end_time` 有 `#[serde(alias="startTime"/"endTime")]`）一致，响应 `total/list`（`src/handlers/stub.rs:700-705`）与 `LogListQuery` 调用方读取方式（`web/src/views/operations/historyLog.vue:110-111`、`web/src/views/operations/realLog.vue:72,97`）一致，`LogEntry` 的 `id/time/level/logger/thread/message/source`（`src/db/log.rs:18-26`）与 `LogRecord`（`web/src/api/log.ts:4-13`）一致；`GET /api/log/file/:file_name`（`src/router.rs:389` / `src/handlers/stub.rs:725`）、`GET /api/server/system/configInfo`（`src/router.rs:255`）、`GET /api/server/resource/info`（`src/router.rs:265`）、`GET /api/server/info`（`src/router.rs:264`）的 method 与路径均与前端一致，后三个的 API 函数（`web/src/api/log.ts:74,81,88`）当前无调用方。`netTotal`（`web/src/api/log.ts:56` 对 `src/handlers/server.rs:882`）一致。

---

> **状态：已修复（2026-09-12 第三十七轮）**。7 条全部落地，真实后端验证。
>
> 「系统信息」页此前**四个卡片全部读不到值**：CPU 卡片把历史采样数组当数字渲染
> （显示 `[object Object]`、进度条 percentage 收到数组而失效）、内存卡片读后端
> 根本不存在的 `memory` 键（恒 0%）、磁盘卡片读 `disk[].total/used` 而后端只有
> `use`(GB)（恒 0%）、版本/构建时间恒 `-`、资源统计三行恒 0；
> dashboard 的「在线设备」也因此恒 0。
>
> 另有一个"用户拿到假 CSV"的问题：「历史日志」页的「导出」按钮发
> `?format=csv`，后端把 `format` 当未知参数静默丢弃、照样返回 JSON，
> 而前端把它存成 `.csv` —— 因为 HTTP 200 让 `r.ok` 为真，失败提示也不会触发。

## 修复对照（第三十七轮）

| # | 问题 | 修复 / 证据 |
|---|------|------|
| 1 | 前端把 `cpu`（历史采样数组）当 number | 前端类型改为 `{time,data}[]`；页面卡片改读标量 `cpu_usage` |
| 2 | 后端没有 `memory` 键 → 内存卡片恒 0% | 新增 `memory: {total, used, free, mem[]}`（字节 + 历史采样数组） |
| 3 | `disk[]` 只有 `use`(GB)/`free`(GB)，前端读 `used`/`total` | 每个挂载点同时给 `total`/`used`（字节，系统信息页）与 `use`/`free`（GB，dashboard 柱状图） |
| 4 | 前端声明 `network`，后端返回 `net` | 后端补 `network: [{name, rx, tx}]`（真实的聚合速率） |
| 5 | 响应没有 `version`/`buildTime` → 恒 `-`、副标题恒「加载中...」 | 补 `version`（`CARGO_PKG_VERSION`）与 `buildTime`（当前 exe 的真实 mtime） |
| 6 | 响应没有资源计数 → 资源统计恒 0、dashboard 在线设备恒 0 | 补 `mediaServerCount` / `deviceOnline` / `deviceTotal` / `channelOnline` / `channelTotal`（真实 DB 计数） |
| 7 | `log/list?format=csv` 返回 JSON，前端存成 `.csv` | `log_list` 支持 `format=csv`：返回 `text/csv` + `Content-Disposition: attachment`，正文带 UTF-8 BOM（Excel 不乱码），内容为**当前筛选条件下的全部**日志（复用 `db::log::export`） |

**实测**（真实后端）：

```
GET /api/server/system/info
  cpu_usage=35.1  mem_usage=48.5  disk_usage=1.27
  memory={total:17179869184, used:8333033472, free:8846835712, mem:[…]}
  disk[0]={path:"/", total:994662584320, used:12633366528, use:11.77(GB), free:914.59(GB)}
  network=[{name:"total", rx:0.00048, tx:1.0728}]
  version="0.1.0"  buildTime="2026-09-13 01:05:59"
  mediaServerCount=1 deviceOnline=2 deviceTotal=2 channelOnline=4 channelTotal=4
GET /api/log/list?format=csv&level=INFO
  content-type: text/csv; charset=utf-8
  content-disposition: attachment; filename="gbserver-log-20260913010620.csv"
  正文以 UTF-8 BOM 开头，4960 行（当前筛选下的全部 INFO 日志）
GET /api/log/list?page=1&count=3 → 仍是 JSON 分页契约
Playwright → 新增 operations.spec.ts 2 条；整套 55 passed
```
