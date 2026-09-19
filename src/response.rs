use serde::Serialize;

/// 全平台统一响应信封：`{ code, msg, data }`。
///
/// `code == 0` 表示成功（前端 `web/src/utils/request.ts` 据此判断），
/// 非 0 一律按业务错误处理；错误响应由 `AppError::into_response()` 生成。
#[derive(Debug, Serialize)]
pub struct ApiResult<T> {
    pub code: i32,
    pub msg: String,
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
