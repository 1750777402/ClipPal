use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::CONTEXT;

/// 更新信息结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// 是否有新版本
    pub has_update: bool,
    /// 当前版本
    pub current_version: String,
    /// 最新版本
    pub latest_version: String,
    /// 更新日志/描述
    pub body: Option<String>,
    /// 更新包大小（字节）
    pub size: Option<u64>,
    /// 发布日期
    pub date: Option<String>,
}

/// 更新进度信息
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    /// 已下载字节数
    pub downloaded: u64,
    /// 总字节数
    pub total: u64,
    /// 进度百分比 (0-100)
    pub percentage: u8,
}

/// 检查软件版本更新
#[tauri::command]
pub async fn check_soft_version() -> Result<UpdateInfo, String> {
    let app_handle = CONTEXT.get::<AppHandle>();
    let updater_res = app_handle.updater();

    match updater_res {
        Ok(updater) => {
            let check_res = updater.check().await;
            match check_res {
                Ok(Some(update)) => {
                    // Tauri updater 返回 Some(update) 即表示有可用更新
                    // 不需要额外的版本比较，updater 已经做了语义版本比较
                    log::info!(
                        "版本检查完成 - 发现新版本: 当前: {}, 最新: {}",
                        update.current_version,
                        update.version
                    );

                    Ok(UpdateInfo {
                        has_update: true,
                        current_version: update.current_version.clone(),
                        latest_version: update.version.clone(),
                        body: update.body.clone(),
                        size: None, // Tauri 插件暂不提供
                        date: None,
                    })
                }
                Ok(None) => {
                    // 获取当前版本号
                    let current_version = app_handle.package_info().version.to_string();
                    log::info!("已是最新版本: {}", current_version);

                    Ok(UpdateInfo {
                        has_update: false,
                        current_version: current_version.clone(),
                        latest_version: current_version,
                        body: None,
                        size: None,
                        date: None,
                    })
                }
                Err(e) => {
                    log::error!("检查版本失败: {}", e);
                    Err(format!("检查版本失败: {}", e))
                }
            }
        }
        Err(e) => {
            log::error!("获取更新器失败: {}", e);
            Err("无法获取更新器实例".to_string())
        }
    }
}

/// 下载并安装更新
#[tauri::command]
pub async fn download_and_install_update() -> Result<bool, String> {
    let app_handle = CONTEXT.get::<AppHandle>();
    let updater_res = app_handle.updater();

    match updater_res {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => {
                log::info!("开始下载更新: {}", update.version);

                // 下载进度回调
                let on_chunk = |chunk_len: usize, content_length: Option<u64>| {
                    if let Some(total) = content_length {
                        let percentage = ((chunk_len as f64 / total as f64) * 100.0) as u8;
                        log::debug!("更新下载进度: {}%", percentage);
                    }
                };

                // 下载完成回调
                let on_download_finish = || {
                    log::info!("更新下载完成，开始安装");
                };

                // 使用 Tauri 的下载和安装方法
                match update
                    .download_and_install(on_chunk, on_download_finish)
                    .await
                {
                    Ok(_) => {
                        log::info!("更新下载并安装成功");
                        Ok(true)
                    }
                    Err(e) => {
                        log::error!("下载或安装更新失败: {}", e);
                        Err(format!("下载或安装更新失败: {}", e))
                    }
                }
            }
            Ok(None) => {
                log::warn!("没有可用的更新");
                Err("没有可用的更新".to_string())
            }
            Err(e) => {
                log::error!("检查更新失败: {}", e);
                Err(format!("检查更新失败: {}", e))
            }
        },
        Err(e) => {
            log::error!("获取更新器失败: {}", e);
            Err("无法获取更新器实例".to_string())
        }
    }
}
