use crate::{
    api::vip_api,
    biz::vip_checker::VipChecker,
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

#[tauri::command]
pub async fn get_vip_status() -> Result<Option<VipInfo>, String> {
    VipChecker::get_local_vip_info().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_vip_permission() -> Result<(bool, String), String> {
    VipChecker::check_cloud_sync_permission()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_vip_limits() -> Result<serde_json::Value, String> {
    // 只调用一次VIP状态检查，避免并发重复请求
    let is_vip = VipChecker::is_vip_user().await.map_err(|e| e.to_string())?;

    // 基于服务端缓存的VIP信息计算各项限制
    let (max_records, max_file_size, max_sync_records) =
        if let Ok(Some(vip_info)) = VipChecker::get_local_vip_info() {
            // 使用服务端返回的动态限制（服务端返回KB，转换为字节用于前端显示）
            (
                vip_info.max_records,
                vip_info.max_file_size * 1024, // 转换KB为字节
                vip_info.max_sync_records,
            )
        } else {
            // 没有VIP信息缓存时，默认为免费用户限制
            (500, 0, 10)
        };

    // 检查云同步权限，传入已知的VIP状态避免重复检查
    let can_cloud_sync = VipChecker::check_cloud_sync_permission_with_vip_status(Some(is_vip))
        .await
        .map_err(|e| e.to_string())?
        .0;

    Ok(serde_json::json!({
        "isVip": is_vip,
        "maxRecords": max_records,
        "maxFileSize": max_file_size,
        "maxSyncRecords": max_sync_records,
        "canCloudSync": can_cloud_sync
    }))
}

#[tauri::command]
pub async fn open_vip_purchase_page(app_handle: AppHandle) -> Result<(), String> {
    let url = "https://jingchuanyuexiang.com";

    // 使用Tauri2官方插件打开浏览器
    use tauri_plugin_opener::OpenerExt;

    app_handle
        .opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("打开浏览器失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn refresh_vip_status(app_handle: AppHandle) -> Result<bool, String> {
    // 调用VIP检查器的服务器刷新方法
    match VipChecker::refresh_vip_from_server().await {
        Ok(updated) => {
            if updated {
                // 发送VIP状态变更事件到前端
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
            log::error!("刷新VIP状态失败: {}", e);
            Err(e.to_string())
        }
    }
}

// 模拟VIP状态更新(用于测试)
#[tauri::command]
pub async fn simulate_vip_upgrade(
    app_handle: AppHandle,
    vip_type: VipType,
    days: u32,
) -> Result<(), String> {
    let expire_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + (days as u64 * 24 * 3600);

    let vip_info = VipInfo {
        vip_flag: true,
        vip_type: vip_type.clone(),
        expire_time: Some(expire_time),
        max_records: 1000,
        max_sync_records: 1000,
        max_file_size: 5 * 1024, // 5MB以KB为单位 (5120KB)
        features: Some(vec!["云同步".to_string(), "大文件上传".to_string()]),
    };

    use crate::utils::secure_store::SECURE_STORE;
    let mut store = SECURE_STORE
        .write()
        .map_err(|_| "获取存储锁失败".to_string())?;
    store
        .set_vip_info(vip_info.clone())
        .map_err(|e| e.to_string())?;
    store.update_vip_check_time().map_err(|e| e.to_string())?;
    drop(store);

    // 发送状态变更事件
    let payload = VipStatusChangedPayload {
        is_vip: true,
        vip_type: Some(vip_type),
        expire_time: Some(expire_time),
        max_records: 1000,
    };

    app_handle
        .emit("vip-status-changed", payload)
        .map_err(|e| format!("发送事件失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_server_config() -> Result<Option<std::collections::HashMap<VipType, crate::api::vip_api::ServerConfigResponse>>, String> {
    vip_api::get_server_config().await.map_err(|e| e.to_string())
}
