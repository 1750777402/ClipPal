use std::sync::Arc;

use crate::{
    biz::{
        clip_record::ClipRecord,
        copy_clip_record::{
            copy_clip_record, copy_clip_record_no_paste, copy_single_file, del_record,
            image_save_as, set_pinned,
        },
        query_clip_record::get_clip_records,
        system_setting::{init_settings, load_settings, save_settings, validate_shortcut},
        simple_search_bin::{load_index_from_disk, rebuild_index_after_crash},
    },
    log_config::init_logging,
};

use biz::clip_board_sync::ClipboardEventTigger;
use clipboard_listener::{ClipboardEvent, EventManager};
use state::TypeMap;
use tauri_plugin_autostart::MacosLauncher;

mod auto_paste;
mod biz;
mod clip_board_listener;
mod errors;
mod global_shortcut;
mod log_config;
mod single_instance;
mod sqlite_storage;
mod tray;
mod utils;
mod window;

// 全局上下文存储
pub static CONTEXT: TypeMap![Send + Sync] = <TypeMap![Send + Sync]>::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    init_logging();

    // 初始化系统设置
    init_settings();

    // 初始化粘贴板内容变化后的监听管理器
    let manager: Arc<EventManager<ClipboardEvent>> = Arc::new(EventManager::default());
    let m1 = manager.clone();

    // 注册粘贴板内容变化的监听器
    manager.add_event_listener(Arc::new(ClipboardEventTigger));

    // 初始化sqlite链接
    let rb_res = match sqlite_storage::init_sqlite().await {
        Ok(rb) => rb,
        Err(e) => {
            log::error!("数据库初始化失败: {}", e);
            std::process::exit(1);
        }
    };

    // 初始化索引文件
    if let Err(e) = load_index_from_disk().await {
        log::error!("索引文件初始化失败: {}", e);
        // 不退出程序，继续运行但记录错误
    }

    // 如果有程序崩溃，则重建索引
    if let Err(e) = rebuild_index_after_crash(|| async {
        // 实现获取所有剪贴板内容的函数
        ClipRecord::select_order_by(&rb_res)
            .await
            .unwrap_or_else(|e| {
                log::error!("获取剪贴板记录失败: {}", e);
                vec![]
            })
    })
    .await
    {
        log::error!("重建索引失败: {}", e);
    }

    tauri::Builder::default()
        // 本机系统对话框，用于打开和保存文件，以及消息对话框
        .plugin(tauri_plugin_dialog::init())
        // 保存窗口位置和大小，并在应用程序重新打开时恢复它们
        .plugin(tauri_plugin_window_state::Builder::new().build())
        // 使用特定或者默认的应用程序打开文件或者 URL
        .plugin(tauri_plugin_opener::init())
        // 粘贴板插件  同时把事件管理器传入在粘贴板插件内部注册
        .plugin(tauri_plugin_clipboard_pal::init())
        // 开机自启插件
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
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
            // 使用单实例插件确保 Tauri 应用程序在同一时间只运行单个实例
            let _ = single_instance::init_single_instance(&app);
            // 开启devtools工具
            // app.app_handle()
            //     .get_webview_window("main")
            //     .unwrap()
            //     .open_devtools();
            // 初始化剪贴板监听器
            let _ = clip_board_listener::init_clip_board_listener(&app, m1);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_clip_records,
            copy_clip_record,
            copy_clip_record_no_paste,
            copy_single_file,
            load_settings,
            save_settings,
            validate_shortcut,
            set_pinned,
            del_record,
            image_save_as
        ])
        .build(tauri::generate_context!())
        .map_err(|e| {
            log::error!("Tauri应用构建失败: {}", e);
            std::process::exit(1);
        })
        .unwrap()
        .run(move |_, event| match event {
            // 程序关闭事件处理
            tauri::RunEvent::ExitRequested { api: _, .. } => {
                // 1.关闭监听器
                let _ = manager.shutdown.0.send_blocking(());
            }
            // 程序启动完成后续事件处理
            tauri::RunEvent::Ready { .. } => {
                // 开启粘贴板内容监听器
                manager.start_event_loop();
            }
            _ => {}
        });

    Ok(())
}
