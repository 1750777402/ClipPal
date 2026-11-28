use clip_core::{domain::settings::Settings, errors::{AppError, AppResult}};

/// 验证新备份目录可以使用。文件写入通过独立的临时文件检查。
pub fn ensure_backup_dir(directory: &str) -> AppResult<()> {
    let root = std::path::Path::new(directory).join("files");
    std::fs::create_dir_all(&root)?;
    if std::fs::symlink_metadata(&root)?.file_type().is_symlink() {
        return Err(AppError::Config("备份目录不能是符号链接".into()));
    }
    tempfile::NamedTempFile::new_in(root)
        .map_err(|_| AppError::Config("备份目录不可写".into()))?;
    Ok(())
}
/// 只校验本地设置值；系统快捷键解析、自动启动由桌面适配负责。
pub fn validate(settings: &Settings, max_records: u32) -> AppResult<()> {
    if settings.max_records < 50 { return Err(AppError::Config("记录条数不能少于 50".into())); }
    // 同时受版本策略和当前分页使用的整数范围约束。
    if settings.max_records > max_records.min(i32::MAX as u32) {
        return Err(AppError::Config(format!("记录条数不能超过 {}", max_records.min(i32::MAX as u32))));
    }
    // 设置文件和 IPC 沿用 0/1 开关，拒绝其他数值以免解释不一致。
    if [settings.auto_start, settings.auto_paste, settings.tutorial_completed].iter().any(|value| *value > 1) {
        return Err(AppError::Config("开关设置只能为 0 或 1".into()));
    }
    if settings.shortcut_key.trim().is_empty() { return Err(AppError::Config("快捷键不能为空".into())); }
    if settings.file_backup_dir.as_deref().is_some_and(|dir| !std::path::Path::new(dir).is_absolute()) {
        return Err(AppError::Config("备份目录必须为绝对路径".into()));
    }
    Ok(())
}
