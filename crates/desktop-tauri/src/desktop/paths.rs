use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};

fn ensure_directory(path: &Path) {
    if let Err(e) = fs::create_dir_all(path) {
        log::error!("创建目录失败: {}", e);
    }
}

/// 获取根目录 ClipPal，不包含 data/config/cache 自动子路径
fn get_clippal_root() -> Option<PathBuf> {
    // 仅调试构建允许隔离运行目录，不触碰已安装版本的正式数据。
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("CLIPPAL_DATA_DIR") {
        let root = PathBuf::from(root);
        if !root.is_absolute() { log::error!("调试数据目录必须为绝对路径"); return None; }
        ensure_directory(&root);
        return Some(root);
    }
    ProjectDirs::from("", "", "ClipPal").map(|dirs| {
        // 直接使用 data_local_dir，它已经包含了应用名称
        // Windows: C:\Users\<User>\AppData\Roaming\ClipPal
        // macOS: /Users/<User>/Library/Application Support/ClipPal
        let clippal_root = dirs.data_local_dir().to_path_buf();
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

/// win: "C:\\Users\\<User>\\AppData\\Roaming\\ClipPal\\logs"
/// mac:  "/Users/<User>/Library/Application Support/ClipPal/logs"
pub fn get_logs_dir() -> Option<PathBuf> {
    get_clippal_root().map(|mut path| {
        path.push("logs");
        ensure_directory(&path);
        path
    })
}
