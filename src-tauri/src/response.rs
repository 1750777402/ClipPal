use serde::Serialize;

use crate::errors::{AppError, AppResult};

/// Tauri command 统一响应结构。
///
/// 业务层继续返回 `AppResult<T>`，command 层只在边界处转换为该结构。
/// 前端只需要判断 `success`，成功时读取 `data`，失败时读取 `error`。
#[derive(Debug, Serialize)]
pub struct CommandResponse<T>
where
    T: Serialize,
{
    /// command 是否执行成功。
    pub success: bool,
    /// 成功时返回的数据。失败时固定为 `None`。
    pub data: Option<T>,
    /// 失败时返回的错误信息。成功时固定为 `None`。
    pub error: Option<CommandErrorBody>,
}

/// 前端可消费的错误信息。
///
/// 这里不直接暴露 Rust 错误类型，避免前端依赖后端内部实现细节。
#[derive(Debug, Serialize)]
pub struct CommandErrorBody {
    /// 稳定错误码，用于前端做精确判断或国际化映射。
    pub code: String,
    /// 用户可读错误信息，可直接用于提示。
    pub message: String,
    /// 错误严重程度，用于前端决定是否提示以及提示样式。
    pub severity: ErrorSeverity,
}

/// 错误严重程度。
///
/// 序列化为 snake_case，保持和前端 TypeScript 字面量类型一致。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    /// 静默错误，前端一般不主动提示。
    Silent,
    /// 普通信息，适合轻量提示。
    Info,
    /// 警告信息，适合提示用户操作失败但应用仍可继续使用。
    Warning,
    /// 严重错误，通常表示关键功能失败或需要用户立即处理。
    Critical,
}

impl<T> CommandResponse<T>
where
    T: Serialize,
{
    /// 构造成功响应。
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// 构造失败响应。
    pub fn error(error: AppError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(CommandErrorBody::from(error)),
        }
    }
}

impl CommandResponse<()> {
    /// 构造无业务数据的成功响应。
    ///
    /// 例如保存设置、删除记录这类 command，只需要表达成功或失败。
    pub fn empty_ok() -> Self {
        Self {
            success: true,
            data: Some(()),
            error: None,
        }
    }
}

impl From<AppError> for CommandErrorBody {
    /// 将后端内部错误转换成前端统一错误体。
    fn from(error: AppError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.user_message(),
            severity: ErrorSeverity::from(&error),
        }
    }
}

impl From<&AppError> for ErrorSeverity {
    /// 根据错误类型推导默认提示级别。
    ///
    /// 后续如果某个 command 需要更细粒度的展示策略，可以在 command 层覆盖。
    fn from(error: &AppError) -> Self {
        match error {
            AppError::Database(_) => ErrorSeverity::Warning,
            AppError::Io(_) => ErrorSeverity::Warning,
            AppError::Serde(_) => ErrorSeverity::Warning,
            AppError::Config(_) => ErrorSeverity::Warning,
            AppError::Window(_) => ErrorSeverity::Warning,
            AppError::Clipboard(_) => ErrorSeverity::Critical,
            AppError::ClipSync(_) => ErrorSeverity::Info,
            AppError::Crypto(_) => ErrorSeverity::Warning,
            AppError::Lock(_) => ErrorSeverity::Warning,
            AppError::GlobalShortcut(_) => ErrorSeverity::Warning,
            AppError::Tray(_) => ErrorSeverity::Warning,
            AppError::Http(_) => ErrorSeverity::Warning,
            AppError::Network(_) => ErrorSeverity::Warning,
            AppError::System(_) => ErrorSeverity::Warning,
            AppError::AutoPaste(_) => ErrorSeverity::Critical,
            AppError::General(_) => ErrorSeverity::Info,
        }
    }
}

/// 快捷函数：构造成功响应。
pub fn ok<T>(data: T) -> CommandResponse<T>
where
    T: Serialize,
{
    CommandResponse::ok(data)
}

/// 快捷函数：构造无业务数据的成功响应。
pub fn empty_ok() -> CommandResponse<()> {
    CommandResponse::empty_ok()
}

/// 快捷函数：构造失败响应。
pub fn fail<T>(error: AppError) -> CommandResponse<T>
where
    T: Serialize,
{
    CommandResponse::error(error)
}

/// 将业务层 `AppResult<T>` 转换为 command 统一响应。
///
/// command 层推荐直接返回该函数结果，避免每个 command 重复写 match。
pub fn command_result<T>(result: AppResult<T>) -> CommandResponse<T>
where
    T: Serialize,
{
    match result {
        Ok(data) => ok(data),
        Err(error) => fail(error),
    }
}

pub fn string_result<T>(result: Result<T, String>) -> CommandResponse<T>
where
    T: Serialize,
{
    match result {
        Ok(data) => ok(data),
        Err(error) => fail(AppError::General(error)),
    }
}
