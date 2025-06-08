use std::sync::Arc;

use biz::clip_board_sync::ClipboardEventTigger;
use clipboard_listener::{ClipboardEvent, EventManager};
use state::TypeMap;
use tauri_plugin_autostart::MacosLauncher;

use crate::biz::{copy_clip_record::copy_clip_record, query_clip_record::get_clip_records};

mod biz;
mod clip_board_listener;
mod global_shortcut;
mod sqlite_storage;
mod tray;
mod utils;
mod window;

// 全局上下文存储
pub static CONTEXT: TypeMap![Send + Sync] = <TypeMap![Send + Sync]>::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    // 初始化粘贴板内容变化后的监听管理器
    let manager: Arc<EventManager<ClipboardEvent>> = Arc::new(EventManager::default());
    let m1 = manager.clone();
    // 注册粘贴板内容变化的监听器
    manager.add_event_listener(Arc::new(ClipboardEventTigger));
    // 开启监听器
    manager.start_event_loop();
    // 初始化sqlite链接
    sqlite_storage::init_sqlite().await;

    tauri::Builder::default()
        // 保存窗口位置和大小，并在应用程序重新打开时恢复它们
        .plugin(tauri_plugin_window_state::Builder::new().build())
        // 使用特定或者默认的应用程序打开文件或者 URL
        .plugin(tauri_plugin_opener::init())
        // 粘贴板插件  同时把事件管理器传入在粘贴板插件内部注册
        .plugin(tauri_plugin_clipboard_pal::init())
        // 开机自启插件
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--flag1", "--flag2"]),
        ))
        // http请求插件
        .plugin(tauri_plugin_http::init())
        .setup(move |app| {
            CONTEXT.set(app.handle().clone());
            // 创建托盘区图标
            tray::create_tray(app.handle())?;
            // 初始化主窗口
            let _ = window::init_main_window(&app);
            // 注册全局快捷键
            let _ = global_shortcut::init_global_shortcut(&app);
            // 开启devtools工具
            // app.app_handle()
            //     .get_webview_window("main")
            //     .unwrap()
            //     .open_devtools();
            // 初始化剪贴板监听器
            let _ = clip_board_listener::init_clip_board_listener(&app, m1);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_clip_records, copy_clip_record])
        .build(tauri::generate_context!())
        .unwrap()
        .run(move |_, event| match event {
            // 程序关闭事件处理
            tauri::RunEvent::ExitRequested { api: _, .. } => {
                // 1.关闭监听器
                let _ = manager.shutdown.0.send_blocking(());
            }
            _ => {}
        });
}
