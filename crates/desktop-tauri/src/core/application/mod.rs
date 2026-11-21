//! 桌面应用适配；通过 AppContext 组装单机应用核心、系统能力和注入的扩展策略。
pub mod clip_record_service;
pub mod clipboard_service;
pub mod settings_service;
pub use clip_record_service::ClipRecordService;
pub use clipboard_service::ClipboardService;
pub use settings_service::SettingsService;
pub mod capture_listener;
pub mod update_service;
