# syCamera.ts 契约审计

审计对象：`web/src/api/syCamera.ts`（2 个导出函数：`cameraListWithChild`、`cameraList`）× `src/router.rs` × `src/handlers/sy_camera.rs` × `src/db/device.rs`。

**路由与 method 核对通过，无 route-missing / http-method 问题**：前端两处均为 `method: 'get'`（`web/src/api/syCamera.ts:42`、`web/src/api/syCamera.ts:58`），后端均注册为 GET（`src/router.rs:919` `/api/sy/camera/list-with-child` → `get(sy_camera::camera_list_with_child)`；`src/router.rs:918` `/api/sy/camera/list` → `get(sy_camera::camera_list)`），都在 `api_protected` 内受鉴权保护（`src/router.rs:1226` 的鉴权回归测试同时断言 `/api/sy/camera/list` 未鉴权返回 401）。URL 前缀也对得上：dev 下 axios `baseURL='/dev-api'`（`web/.env.development:2`）经 Vite `rewrite: /dev-api → /api`（`web/vite.config.ts:58`），prod 下 `baseURL='/api'`（`web/.env.production:2`）。

以下 6 条是逐条比对后仍然真实存在的不一致。真值参照 WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master/src/main/java/com/genersoft/iot/vmp/web/custom/CameraChannelController.java` 与 `.../web/custom/service/CameraChannelService.java`。

前端唯一调用方：`web/src/views/live/index.vue:179` 导入 `cameraListWithChild`，`web/src/views/live/index.vue:225` `await cameraListWithChild({ page: 1, count: 1000 })`。`cameraList` 当前无调用方。

## 1. request-field GET /api/sy/camera/list-with-child

- 前端：`web/src/api/syCamera.ts:34-39` 声明的查询参数是 `page / count / query / online / civilCode`，第 37 行 `query?: string`
- 后端：`src/handlers/sy_camera.rs:106-114` `struct PageQuery { page, count, keyword: Option<String> }`，关键字字段名是 `keyword`（第 113 行）且**没有** `#[serde(alias = "query")]`；handler 用它做过滤：`src/handlers/sy_camera.rs:214` `let kw = q.keyword.as_deref();`，最终传给 `src/db/device.rs:334` `list_devices_paged(..., query, ...)` 的 `device_id LIKE ? OR name LIKE ?`
- 交叉验证：WVP Java 该端点的关键字参数名正是 `query` —— `CameraChannelController.java:121` `@RequestParam(required = false) String query`，最终落到 `ChannelProvider.java:656-658` 的 `... LIKE concat('%',#{query},'%')`
- 影响：当前调用方只传 `{page:1,count:1000}`（`web/src/views/live/index.vue:225`），所以今天不发作；任何按本模块签名传 `query` 做搜索的页面，参数被 serde 静默丢弃、`kw` 恒为 `None`，SQL 落到无关键字分支（`src/db/device.rs` 的 `else` 分支），**搜索框输入后列表毫无变化（返回全量而非命中结果）**

## 2. request-field GET /api/sy/camera/list-with-child

- 前端：`web/src/api/syCamera.ts:38` `online?: boolean`
- 后端：`src/handlers/sy_camera.rs:106-114` 的 `PageQuery` 里既无 `online` 也无 `status` 字段；且 handler 把 db 层的状态过滤参数**硬编码为 None**：`src/handlers/sy_camera.rs:215` `db::device::list_devices_paged(&state.pool, page, count, kw, None)`，而该形参是 `status: Option<bool>`（`src/db/device.rs:334-339`，第 339 行）
- 交叉验证：WVP Java 用 `Boolean status` 过滤（`CameraChannelController.java:126`，`CameraChannelService.java:422`）
- 影响：调用方传 `online: true` 时期望只列在线设备，实际后端连接收字段都没有（`PageQuery` 也无 `deny_unknown_fields`，静默丢弃），并且即使字段存在 handler 也只传 `None` —— **离线设备与通道照样出现在通道树里，前端"在线过滤"完全失效**（当前无调用方传该参数，属已声明但不可用的能力）

## 3. request-field GET /api/sy/camera/list-with-child

