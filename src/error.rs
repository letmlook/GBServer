use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;
use crate::response::WVPResult;

/// 与 Java ErrorCode 对齐的业务错误码
#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    Success = 0,
    Error100 = 100,   // 失败
    Error400 = 400,   // 参数或方法错误
    Error401 = 401,   // 请登录后重新请求
    Error403 = 403,   // 无权限操作
    Error404 = 404,   // 资源未找到
    Error408 = 408,   // 请求超时
    Error486 = 486,   // 超时或无响应
    Error500 = 500,   // 系统异常
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

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, status, msg) = match &self {
            AppError::Unauthorized => (ErrorCode::Error401.code(), StatusCode::UNAUTHORIZED, ErrorCode::Error401.msg().to_string()),
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
            AppError::Db(e) => (ErrorCode::Error500.code(), StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Config(e) => (ErrorCode::Error500.code(), StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        let body = Json(WVPResult::<()>::fail(code, msg));
        (status, body).into_response()
    }
}
