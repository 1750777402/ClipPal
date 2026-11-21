use clip_core::{domain::settings::Settings, errors::{AppError, AppResult}};
/// 只校验本地设置值；系统快捷键解析、自动启动由桌面适配负责。
pub fn validate(settings: &Settings, max_records: u32) -> AppResult<()> {
    if settings.max_records < 50 { return Err(AppError::Config("记录条数不能少于 50".into())); }
    if settings.max_records > max_records.min(i32::MAX as u32) {
        return Err(AppError::Config(format!("记录条数不能超过 {}", max_records.min(i32::MAX as u32))));
    }
    if [settings.auto_start, settings.auto_paste, settings.cloud_sync, settings.tutorial_completed].iter().any(|value| *value > 1) {
        return Err(AppError::Config("开关设置只能为 0 或 1".into()));
    }
    if settings.shortcut_key.trim().is_empty() { return Err(AppError::Config("快捷键不能为空".into())); }
    Ok(())
}
