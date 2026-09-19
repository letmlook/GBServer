//! OpenAPI 文档配置（安全方案 / 标签 / 各域路由注册）。
//!
//! 接口清单**不手工维护**：每个域在 `routes_*.rs` 里用 `routes!()` 注册路由，
//! 同时产出 axum 路由与 OpenAPI path，因此不存在「加了路由忘了写文档」。
//!
//! 访问方式：
//! * `/swagger-ui` —— 交互式文档（右上角 Authorize 填 token 后可直接调用）
//! * `/api/openapi.json` —— OpenAPI 3.1 规范，供 Postman / Apifox / 前端代码生成

use axum::Router;
use utoipa::openapi::path::Paths;
use utoipa::openapi::{RefOr, Schema};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::AppState;


/// 一个域注册完的产物：`(schemas, paths, 真实 axum Router)`。
pub mod registry_protected_routes;
pub mod registry_public_routes;
pub mod routes_channel;
pub mod routes_cloud_record;
pub mod routes_device;
pub mod routes_device_public;
pub mod routes_health_public;
pub mod routes_jt1078;
pub mod routes_live;
pub mod routes_live_public;
pub mod routes_misc;
pub mod routes_platform;
pub mod routes_public;
pub mod routes_region;
pub mod routes_stream;
pub mod routes_sy;
pub mod routes_system;
pub mod routes_user;
pub mod routes_user_public;

pub type DocumentedRoutes = (Vec<(String, RefOr<Schema>)>, Paths, Router<AppState>);

/// 各域 `routes!()` 的累加器。
///
/// 存在的意义：`routes!()` 一次只能放**一条**路由（多条会让宏为每个 handler 重复
/// 注册同一组 method → 启动 panic `Overlapping method route`），所以需要一个地方
/// 把逐条结果攒起来。
#[derive(Default)]
pub struct RoutesAccumulator {
    schemas: Vec<(String, RefOr<Schema>)>,
    paths: Paths,
    router: Router<AppState>,
}

impl RoutesAccumulator {
    /// 并入一条 `routes!()` 产物。方法路由会按其 paths.paths 里声明的 path
    /// 逐条 `router.route(path, m)` 注册 —— path 与 spec 来自同一宏产物，不会漂移。
    ///
    /// axum 0.8 的 `Router::merge` 不接受 `MethodRouter`，所以这里手动按 path 注册。
    pub fn add(
        &mut self,
        (schemas, mut paths, method_router): (
            Vec<(String, RefOr<Schema>)>,
            Paths,
            axum::routing::MethodRouter<AppState>,
        ),
    ) {
        self.schemas.extend(schemas);
        let route_paths: Vec<String> = paths.paths.keys().cloned().collect();
        for path in route_paths {
            if let Some(item) = paths.paths.remove(&path) {
                // 同一 path 多 method 的合并（如 GET+POST 共用路径）
                match self.paths.paths.get_mut(&path) {
                    Some(existing) => existing.merge_operations(item),
                    None => {
                        self.paths.paths.insert(path.clone(), item);
                    }
                }
                self.router = self.router.clone().route(&path, method_router.clone());
            }
        }
    }

    /// 并入整个域的产物（域模块返回的 `DocumentedRoutes`）。
    pub fn merge(&mut self, (schemas, paths, router): DocumentedRoutes) {
        self.schemas.extend(schemas);
        for (path, item) in paths.paths {
            match self.paths.paths.get_mut(&path) {
                Some(existing) => existing.merge_operations(item),
                None => {
                    self.paths.paths.insert(path, item);
                }
            }
        }
        self.router = std::mem::take(&mut self.router).merge(router);
    }

    pub fn finish(self) -> DocumentedRoutes {
        (self.schemas, self.paths, self.router)
    }
}

