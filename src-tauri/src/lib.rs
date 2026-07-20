use crate::commands::{
    check_login_status, check_soft_version, check_username, check_vip_permission, copy_clip_record,
    copy_clip_record_no_paste, copy_single_file, del_record, download_and_install_update,
    get_clip_records, get_full_text_content, get_image_info_batch, get_image_path, get_pay_result,
    get_pay_url, get_server_config, get_user_info, get_vip_limits, get_vip_status, image_save_as,
    load_settings, login, logout, open_vip_purchase_page, refresh_vip_status, save_settings,
    send_email_code, set_pinned, update_user_info, user_register, validate_shortcut,
    validate_token,
};

mod api;
mod app_context;
mod auto_paste;
mod biz;
mod bootstrap;
mod clip_board_listener;
mod commands;
pub mod domain;
mod dto;
mod errors;
mod global_shortcut;
mod infra;
mod log_config;
mod menu;
mod response;
mod services;
mod sqlite_storage;
mod system;
mod tray;
mod utils;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 组装并运行 ClipPal Tauri 应用。
///
/// 该入口只负责初始化核心依赖、注册插件和 command、连接生命周期事件，
/// 具体业务由 service、domain、repository 和 system 模块承担。
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 在构建 Tauri 应用前完成数据库、仓储、搜索引擎和共享上下文初始化。
    let core = bootstrap::init_core().await?;
    let app_context = core.app_context.clone();
    let core_for_setup = core.clone();
    let core_for_run = core.clone();

    // AppContext 作为 Tauri managed state 注入，所有 command 从同一上下文获取依赖。
    bootstrap::register_plugins(tauri::Builder::default())
        .manage(app_context)
        .setup(move |app| Ok(bootstrap::setup_app(app, core_for_setup.clone())?))
        // command 注册表是前端调用后端的唯一 IPC 边界。
        .invoke_handler(tauri::generate_handler![
            get_clip_records,
            get_image_path,
            get_image_info_batch,
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
            update_user_info,
            // VIP相关命令
            get_vip_status,
            check_vip_permission,
            get_vip_limits,
            open_vip_purchase_page,
            refresh_vip_status,
            get_server_config,
            get_pay_url,
            get_pay_result,
            // 检查版本和更新
            check_soft_version,
            download_and_install_update,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("应用构建失败: {}", e);
            std::process::exit(1);
        })
        // 将 Tauri RunEvent 交给 bootstrap 统一处理 Ready 和退出清理。
        .run(move |_, event| {
            bootstrap::handle_run_event(event, core_for_run.clone());
        });

    Ok(())
}
