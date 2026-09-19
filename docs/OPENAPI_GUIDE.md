# OpenAPI 文档维护指南

本仓库的接口文档是**代码优先**的：`#[utoipa::path]` 注解既产出 OpenAPI 条目，
也通过 `routes!()` 注册真实路由 —— 两者是同一个动作，因此**结构上不可能出现
「加了路由没写文档」**。

## 访问入口

| 入口 | 内容 |
|---|---|
| `/swagger-ui` | 交互式文档。点右上角 **Authorize** 填 token 后可直接调用接口 |
| `/api/openapi.json` | OpenAPI 3.1 规范。可导入 Postman / Apifox，或用于生成前端 TS 客户端 |

## 鉴权方案

Swagger UI 的 Authorize 支持三种（与 `src/auth.rs` 的实际校验一致）：

* `access_token` —— JWT，请求头 `access-token`（前端 axios 拦截器用的就是这个）
* `bearer_auth` —— JWT，标准 `Authorization: Bearer <token>`
* `api_key` —— API Key，请求头 `X-API-Key`

公开端点（登录、健康检查、`/metrics`、ZLM hook、集群 RPC、分享播放、缩略图、
WebSocket 与对讲音频）**不声明** `security(...)`，在 Swagger 里可直接调用。

## 新增一个接口

1. **写注解**（`src/handlers/<模块>.rs`）：

```rust
#[utoipa::path(
    get,
    path = "/api/device/query/devices",        // 必须与注册用的路径一致
    tag = "device",                             // 取值见 src/openapi/mod.rs 的 tags
    operation_id = "device_query_devices",      // 全局唯一
    summary = "分页查询国标设备",
    params(DevicesQuery),                       // Query DTO 需 derive IntoParams
    responses(
        (status = 200, description = "成功", body = ApiResult<serde_json::Value>),
        (status = 401, description = "未鉴权"),
    ),
    security(("access_token" = [])),            // 公开端点则不写这一行
)]
pub async fn query_devices(...) -> ... { }
```

2. **DTO 加派生**：
   * `Query<X>` → `#[derive(Deserialize, utoipa::IntoParams)]`
   * `Json<X>` → `#[derive(Deserialize, Serialize, utoipa::ToSchema)]`

3. **重新生成路由模块**：

```bash
python3 scripts/gen_openapi_routes.py
```

脚本会按模块把接口写进 `src/openapi/routes_<域>.rs`（受保护）或
`routes_<域>_public.rs`（公开），并重新生成两个聚合文件。

4. **验证**：

```bash
cargo test --lib router::     # 含「文档路径可达」门禁
```

## 目录结构

```
src/openapi/
├── mod.rs                          # ApiDoc（info/标签/安全方案）+ 注册表累加器
├── registry_protected_routes.rs    # 受保护路由聚合（生成）
├── registry_public_routes.rs       # 公开路由聚合（生成）
├── routes_<域>.rs                  # 各域受保护接口（生成）
├── routes_<域>_public.rs           # 各域公开接口（生成）
└── routes_public.rs                # handler 不在 src/handlers/ 下的两个（手写）
```

`registry_*.rs` 与 `routes_*.rs` 由脚本产出，**不要手改**；改动会在下次生成时丢失。

## 迁移时踩过的四个坑（务必遵守）

1. **`routes!()` 一次只能放一条路由。** 多条会让宏为每个 handler 重复注册同一组
   method，启动时 panic：`Overlapping method route ... handle GET`。
2. **公开路由不能并进受保护注册表。** 受保护注册表会挂 `auth_middleware`；
   若把 `/api/ws`、`/api/talk/audio/...` 放进去，浏览器 WebSocket 握手会被判 401
   （它们无法设置自定义请求头，JWT 走 `?token=`）。分类依据是**注解里有没有
   `security(...)`**，不是文件归属。
3. **注解内不要写 `body = ApiResult<()>`。** 单元类型在 utoipa 里没有 path，会 panic
   `TypeTree must have a path`；空响应用 `ApiResult<serde_json::Value>`。
4. **注解内不要写 `body = ApiResult<Vec<serde_json::Value>>`。** `Vec<T>` 走
   `ComposeSchema`，而 `serde_json::Value` 未实现它，会编译失败；改用
   `ApiResult<serde_json::Value>` 或自建 wrapper 结构体。

## 覆盖门禁

两条测试守住「文档 ↔ 实现」一致：

* `router::tests::test_every_documented_path_is_routable` —— 遍历
  `/api/openapi.json` 的每一条路径真实发请求，断言**不是 404**。漏登记、路径写错、
  迁移丢路由都会在这里暴露。
* `router::tests::test_openapi_document_is_served_with_migrated_paths` —— 断言
  `/swagger-ui` 可访问、spec 含预期路径、安全方案齐全、鉴权要求已声明。
