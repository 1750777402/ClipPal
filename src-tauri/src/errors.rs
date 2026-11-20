use std::sync::PoisonError;
use thiserror::Error;

/// 应用程序统一错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rbatis::Error),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serde(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("窗口操作错误: {0}")]
    Window(String),

    #[error("剪贴板操作错误: {0}")]
    Clipboard(String),

    #[error("云同步错误: {0}")]
    ClipSync(String),

    #[error("加密解密错误: {0}")]
    Crypto(String),

    #[error("锁争用错误: {0}")]
    Lock(String),

    #[error("全局快捷键错误: {0}")]
    GlobalShortcut(String),

    #[error("系统托盘错误: {0}")]
    Tray(String),

    #[error("HTTP请求错误: {0}")]
    Http(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("系统操作错误: {0}")]
    System(String),

    #[error("自动粘贴错误: {0}")]
    AutoPaste(String),

    #[error("通用错误: {0}")]
    General(String),
}

impl AppError {
    /// 前端统一识别用的稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database_error",
            AppError::Io(_) => "io_error",
            AppError::Serde(_) => "serde_error",
            AppError::Config(_) => "config_error",
            AppError::Window(_) => "window_error",
            AppError::Clipboard(_) => "clipboard_error",
            AppError::ClipSync(_) => "sync_error",
            AppError::Crypto(_) => "crypto_error",
            AppError::Lock(_) => "lock_error",
            AppError::GlobalShortcut(_) => "global_shortcut_error",
            AppError::Tray(_) => "tray_error",
            AppError::Http(_) => "http_error",
            AppError::Network(_) => "network_error",
            AppError::System(_) => "system_error",
            AppError::AutoPaste(_) => "auto_paste_error",
            AppError::General(_) => "internal_error",
        }
    }

    /// 返回适合前端展示的安全消息；技术细节统一写入后端日志。
    pub fn user_message(&self) -> String {
        match self {
            // 这些错误通常包含数据库、网络或系统实现细节，不直接暴露给用户。
            AppError::Database(_)
            | AppError::Io(_)
            | AppError::Serde(_)
            | AppError::Lock(_)
            | AppError::Crypto(_) => "系统异常，请稍后重试".to_string(),
            AppError::Http(_) | AppError::Network(_) => "网络服务暂时不可用，请稍后重试".to_string(),
            AppError::Window(_) | AppError::Tray(_) | AppError::System(_) => {
                "系统操作异常，请稍后重试".to_string()
            }
            AppError::GlobalShortcut(_) => "快捷键设置失败，请检查后重试".to_string(),
            AppError::Clipboard(_) => "剪贴板操作失败，请稍后重试".to_string(),
            AppError::AutoPaste(_) => "自动粘贴失败，请稍后重试".to_string(),
            AppError::ClipSync(_) => "云同步异常，请稍后重试".to_string(),
            // General / Config 目前承载业务校验消息，需要保留可操作提示。
            AppError::Config(_) | AppError::General(_) => self.to_string(),
        }
    }
}

/// String 类型的错误转换
impl From<AppError> for String {
    /// 将统一应用错误转换为边界层可直接返回的字符串消息。
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

/// 锁操作的安全包装
impl<T> From<PoisonError<T>> for AppError {
    /// 将标准库锁中毒错误转换为统一锁错误。
    fn from(err: PoisonError<T>) -> Self {
        AppError::Lock(format!("锁已中毒: {}", err))
    }
}

/// HttpError 转换为 AppError
impl From<crate::utils::http_client::HttpError> for AppError {
    /// 将项目 HTTP Client 错误转换为统一 HTTP 错误。
    fn from(err: crate::utils::http_client::HttpError) -> Self {
        AppError::Http(err.to_string())
    }
}

/// anyhow::Error 转换为 AppError
impl From<anyhow::Error> for AppError {
    /// 将缺少专用分类的 anyhow 错误收敛为通用错误。
    fn from(err: anyhow::Error) -> Self {
        AppError::General(err.to_string())
    }
}

/// serde_json::Error 转换为 AppError
impl From<serde_json::Error> for AppError {
    /// 将 JSON 序列化或反序列化错误转换为统一序列化错误。
    fn from(err: serde_json::Error) -> Self {
        AppError::Serde(err.to_string())
    }
}

/// base64::DecodeError 转换为 AppError
impl From<base64::DecodeError> for AppError {
    /// 将 Base64 解码失败归类为加解密错误。
    fn from(err: base64::DecodeError) -> Self {
        AppError::Crypto(format!("Base64解码错误: {}", err))
    }
}

/// 应用程序结果类型
pub type AppResult<T> = Result<T, AppError>;

/// 错误日志记录宏
#[macro_export]
macro_rules! log_error {
    ($err:expr, $context:expr) => {
        log::error!("{}: {}", $context, $err);
        $err
    };
}

/// 安全转换宏，将Option转换为Result
#[macro_export]
macro_rules! ok_or_err {
    ($option:expr, $err_msg:expr) => {
        $option.ok_or_else(|| AppError::General($err_msg.to_string()))
    };
}
