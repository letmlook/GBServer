# platform.ts 契约审计

> **状态：已修复（2026-09-12 第三十四轮）**。11 条全部落地，并用真实后端验证：
> `serverGBId`/`serverGbId`/`realm` 三种写法都能绑定、`expires` 数字与字符串都能反序列化、
> 列表与详情返回同一套键、`expires`/`keepTimeout` 回的是数字（与 WVP 的 `int` 一致）、
> 「注销」按 `serverGBId` 定位并真的发出 `Expires: 0` 的注销 REGISTER
> （后端日志出现 `Cascade platform … unregistered and removed`）并把平台置为停用。
>
> 修复过程中的额外发现：
> 1. `GET /api/platform/server_config` 把 `serverGBId` 与 `serverGBDomain` **都填成了
>    `sip.realm`** —— 本平台的 20 位国标编码（`sip.device_id`）从未对外暴露，
>    拿它去上级平台登记会用错编号。已修正，并补 `realm`/`ip`/`port` 兼容键。
> 2. 新增平台原先是"先 INSERT、再按 `server_gb_id` 补 UPDATE 扩展字段"。一旦
>    `server_gb_id` 为空（第 2 条），这条 UPDATE 会**打到所有空国标ID的行**上。
>    现在必填校验 + 唯一性校验在前，这条路径不可达。
> 3. `platform_row_json` 一度同时输出 `keepTimeout` 与 `heartBeatInterval`（互为 DTO 别名），
>    而编辑弹窗会把整行原样回提交 → serde `duplicate field` → 更新稳定 422。
>    已只保留 WVP 的 `keepTimeout`，并加了"列表行必须能原样反序列化成 `PlatformAddBody`"
>    的回归测试。

> 路由/方法层面：`web/src/api/platform.ts` 的 9 个 url + method 与 `src/router.rs:297-338,913` 全部对得上
> （`/platform/query` get、`/platform/info/:id` get、`/platform/add` post、`/platform/update` post、
> `/platform/delete` delete、`/platform/exit/:device_gb_id` get、`/platform/server_config` get、
> `/platform/catalog/add` post、`/platform/catalog/edit` post），无 route-missing / http-method 问题。
> 以下为字段级不一致。

## 1. response-field GET /api/platform/query

- 前端：`web/src/api/platform.ts:12` `serverGbId: string`；消费处 `web/src/views/platform/index.vue:17` `prop="serverGbId"`、`index.vue:18` `{{ row.serverGbId }}`、`index.vue:83` `if (!row.serverGbId) return`
- 后端：`src/handlers/platform.rs:315` `"serverGBId": item.server_gb_id,`
- 影响：级联平台列表的「国标ID」列永远空白；「注销」按钮因为 `index.vue:83` 的守卫直接 return，点了没有任何请求、也没有提示，功能静默失效。真值参照 WVP `Platform.java:23` `private String serverGBId;` —— 后端拼写与 WVP 一致，前端把 `B` 写成了小写 `b`。

## 2. request-field POST /api/platform/add

- 前端：`web/src/views/platform/EditDialog.vue:8` `<el-input v-model="form.serverGbId" />`、`EditDialog.vue:77` `serverGbId: ''`（经 `web/src/api/platform.ts:45-51 addPlatform` 原样 JSON 提交）
- 后端：`src/handlers/platform.rs:781-782` `#[serde(alias = "serverGBId")]  pub server_gb_id: Option<String>`
- 影响：用户填的国标ID绑定不上，`src/handlers/platform.rs:839` `body.server_gb_id.clone().unwrap_or_default()` 得到空串并写库（`src/db/platform.rs:171` `pub async fn add(` 的 sqlite 分支 `src/db/platform.rs:226` `.bind(server_gb_id)` 直接绑空串）。接口仍返回「平台添加成功」，但库里 `server_gb_id` 为空，后续 `get_by_server_gb_id`/级联注册都以空串为键，平台实际不可用。大小写仅差 `B`/`b`。

## 3. request-field POST /api/platform/add（expires 数字 vs 字符串）

- 前端：`web/src/views/platform/EditDialog.vue:41` `<el-input-number v-model="form.expires" :min="0" />`、`EditDialog.vue:87` `expires: 3600`、`EditDialog.vue:114` `expires: 3600`、`web/src/views/platform/index.vue:73` `expires: 3600`
- 后端：`src/handlers/platform.rs:828` `pub expires: Option<String>,`
- 影响：新增表单默认提交 JSON 数字 `3600`，反序列化阶段即失败（用本仓库编译出的 serde/serde_json 实测：`invalid type: integer '3600', expected a string`），Axum `Json` 提取器返回 422，请求根本进不到 handler ⇒ 新增平台直接报错、保存不了。编辑时初值来自 DB 的 varchar（字符串）可以保存，但只要用户改动「有效期」输入框（el-input-number 输出 number）同样 422。真值参照 WVP `Platform.java:50` `private int expires;`（Java 端为数字），legacy 前端用字符串输入框 `web-legacy-vue2/src/views/platform/edit.vue:46` `<el-input v-model="value.expires" />` —— Vue3 改写换成 el-input-number 后契约被破坏。

