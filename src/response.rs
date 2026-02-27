use serde::Serialize;

/// 与 Java WVPResult 一致：前端通过 code===0 判断成功
#[derive(Debug, Serialize)]
pub struct WVPResult<T> {
    pub code: i32,
    pub msg: String,
    pub data: Option<T>,
}

impl<T> WVPResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "成功".to_string(),
            data: Some(data),
        }
    }

    pub fn success_empty() -> WVPResult<()> {
        WVPResult {
            code: 0,
            msg: "成功".to_string(),
            data: None,
        }
    }

    pub fn fail(code: i32, msg: impl Into<String>) -> WVPResult<()> {
        WVPResult {
            code,
            msg: msg.into(),
            data: None,
        }
    }
}
