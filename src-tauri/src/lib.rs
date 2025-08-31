use std::sync::Arc;

use crate::{
    biz::{
        clip_async_queue::{AsyncQueue, consume_clip_record_queue},
        clip_record::ClipRecord,
        cloud_sync_timer::start_cloud_sync_timer,
        content_search::initialize_search_index,
        copy_clip_record::{
            copy_clip_record, copy_clip_record_no_paste, copy_single_file, del_record,
            image_save_as, set_pinned,
        },
        download_cloud_file::start_cloud_file_download_timer,
        query_clip_record::{get_clip_records, get_image_base64, get_full_text_content},
        system_setting::{init_settings, load_settings, save_settings, validate_shortcut},
        upload_cloud_timer::start_upload_cloud_timer,
        user_auth::{
            check_login_status, get_user_info, login, logout, send_email_code, user_register,
            validate_token, check_username,
        },
        vip_management::{
            get_vip_status, check_vip_permission, get_vip_limits, open_vip_purchase_page,
            refresh_vip_status, simulate_vip_upgrade,
        },
    },
    log_config::init_logging,
    utils::lock_utils::create_global_sync_lock,
};

use biz::clip_record_sync::ClipboardEventTigger;
use clipboard_listener::{ClipboardEvent, EventManager};
use log::LevelFilter;
use state::TypeMap;
use tauri_plugin_autostart::MacosLauncher;

mod api;
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
    init_logging(LevelFilter::Info);

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

    // 初始化搜索索引
    let all_clips = ClipRecord::select_order_by(&rb_res)
        .await
        .unwrap_or_else(|e| {
            log::error!("获取剪贴板记录失败: {}", e);
            vec![]
        });

    if let Err(e) = initialize_search_index(all_clips).await {
        log::error!("搜索索引初始化失败: {}", e);
    }

    // 为不同的地方克隆RBatis实例
    let rb_for_setup = rb_res.clone();
    let rb_for_run = rb_res.clone();

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

            // 初始化剪贴板监听器
            let _ = clip_board_listener::init_clip_board_listener(&app, m1);

            // 启动云同步定时任务
            let app_handle = app.handle().clone();
            let rb = rb_for_setup.clone();
            tokio::spawn(async move {
                start_cloud_sync_timer(app_handle, rb).await;
            });

            // 启动云文件下载定时任务
            let app_handle_download = app.handle().clone();
            tokio::spawn(async move {
                start_cloud_file_download_timer(app_handle_download).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_clip_records,
            get_image_base64,
            get_full_text_content,
            copy_clip_record,
            copy_clip_record_no_paste,
            copy_single_file,
            load_settings,
            save_settings,
            validate_shortcut,
            set_pinned,
            del_record,
            image_save_as,
            login,
            user_register,
            send_email_code,
            logout,
            validate_token,
            get_user_info,
            check_login_status,
            check_username,
            // VIP相关命令
            get_vip_status,
            check_vip_permission,
            get_vip_limits,
            open_vip_purchase_page,
            refresh_vip_status,
            simulate_vip_upgrade
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("应用构建失败: {}", e);
            std::process::exit(1);
        })
        .run(move |_, event| match event {
            // 程序关闭事件处理
            tauri::RunEvent::ExitRequested { api: _, .. } => {
                // 1.关闭监听器
                let _ = manager.shutdown.0.send_blocking(());
            }
            // 程序启动完成后续事件处理
            tauri::RunEvent::Ready { .. } => {
                // 创建全局同步锁
                let sync_lock = create_global_sync_lock();
                CONTEXT.set(sync_lock.clone());

                // 创建一个内存队列  用来处理粘贴板记录的同步操作记录
                let queue: AsyncQueue<ClipRecord> = AsyncQueue::new(1000);
                CONTEXT.set(queue.clone());
                // 启动队列消费
                consume_clip_record_queue(queue);

                // 启动文件同步定时任务
                start_upload_cloud_timer();

                // 开启粘贴板内容监听器
                manager.start_event_loop();

                // 初始化VIP状态并执行权益限制检查
                let rb_for_vip = rb_for_run.clone();
                tokio::spawn(async move {
                    CONTEXT.set(rb_for_vip);
                    if let Err(e) = crate::biz::vip_checker::VipChecker::initialize_vip_and_enforce_limits().await {
                        log::error!("VIP状态初始化失败: {}", e);
                    }
                });
            }
            _ => {}
        });

    Ok(())
}
