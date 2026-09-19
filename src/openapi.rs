//! OpenAPI 文档配置（安全方案 / 标签 / 通用响应）。
//!
//! 接口清单**不在这里手工维护**：路由与文档由 `src/router.rs` 的
//! `OpenApiRouter` + `routes!()` 在同一处注册，避免「文档漏登记」。
//! 本模块只放跨接口共享的部分。
//!
//! 访问方式：
//! * `/swagger-ui` —— 交互式文档（右上角 Authorize 填 token 后可直接调用）
//! * `/api/openapi.json` —— OpenAPI 3.1 规范，供 Postman / Apifox / 前端代码生成

use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

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
/// 路径（paths）全部由 `router.rs` 的 `routes!()` 在注册路由时收集，
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
