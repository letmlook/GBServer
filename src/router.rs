use axum::{extract::State, middleware, Json, Router};
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::auth_middleware;
use crate::middleware::audit_middleware;
use crate::rpc::{RpcRequest, RpcResponse};
use crate::zlm::hook_routes as zlm_hook_routes;
use crate::AppState;

/// E2: HTTP RPC 端点 — 接收 JSON-RPC envelope 并通过 RpcRouter 分发到本地 handler
///
/// 2026-09-11：新增共享密钥校验。此前该端点**完全无鉴权**（出站也不带凭证），
/// 任何能访问端口的人都能直接调用集群 RPC 方法。现在 `[rpc].secret` 非空时，
/// 入站必须携带匹配的 `X-RPC-Secret`，否则 401。
#[utoipa::path(
    post,
    path = "/api/rpc",
    tag = "system",
    operation_id = "rpc_endpoint",
    summary = "集群节点间 RPC（公开端点，靠 X-RPC-Secret 校验）",
    description = "`[rpc].secret` 非空时，入站请求必须携带匹配的 `X-RPC-Secret`，否则 401。",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "RPC 响应"),
        (status = 401, description = "共享密钥不匹配"),
    ),
)]
pub async fn rpc_endpoint(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RpcRequest>,
) -> Result<Json<RpcResponse>, (axum::http::StatusCode, Json<RpcResponse>)> {
    if let Some(expected) = state.config.rpc.secret.as_deref().filter(|s| !s.is_empty()) {
        let provided = headers
            .get("x-rpc-secret")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != expected {
            tracing::warn!(
                "拒绝 RPC 入站：X-RPC-Secret 缺失或不匹配（method={}）",
                req.method
            );
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(RpcResponse {
                    ok: false,
                    result: None,
                    error: Some("invalid or missing X-RPC-Secret".to_string()),
                }),
            ));
        }
    }

    tracing::debug!("RPC inbound: method={} target={}", req.method, req.target);
    if let Some(router) = state.rpc_router.as_ref() {
        let response = router.route(&req).await;
        Ok(Json(response))
    } else {
        Ok(Json(RpcResponse {
            ok: false,
            result: None,
            error: Some("RPC router not configured on this node".to_string()),
        }))
    }
}

