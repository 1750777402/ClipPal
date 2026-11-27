use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

fn ensure_directory(path: &Path) {
    if let Err(e) = fs::create_dir_all(path) {
        log::error!("创建目录失败: {}", e);
    }
}

/// 获取 ClipPal 的运行数据根目录，供数据库、资源、配置和日志目录共用。
fn get_clippal_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "ClipPal").map(|dirs| {
        // data_local_dir 已包含应用名称，Windows 上还包含一层 data。
        // Windows: %LOCALAPPDATA%\ClipPal\data
        // macOS: /Users/<User>/Library/Application Support/ClipPal
        let clippal_root = dirs.data_local_dir().to_path_buf();
        ensure_directory(&clippal_root);
        clippal_root
    })
}

/// win: `%LOCALAPPDATA%\ClipPal\data\data`
/// mac: "/Users/<User>/Library/Application Support/ClipPal/data"
pub fn get_data_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("data");
        ensure_directory(&path);
        path
    })
}

/// win: `%LOCALAPPDATA%\ClipPal\data\resources`
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/resources"
pub fn get_resources_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("resources");
        ensure_directory(&path);
        path
    })
}

/// win: `%LOCALAPPDATA%\ClipPal\data\config`
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/config"
pub fn get_config_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("config");
        ensure_directory(&path);
        path
    })
}

/// win: `%LOCALAPPDATA%\ClipPal\data\logs`
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/logs"
pub fn get_logs_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("logs");
        ensure_directory(&path);
        path
    })
}
