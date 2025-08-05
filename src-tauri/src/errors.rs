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

    #[error("通用错误: {0}")]
    General(String),
}

/// String 类型的错误转换
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

/// 锁操作的安全包装
impl<T> From<PoisonError<T>> for AppError {
    fn from(err: PoisonError<T>) -> Self {
        AppError::Lock(format!("锁已中毒: {}", err))
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
