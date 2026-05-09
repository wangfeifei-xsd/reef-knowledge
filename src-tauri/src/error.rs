//! 统一错误类型：复刻 Python 版本"HTTP 状态码 + 简短中文 detail"风格。
//! 所有 IPC command 返回 `Result<T, AppError>`；前端拿到的是 JSON 字符串。

use serde::{Serialize, Serializer};
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 400 Bad Request
    #[error("{0}")]
    BadRequest(String),

    /// 401 Unauthorized
    #[error("{0}")]
    Unauthorized(String),

    /// 403 Forbidden
    #[error("{0}")]
    Forbidden(String),

    /// 404 Not Found
    #[error("{0}")]
    NotFound(String),

    /// 413 Payload Too Large
    #[error("{0}")]
    PayloadTooLarge(String),

    /// 415 Unsupported Media Type
    #[error("{0}")]
    UnsupportedMedia(String),

    /// 502 Bad Gateway（上游模型异常）
    #[error("{0}")]
    BadGateway(String),

    /// 503 Service Unavailable（未配置密钥等）
    #[error("{0}")]
    ServiceUnavailable(String),

    /// 504 Gateway Timeout（模型超时）
    #[error("{0}")]
    GatewayTimeout(String),

    /// 500 Internal Server Error
    #[error("{0}")]
    Internal(String),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),

    #[error(transparent)]
    Walk(#[from] walkdir::Error),
}

impl AppError {
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::PayloadTooLarge(_) => 413,
            AppError::UnsupportedMedia(_) => 415,
            AppError::BadGateway(_) => 502,
            AppError::ServiceUnavailable(_) => 503,
            AppError::GatewayTimeout(_) => 504,
            AppError::Internal(_) => 500,
            AppError::Io(_) | AppError::Json(_) | AppError::Yaml(_) => 500,
            AppError::Zip(_) | AppError::Walk(_) => 500,
        }
    }

    pub fn detail(&self) -> String {
        self.to_string()
    }
}

/// 将错误序列化为前端友好的 `{ status, detail }`。
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("status", &self.status_code())?;
        s.serialize_field("detail", &self.detail())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

// ----- 便捷转换 -----

impl From<async_openai::error::OpenAIError> for AppError {
    /// 复刻 Python `_raise_http_from_openai_error`：将 OpenAI SDK 错误映射为合理的 HTTP 风格错误。
    /// 优先级：reqwest HTTP status → ApiError code/type → 兜底 502。
    fn from(e: async_openai::error::OpenAIError) -> Self {
        use async_openai::error::OpenAIError as E;
        match &e {
            E::ApiError(api) => {
                let code = api.code.as_deref().unwrap_or("");
                let r#type = api.r#type.as_deref().unwrap_or("");
                let msg: String = api.message.chars().take(400).collect();

                if code.eq_ignore_ascii_case("invalid_api_key")
                    || r#type.eq_ignore_ascii_case("invalid_request_error")
                        && msg.to_lowercase().contains("api key")
                {
                    return AppError::Unauthorized(format!(
                        "模型接口鉴权失败(401)，请检查 API Key 与 Base URL：{msg}"
                    ));
                }
                if code.eq_ignore_ascii_case("rate_limit_exceeded")
                    || r#type.eq_ignore_ascii_case("rate_limit_error")
                    || r#type.eq_ignore_ascii_case("rate_limit_exceeded")
                {
                    return AppError::BadGateway(format!("模型接口限流(429)：{msg}"));
                }
                if r#type.eq_ignore_ascii_case("server_error")
                    || code.eq_ignore_ascii_case("server_error")
                {
                    return AppError::BadGateway(format!("模型上游错误：{msg}"));
                }
                if r#type.eq_ignore_ascii_case("service_unavailable")
                    || code.eq_ignore_ascii_case("service_unavailable")
                {
                    return AppError::BadGateway(format!("模型服务暂不可用(503)：{msg}"));
                }
                if r#type.eq_ignore_ascii_case("timeout")
                    || code.eq_ignore_ascii_case("timeout")
                {
                    return AppError::GatewayTimeout(format!(
                        "模型接口在等待响应时超时(504)：{msg}"
                    ));
                }
                AppError::BadGateway(format!("模型接口错误：{msg}"))
            }
            E::Reqwest(re) => {
                if re.is_timeout() {
                    return AppError::GatewayTimeout(
                        "模型接口在等待响应时超时(504)。可适当增大 openai_timeout_seconds；若前面还有 nginx 等网关，请同步增大 proxy_read_timeout。".to_string(),
                    );
                }
                if re.is_connect() {
                    return AppError::BadGateway(format!("无法连接模型接口(502)：{re}"));
                }
                if let Some(status) = re.status() {
                    let sc = status.as_u16();
                    return match sc {
                        504 => AppError::GatewayTimeout(
                            "上游网关超时(504)：多为反向代理在模型返回前断开。请在网关侧增大 proxy_read_timeout / send_timeout，或减少单次编译素材长度、换更快模型。".to_string(),
                        ),
                        503 => AppError::ServiceUnavailable("模型服务暂不可用(503)，请稍后重试".to_string()),
                        502 => AppError::BadGateway("模型网关返回 502，上游不可用或路由错误".to_string()),
                        429 => AppError::BadGateway("模型接口限流(429)，请稍后重试".to_string()),
                        401 => AppError::Unauthorized("模型接口鉴权失败(401)，请检查 API Key 与 Base URL".to_string()),
                        500..=599 => AppError::BadGateway(format!("模型上游错误(HTTP {sc})")),
                        _ => AppError::BadGateway(format!("模型接口错误(HTTP {sc})：{re}")),
                    };
                }
                AppError::BadGateway(format!("模型调用失败：{re}"))
            }
            E::JSONDeserialize(je) => AppError::BadGateway(format!("模型响应解析失败：{je}")),
            E::FileSaveError(s) | E::FileReadError(s) => AppError::Internal(s.clone()),
            E::StreamError(s) => AppError::BadGateway(format!("模型流错误：{s}")),
            E::InvalidArgument(s) => AppError::BadRequest(s.clone()),
        }
    }
}

#[macro_export]
macro_rules! bail_bad_request {
    ($($arg:tt)*) => {
        return Err($crate::error::AppError::BadRequest(format!($($arg)*)))
    };
}
