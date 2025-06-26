use directories::ProjectDirs;
use log::error;
use std::fs;
use std::path::{Path, PathBuf};

fn ensure_directory(path: &Path) {
    if let Err(e) = fs::create_dir_all(path) {
        error!("创建目录失败: {}", e);
    }
}

/// 获取根目录 ClipPal，不包含 data/config/cache 自动子路径
fn get_clippal_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "ClipPal").map(|dirs| {
        let config_path = dirs.config_dir().to_path_buf(); // 如：C:\Users\<User>\AppData\Roaming\ClipPal\config
        let clippal_root = config_path.parent().unwrap().to_path_buf(); // ✅ 去掉 config 层
        ensure_directory(&clippal_root);
        clippal_root
    })
}

/// win: "C:\\Users\\<User>\\AppData\\Roaming\\ClipPal\\data"
/// mac: "/Users/<User>/Library/Application Support/ClipPal/data"
pub fn get_data_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("data");
        ensure_directory(&path);
        path
    })
}

/// win: "C:\\Users\\<User>\\AppData\\Roaming\\ClipPal\\resources"
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/resources"
pub fn get_resources_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("resources");
        ensure_directory(&path);
        path
    })
}

/// win: "C:\\Users\\<User>\\AppData\\Roaming\\ClipPal\\config"
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/config"
pub fn get_config_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("config");
        ensure_directory(&path);
        path
    })
}
