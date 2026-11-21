//! 桌面边界统一使用核心错误，HTTP 适配转换保留在应用层。
pub use clip_core::errors::{AppError, AppResult};

/// HttpError 转换为 AppError
#[cfg(any(feature = "sync", feature = "commercial-extension"))]
impl From<crate::network::http_client::HttpError> for AppError {
    /// 将项目 HTTP Client 错误转换为统一 HTTP 错误。
    fn from(err: crate::network::http_client::HttpError) -> Self {
        AppError::Http(err.to_string())
    }
}