pub fn app(state: AppState) -> Router<AppState> {
    let state_clone = state.clone();
    // 受保护路由完全由 handler 上的 `#[utoipa::path]` 注解驱动：`routes!()`
    // 一次注册同时产出 axum 路由与 OpenAPI path，因此「加了路由忘写文档」
    // 在结构上不可能发生。聚合文件见 `src/openapi/registry_protected_routes.rs`。
    let (_, _, api_protected) = crate::openapi::registry_protected_routes::protected_routes();
    let api_protected = api_protected
        // Phase 7.4: audit middleware outermost — captures all responses (including 401)
        .route_layer(middleware::from_fn_with_state(
            state_clone.clone(),
            audit_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            state_clone.clone(),
            auth_middleware,
        ));

    // 公开路由同样由注解驱动，见 `registry_public_routes.rs`。
    // 这些端点不挂鉴权中间件，各自的替代鉴权方式见 `src/openapi/routes_health_public.rs`。
    let (_, _, api_public) = crate::openapi::registry_public_routes::public_routes();

    let api = api_public.merge(api_protected);
    // 注：原 `zlm_protected`（ZLM 反向代理）已并入受保护注册表（`server::zlm_proxy`
    // 的注解在 system 域），因此现在它与其它受保护接口一样会经过审计中间件。
    let app = Router::new()
        .merge(api)
        // Phase 4.1: 兼容多路径 hook 路由（/api/hook/*）
        // 公共端点，与既有 /api/zlm/hook 单路径并存
        .merge(zlm_hook_routes::hook_routes())
        .with_state(state.clone());

    // ===== OpenAPI 文档 =====
    // 文档感知的路由用 `routes!()` 注册：一次注册同时产出 axum 路由与 OpenAPI path，
    // 因此不存在「加了路由忘了写文档」的可能。未迁移的路由仍走上面的字符串注册，
    // 它们不出现在文档里（迁移进度见 `documented_routes()`）。
    let openapi = crate::openapi::build_openapi();
    let path_count = openapi.paths.paths.len();
    let app = app
        .merge(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui").url("/api/openapi.json", openapi));
    tracing::info!(
        "OpenAPI 文档已挂载：/swagger-ui（规范：/api/openapi.json），当前已收录 {} 条路径",
        path_count
    );

    // WebSocket：设备状态实时通知 (Phase 7.3 + 7.4: JWT 校验在 ws_handler 内部)

    // Phase 7.4: alarm endpoints moved into api_protected (now require JWT).
    // The legacy public routes below are intentionally removed.

    // 云录像 ZIP 打包产物：通过 /downloads/<file> 对外下载。
    // 必须注册在下面的 `fallback_service(..)`（SPA 兜底）之前，否则会被
    // index.html 吞掉。文件名含随机段，避免被枚举遍历。
    let download_dir = state.config.server.effective_download_dir();
    let app = match std::fs::create_dir_all(&download_dir) {
        Ok(()) => app.nest_service(
            "/downloads",
            tower_http::services::ServeDir::new(download_dir),
        ),
        Err(e) => {
            tracing::warn!("无法创建下载目录（/downloads 不可用）: {}", e);
            app
        }
    };

    // 静态资源：前端构建产物目录（`static_dir`，默认 web/dist）
    let static_dir = state
        .config
        .static_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.exists());
    let app = if let Some(dir) = static_dir {
        let index_path = dir.join("index.html");
        let serve_dir = tower_http::services::ServeDir::new(dir)
            .fallback(tower_http::services::ServeFile::new(index_path));
        // axum 0.8 起**不再允许** `nest_service("/", ..)` —— 会在启动时 panic
        // `Nesting at the root is no longer supported`。根路径必须改用
        // `fallback_service`：未命中任何路由时才走它，与原 SPA 兜底语义一致。
        app.fallback_service(serve_dir)
    } else {
        tracing::warn!("未配置 static_dir 或目录不存在，仅提供 API");
        app
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    app.layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::app_state;

    /// 在临时端口起一个真实 HTTP 服务，返回 base URL。
    ///
    /// 不用 `axum-test`：它会把另一个 axum 版本带进依赖图。直接起服务 +
    /// 用已有的 reqwest 请求，既真实又不引入额外依赖。
    async fn spawn(state: AppState) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr");
        let app = app(state.clone()).with_state(state);
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    async fn status_of(base: &str, path: &str) -> u16 {
        reqwest::get(format!("{}{}", base, path))
            .await
            .unwrap_or_else(|e| panic!("请求 {} 失败: {}", path, e))
            .status()
            .as_u16()
    }

    async fn post_status_of(base: &str, path: &str) -> u16 {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
            .post(format!("{}{}", base, path))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap_or_else(|e| panic!("POST {} 失败: {}", path, e))
            .status()
            .as_u16()
    }

    /// 路由表必须能成功构建。
    ///
    /// axum 检测到重复路由时会在**启动瞬间 panic** —— 本仓库历史上正是被
    /// `Overlapping method route` 打断过启动。
    /// 这条测试把该风险从「部署时才炸」前移到测试阶段。
    #[tokio::test]
    async fn test_router_builds_without_conflicts() {
        let state = app_state().await;
        let _ = app(state);
    }

    /// OpenAPI 文档必须可访问，且**已迁移的路由确实出现在 spec 里**。
    ///
    /// 这条测试是「文档与路由同步」的第一道闸：只注册路由、忘了 `routes!()`，
    /// 或者只写注解、忘了从字符串注册里搬走（后者会 panic，由上面的测试兜），
    /// 都会在这里暴露。
    #[tokio::test]
    async fn test_openapi_document_is_served_with_migrated_paths() {
        let base = spawn(app_state().await).await;

        let swagger = reqwest::get(format!("{}/swagger-ui", base))
            .await
            .expect("GET /swagger-ui");
        assert_eq!(swagger.status().as_u16(), 200, "/swagger-ui 未挂载");

        let resp = reqwest::get(format!("{}/api/openapi.json", base))
            .await
            .expect("GET /api/openapi.json");
        assert_eq!(resp.status().as_u16(), 200, "/api/openapi.json 不可访问");
        let doc: serde_json::Value = resp.json().await.expect("spec 必须是合法 JSON");

        // 试点迁移的 3 条区域路由必须都在
        for p in [
            "/api/region/one",
            "/api/region/page/list",
            "/api/region/sync",
        ] {
            assert!(
                doc["paths"].get(p).is_some(),
                "已迁移路由 {p} 未出现在 OpenAPI paths 里"
            );
        }

        // 安全方案必须存在，否则 Swagger UI 的 Authorize 按钮无法配置 token
        for scheme in ["access_token", "bearer_auth", "api_key"] {
            assert!(
                doc["components"]["securitySchemes"].get(scheme).is_some(),
                "缺少安全方案 {scheme}"
            );
        }

        // 鉴权要求：区域接口必须声明 access_token
        let declared = doc["paths"]["/api/region/one"]["get"]["security"].to_string();
        assert!(
            declared.contains("access_token"),
            "区域接口未声明鉴权要求: {declared}"
        );
    }

    /// **覆盖门禁**：OpenAPI 文档里的每条路径都必须真的能从路由表访问到。
    ///
    /// 这是「文档 ↔ 实现一致」的核心断言。有了它：
    /// * 只写注解、忘了在域模块里登记 → 文档里有、路由没有 → 404 → 测试红
    /// * 只登记路由、路径字符串写错 → 同上
    /// * 迁移时丢了一条路由 → 立刻暴露，而不是等上线
    ///
    /// 只断言「不等于 404」：鉴权(401)、参数错误(400)、业务错误(500) 都算路由存在。
    /// 含通配符（`{*path}`）的路径无法构造合法 URL，跳过。
    #[tokio::test]
    async fn test_every_documented_path_is_routable() {
        let base = spawn(app_state().await).await;
        let openapi = crate::openapi::build_openapi();
        let paths: Vec<String> = openapi.paths.paths.keys().cloned().collect();
        assert!(
            paths.len() > 400,
            "文档路径数异常偏少（{}），注册表可能没接上",
            paths.len()
        );

        let mut unreachable = Vec::new();
        for raw in &paths {
            if raw.contains("{*") {
                continue; // 通配路径跳过
            }
            // 路径参数替换为占位值（能让 Path 提取器通过即可）
            let concrete = raw
                .split('/')
                .map(|seg| {
                    if seg.starts_with('{') && seg.ends_with('}') {
                        "1"
                    } else {
                        seg
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            let status = status_of(&base, &concrete).await;
            if status == 404 {
                unreachable.push(format!("{} -> 404", raw));
            }
        }
        assert!(
            unreachable.is_empty(),
            "以下 {} 条文档路径在路由表里不存在（404）：\n{}",
            unreachable.len(),
            unreachable.join("\n")
        );
    }

    /// A1: SY 前缀的取消收藏别名必须与平台版本一样，GET 与 DELETE 都能进鉴权层。
    ///
    /// 若只注册了其中一个方法，另一个会落到路由层的 405（Method Not Allowed），
    /// 对接方拿到的是"方法不支持"而不是"未登录"。
    #[tokio::test]
    async fn sy_record_collect_delete_accepts_both_methods() {
        let base = spawn(app_state().await).await;
        let url = format!("{base}/api/sy/record/collect/delete?id=1");
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
            let status = client
                .request(method.clone(), &url)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16();
            assert_eq!(status, 401, "{method} should reach JWT middleware");
        }
    }

    /// C9: 前端的 GET 取消收藏与兼容 DELETE 路由都应存在；
    /// 未登录时由鉴权拒绝，而非路由层 405。
    #[tokio::test]
    async fn cloud_record_collect_delete_accepts_both_methods() {
        let base = spawn(app_state().await).await;
        let url = format!("{base}/api/cloud/record/collect/delete?id=1");
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
            let status = client
                .request(method.clone(), &url)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16();
            assert_eq!(status, 401, "{method} should reach JWT middleware");
        }
    }

    /// 回归：SY 契约面遇到**非法的查询串**也必须回契约信封。
    ///
    /// axum 的 `Query` 提取器拒绝会**绕过 handler** 直接回 `text/plain` 的 400
    /// （`Failed to deserialize query string: ...`）—— SY 是第三方对接面，
    /// 对接方按 `{code,msg,data}` 解析，裸文本会直接让它的解析器炸。
    /// 现由 `crate::error::envelope_rejection_errors` 中间件包回信封。
    #[tokio::test]
    async fn sy_endpoint_returns_contract_envelope_for_malformed_query() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        let resp = client
            .get(format!("{base}/api/sy/camera/list?page=abc"))
            .header("access-token", &token)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 400);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ct.starts_with("application/json"),
            "错误响应必须是 JSON（契约信封），实际 content-type={ct}"
        );
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["code"], 400, "{body:?}");
        assert!(body["msg"].is_string(), "{body:?}");
    }

    /// 回归：SY 相机列表的 `online` 要同时接受 `true/false` 与 `1/0`。
    ///
    /// 原生 `bool` 在查询串里只认 `"true"`/`"false"`，第三方传 `online=1`
    /// 会整个请求 422 —— 违反仓库自己的 `serde_flex::de_opt_bool` 约定
    /// （它的注释里就写着"只认 bool 会 422"）。
    #[tokio::test]
    async fn sy_camera_list_accepts_numeric_boolean_filter() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        for v in ["true", "false", "1", "0"] {
            let resp = client
                .get(format!("{base}/api/sy/camera/list?online={v}"))
                .header("access-token", &token)
                .send()
                .await
                .unwrap();
            assert_eq!(
                resp.status().as_u16(),
                200,
                "online={v} 应当被接受（查询串里的布尔值要兼容 1/0）"
            );
            let body: serde_json::Value = resp.json().await.unwrap();
            assert_eq!(body["code"], 0, "online={v} → {body:?}");
        }
    }

    /// 回归：API Key 的写操作在**目标不存在**时不得回"成功"。
    ///
    /// `remark` / `enable` / `disable` / `reset` 都忽略 `UPDATE` 的
    /// `rows_affected` —— id 不存在时也回 200。而 `reset` 更糟：它会回一个
    /// **从未被存储**的密钥，前端弹出"请立即复制密钥"、管理员把它抄走，
    /// 用起来必然 401，而界面全程显示"成功"。
    ///
    /// 仓库既有约定是"影响 0 行 → 404"（见 `jt1078` 的终端删除、`platform` 的
    /// 目录删除），这里跟它保持一致。
    #[tokio::test]
    async fn api_key_write_on_missing_row_returns_404() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        // 内存库里没有任何 API Key，取一个必然不存在的 id。
        for (label, url) in [
            ("启用", format!("{base}/api/userApiKey/enable?id=999999")),
            ("停用", format!("{base}/api/userApiKey/disable?id=999999")),
            ("重置", format!("{base}/api/userApiKey/reset?id=999999")),
            (
                "备注",
                format!("{base}/api/userApiKey/remark?id=999999&remark=x"),
            ),
        ] {
            let resp = client
                .post(&url)
                .header("access-token", &token)
                .send()
                .await
                .unwrap();
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("必须是契约信封（JSON），实际是 {text:?}（{e}）"));
            assert_eq!(parsed["code"], 404, "{label} 必须回 404，实际 {parsed:?}");
            assert_eq!(status, 404, "{label} 的 HTTP 状态必须是 404，实际 {status}");
            assert!(
                parsed["data"]["apiKey"].is_null(),
                "{label} 不得回出密钥（那是从未被存储的字符串），实际 {parsed:?}"
            );
        }
    }

    /// 回归：按主键定位的**单行 UPDATE** 写端点，目标不存在时必须回 **404**。
    ///
    /// 第九轮审计把全仓 **242 个写端点**逐个核过，在 API Key 那 4 个之外又挖出
    /// **7 处**丢弃 `rows_affected` 的地方 —— 目标不存在时照旧回 200 + `code:0`，
    /// 于是前端弹「保存成功」，实际**一行都没改**（管理员以为改完了）。
    ///
    /// 其中 `push/update` 写的是 `Ok(_)`：看着像「已 match 检查过」，实际把
    /// `u64` 直接丢掉；而 `platform/channel/custom/update` **不能**用返回值判
    /// 存在性（`db::platform_channel::update` 在两个字段皆空时提前
    /// `return Ok(0)`），它改为先按 id 查存在性 —— 两条路径都要被这条用例挡住。
    #[tokio::test]
    async fn write_endpoints_on_missing_row_return_404() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        // 内存库里没有这些 id（999999 必然不存在）。
        // body 只给 `id`（+ 一个可变字段，避免 db 层「没有要改的字段」的短路）。
        //
        // 这里**收集全部结果再统一断言**：若逐个 `assert_eq!`，回归时只会在
        // 第一个失败的端点上 panic，看不到其余 6 个是修好了还是同样坏了。
        let mut failures: Vec<String> = Vec::new();
        for (label, path, body) in [
            (
                "公共通道更新",
                "/api/common/channel/update",
                r#"{"id":999999,"name":"x"}"#,
            ),
            (
                "公共通道重置",
                "/api/common/channel/reset",
                r#"{"id":999999}"#,
            ),
            (
                "区域更新",
                "/api/region/update",
                r#"{"id":999999,"name":"x"}"#,
            ),
            (
                "分组更新",
                "/api/group/update",
                r#"{"id":999999,"name":"x"}"#,
            ),
            (
                "级联平台更新",
                "/api/platform/update",
                r#"{"id":999999,"name":"x"}"#,
            ),
            (
                "共享通道自定义更新",
                "/api/platform/channel/custom/update",
                r#"{"id":999999,"customName":"x"}"#,
            ),
            (
                "推流更新",
                "/api/push/update",
                r#"{"id":999999,"app":"live"}"#,
            ),
        ] {
            let resp = client
                .post(format!("{base}{path}"))
                .header("access-token", &token)
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap();
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| {
                panic!("{label} 必须是契约信封（JSON），实际是 {text:?}（{e}）")
            });
            if parsed["code"] != 404 || status != 404 {
                failures.push(format!(
                    "{label}: code={} http={status} body={parsed}",
                    parsed["code"]
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "以下写端点在目标不存在时没有回 404（假成功）：\n{}",
            failures.join("\n")
        );
    }

    /// 回归：云录像删除端点的**请求体不合法**时，也必须回**契约信封**。
    ///
    /// 此前用 `Json<T>` 提取器，解析失败由 axum 直接回 **422 + `text/plain` 裸文本** ——
    /// 第三方按 `{code,msg,data}` 解析会直接炸（body 不是 JSON）。
    ///
    /// 还要挡住两类"看起来成功、实际什么都没删"的情况：
    /// - **空 body**：请求体丢失/截断。此前 `Json<T>` 回 422；改用自解析后若把它
    ///   当成"没给 ids"，就会回 `code:0` + 空删除列表 —— 看起来成功。
    /// - **缺 ids**（`{}` / `{"ids":null}` / `{"ids":[]}`）：同理必须明确报错。
    #[tokio::test]
    async fn cloud_record_delete_rejects_bad_body_with_contract_envelope() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        for (label, body) in [
            ("语法错误", r#"{"ids": ["1",]}"#),
            ("类型错误：ids 不是数组", r#"{"ids": "1"}"#),
            (
                "元素非法：20 位国标号超出整数可表示范围",
                r#"{"ids": [34020000001320000001]}"#,
            ),
            ("空 body（请求体丢失/截断）", ""),
            ("纯空白 body", "   "),
            ("缺 ids 的空对象", "{}"),
            ("ids 为 null", r#"{"ids": null}"#),
            ("ids 为空数组", r#"{"ids": []}"#),
        ] {
            let resp = client
                .delete(format!("{base}/api/cloud/record/delete"))
                .header("access-token", &token)
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await
                .unwrap();
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("body 必须是契约信封（JSON），实际是 {text:?}（{e}）"));
            assert_eq!(
                parsed["code"], 400,
                "{label} 应回业务错误码 400，实际 {parsed:?}（body={body:?}）"
            );
            assert!(parsed["msg"].is_string(), "{parsed:?}");
            assert!(status == 400 || status == 200, "{status}");
            assert!(
                parsed.get("deleted").is_none(),
                "{label} 不得回出\"删成功但其实没删\"的 deleted 字段: {parsed:?}"
            );
        }
    }

    /// 回归：**超大请求体**（超过 axum 的 2MB 默认上限）也必须回契约信封。
    ///
    /// 长度超限的拒绝发生在**提取器内**，直接冒泡出去是裸文本 413。
    #[tokio::test]
    async fn cloud_record_delete_rejects_oversized_body_with_contract_envelope() {
        let state = app_state().await;
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();

        // 3MB 的合法 JSON（ids 数组很长）—— 超过 2MB 上限
        let big = format!(
            r#"{{"ids":[{}]}}"#,
            (0..400_000)
                .map(|i| format!("\"{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(
            big.len() > 2 * 1024 * 1024,
            "body 应超过 2MB，实际 {}",
            big.len()
        );

        let resp = client
            .delete(format!("{base}/api/cloud/record/delete"))
            .header("access-token", &token)
            .header("content-type", "application/json")
            .body(big)
            .send()
            .await
            .unwrap();
        let text = resp.text().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| {
            panic!(
                "超大 body 也必须回契约信封（JSON），实际是 {} 字节的 {text:?}（{e}）",
                text.len()
            )
        });
        assert_eq!(parsed["code"], 400, "{parsed:?}");
    }

    #[tokio::test]
    async fn authenticated_collect_roundtrip_updates_business_storage() {
        let state = app_state().await;
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query(
                "INSERT INTO gb_cloud_record (app, stream, start_time, end_time, file_name, file_path, media_server_id) \
                 VALUES ('rtp', 'auth_roundtrip', 1, 2, 'a.mp4', '/tmp/a.mp4', 'zlm-1')",
            )
            .execute(p)
        })
        .unwrap();
        let record_pk: i64 = crate::dispatch!(&state.pool, |p| {
            sqlx::query_scalar::<_, i64>("SELECT id FROM gb_cloud_record ORDER BY id DESC LIMIT 1")
                .fetch_one(p)
        })
        .unwrap();
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let token = jwt.create_token("admin", 60).unwrap();
        let pool = state.pool.clone();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let add: serde_json::Value = client
            .get(format!(
                "{base}/api/cloud/record/collect/add?recordId={record_pk}"
            ))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(add["code"], 0, "{add}");
        let duplicate: serde_json::Value = client
            .get(format!(
                "{base}/api/cloud/record/collect/add?recordId={record_pk}"
            ))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            duplicate["data"]["status"], "already_collected",
            "{duplicate}"
        );
        let list: serde_json::Value = client
            .get(format!("{base}/api/cloud/record/collect/list"))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(list["data"]["total"], 1, "{list}");
        let collect_flag: i64 = crate::dispatch!(&pool, |p| {
            sqlx::query_scalar::<_, i64>(
                &pool.sql("SELECT collect FROM gb_cloud_record WHERE id = ?"),
            )
            .bind(record_pk)
            .fetch_one(p)
        })
        .unwrap();
        assert_eq!(collect_flag, 1, "add must set gb_cloud_record.collect");
        let deleted: serde_json::Value = client
            .get(format!(
                "{base}/api/cloud/record/collect/delete?id={record_pk}"
            ))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(deleted["code"], 0, "{deleted}");
        let deleted_again: serde_json::Value = client
            .get(format!(
                "{base}/api/cloud/record/collect/delete?id={record_pk}"
            ))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(deleted_again["code"], 0, "{deleted_again}");
        let list: serde_json::Value = client
            .get(format!("{base}/api/cloud/record/collect/list"))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(list["data"]["total"], 0, "{list}");
        let collect_flag: i64 = crate::dispatch!(&pool, |p| {
            sqlx::query_scalar::<_, i64>(
                &pool.sql("SELECT collect FROM gb_cloud_record WHERE id = ?"),
            )
            .bind(record_pk)
            .fetch_one(p)
        })
        .unwrap();
        assert_eq!(collect_flag, 0, "GET must clear gb_cloud_record.collect");

        let add: serde_json::Value = client
            .get(format!(
                "{base}/api/cloud/record/collect/add?recordId={record_pk}"
            ))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(add["code"], 0, "{add}");
        let collect_flag: i64 = crate::dispatch!(&pool, |p| {
            sqlx::query_scalar::<_, i64>(
                &pool.sql("SELECT collect FROM gb_cloud_record WHERE id = ?"),
            )
            .bind(record_pk)
            .fetch_one(p)
        })
        .unwrap();
        assert_eq!(collect_flag, 1, "second add must restore collect flag");
        let delete_response = client
            .delete(format!("{base}/api/cloud/record/collect/delete"))
            .header("access-token", &token)
            .json(&serde_json::json!({"id": record_pk}))
            .send()
            .await
            .unwrap();
        let delete_status = delete_response.status();
        let delete_body = delete_response.text().await.unwrap();
        let deleted: serde_json::Value = serde_json::from_str(&delete_body)
            .unwrap_or_else(|_| panic!("DELETE returned {delete_status}: {delete_body}"));
        assert_eq!(deleted["code"], 0, "{deleted}");
        let deleted_again: serde_json::Value = client
            .delete(format!("{base}/api/cloud/record/collect/delete"))
            .header("access-token", &token)
            .json(&serde_json::json!({"id": record_pk}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(deleted_again["code"], 0, "{deleted_again}");
        let list: serde_json::Value = client
            .get(format!("{base}/api/cloud/record/collect/list"))
            .header("access-token", &token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(list["data"]["total"], 0, "{list}");
        let collect_flag: i64 = crate::dispatch!(&pool, |p| {
            sqlx::query_scalar::<_, i64>(
                &pool.sql("SELECT collect FROM gb_cloud_record WHERE id = ?"),
            )
            .bind(record_pk)
            .fetch_one(p)
        })
        .unwrap();
        assert_eq!(collect_flag, 0, "DELETE must clear gb_cloud_record.collect");
    }

    #[tokio::test]
    async fn diagnostics_routes_require_admin_and_hide_internal_state() {
        let state = app_state().await;
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("INSERT INTO gb_user_role (id, name, authority, create_time, update_time) VALUES (2, 'viewer', '1', '2026-10-17', '2026-10-17')")
                .execute(p)
        })
        .unwrap();
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("INSERT INTO gb_user (id, username, password, role_id, create_time, update_time) VALUES (2, 'viewer', 'unused', 2, '2026-10-17', '2026-10-17')")
                .execute(p)
        })
        .unwrap();
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let admin_token = jwt.create_token("admin", 60).unwrap();
        let viewer_token = jwt.create_token("viewer", 60).unwrap();
        let base = spawn(state).await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for path in ["/api/test/hook/list", "/api/test/redis"] {
            let url = format!("{base}{path}");
            let anonymous = client.get(&url).send().await.unwrap();
            assert_eq!(anonymous.status().as_u16(), 401, "{path}");
            let viewer = client
                .get(&url)
                .header("access-token", &viewer_token)
                .send()
                .await
                .unwrap();
            assert_eq!(viewer.status().as_u16(), 403, "{path}");
            let admin = client
                .get(&url)
                .header("access-token", &admin_token)
                .send()
                .await
                .unwrap();
            assert_eq!(admin.status().as_u16(), 200, "{path}");
            let body: serde_json::Value = admin.json().await.unwrap();
            assert_eq!(body["code"], 0, "{path}: {body}");
            assert!(!body.to_string().contains("redis://"));
            if path.ends_with("/redis") {
                assert_eq!(body["data"]["backend"], "memory");
                assert_eq!(body["data"]["status"], "not_configured");
            } else {
                assert_eq!(
                    body["data"]["capturedTypes"],
                    serde_json::json!(["on_play", "on_publish"])
                );
                assert_eq!(body["data"]["items"], serde_json::json!([]));
            }
        }
    }

    #[tokio::test]
    async fn zlm_proxy_requires_admin_and_blocks_raw_server_config() {
        let state = app_state().await;
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("INSERT INTO gb_user_role (id, name, authority, create_time, update_time) VALUES (2, 'viewer', '1', '2026-10-17', '2026-10-17')")
                .execute(p)
        })
        .unwrap();
        crate::dispatch_exec!(&state.pool, |p| {
            sqlx::query("INSERT INTO gb_user (id, username, password, role_id, create_time, update_time) VALUES (2, 'viewer', 'unused', 2, '2026-10-17', '2026-10-17')")
                .execute(p)
        })
        .unwrap();
        let jwt = crate::auth::JwtKeys::new(state.config.jwt.secret.as_bytes());
        let viewer_token = jwt.create_token("viewer", 60).unwrap();
        let admin_token = jwt.create_token("admin", 60).unwrap();
        let base = spawn(state).await;
        let url = format!("{base}/zlm/unknown/index/api/getServerConfig");
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let viewer_response = client
            .post(&url)
            .header("access-token", viewer_token)
            .send()
            .await
            .unwrap();
        let viewer_status = viewer_response.status();
        let viewer_text = viewer_response.text().await.unwrap();
        let viewer: serde_json::Value = serde_json::from_str(&viewer_text)
            .unwrap_or_else(|_| panic!("viewer {viewer_status}: {viewer_text}"));
        assert_eq!(viewer["code"], 403);
        let admin: serde_json::Value = client
            .post(&url)
            .header("access-token", admin_token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(admin["code"], 403);
    }

    /// 启用 `static_dir`（SPA 静态资源）时路由表仍必须能构建。
    ///
    /// 回归保护：axum 0.8 **禁止** `nest_service("/", ..)`，根路径必须用
    /// `fallback_service`，否则**启动瞬间 panic**（`Nesting at the root is no
    /// longer supported`）。默认测试状态没配 `static_dir`，所以这个 panic
    /// 只在真实部署时暴露 —— 本测试专门把它前移到测试阶段。
    #[tokio::test]
    async fn test_router_builds_with_static_dir_enabled() {
        let mut state = app_state().await;
        // web/dist 由 `npm run build` 产出；不存在时用当前目录兜底，同样能触发该分支。
        let dir = if std::path::Path::new("web/dist").exists() {
            "web/dist"
        } else {
            "."
        };
        let mut cfg = (*state.config).clone();
        cfg.static_dir = Some(dir.to_string());
        state.config = std::sync::Arc::new(cfg);
        let _ = app(state);
    }

    #[tokio::test]
    async fn test_public_routes_ok_and_unknown_is_404() {
        let base = spawn(app_state().await).await;
        assert_eq!(status_of(&base, "/api/health").await, 200);
        assert_eq!(status_of(&base, "/metrics").await, 200);
        // 未注册路径必须 404 —— 否则说明上面的 200 可能是被兜底 handler 吞掉的
        assert_eq!(status_of(&base, "/api/definitely-not-a-route").await, 404);
    }

    // ================= RPC 入站共享密钥（2026-09-11 修复） =================

    fn rpc_body() -> serde_json::Value {
        serde_json::json!({"method": "health", "target": "local", "payload": {}, "reply_to": null})
    }

    /// 配置了 `[rpc].secret` 时，入站必须携带匹配的 `X-RPC-Secret`。
    ///
    /// 此前 `/api/rpc` 完全无鉴权，任何能访问端口的人都能直接调用集群 RPC 方法。
    #[tokio::test]
    async fn test_rpc_endpoint_enforces_shared_secret() {
        let mut state = app_state().await;
        let mut cfg = (*state.config).clone();
        cfg.rpc.secret = Some("rpc-s3cret".to_string());
        state.config = std::sync::Arc::new(cfg);
        let base = spawn(state).await;

        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let url = format!("{}/api/rpc", base);

        // 不带密钥 → 401
        let r = client.post(&url).json(&rpc_body()).send().await.unwrap();
        assert_eq!(r.status().as_u16(), 401, "缺少 X-RPC-Secret 必须被拒绝");

        // 密钥错误 → 401
        let r = client
            .post(&url)
            .header("X-RPC-Secret", "wrong")
            .json(&rpc_body())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 401, "密钥错误必须被拒绝");

        // 密钥正确 → 放行（本测试未配 rpc_router，业务层返回 ok:false，但状态码 200）
        let r = client
            .post(&url)
            .header("X-RPC-Secret", "rpc-s3cret")
            .json(&rpc_body())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 200, "密钥正确应放行");
    }

    /// 未配置 secret 时保持向后兼容（单节点 / 受信内网）
    #[tokio::test]
    async fn test_rpc_endpoint_allows_when_no_secret_configured() {
        let base = spawn(app_state().await).await;
        let r = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
            .post(format!("{}/api/rpc", base))
            .json(&rpc_body())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status().as_u16(), 200, "未配置密钥时应放行以兼容旧部署");
    }

    /// 受保护端点「已注册」的判据是 **401 而非 404**（未注册才会 404）。
    ///
    /// 同时这是一条**安全回归测试**：2026-09-11 发现 `api_public`（无任何中间件）
    /// 里混入了 71 个敏感端点 —— 云录像下载、角色增删、JT1078 控制、RTP/PS 控制、
    /// 服务器配置、区域/告警等全部可**未鉴权访问**（实测返回 200）。
    /// 已全部移入带 audit + auth 中间件的 `api_protected`。
    #[tokio::test]
    async fn test_protected_routes_require_auth() {
        let base = spawn(app_state().await).await;
        for path in [
            // 云录像
            "/api/cloud/record/download/1",
            "/api/cloud/record/download/zip",
            "/api/cloud/record/list-url",
            "/api/cloud/record/zip",
            // 区域
            "/api/region/one?id=1",
            "/api/region/page/list",
            "/api/region/sync",
            // 地图 / 告警 / 前端指令
            "/api/common/channel/map/tile/10/1/1",
            "/api/common/channel/map/thin/tile/10/1/1",
            "/api/common/channel/playback/pause",
            "/api/alarm/snap/dev1",
            "/api/alarm/clear",
            "/api/front-end/common/ptz/34020000001310000001",
            // 服务器信息 / 平台 / 中亿视图
            "/api/server/config",
            "/api/server/version",
            "/api/platform/info/1",
            "/api/sy/camera/list",
            // 中亿视图对接契约（A1 补齐的别名端点）
            "/api/sy/camera/one",
            "/api/sy/camera/update",
            "/api/sy/push/play",
            "/api/sy/push/play-without-check",
            "/api/sy/test",
            "/api/sy/record/collect/add",
            "/api/sy/record/collect/delete",
            "/api/sy/record/list-url",
            "/api/sy/record/zip",
            "/api/sy/forceClose",
            // JT1078 / RTP / PS / 推流代理
            "/api/jt1078/route/query",
            "/api/jt1078/record/start",
            "/api/jt1078/record/stop",
            "/api/jt1078/control/temp-position-tracking",
            "/api/jt1078/playback/download",
            "/api/jt1078/area/circle/query",
            "/api/rtp/send/stop/abc",
            "/api/ps/getTestPort",
            "/api/proxy/one",
            "/api/push/forceClose",
            "/api/user/logout",
        ] {
            let status = status_of(&base, path).await;
            assert_ne!(status, 404, "{} 未注册到路由表（返回 404）", path);
            assert_eq!(status, 401, "{} 未受鉴权保护（越权风险）", path);
        }

        // POST 型端点同样必须受保护（role/add 可提权，尤为关键）
        for path in [
            "/api/role/add",
            "/api/jt1078/area/circle/add",
            "/api/jt1078/confirmation-alarm-message",
            "/api/platform/catalog/add",
            "/api/platform/catalog/edit",
        ] {
            let status = post_status_of(&base, path).await;
            assert_ne!(status, 404, "{} 未注册到路由表（返回 404）", path);
            assert_eq!(status, 401, "{} 未受鉴权保护（越权风险）", path);
        }
    }
}
