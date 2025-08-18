use std::path::PathBuf;

use clipboard_listener::ClipType;
use tauri::{AppHandle, Emitter};
use tokio::time::Duration;

use crate::{
    CONTEXT,
    api::cloud_sync_api::{DownloadCloudFileParam, get_dowload_url},
    biz::clip_record::{ClipRecord, SYNCHRONIZING},
    errors::{AppError, AppResult},
    utils::{file_dir::get_resources_dir, http_client},
};
use rbatis::RBatis;

pub async fn start_cloud_file_download_timer(app_handle: AppHandle) {
    log::info!("Starting cloud file download timer");

    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval_timer.tick().await;

            if !crate::biz::system_setting::check_cloud_sync_enabled().await {
                continue;
            }

            if let Err(e) = scan_and_download_cloud_files(&app_handle).await {
                log::error!("Failed to scan and download cloud files: {}", e);
            }
        }
    });
}

async fn scan_and_download_cloud_files(app_handle: &AppHandle) -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    let pending_records = ClipRecord::select_by_sync_flag_limit(rb, SYNCHRONIZING, 1, 10)
        .await
        .map_err(|e| AppError::Database(e))?;

    if pending_records.is_empty() {
        return Ok(());
    }

    log::info!("Found {} pending cloud file records", pending_records.len());

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(3));

    let tasks: Vec<_> = pending_records
        .into_iter()
        .map(|record| {
            let app_handle = app_handle.clone();
            let semaphore = semaphore.clone();

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                download_cloud_file_for_record(app_handle, record).await
            })
        })
        .collect();

    for task in tasks {
        if let Err(e) = task.await {
            log::error!("Download task failed: {}", e);
        }
    }

    Ok(())
}

async fn download_cloud_file_for_record(
    app_handle: AppHandle,
    record: ClipRecord,
) -> AppResult<()> {
    if record.r#type != ClipType::Image.to_string() && record.r#type != ClipType::File.to_string() {
        return Ok(());
    }

    log::info!(
        "Starting cloud file download: record_id={}, type={}, md5={}",
        record.id,
        record.r#type,
        record.md5_str
    );

    let download_param = DownloadCloudFileParam {
        md5_str: record.md5_str.clone(),
        r#type: record.r#type.clone(),
    };

    let download_response = match get_dowload_url(&download_param).await {
        Ok(Some(response)) => response,
        Ok(None) => {
            log::error!("No download URL received for record_id: {}", record.id);
            return Err(AppError::ClipSync("No download URL received".to_string()));
        }
        Err(e) => {
            let error_msg = format!("Failed to get download URL: {}", e);
            log::error!(
                "Download URL error for record_id {}: {}",
                record.id,
                error_msg
            );
            return Err(AppError::ClipSync(error_msg));
        }
    };

    match download_cloud_file_to_local(
        &download_response.url,
        &download_response.file_name,
        &record.r#type,
        &record.id,
    )
    .await
    {
        Ok(local_path) => {
            let rb: &RBatis = CONTEXT.get::<RBatis>();
            if let Err(e) =
                ClipRecord::update_after_cloud_download(rb, &record.id, &local_path).await
            {
                log::warn!("Failed to update record status: {}", e);
            }

            // 通知前端刷新数据显示
            if let Err(e) = app_handle.emit("clip_record_change", ()) {
                log::warn!("Failed to notify frontend about download completion: {}", e);
            }

            log::info!(
                "Cloud file download completed: record_id={}, type={}, path={}",
                record.id,
                record.r#type,
                local_path
            );
        }
        Err(e) => {
            log::error!(
                "Cloud file download failed: record_id={}, type={}, error={}",
                record.id,
                record.r#type,
                e
            );
            return Err(e);
        }
    }

    Ok(())
}

async fn download_cloud_file_to_local(
    url: &str,
    cloud_file_name: &str,
    file_type: &str,
    record_id: &str,
) -> AppResult<String> {
    // 确定保存路径 - 使用云端返回的原始文件名
    let save_path = determine_save_path_from_cloud(file_type, cloud_file_name)?;

    log::debug!(
        "Downloading cloud file: record_id={}, url={}, save_path={:?}",
        record_id,
        url,
        save_path
    );

    // 使用http_client下载文件
    http_client::download_file(url, &save_path)
        .await
        .map_err(|e| AppError::Network(format!("File download failed: {}", e)))?;

    log::debug!(
        "Cloud file download completed: record_id={}, save_path={:?}",
        record_id,
        save_path
    );

    // 返回相对路径（与云端存储的格式保持一致）
    if let Some(resources_dir) = get_resources_dir() {
        if let Ok(relative_path) = save_path.strip_prefix(&resources_dir) {
            let relative_path_str = relative_path.to_string_lossy().to_string();
            // 标准化路径分隔符，确保跨平台一致性
            let normalized_path = relative_path_str.replace('\\', "/");
            Ok(normalized_path)
        } else {
            // 如果无法获取相对路径，返回云端原始文件名（兜底方案）
            log::warn!("无法计算相对路径，使用原始云端文件名: {}", cloud_file_name);
            Ok(cloud_file_name.to_string())
        }
    } else {
        // 如果无法获取resources目录，返回云端原始文件名（兜底方案）
        log::warn!(
            "无法获取resources目录，使用原始云端文件名: {}",
            cloud_file_name
        );
        Ok(cloud_file_name.to_string())
    }
}

fn determine_save_path_from_cloud(file_type: &str, cloud_file_name: &str) -> AppResult<PathBuf> {
    let resources_dir = get_resources_dir()
        .ok_or_else(|| AppError::Config("Failed to get resources directory".to_string()))?;

    match file_type {
        x if x == ClipType::Image.to_string() => {
            // 图片文件直接使用云端返回的文件名
            // 云端的文件名应该已经是相对于resources目录的路径
            Ok(resources_dir.join(cloud_file_name))
        }
        x if x == ClipType::File.to_string() => {
            // 文件类型使用云端返回的路径
            // 云端存储的应该是 "files/原始文件名" 的格式
            let file_path = if cloud_file_name.starts_with("files/") {
                // 如果云端已经包含files/前缀，直接使用
                cloud_file_name.to_string()
            } else {
                // 如果没有前缀，添加files/前缀（兼容性处理）
                format!("files/{}", cloud_file_name)
            };

            let full_path = resources_dir.join(&file_path);

            // 确保父目录存在
            if let Some(parent_dir) = full_path.parent() {
                if !parent_dir.exists() {
                    std::fs::create_dir_all(parent_dir).map_err(|e| {
                        log::error!("创建目录失败: {:?}, 错误: {}", parent_dir, e);
                        AppError::Io(e)
                    })?;
                }
            }

            Ok(full_path)
        }
        _ => Err(AppError::Config("Unsupported file type".to_string())),
    }
}
