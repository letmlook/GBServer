# region.ts 契约审计

> **状态：已修复（2026-09-12 第三十五轮）**。9 条全部落地，并补上了**整个模块缺失的界面**：
> 此前 12 个 API 函数里只有 `getRegionTreeList` 有调用方（地图页），行政区划/业务分组的
> 增删改在 Vue3 前端**完全不可达**。本轮新增 `web/src/views/region/`（行政区划 + 业务分组
> 两个 tab、树形展示、增删改、按名称/国标编码过滤、一键同步行政区划），并接到路由与侧边栏。
>
> 契约层面：
> * `deleteRegion` / `deleteGroup` 改用 DELETE（实测 GET→405、DELETE→200）；
> * `getRegionTreeQuery` / `getGroupTreeQuery` 返回类型改为分页对象
>   `{total, list, pageNum, pageSize, pages}`（与 WVP 的 `PageInfo` 一致）；
> * `RegionUpdate` / `GroupUpdate` 补 camelCase；`parent_id` 改为 `COALESCE(?, parent_id)`
>   —— 此前"只改名字"会把节点从子级抬到根级（`parent_id` 被写成 NULL），
>   移到顶级改用前端既有的 `-1` 哨兵（`build_region_tree` 本来就认它）；
> * `tree/query` 补齐 `parentId`（并支持 WVP 的 `query` 关键字）与分页；
> * 顺带补 `GET /api/group/one`（此前只有 region 有 one 接口）。
>
> 新页面还暴露了两个只有真渲染才看得见的问题（已修）：两个 tab 的 `el-tree`
> 不能共用一个 `ref` 名（Vue 只保留最后注册的那个，`filter()` 会作用在隐藏的那棵树上），
> 也不能共用一个 `data`（`el-tabs` 两个 pane 同时存在 DOM 里，切换后隐藏的树也会
> 跟着渲染另一棵树的数据）。

审计对象：`web/src/api/region.ts`（12 个导出函数：8 个 region + 4 个 group）。
后端路由：`src/router.rs`（`api_protected` 链，`src/router.rs:69` 起）。
真值交叉验证：WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master`。

调用方普查（`grep -rn "<函数名>" web/src`）：**12 个函数中只有 `getRegionTreeList` 有调用方**
（`web/src/views/map/index.vue:62` 导入、`:125` 调用）；其余 11 个（含全部 group 函数）
在 `web/src` 内均**当前无调用方**，因此下列问题除第 1 条外都是**潜在契约缺陷**，
一旦接线/回归即会暴露。

---

## 1. http-method GET /api/region/delete

- 前端：`web/src/api/region.ts:64-70` `export function deleteRegion(id: number | string) { return request<WvpResult>({ method: 'get', url: '/region/delete', params: { id } }) }`（`method: 'get'` 在第 66 行，`url` 在第 67 行）
- 后端：`src/router.rs:364` `.route("/api/region/delete", delete(stub::region_delete))`；handler 注释 `src/handlers/stub.rs:270` 明确写 `/// DELETE /api/region/delete?id= 或 deviceId=`
- 交叉验证：WVP 上游同为 DELETE —— `.../gb28181/controller/RegionController.java:89` `@DeleteMapping("/delete")`；WVP 前端 `.../web/src/api/region.js:18-26`（`method: "delete"` 在第 20 行）。仓库内 `mock/docs/test-cases.md:42` 也记为 `DELETE /api/region/delete`
- 影响：前端用 GET 打只注册了 DELETE 的路由，Axum 返回 **405 Method Not Allowed**，删除行政区划永远失败。当前无调用方（`deleteRegion` 仅在 region.ts 内定义）。

## 2. http-method GET /api/group/delete

- 前端：`web/src/api/region.ts:122-128` `export function deleteGroup(id: number | string) { return request<WvpResult>({ method: 'get', url: '/group/delete', params: { id } }) }`（`method: 'get'` 在第 123 行，`url` 在第 125 行）
- 后端：`src/router.rs:385` `.route("/api/group/delete", delete(stub::group_delete))`；handler 注释 `src/handlers/stub.rs:578` `/// DELETE /api/group/delete?id=`
- 交叉验证：`.../gb28181/controller/GroupController.java:75` `@DeleteMapping("/delete")`；WVP 前端 `.../web/src/api/group.js:32-40`（`method: 'delete'` 在第 34 行）；`mock/docs/test-cases.md:52` 记为 `DELETE /api/group/delete`
- 影响：同第 1 条，删除分组返回 **405**，功能不可用。当前无调用方。

## 3. request-field POST /api/region/update

