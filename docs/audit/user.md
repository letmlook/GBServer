# user.ts 契约审计

审计对象：`web/src/api/user.ts`（12 个导出函数/接口）× `src/router.rs` × `src/handlers/user.rs` × `src/db/user.rs` × `src/auth.rs`。

路由与 method 全部核对通过：`/api/user/login`(get+post, `src/router.rs:995`)、`/api/user/logout`(get, `src/router.rs:912`)、`/api/user/userInfo`(get+post, `src/router.rs:71-73`)、`/api/user/users`(get, `src/router.rs:74`)、`/api/user/add`(post, `src/router.rs:75`)、`/api/user/delete`(delete, `src/router.rs:76`)、`/api/user/changePassword`(post, `src/router.rs:77`)、`/api/user/changePasswordForAdmin`(post, `src/router.rs:78-81`)、`/api/user/changePushKey`(post, `src/router.rs:82`)、`/api/role/all`(get, `src/router.rs:217`)，前端 method 与后端注册一致，**无 route-missing / http-method 类问题**。

以下 6 条是逐条比对后仍然真实存在的不一致（参数名 `roleId`/`userId`/`pushKey` 等 camelCase 后端均已加 `#[serde(alias)]`，`login`/`users` 响应键名也已 camelCase，这些不算问题）。真值参照 WVP-PRO Java 源码 `/tmp/wvpsrc/wvp-GB28181-pro-master`（后端 `UserController.java`、前端 `web/src`）。

## 1. request-field POST /api/user/add

- 前端：`web/src/api/user.ts:78-84` `addUser(data)` 原样 `params: data`（`data.password` 未做 md5）；调用方 `web/src/views/user/AddDialog.vue:67` `await addUser(form)`，`form.password` 来自明文输入框 `AddDialog.vue:7-9`（`<el-input v-model="form.password" type="password">`）。
- 前端（同一模块内的登录约定）：`web/src/api/user.ts:32-34` `login` 送的是 `password: md5(payload.password)`。
- 后端：`src/handlers/user.rs:193-197` `let password_hash = crate::auth::hash_password(password)` → 直接对**收到的明文**做 Argon2；`src/db/user.rs:206-226` `add_user(..., password_md5: &str, ...)` 只是 `.bind(password_md5)` 原样入库。
- 后端（登录侧）：`src/handlers/user.rs:41` `verify_password_compat(password, &stored)`；`src/auth.rs:154-156` 存储为 `$argon2` 时执行的是 `verify_password(plaintext, stored)` 的 Argon2 明文校验。
- 交叉验证：WVP `UserController.java:58` login 参数说明「密码（32位md5加密）」、`UserController.java:134` `user.setPassword(DigestUtils.md5DigestAsHex(password.getBytes()))` —— WVP 的 add 收明文但**服务端补 md5**，所以登录送 md5 能对上；本仓库改为直接 Argon2(明文)，与自己的登录约定断裂。
- 影响：通过「用户管理 → 新增用户」创建的账号，库里存的是 `Argon2(明文)`，而登录提交 `md5(明文)`，Argon2 校验必然失败（仓库自带回归测试 `src/auth.rs:449-457` 明确断言 `!verify_password_compat(&md5, &argon)`）→ 新建用户永远登录不上，只会看到「用户名或密码错误」。

## 2. request-field POST /api/user/changePassword

- 前端：`web/src/api/user.ts:94-100` `changePassword({ oldPassword, password })` 原样下发，未 md5；调用方 `web/src/views/user/index.vue:123-126` `await changePassword({ oldPassword: pwdForm.oldPassword, password: pwdForm.password })`，两个值都取自明文输入框（`index.vue:44-49`）。
- 后端：`src/handlers/user.rs:241` `let old_md5 = params.old_password`（变量名是 md5 但收到的是明文）→ `src/handlers/user.rs:249` `verify_password_compat(old_md5.as_str(), &stored)`；新密码 `src/handlers/user.rs:253-255` `hash_password(new_pwd)` → 存 `Argon2(明文)`。
- 交叉验证：WVP `UserController.java:81` 声明 oldPassword 为「旧密码（**已md5加密**的密码）」、`UserController.java:101` 新密码由服务端 `DigestUtils.md5DigestAsHex` 处理；WVP 前端 `web/src/layout/components/dialog/changePassword.vue:104` 确实先 `crypto.createHash('md5').update(oldPassword)` 再发。本仓库 Vue3 版漏了这一步。
- 交叉验证（本仓库内的旧版参照）：`web-legacy-vue2/src/store/modules/user.js:60` 登录同样先 md5，说明「该模块内密码以 md5 传输」是既有约定。
- 影响：① 对任何 `$argon2` 存储的账号（`src/handlers/user.rs:46-53` 会在登录成功一次后把旧账号自动升级为 `Argon2(md5(明文))`），前端送明文旧密码 → Argon2 校验失败，用户管理页点「改密」永远报「旧密码错误」，无法自助改密；② 若旧库仍是 MD5 存储（能过旧密码校验），新密码被存成 `Argon2(明文)`，而下次登录送 `md5(明文)` → 改密成功后该账号立即登录不上。

