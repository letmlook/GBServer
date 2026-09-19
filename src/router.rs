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
    let app = app.merge(
        utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
            .url("/api/openapi.json", openapi),
    );
    tracing::info!(
        "OpenAPI 文档已挂载：/swagger-ui（规范：/api/openapi.json），当前已收录 {} 条路径",
        path_count
    );

    // WebSocket：设备状态实时通知 (Phase 7.3 + 7.4: JWT 校验在 ws_handler 内部)

    // Phase 7.4: alarm endpoints moved into api_protected (now require JWT).
    // The legacy public routes below are intentionally removed.

    // 云录像 ZIP 打包产物：通过 /downloads/<file> 对外下载。
    // 必须注册在下面的 `nest_service("/", ...)` 之前，否则会被 SPA 的
    // index.html 兜底吞掉。文件名含随机段，避免被枚举遍历。
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
        app.nest_service("/", serve_dir)
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

#[cfg(all(test, feature = "sqlite"))]
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
        let declared = doc["paths"]["/api/region/one"]["get"]["security"]
            .to_string();
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
