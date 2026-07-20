use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::domain::update::UpdateInfo;

/// Tauri Updater 系统适配器，封装插件检查、下载和安装流程。
pub struct TauriUpdater {
    app_handle: AppHandle,
}

impl TauriUpdater {
    /// 使用已经完成 setup 的 AppHandle 创建更新适配器。
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// 检查更新并转换为稳定的领域模型；无更新时返回当前版本作为最新版本。
    pub async fn check(&self) -> Result<UpdateInfo, String> {
        // Updater 插件可能因配置或平台原因不可用，统一转换为用户可读错误。
        let updater = self.app_handle.updater().map_err(|error| {
            log::error!("获取更新器失败: {}", error);
            "无法获取更新器实例".to_string()
        })?;

        match updater.check().await {
            // Tauri 已完成语义版本比较，返回 Some 即表示存在可安装版本。
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

    /// 再次确认可用更新后下载并安装，返回安装流程是否成功完成。
    pub async fn download_and_install(&self) -> Result<bool, String> {
        let updater = self.app_handle.updater().map_err(|error| {
            log::error!("获取更新器失败: {}", error);
            "无法获取更新器实例".to_string()
        })?;

        match updater.check().await {
            Ok(Some(update)) => {
                log::info!("开始下载更新: {}", update.version);

                // 插件按数据块回调；当前仅记录可用的粗粒度进度日志。
                let on_chunk = |chunk_len: usize, content_length: Option<u64>| {
                    if let Some(total) = content_length {
                        let percentage = ((chunk_len as f64 / total as f64) * 100.0) as u8;
                        log::debug!("更新下载进度: {}%", percentage);
                    }
                };

                // 下载结束后插件将直接进入安装阶段。
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
