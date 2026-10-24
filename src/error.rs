use crate::response::ApiResult;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;

/// 全平台统一业务错误码（HTTP 状态码 + 业务码双层表达）。
#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    Success = 0,
    Error100 = 100, // 失败
    Error400 = 400, // 参数或方法错误
    Error401 = 401, // 请登录后重新请求
    Error403 = 403, // 无权限操作
    Error404 = 404, // 资源未找到
    Error408 = 408, // 请求超时
    Error486 = 486, // 超时或无响应
    Error500 = 500, // 系统异常
}

impl ErrorCode {
    pub fn code(self) -> i32 {
        self as i32
    }
    pub fn msg(self) -> &'static str {
        match self {
            ErrorCode::Success => "成功",
            ErrorCode::Error100 => "失败",
            ErrorCode::Error400 => "参数或方法错误",
            ErrorCode::Error401 => "请登录后重新请求",
            ErrorCode::Error403 => "无权限操作",
            ErrorCode::Error404 => "资源未找到",
            ErrorCode::Error408 => "请求超时",
            ErrorCode::Error486 => "超时或无响应",
            ErrorCode::Error500 => "系统异常",
        }
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("业务错误: {1}")]
    Business(ErrorCode, String),
    #[error("未授权")]
    Unauthorized,
    #[error("数据库: {0}")]
    Db(#[from] sqlx::Error),
    #[error("配置: {0}")]
    Config(#[from] anyhow::Error),
}

impl AppError {
    pub fn business(code: ErrorCode, msg: impl Into<String>) -> Self {
        AppError::Business(code, msg.into())
    }
}

/// 把 axum 提取器拒绝产生的**裸文本**错误响应改写成契约信封 `{code,msg,data}`。
///
/// axum 的 `Query` / `Json` / `Path` 提取器在解析失败时**直接**回
/// `text/plain` 的 400/415/422（例：`Failed to deserialize query string: ...`），
/// 绕过了 handler —— 所以 handler 里怎么写都救不回来。
///
/// 对**第三方对接面**（`/api/sy/*`）这是硬伤：对接方按 `{code,msg,data}` 解析，
/// 拿到裸文本会直接让它的解析器炸。本中间件把这类响应包回信封，
/// **只改写「纯文本的错误响应」**，其它响应（JSON 信封、404 路由未命中等）一律放行。
///
/// 用法：`router.layer(middleware::from_fn(envelope_rejection_errors))`。
///
/// 只挂在第三方对接面上：平台自己的端点调用方是本仓库的前端（axios 对非 2xx
/// 有统一处理），全量迁移属于独立改动（见 `docs/OPEN_ISSUES.md` 的 B20 记录）。
pub async fn envelope_rejection_errors(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let resp = next.run(req).await;

    let is_plain_text = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("text/plain"))
        .unwrap_or(false);
    let status = resp.status();
    let is_rejection = status == StatusCode::BAD_REQUEST
        || status == StatusCode::UNSUPPORTED_MEDIA_TYPE
        || status == StatusCode::UNPROCESSABLE_ENTITY;
    if !is_plain_text || !is_rejection {
        return resp;
    }

    // 取回裸文本正文（上限 64KB，避免异常大的错误页被整体读进内存）
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap_or_default();
    let raw = String::from_utf8_lossy(&body);
    let msg: String = raw.trim().chars().take(200).collect::<String>();
    let msg = if msg.is_empty() {
        "请求参数格式不正确".to_string()
    } else {
        msg
    };
    (
        status,
        Json(ApiResult::<()>::fail(status.as_u16() as i32, msg)),
    )
        .into_response()
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, status, msg) = match &self {
            AppError::Unauthorized => (
                ErrorCode::Error401.code(),
                StatusCode::UNAUTHORIZED,
                ErrorCode::Error401.msg().to_string(),
            ),
            AppError::Business(ec, m) => {
                let status = match ec.code() {
                    401 => StatusCode::UNAUTHORIZED,
                    403 => StatusCode::FORBIDDEN,
                    404 => StatusCode::NOT_FOUND,
                    400 => StatusCode::BAD_REQUEST,
                    _ => StatusCode::OK,
                };
                (ec.code(), status, m.clone())
            }
            AppError::Db(e) => (
                ErrorCode::Error500.code(),
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ),
            AppError::Config(e) => (
                ErrorCode::Error500.code(),
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ),
        };
        let body = Json(ApiResult::<()>::fail(code, msg));
        (status, body).into_response()
    }
}
