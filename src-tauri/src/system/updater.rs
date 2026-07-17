use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::updater::UpdateInfo;

pub struct TauriUpdater {
    app_handle: AppHandle,
}

impl TauriUpdater {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub async fn check(&self) -> Result<UpdateInfo, String> {
        let updater = self.app_handle.updater().map_err(|error| {
            log::error!("获取更新器失败: {}", error);
            "无法获取更新器实例".to_string()
        })?;

        match updater.check().await {
            Ok(Some(update)) => {
                log::info!(
                    "版本检查完成 - 发现新版本，当前: {}, 最新: {}",
                    update.current_version,
                    update.version
                );

                Ok(UpdateInfo {
                    has_update: true,
                    current_version: update.current_version.clone(),
                    latest_version: update.version.clone(),
                    body: update.body.clone(),
                    size: None,
                    date: None,
                })
            }
            Ok(None) => {
                let current_version = self.app_handle.package_info().version.to_string();
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
            Err(error) => {
                log::error!("检查版本失败: {}", error);
                Err(format!("检查版本失败: {}", error))
            }
        }
    }

    pub async fn download_and_install(&self) -> Result<bool, String> {
        let updater = self.app_handle.updater().map_err(|error| {
            log::error!("获取更新器失败: {}", error);
            "无法获取更新器实例".to_string()
        })?;

        match updater.check().await {
            Ok(Some(update)) => {
                log::info!("开始下载更新: {}", update.version);

                let on_chunk = |chunk_len: usize, content_length: Option<u64>| {
                    if let Some(total) = content_length {
                        let percentage = ((chunk_len as f64 / total as f64) * 100.0) as u8;
                        log::debug!("更新下载进度: {}%", percentage);
                    }
                };

                let on_download_finish = || {
                    log::info!("更新下载完成，开始安装");
                };

                update
                    .download_and_install(on_chunk, on_download_finish)
                    .await
                    .map(|_| {
                        log::info!("更新下载并安装成功");
                        true
                    })
                    .map_err(|error| {
                        log::error!("下载或安装更新失败: {}", error);
                        format!("下载或安装更新失败: {}", error)
                    })
            }
            Ok(None) => {
                log::warn!("没有可用的更新");
                Err("没有可用的更新".to_string())
            }
            Err(error) => {
                log::error!("检查更新失败: {}", error);
                Err(format!("检查更新失败: {}", error))
            }
        }
    }
}
