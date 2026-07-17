use crate::{
    api::vip_api,
    biz::vip_checker::VipChecker,
    response::{string_result, CommandResponse},
    utils::secure_store::{VipInfo, VipType},
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct VipStatusChangedPayload {
    is_vip: bool,
    vip_type: Option<VipType>,
    expire_time: Option<u64>,
    max_records: u32,
}

#[allow(dead_code)]
pub async fn get_vip_status() -> Result<CommandResponse<Option<VipInfo>>, String> {
    Ok(string_result(
        VipChecker::get_local_vip_info().map_err(|e| e.to_string()),
    ))
}

#[allow(dead_code)]
pub async fn check_vip_permission() -> Result<CommandResponse<(bool, String)>, String> {
    Ok(string_result(
        VipChecker::check_cloud_sync_permission()
            .await
            .map_err(|e| e.to_string()),
    ))
}

#[allow(dead_code)]
pub async fn get_vip_limits() -> Result<CommandResponse<serde_json::Value>, String> {
    Ok(string_result(
        async {
            let is_vip = VipChecker::is_vip_user().await.map_err(|e| e.to_string())?;

            let (max_records, max_file_size) =
                if let Ok(Some(vip_info)) = VipChecker::get_local_vip_info() {
                    (vip_info.max_records, vip_info.max_file_size * 1024)
                } else {
                    (300, 0)
                };

            let can_cloud_sync =
                VipChecker::check_cloud_sync_permission_with_vip_status(Some(is_vip))
                    .await
                    .map_err(|e| e.to_string())?
                    .0;

            Ok(serde_json::json!({
                "isVip": is_vip,
                "maxRecords": max_records,
                "maxFileSize": max_file_size,
                "canCloudSync": can_cloud_sync
            }))
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn open_vip_purchase_page(app_handle: AppHandle) -> Result<CommandResponse<()>, String> {
    Ok(string_result({
        let url = "https://jingchuanyuexiang.com";

        use tauri_plugin_opener::OpenerExt;

        app_handle
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| format!("打开浏览器失败: {}", e))
    }))
}

#[allow(dead_code)]
pub async fn refresh_vip_status(app_handle: AppHandle) -> Result<CommandResponse<bool>, String> {
    Ok(string_result(
        async {
            match VipChecker::refresh_vip_from_server().await {
                Ok(updated) => {
                    if updated {
                        if let Ok(vip_info) = VipChecker::get_local_vip_info() {
                            if let Some(info) = vip_info {
                                let payload = VipStatusChangedPayload {
                                    is_vip: info.vip_flag,
                                    vip_type: Some(info.vip_type),
                                    expire_time: info.expire_time,
                                    max_records: info.max_records,
                                };

                                let _ = app_handle.emit("vip-status-changed", payload);
                            }
                        }
                    }
                    Ok(updated)
                }
                Err(e) => {
                    log::error!("刷新 VIP 状态失败: {}", e);
                    Err(e.to_string())
                }
            }
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn get_server_config() -> Result<
    CommandResponse<
        Option<std::collections::HashMap<VipType, crate::api::vip_api::ServerConfigResponse>>,
    >,
    String,
> {
    Ok(string_result(
        vip_api::get_server_config()
            .await
            .map_err(|e| e.to_string()),
    ))
}

#[allow(dead_code)]
pub async fn get_pay_url(
    param: vip_api::PayParam,
) -> Result<CommandResponse<Option<vip_api::PayCodrUrlResponse>>, String> {
    Ok(string_result(
        vip_api::get_pay_url(&param)
            .await
            .map_err(|e| e.to_string()),
    ))
}

#[allow(dead_code)]
pub async fn get_pay_result(
    param: vip_api::QueryPayParam,
) -> Result<CommandResponse<Option<vip_api::QueryPayResponse>>, String> {
    Ok(string_result(
        vip_api::get_pay_result(&param)
            .await
            .map_err(|e| e.to_string()),
    ))
}
