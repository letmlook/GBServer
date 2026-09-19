use serde::Serialize;
use utoipa::ToSchema;

/// 全平台统一响应信封：`{ code, msg, data }`。
///
/// `code == 0` 表示成功（前端 `web/src/utils/request.ts` 据此判断），
/// 非 0 一律按业务错误处理；错误响应由 `AppError::into_response()` 生成。
///
/// OpenAPI 文档里每个接口都写成 `ApiResult<具体类型>`，信封结构只描述这一次
/// （见 `src/openapi.rs` 的 `ApiDoc`）。
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResult<T> {
    /// 业务码：`0` = 成功，非 0 见 `ErrorCode`
    pub code: i32,
    pub msg: String,
    /// 业务数据；出错时为 `null`
    pub data: Option<T>,
}

impl<T> ApiResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "成功".to_string(),
            data: Some(data),
        }
    }

    pub fn success_empty() -> ApiResult<()> {
        ApiResult {
            code: 0,
            msg: "成功".to_string(),
            data: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> ApiResult<T> {
        ApiResult {
            code: -1,
            msg: msg.into(),
            data: None,
        }
    }
}

impl ApiResult<()> {
    pub fn fail(code: i32, msg: impl Into<String>) -> Self {
        ApiResult {
            code,
            msg: msg.into(),
            data: None,
        }
    }
}