## 4. request-field POST /api/platform/add 与 /api/platform/update（realm）

- 前端：`web/src/views/platform/EditDialog.vue:17` `v-model="form.realm"`、`EditDialog.vue:80` `realm: ''`、`EditDialog.vue:107` `realm: ''`、`web/src/api/platform.ts:19` `realm?: string`
- 后端：`src/handlers/platform.rs:790-791` `#[serde(alias = "serverGBDomain")]  pub server_gb_domain: Option<String>`（DTO `PlatformAddBody` 见 `platform.rs:777-831`，无 `realm` 字段）
- 影响：弹窗「域名」输入框的内容被 serde 当未知字段静默丢弃、永不落库（`gb_platform.server_gb_domain` 永远为空），用户重新打开编辑框看到域名消失。真值参照 WVP `Platform.java:26` `private String serverGBDomain;` 与 legacy 前端 `web-legacy-vue2/src/views/platform/edit.vue:13-14` `prop="serverGBDomain"`。

## 5. request-field POST /api/platform/add（registerInterval / heartBeatInterval / heartBeatCount）

- 前端：`web/src/views/platform/EditDialog.vue:32` `v-model="form.registerInterval"`、`EditDialog.vue:35` `v-model="form.heartBeatInterval"`、`EditDialog.vue:38` `v-model="form.heartBeatCount"`、`web/src/views/platform/index.vue:73` `registerInterval: 60, heartBeatInterval: 60, heartBeatCount: 3`
- 后端：`src/handlers/platform.rs:777-831` `PlatformAddBody` 无这三个字段；`database/init-sqlite-2.7.4.sql:313` `CREATE TABLE IF NOT EXISTS gb_platform` 定义中无对应列（WVP `Platform.java` 亦无这三个字段）
- 影响：「注册间隔 / 心跳间隔 / 心跳次数」三个输入框是纯装饰：值被后端忽略，数据库也没有列可落，用户改了保存后不生效、重新打开又回到默认值。

## 6. response-field GET /api/platform/query（heartBeatInterval）

- 前端：`web/src/views/platform/index.vue:33` `prop="heartBeatInterval"`、`index.vue:34` `{{ row.heartBeatInterval ?? '-' }} s`
- 后端：`src/handlers/platform.rs:311-354` 列表 `json!({...})` 无 `heartBeatInterval` 键（同表也无该列，见 `database/init-sqlite-2.7.4.sql:313`）
- 影响：列表「心跳」列永远显示 `-`；与条目 5 对应，该字段前后端都没有真实来源。

## 7. request-field POST /api/platform/catalog/add

- 前端：`web/src/api/platform.ts:83-88` `addPlatformCatalog(data: { platformId; name; parentId?; civilCode?; businessGroup? })`
- 后端：`src/handlers/platform.rs:1607-1614` `pub struct CatalogAddBody { id, name, parent, civil_code, business_group, platform_id }`（无任何 `#[serde(alias=...)]`）
- 影响：`platformId`/`parentId`/`civilCode`/`businessGroup` 全部绑定不上，`platform.rs:1626-1630` 的 `unwrap_or_default()` 把 `parent`/`civil_code`/`business_group` 写成空串、`platform_id` 写成 0；`platform.rs:1664` 因 `platform_id > 0` 不成立而跳过上级目录刷新，接口仍返回 `{"code":0,"msg":"目录添加成功"}`。真值参照同仓库 legacy 前端 `web-legacy-vue2/src/views/dialog/catalogEdit.vue:116-117` 提交的也是 `platformId` / `parentId`（camelCase）。当前无调用方：`web/src` 内除 `api/platform.ts:86` 外无引用，新 Vue3 应用没有 catalogEdit.vue / commonChannelEditDialog.vue（这两个文件只存在于归档的 `web-legacy-vue2/src/views/dialog/`）。

## 8. request-field POST /api/platform/catalog/edit

- 前端：`web/src/api/platform.ts:91-96` `editPlatformCatalog(data: { id; name; parentId?; civilCode?; businessGroup? })`
- 后端：`src/handlers/platform.rs:1677-1684` `pub struct CatalogAddBodyEdit { id, name, parent, civil_code, business_group, platform_id }`（同样无 alias）
- 影响：只有 `id`/`name` 能绑上；`platform.rs:1709-1711` 把 `parent`/`civil_code`/`business_group` 取成空串，`platform.rs:1716-1722` 的 `COALESCE(?, parent)` 中空串不是 NULL，会把已有值**清空**。当前无调用方（同上，仅 `api/platform.ts:94` 定义）。

## 9. other GET /api/platform/exit/:deviceGbId

