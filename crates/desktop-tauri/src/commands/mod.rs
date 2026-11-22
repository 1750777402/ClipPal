#[cfg(feature = "commercial-extension")]
pub mod auth_commands;
pub mod clip_commands;
pub mod clipboard_commands;
pub mod settings_commands;
pub mod update_commands;
#[cfg(feature = "commercial-extension")]
pub mod vip_commands;

pub mod capability_commands;
#[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
mod sync_commands;

/// command 注册表是前端调用后端的唯一 IPC 边界。
/// VIP相关命令、账号与支付仅在商业扩展启用时注册；检查版本和更新保持公共入口。
#[cfg(feature = "commercial-extension")]
pub fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        capability_commands::get_app_capabilities,
        clip_commands::get_clip_records,
        clip_commands::get_image_path,
        clip_commands::get_image_info_batch,
        clip_commands::get_full_text_content,
        clipboard_commands::copy_clip_record,
        clipboard_commands::copy_clip_record_no_paste,
        clipboard_commands::copy_single_file,
        settings_commands::load_settings,
        settings_commands::save_settings,
        settings_commands::validate_shortcut,
        settings_commands::get_auto_paste_permission,
        settings_commands::request_auto_paste_permission,
        settings_commands::open_accessibility_settings,
        clip_commands::set_pinned,
        clip_commands::del_record,
        clipboard_commands::image_save_as,
        update_commands::check_soft_version,
        update_commands::download_and_install_update,
        auth_commands::login,
        auth_commands::user_register,
        auth_commands::send_email_code,
        auth_commands::logout,
        auth_commands::validate_token,
        auth_commands::get_user_info,
        auth_commands::check_login_status,
        auth_commands::check_username,
        auth_commands::update_user_info,
        vip_commands::get_vip_status,
        vip_commands::check_vip_permission,
        vip_commands::get_vip_limits,
        vip_commands::open_vip_purchase_page,
        vip_commands::refresh_vip_status,
        vip_commands::get_server_config,
        vip_commands::get_pay_url,
        vip_commands::get_pay_result,
    ])
}

// 由外层 cfg 选择命令，避免单机包暴露跨端删除入口。
#[cfg(not(feature = "commercial-extension"))]
macro_rules! register_standard_commands {
    ($builder:ident $(, $command:path)*) => {
    $builder.invoke_handler(tauri::generate_handler![
        capability_commands::get_app_capabilities,
        clip_commands::get_clip_records,
        clip_commands::get_image_path,
        clip_commands::get_image_info_batch,
        clip_commands::get_full_text_content,
        clipboard_commands::copy_clip_record,
        clipboard_commands::copy_clip_record_no_paste,
        clipboard_commands::copy_single_file,
        settings_commands::load_settings,
        settings_commands::save_settings,
        settings_commands::validate_shortcut,
        settings_commands::get_auto_paste_permission,
        settings_commands::request_auto_paste_permission,
        settings_commands::open_accessibility_settings,
        clip_commands::set_pinned,
        clip_commands::del_record,
        clipboard_commands::image_save_as,
        update_commands::check_soft_version,
        update_commands::download_and_install_update,
        $($command),*
    ])
    };
}

#[cfg(not(feature = "commercial-extension"))]
pub fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    #[cfg(feature = "sync")]
    return register_standard_commands!(builder, sync_commands::get_sync_status, sync_commands::delete_synced_record);
    #[cfg(not(feature = "sync"))]
    register_standard_commands!(builder)
}

#[cfg(all(test, not(feature = "sync")))]
mod tests;
