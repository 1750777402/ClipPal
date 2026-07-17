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
mod updater;
mod utils;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let core = bootstrap::init_core().await?;
    let app_context = core.app_context.clone();
    let core_for_setup = core.clone();
    let core_for_run = core.clone();

    bootstrap::register_plugins(tauri::Builder::default())
        .manage(app_context)
        .setup(move |app| Ok(bootstrap::setup_app(app, core_for_setup.clone())?))
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
        .run(move |_, event| {
            bootstrap::handle_run_event(event, core_for_run.clone());
        });

    Ok(())
}