/// 注入鉴权方案，供 Swagger UI 的 Authorize 按钮使用。
///
/// 与 `src/auth.rs` 的实际校验保持一致：
/// * `access_token` —— JWT，请求头 `access-token`（前端 axios 拦截器用的就是这个）
/// * `bearer_auth`  —— JWT，标准 `Authorization: Bearer <token>`
/// * `api_key`      —— API Key，请求头 `X-API-Key`
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let Some(components) = openapi.components.as_mut() else {
            return;
        };
        components.add_security_scheme(
            "access_token",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("access-token"))),
        );
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("与 access_token 等价的 JWT 承载方式"))
                    .build(),
            ),
        );
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
        );
    }
}

/// 文档骨架：仅描述信息与标签分组。
///
/// 路径（paths）全部由各域 `routes_*.rs` 在注册路由时收集，
/// 所以这里**不需要也不应该**列出 path 清单。
#[derive(OpenApi)]
#[openapi(
    info(
        title = "GBServer API",
        description = "GB28181 国标信令 / 流媒体接入 / 级联管理平台接口文档。\n\n\
             **统一响应信封**：所有接口返回 `{ code, msg, data }`，`code == 0` 表示成功。\n\n\
             **鉴权**：点击右上角 Authorize，填 JWT（`access-token`）或 API Key（`X-API-Key`）。\n\
             标注为公开的接口（登录、健康检查、ZLM hook、分享播放等）无需鉴权。",
        version = env!("CARGO_PKG_VERSION"),
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "user", description = "登录 / 用户 / 角色 / API Key"),
        (name = "device", description = "国标设备接入与查询"),
        (name = "channel", description = "通道 / 区域 / 业务分组"),
        (name = "live", description = "实时点播 / 抓图 / WebRTC / 分享"),
        (name = "playback", description = "历史回放与回放控制"),
        (name = "cloud-record", description = "云端录像 / 录像计划 / 收藏"),
        (name = "stream", description = "推流与拉流代理"),
        (name = "platform", description = "级联平台（上级平台对接）"),
        (name = "media-server", description = "ZLMediaKit 媒体节点"),
        (name = "alarm", description = "报警与移动位置"),
        (name = "jt1078", description = "JT1078 车辆终端"),
        (name = "control", description = "云台 / 设备控制 / 配置下发"),
        (name = "system", description = "系统信息 / 健康检查 / 日志"),
    ),
)]
pub struct ApiDoc;

/// 组装完整 OpenAPI 文档：`ApiDoc`（info/安全方案/标签）+ 两个注册表的 paths/schemas。
///
/// `router.rs` 与「spec 路径可达性」测试都用它，避免两处各写一遍组装逻辑。
pub fn build_openapi() -> utoipa::openapi::OpenApi {
    let (prot_schemas, prot_paths, _) = registry_protected_routes::protected_routes();
    let (pub_schemas, pub_paths, _) = registry_public_routes::public_routes();

    let mut routes = utoipa::openapi::OpenApiBuilder::new().build();
    let mut components = utoipa::openapi::Components::new();
    components.schemas.extend(prot_schemas);
    components.schemas.extend(pub_schemas);
    routes.components = Some(components);

    for (path, item) in prot_paths.paths {
        routes.paths.paths.insert(path, item);
    }
    for (path, item) in pub_paths.paths {
        match routes.paths.paths.get_mut(&path) {
            Some(existing) => existing.merge_operations(item),
            None => {
                routes.paths.paths.insert(path, item);
            }
        }
    }

    // `merge_from` 只补 `self` 中不存在的项，因此 Info 与安全方案不会被覆盖。
    ApiDoc::openapi().merge_from(routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 安全方案必须齐全：缺一个，Swagger UI 的 Authorize 就会少一种，
    /// 而「交互调用」正是本次要交付的能力。
    #[test]
    fn security_schemes_are_registered() {
        let doc = ApiDoc::openapi();
        let components = doc.components.expect("components 必须存在");
        for name in ["access_token", "bearer_auth", "api_key"] {
            assert!(
                components.security_schemes.contains_key(name),
                "缺少安全方案 {name}"
            );
        }
    }

    #[test]
    fn info_is_filled_from_cargo() {
        let doc = ApiDoc::openapi();
        assert_eq!(doc.info.title, "GBServer API");
        assert!(!doc.info.version.is_empty());
    }
}
