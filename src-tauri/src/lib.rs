use std::sync::Arc;

use clipboard_listener::{ClipboardEvent, EventManager};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

mod biz;
mod clip_board;
mod tray;
mod window;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化粘贴板内容变化后的监听管理器
    let manager: Arc<EventManager<ClipboardEvent>> = Arc::new(EventManager::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 粘贴板插件  同时把事件管理器传入在粘贴板插件内部注册
        .plugin(tauri_plugin_clipboard_pal::init(manager.clone()))
        // 开机自启插件
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--flag1", "--flag2"]),
        ))
        // http请求插件
        .plugin(tauri_plugin_http::init())
        // 全局快捷键设置插件
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // sql功能插件，比如使用SQLite等
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            // 把事件管理器放入上下文manager  便于后续使用
            app.handle().manage(manager.clone());
            // 创建托盘区图标
            tray::create_tray(app.handle())?;
            // 初始化主窗口
            let _ = window::init_main_window(&app);
            // 初始化剪贴板监听器
            let _ = clip_board::init_clip_board_listener(&app, manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error running tauri");
}