- 前端：`web/src/api/region.ts:56-62` `updateRegion(data: Partial<Region>)` 直接把 `Region` 对象当 body 发；该接口字段为 camelCase —— `web/src/api/region.ts:6` `deviceId?: string`、`:8` `parentId?: number`、`:9` `parentName?: string`
- 后端：`src/db/region.rs:31-37` `pub struct RegionUpdate { pub id: Option<i64>, pub device_id: Option<String>, pub name: Option<String>, pub parent_id: Option<i32>, pub parent_device_id: Option<String> }` —— **没有任何 `#[serde(alias = ...)]`**；对比同文件 `RegionAdd` 在 `src/db/region.rs:20-28` 明确带了 `#[serde(alias = "deviceId")]` / `"parentId"` / `"parentDeviceId"`
- 消费点：`src/handlers/stub.rs:389` `Json(body): Json<region::RegionUpdate>`，随后 `src/handlers/stub.rs:393-402` 把 `body.device_id` / `body.parent_id` / `body.parent_device_id` 原样传给 `region::update`
- SQL：`src/db/region.rs:177`（MySQL/SQLite）、`src/db/region.rs:189`（PostgreSQL）`... SET device_id = COALESCE(?, device_id), name = COALESCE(?, name), parent_id = ?, parent_device_id = COALESCE(?, parent_device_id) ...` —— `parent_id` 没有 COALESCE 保护
- 交叉验证：WVP `Region.java` 的 JSON 字段为 camelCase（`private String deviceId;` / `private Integer parentId;`），即该接口约定就是 camelCase body
- 影响：前端提交 `deviceId` / `parentId` / `parentDeviceId` 全部落到 `None`，只有 `name` 生效；且 `parent_id` 被直接绑定为 NULL，**编辑区域名称会把该区域从子级搬到根级**（`build_region_tree` 在 `src/handlers/stub.rs:250-253` 把 `parent_id.is_none()` 当根节点）。当前无调用方。

## 4. request-field POST /api/group/update

- 前端：`web/src/api/region.ts:114-120` `updateGroup(data: Partial<Group>)` 直接发 `Group` 对象，字段 camelCase —— `web/src/api/region.ts:81` `deviceId?: string`、`:83` `parentId?: number`、`:84` `parentName?: string`
- 后端：`src/db/group.rs:37-45` `pub struct GroupUpdate { pub id: Option<i64>, pub device_id: Option<String>, pub name: Option<String>, pub parent_id: Option<i32>, pub parent_device_id: Option<String>, pub business_group: Option<String>, pub civil_code: Option<String> }` —— **没有任何 alias**；对比 `GroupAdd` 在 `src/db/group.rs:22-34` 带了 `"deviceId"` / `"parentId"` / `"parentDeviceId"` / `"businessGroup"` / `"civilCode"` 全部 alias
- 消费点：`src/handlers/stub.rs:559` `Json(body): Json<group::GroupUpdate>`，`src/handlers/stub.rs:563-573` 原样转发
- SQL：`src/db/group.rs:195`（MySQL/SQLite）、`src/db/group.rs:209`（PostgreSQL）、`src/db/group.rs:223` 三处均为 `... business_group = COALESCE(?, business_group), civil_code = COALESCE(?, civil_code) ... parent_id = ?, ...`（`parent_id` 无 COALESCE）
- 交叉验证：WVP `Group.java` 字段为 camelCase（`deviceId` / `parentId` / `parentDeviceId` / `businessGroup` / `civilCode`）
- 影响：`deviceId` / `parentId` / `parentDeviceId` / `businessGroup` / `civilCode` 全部绑定失败，只更新 `name`；同时 `parent_id` 被清成 NULL，**分组会被抬到根级**。当前无调用方。

## 5. query-param GET /api/region/tree/query

- 前端：`web/src/api/region.ts:24-30` `getRegionTreeQuery(parentId?: number)` → `params: { parentId }`（第 28 行）
- 后端：`src/router.rs:381` `.route("/api/region/tree/query", get(stub::region_tree_query))`；handler `src/handlers/stub.rs:466-469` 的 DTO 是 `Query<PageQuery>`，而 `PageQuery` 定义在 `src/handlers/stub.rs:460-464`，**只有 `page` 和 `count`，没有 `parentId`**
- 交叉验证：WVP 同名接口用的是 `query` 参数而不是 `parentId`（`.../web/src/api/region.js:84-99`，`query: query` 在第 90 行）
- 影响：`parentId` 被 serde 静默丢弃（`PageQuery` 未开启 `deny_unknown_fields`），接口退化为「全部区域的第 1 页」而非「某父节点的子区域」，前端拿不到想要的分支数据。当前无调用方。

## 6. query-param GET /api/group/tree/query

