use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// 应用启动时检查一次更新
pub async fn check_update_on_startup(app_handle: AppHandle) {
    log::info!("执行一次更新检查");
    if let Err(e) = check_and_notify_update(&app_handle).await {
        log::error!("启动时更新检查失败: {}", e);
    }
}

/// 检查更新并通知前端
async fn check_and_notify_update(app_handle: &AppHandle) -> Result<(), String> {
    let updater_res = app_handle.updater();

    match updater_res {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => {
                    // Tauri updater 返回 Some(update) 即表示有可用更新
                    // 不需要额外的版本比较，updater 已经做了语义版本比较
                    log::info!(
                        "发现新版本: {} -> {}",
                        update.current_version,
                        update.version
                    );

                    // 发送事件到前端，通知有新版本
                    let _ = app_handle.emit(
                        "update-available",
                        serde_json::json!({
                            "current_version": update.current_version,
                            "latest_version": update.version,
                            "body": update.body,
                        }),
                    );

                    Ok(())
                }
                Ok(None) => {
                    log::debug!("没有更新信息");
                    Ok(())
                }
                Err(e) => {
                    log::error!("检查更新失败: {}", e);
                    Err(format!("检查更新失败: {}", e))
                }
            }
        }
        Err(e) => {
            log::error!("获取更新器失败: {}", e);
            Err("无法获取更新器实例".to_string())
        }
    }
}