- 前端：`web/src/api/syCamera.ts:39` `civilCode?: string`
- 后端：`src/handlers/sy_camera.rs:106-114` 的 `PageQuery` 没有 `civil_code` 字段、也没有 `civilCode` 别名。带别名的 `civilCode` 只存在于**另一个** DTO `AddressQuery`（`src/handlers/sy_camera.rs:55-63`，第 57 行 `#[serde(alias = "civilCode")]`），那个 DTO 只服务 `/api/sy/camera/list/address`（`src/router.rs:925`）
- 交叉验证：WVP Java 的 `list-with-child` 亦无 civilCode 参数（`CameraChannelController.java:118-126` 只有 page/count/query/sortName/order/groupAlias/geoCoordSys/status）
- 影响：按声明传 `civilCode` 会被静默忽略，行政区域过滤不生效 —— 列表返回全部设备而非该区域设备（当前无调用方传该参数）

## 4. query-param GET /api/sy/camera/list-with-child

- 前端：`web/src/api/syCamera.ts:36` `count?: number`，调用方 `web/src/views/live/index.vue:225` `cameraListWithChild({ page: 1, count: 1000 })`（本意是一次取全量以便建树）
- 后端：`src/handlers/sy_camera.rs:213` `let count = q.count.unwrap_or(15);` 原样接收 1000，但 `src/handlers/sy_camera.rs:215` 把它交给 `db::device::list_devices_paged`，后者在 `src/db/device.rs:342` 做 `let limit = count.min(100) as i64;` —— **请求 1000 实际最多取 100 台设备**
- 交叉验证：WVP Java 用 PageHelper 真实分页，`count` 就是页大小（`CameraChannelService.java:388`、`413`），无 100 的静默上限
- 影响：设备总数超过 100 时，通道树只出现前 100 台设备展开出的通道，**第 101 台及以后设备的通道在实时预览页彻底看不到（画面里"设备少了"，且无任何报错）**；更隐蔽的是响应把 `count` 原样回显为请求值（`src/handlers/sy_camera.rs:236` `"count": count`），前端无法从响应判断已被截断

## 5. response-field GET /api/sy/camera/list-with-child

- 前端：`web/src/api/syCamera.ts:23-28` `interface CameraListResponse { total: number; count: number; page: number; list: CameraItem[] }`
- 后端：`src/handlers/sy_camera.rs:232-237` 返回 `"total": rows.len()`（第 234 行）—— 这里的 `rows` 是**当前页设备展开后的通道行数**，不是匹配的设备/通道总数；而 `"page": page`、`"count": count`（第 235-236 行）又按设备维度回显
- 交叉验证：WVP Java 返回 PageHelper 的 `PageInfo`，其 `total` 是满足条件的总记录数（`CameraChannelService.java:423-426`）
- 影响：当前调用方忽略 `total`（`web/src/views/live/index.vue:226` 只读 `res.data?.list`），**暂无用户可见影响**；但任何按本模块类型做分页/总数展示的调用方，会把"本页行数"当成总数 —— 分页器只剩一页、总数显示错误

## 6. other GET /api/sy/camera/list

- 前端：`web/src/api/syCamera.ts:48-62`，注释第 49 行称「取所有摄像机（含通道）的另一别名（不带分页）」，实际 `url: '/sy/camera/list'`（第 59 行）并携带 `page / count / query / online` 参数（第 52-55 行）
- 后端：`src/router.rs:918` 注册到 `sy_camera::camera_list`；`src/handlers/sy_camera.rs:197` `let rows: Vec<CameraRow> = devices.iter().map(|d| device_to_row(d, None)).collect();` —— 第二个参数传 `None` 表示**不展开通道**，每行都是设备自身：`channel_id` 取 `d.device_id`、`is_device` 为 `true`（`src/handlers/sy_camera.rs:141-151` 与第 155 行 `is_device: ch.is_none()`）；且它仍是分页接口，默认 `count = 15`（第 193 行）
- 交叉验证：WVP Java 同名端点返回的是**通道**（`CameraChannelService.java:390` `channelMapper.queryListForSy(groupDeviceId, status)` 返回 `List<CameraChannel>`，`CameraChannel` 继承通道 Bean `CommonGBChannel`）
- 影响：**当前无调用方**（`grep -rnw "cameraList" web/src` 只命中定义处 `web/src/api/syCamera.ts:51`，无任何 import）。若后续页面按注释调用它取通道，会拿到一列 `is_device=true`、`channel_id == device_id` 的设备行；照 live 页既有的过滤口径（`web/src/views/live/index.vue:230` `.filter((c: any) => c.channel_id && !c.is_device)`）会被全部滤掉 → **通道树为空**，且因默认 `count=15`，设备多时只会拿到 15 行。