## 3. request-field POST /api/user/changePasswordForAdmin

- 前端：`web/src/api/user.ts:102-108` `changePasswordForAdmin({ userId, password })` 原样下发明文；调用方 `web/src/views/user/index.vue:145` `await changePasswordForAdmin({ userId: row.id, password: value })`（「重置」按钮）与 `web/src/layout/components/Navbar.vue:201-202` `changePasswordForAdmin({ userId: userStore.userId ?? 0, password: newPassword.value })`（顶栏自助改密）。
- 后端：`src/handlers/user.rs:282-284` `let new_hash = crate::auth::hash_password(password)` → 存 `Argon2(明文)`；随后该账号登录仍走 `web/src/api/user.ts:34` 的 `md5(...)` + `src/auth.rs:154-156` Argon2 校验。
- 交叉验证：WVP `UserController.java:210` 说明该接口收「新密码（未md5加密的密码）」，但 `UserController.java:225` 服务端 `userService.changePassword(userId, DigestUtils.md5DigestAsHex(password.getBytes()))` 会把明文转 md5 再存，登录因此仍能对上。
- 影响：管理员「重置」某用户密码、或任意用户在顶栏自助改密后，新密码以 `Argon2(明文)` 落库而登录送 md5 → 被重置/改密的账号从此登录不上（提示「用户名或密码错误」），与页面提示「密码已重置，下次登录使用新密码」相反。

## 4. response-field GET /api/user/users

- 前端：`web/src/api/user.ts:53-62` `User` 接口声明扁平字段 `roleId?: number` / `roleName?: string`；消费方 `web/src/views/user/index.vue:18` `<el-table-column prop="roleName" label="角色" ...>`（数据源 `index.vue:101-102` `getUserList({ page: 1, count: 200 })` → `rows.value = res.data?.list ?? []`）。
- 后端：`src/handlers/user.rs:139-156` 构造 `UserListRow { id, username, push_key, role: RoleInfo{...}, create_time, update_time }`；`src/db/user.rs:50-59` `#[serde(rename_all = "camelCase")] pub struct UserListRow` → 实际键为 `id` / `username` / `pushKey` / `role: {id, name, authority}` / `createTime` / `updateTime`，**没有 `roleName`、也没有 `roleId`**。
- 交叉验证：WVP `storager/dao/dto/User.java:14` `private Role role;`（嵌套 role），本仓库旧版 Vue2 页 `web-legacy-vue2/src/views/user/index.vue:43` 用的也是 `prop="role"`；Vue3 版擅自改成了扁平 `roleName`。
- 影响：用户管理页「角色」列恒为空白（`row.roleName` 为 `undefined`），`User.roleId` 也读不到；后端其实已经把角色名放在 `row.role.name` 里。

## 5. query-param GET /api/user/users

- 前端：`web/src/views/user/index.vue:101` `getUserList({ page: 1, count: 200 })`（`web/src/api/user.ts:70-76` 原样下发 `params`）。
- 后端：`src/handlers/user.rs:135-136` `let count = q.count.unwrap_or(10).min(100);` —— 静默把 `count` 截断到 100。
- 影响：`count=200` 的请求永远只回 100 条，而该页没有分页控件（`index.vue:15-35` 只有普通 `el-table`）→ 第 101 个及之后的用户在列表里完全不出现，也无法对其改密/重置 PushKey/删除。

## 6. query-param GET /api/user/users

- 前端：`web/src/api/user.ts:64-68` `UserQueryParams` 声明 `query?: string`。
- 后端：`src/handlers/user.rs:124-128` `struct UsersQuery { page, count }` 没有 `query` 字段，且未加 `#[serde(deny_unknown_fields)]` → 该参数被 serde 静默丢弃，不做任何过滤（`src/handlers/user.rs:137-138` 只按 page/count 查询）。
- 调用方：当前无调用方传 `query`（唯一调用点 `web/src/views/user/index.vue:101` 只传 `page`/`count`），因此暂无用户可见影响；一旦接入搜索框会静默失效。