- 前端：`web/src/views/platform/index.vue:85` `await platformExit(row.serverGbId)` + `index.vue:86` `ElMessage.success('注销请求已发送')`；`web/src/api/platform.ts:69-74`
- 后端：`src/handlers/platform.rs:1276-1283` `Path(device_gb_id)` → `platform_db::get_by_device_gb_id(...)`，`platform.rs:1282` 只回一个「是否存在」布尔值，没有任何注销动作
- 影响：按钮文案是「注销」并始终弹「注销请求已发送」，但后端既不注销、又是按 `device_gb_id` 列查询，而前端传的是平台 `serverGBId`，两者不是同一列 ⇒ 即使修好条目 1 的键名也恒返回 `false`。真值参照 WVP `PlatformController.java:174-178`：`@GetMapping("/exit/{serverGBId}") ... queryPlatformByServerGBId(serverGBId)`（按 serverGBId 做存在性校验，WVP 前端只把它当国标ID重复性校验用，见 WVP `web/src/views/platform/edit.vue:158`）。当前实际表现为条目 1 的 `undefined` 提前 return 掩盖。

## 10. response-field GET /api/platform/server_config

- 前端：`web/src/api/platform.ts:77` `WvpResult<{ ip: string; port: number; id: string; realm: string }>`
- 后端：`src/handlers/platform.rs:370-384` 返回 `id / name / serverGBId / serverGBDomain / serverHost / serverIp / serverPort / deviceIp / devicePort / username / password / transport / sendStreamIp`
- 影响：按声明的类型读 `data.ip` / `data.port` / `data.realm` / `data.id`(实为 null) 全部是 `undefined`，任何按该类型取值的地方都会拿到空值。当前无调用方：`web/src` 内除 `api/platform.ts:76` 的定义外无引用。

## 11. response-field GET /api/platform/info/:id

- 前端：`web/src/api/platform.ts:38-43` 返回类型 `WvpResult<Platform>`，而 `Platform`（`platform.ts:12`）用的是 `serverGbId`
- 后端：`src/handlers/platform.rs:1765` `"serverGBId": p.server_gb_id,`
- 影响：与条目 1 同源的拼写问题——调用方按类型读 `data.serverGbId` 会得到 `undefined`。当前无调用方：`web/src` 内除 `api/platform.ts:38` 的定义外无引用。


---

## 修复对照（第三十四轮）

| # | 问题 | 修复 |
|---|------|------|
| 1 | 前端 `serverGbId`（小写 b）vs 后端/WVP `serverGBId` → 列表国标ID空白、「注销」被守卫静默拦掉 | 前端改用 `serverGBId`；后端同时收 `serverGBId`/`serverGbId` 两种拼写 |
| 2 | 新增时国标ID 绑不上 → 写空串，接口仍回"成功"，平台实际不可用 | 同 1；并加必填校验（名称/国标ID/IP）与国标ID 唯一性校验，空值直接 400 |
| 3 | `expires` 用 el-input-number 发数字，后端 DTO 只要字符串 → 反序列化 422，新增平台必失败 | `deserialize_opt_int_string` 数字/字符串都收；响应统一回数字（WVP `int expires`） |
| 4 | 「域名」用 `realm`，后端字段是 `serverGBDomain` → 静默丢弃 | 前端改用 `serverGBDomain`；后端加 `realm` 别名 |
| 5 | 「注册间隔/心跳间隔/心跳次数」三个输入框是凭空发明的字段（WVP 与 DB 都没有） | 换成真实存在的 `expires`（注册周期）与 `keepTimeout`（心跳周期）；后端 `keepTimeout` 落 `keep_timeout` 列 |
| 6 | 列表 `heartBeatInterval` 恒为 `-` | 列表/详情统一由 `platform_row_json` 输出，含 `keepTimeout` |
| 7 | `catalog/add` 无 alias，`platformId`/`parentId`/`civilCode`/`businessGroup` 全绑不上 → 目录写进空 platform_id | DTO 改 `rename_all = "camelCase"` + `parent` 别名 |
| 8 | `catalog/edit` 同样绑不上，且 `unwrap_or_default()` 把未传字段变成空串 → **清空**已有值 | 同 7；并改为 `Option` 直接绑定，未传即 NULL（COALESCE 保留原值） |
| 9 | `/platform/exit/:deviceGbId` 按错误列查询、只回布尔、无任何注销动作 | 改为按 `serverGBId` 定位，发 `Expires: 0` REGISTER 并落 `enable=false/status=false`；不存在则 404 |
| 10 | `server_config` 返回类型声明 `{ip, port, id, realm}` 与实际键完全不符 | 前端类型对齐真实键；后端修正 `serverGBId`（本平台 20 位编码）并用 `realm`/`ip`/`port` 兼容旧声明 |
| 11 | `/platform/info/:id` 只回 10 个字段，与列表不一致 | 与列表共用 `platform_row_json` |