- 前端：`web/src/api/region.ts:98-104` `getGroupTreeQuery(parentId?: number)` → `params: { parentId }`（第 102 行）
- 后端：`src/router.rs:387` `.route("/api/group/tree/query", get(stub::group_tree_query))`；handler `src/handlers/stub.rs:623-626` 用同一个 `PageQuery`（`src/handlers/stub.rs:460-464`，仅 `page` / `count`）
- 交叉验证：WVP `.../web/src/api/group.js:52-65` 用 `query` 参数
- 影响：同第 5 条，`parentId` 无效，分组树只返回全量第 1 页。当前无调用方。

## 7. response-field GET /api/region/tree/query

- 前端：`web/src/api/region.ts:24-30` 返回类型声明为 `request<WvpResult<Region[]>>`，即期望 `data` 是**数组**
- 后端：`src/handlers/stub.rs:490-493` 实际返回 `serde_json::json!({ "total": total, "list": list })`，即 `data` 是**对象**
- 交叉验证：WVP 上游该接口返回 `PageInfo<Region>`（`.../controller/RegionController.java:72-76`），同样是分页对象，说明是**前端类型声明错**
- 影响：任何形如 `res.data.map(...)` / `.length` 的消费都会在运行时得到 `undefined` 并抛错。当前无调用方。

## 8. response-field GET /api/group/tree/query

- 前端：`web/src/api/region.ts:98-104` 返回类型声明为 `request<WvpResult<Group[]>>`，期望数组
- 后端：`src/handlers/stub.rs:649-652` 返回 `json!({ "total": total, "list": list })`，实际是对象
- 交叉验证：WVP `.../web/src/api/group.js:52-65` 对应接口为分页对象
- 影响：同第 7 条。当前无调用方。

---

## 已核对但**不属于**不一致的项（避免误报）

- `GET /api/region/tree/list`（`web/src/api/region.ts:17-22` ↔ `src/router.rs:363`）：后端 `src/handlers/stub.rs:244-268` 返回 `id/deviceId/name/parentId/parentDeviceId/createTime/updateTime/children`，与地图页 `web/src/views/map/index.vue` 消费的 `node.deviceId`、`node.name` 及 `el-tree` 的 `children` 一致，可用。
- `GET /api/region/one`（`web/src/api/region.ts:40-46`，参数 `id` ↔ `src/handlers/region.rs:111-119` + `RegionOne { id }` at `src/handlers/region.rs:149-152`）：字段名一致。
- `POST /api/region/add`（`web/src/api/region.ts:48-54` ↔ `src/db/region.rs:20-28`）：`RegionAdd` 已带 camelCase alias，一致。
- `POST /api/group/add`（`web/src/api/region.ts:106-112` ↔ `src/db/group.rs:22-34`）：`GroupAdd` 已带 camelCase alias，一致。
- `GET /api/region/path`（`web/src/api/region.ts:32-38`，参数 `id` ↔ `src/handlers/stub.rs:434-437` 的 `RegionQuery { id }`）：后端接受 `id`，一致。
- `GET /api/region/sync`（`web/src/api/region.ts:72-77` 期望 `{ count }` ↔ `src/handlers/region.rs:142-146` 返回 `count`）：一致。


---

## 修复对照（第三十五轮）

| # | 问题 | 修复 |
|---|------|------|
| 1 | `deleteRegion` 用 GET，后端只注册 DELETE → 405 | 前端改 DELETE（实测 `GET=405 / DELETE=200`） |
| 2 | `deleteGroup` 同上 | 同上 |
| 3 | `RegionUpdate` 无 camelCase 别名 → 只有 `name` 生效，`parent_id` 被写 NULL（节点被抬到根级） | DTO 加 `rename_all = "camelCase"`；`parent_id` 改 `COALESCE(?, parent_id)`；移到顶级用 `-1` 哨兵 |
| 4 | `GroupUpdate` 同上（另有 `businessGroup`/`civilCode` 丢失） | 同上 |
| 5 | `tree/query` 的 DTO 只有 page/count，`parentId` 被静默丢弃 | 新增 `TreeNodeQuery`（`parentId` 别名 + WVP 的 `query`），真正按父节点/关键字过滤 |
| 6 | 同上（group） | 同上 |
| 7 | `getRegionTreeQuery` 声明返回 `Region[]`，实际是 `{total,list}` | 前端类型改 `TreeNodePage<Region>`；后端补 `pageNum/pageSize/pages` |
| 8 | 同上（group） | 同上 |

本轮另外补上的两项（审计里没有单列，但属于"功能不完整"）：

* `GET /api/group/one?id=` 此前**不存在**（只有 `region/one`），已按同样形状补上；
* **整个模块没有界面**：12 个 API 里 11 个无调用方。新增
  `web/src/views/region/index.vue` + `NodeEditDialog.vue`（行政区划 / 业务分组两个 tab、
  树形展示、新增子节点、编辑、删除、按名称或国标编码过滤、一键同步行政区划），
  注册路由 `/region` 与侧边栏入口；Playwright 新增 `region.spec.ts` 5 条。
