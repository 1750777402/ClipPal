use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use crate::domain::update::{UpdateInfo, UpdateProgress};

const UPDATE_DOWNLOAD_PROGRESS_EVENT: &str = "update-download-progress";
const UPDATE_INSTALLING_EVENT: &str = "update-installing";
const RESTART_DELAY: Duration = Duration::from_secs(1);

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
                    date: update.date.map(|date| date.to_string()),
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

    /// 再次确认可用更新，下载并验证签名后执行安装。
    ///
    /// 下载过程中向前端发送累计字节进度，签名验证通过后发送安装阶段事件。
    /// Windows 安装器会自动退出并重新启动应用；其他桌面平台安装成功后主动重启。
    pub async fn download_and_install(&self) -> Result<bool, String> {
        let updater = self.app_handle.updater().map_err(|error| {
            log::error!("获取更新器失败: {}", error);
            "无法获取更新器实例".to_string()
        })?;

        match updater.check().await {
            Ok(Some(update)) => {
                log::info!("开始下载更新: {}", update.version);

                let downloaded_bytes = Arc::new(AtomicU64::new(0));
                let total_bytes = Arc::new(AtomicU64::new(0));

                let progress_downloaded = downloaded_bytes.clone();
                let progress_total = total_bytes.clone();
                let progress_app_handle = self.app_handle.clone();
                let on_chunk = move |chunk_len: usize, content_length: Option<u64>| {
                    if let Some(content_length) = content_length {
                        progress_total.store(content_length, Ordering::Relaxed);
                    }

                    let downloaded = progress_downloaded
                        .fetch_add(chunk_len as u64, Ordering::Relaxed)
                        .saturating_add(chunk_len as u64);
                    let total = progress_total.load(Ordering::Relaxed);
                    emit_download_progress(&progress_app_handle, downloaded, total);
                };

                let finished_downloaded = downloaded_bytes.clone();
                let finished_total = total_bytes.clone();
                let finished_app_handle = self.app_handle.clone();
                let on_download_finish = move || {
                    let downloaded = finished_downloaded.load(Ordering::Relaxed);
                    let announced_total = finished_total.load(Ordering::Relaxed);
                    let total = if announced_total == 0 {
                        downloaded
                    } else {
                        announced_total
                    };
                    emit_download_progress(&finished_app_handle, downloaded, total);
                    log::info!("更新包下载完成，等待签名验证");
                };

                // 分开下载和安装，确保下载包完成签名验证后再通知前端进入安装阶段。
                let update_bytes = update
                    .download(on_chunk, on_download_finish)
                    .await
                    .map_err(|error| {
                        log::error!("更新下载或签名验证失败: {}", error);
                        format!("更新下载或签名验证失败: {}", error)
                    })?;

                if let Err(error) = self.app_handle.emit(UPDATE_INSTALLING_EVENT, ()) {
                    log::warn!("发送更新安装阶段事件失败: {}", error);
                }

                update.install(update_bytes).map_err(|error| {
                    log::error!("安装更新失败: {}", error);
                    format!("安装更新失败: {}", error)
                })?;

                log::info!("更新安装成功，准备重启应用");
                schedule_restart(self.app_handle.clone());
                Ok(true)
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

/// 向前端发送累计下载进度；服务端未提供总大小时，百分比保持为 0。
fn emit_download_progress(app_handle: &AppHandle, downloaded: u64, total: u64) {
    let progress = UpdateProgress {
        downloaded,
        total,
        percentage: calculate_percentage(downloaded, total),
    };

    if let Err(error) = app_handle.emit(UPDATE_DOWNLOAD_PROGRESS_EVENT, progress) {
        log::warn!("发送更新下载进度失败: {}", error);
    }
}

/// 根据累计下载字节计算 0 到 100 的稳定百分比。
fn calculate_percentage(downloaded: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }

    downloaded
        .saturating_mul(100)
        .saturating_div(total)
        .min(100) as u8
}

/// 延迟重启应用，让前端有时间展示安装成功状态。
fn schedule_restart(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(RESTART_DELAY).await;
        app_handle.restart();
    });
}

#[cfg(test)]
mod tests {
    use super::calculate_percentage;

    #[test]
    fn calculates_cumulative_download_percentage() {
        assert_eq!(calculate_percentage(25, 100), 25);
        assert_eq!(calculate_percentage(100, 100), 100);
    }

    #[test]
    fn handles_unknown_or_exceeded_content_length() {
        assert_eq!(calculate_percentage(10, 0), 0);
        assert_eq!(calculate_percentage(120, 100), 100);
    }
}
