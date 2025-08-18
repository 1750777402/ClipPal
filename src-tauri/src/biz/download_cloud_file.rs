use std::path::PathBuf;

use clipboard_listener::ClipType;
use chrono::prelude::*;
use tauri::{AppHandle, Emitter};
use tokio::time::Duration;
use uuid::Uuid;

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
        &record.md5_str,
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
    md5_str: &str,
    file_type: &str,
    record_id: &str,
) -> AppResult<String> {
    // 确定保存路径
    let save_path = determine_save_path_simple(file_type, md5_str)?;

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

    // 返回相对路径
    if let Some(resources_dir) = get_resources_dir() {
        if let Ok(relative_path) = save_path.strip_prefix(&resources_dir) {
            Ok(relative_path.to_string_lossy().to_string())
        } else {
            Ok(save_path.to_string_lossy().to_string())
        }
    } else {
        Ok(save_path.to_string_lossy().to_string())
    }
}

fn determine_save_path_simple(file_type: &str, md5_str: &str) -> AppResult<PathBuf> {
    let resources_dir = get_resources_dir()
        .ok_or_else(|| AppError::Config("Failed to get resources directory".to_string()))?;

    match file_type {
        x if x == ClipType::Image.to_string() => {
            // 按照现有逻辑生成图片文件名：{timestamp}_{uuid}.png
            let now = Local::now().format("%Y%m%d%H%M%S").to_string();
            let uid = Uuid::new_v4().to_string();
            let filename = format!("{}_{}.png", now, uid);
            Ok(resources_dir.join(filename))
        }
        x if x == ClipType::File.to_string() => {
            // 混合存储方案：所有云下载的文件都保存到resources/files目录
            // 使用md5 + uuid确保文件名唯一性，避免冲突
            // 由于是从云端下载，我们无法获知原始文件扩展名，统一使用.dat
            let uid = Uuid::new_v4().simple();
            let filename = format!("{}_{}.dat", md5_str, uid);
            
            // 确保files子目录存在
            let files_dir = resources_dir.join("files");
            if !files_dir.exists() {
                std::fs::create_dir_all(&files_dir).map_err(|e| {
                    AppError::Io(e)
                })?;
            }
            
            Ok(files_dir.join(filename))
        }
        _ => Err(AppError::Config("Unsupported file type".to_string())),
    }
}
