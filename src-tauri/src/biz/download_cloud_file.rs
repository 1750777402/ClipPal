use std::path::PathBuf;

use chrono::Local;
use clipboard_listener::ClipType;
use tauri::{AppHandle, Emitter};
use tokio::time::Duration;
use uuid::Uuid;

use crate::{
    CONTEXT,
    api::cloud_sync_api::{DownloadCloudFileParam, get_dowload_url},
    biz::clip_record::{ClipRecord, SYNCHRONIZING},
    errors::{AppError, AppResult},
    utils::{
        file_dir::get_resources_dir, file_ext::extract_full_extension_from_str, http_client,
        token_manager::has_valid_auth,
    },
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

            // 检查用户登录状态
            if !has_valid_auth() {
                log::debug!("用户未登录或认证已过期，跳过云文件下载任务");
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
        Ok((filename, absolute_path)) => {
            let rb: &RBatis = CONTEXT.get::<RBatis>();
            if let Err(e) =
                ClipRecord::update_after_cloud_download(rb, &record.id, &filename, &absolute_path)
                    .await
            {
                log::warn!("Failed to update record status: {}", e);
            }

            // 通知前端刷新数据显示
            if let Err(e) = app_handle.emit("clip_record_change", ()) {
                log::warn!("Failed to notify frontend about download completion: {}", e);
            }

            log::info!(
                "Cloud file download completed: record_id={}, type={}, filename={}, path={}",
                record.id,
                record.r#type,
                filename,
                absolute_path
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
) -> AppResult<(String, String)> {
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

    // content字段使用云端返回的原始文件名（用户看到的显示名称）
    let display_filename = cloud_file_name.to_string();

    // local_file_path使用下载后的实际绝对路径（程序访问用的路径）
    let absolute_path = save_path.to_string_lossy().to_string();

    log::info!(
        "Cloud file processed: record_id={}, display_filename={}, local_path={}",
        record_id,
        display_filename,
        absolute_path
    );

    Ok((display_filename, absolute_path))
}

fn determine_save_path_from_cloud(file_type: &str, cloud_file_name: &str) -> AppResult<PathBuf> {
    let resources_dir = get_resources_dir()
        .ok_or_else(|| AppError::Config("Failed to get resources directory".to_string()))?;

    match file_type {
        x if x == ClipType::Image.to_string() => Ok(resources_dir.join(cloud_file_name)),
        x if x == ClipType::File.to_string() => {
            // 确保files目录存在
            let files_dir = resources_dir.join("files");
            if !files_dir.exists() {
                std::fs::create_dir_all(&files_dir).map_err(|e| {
                    log::error!("创建files目录失败: {:?}, 错误: {}", files_dir, e);
                    AppError::Io(e)
                })?;
            }

            // 提取原文件的扩展名，支持复合扩展名（如 tar.gz, tar.bz2 等）
            let extension = extract_full_extension_from_str(cloud_file_name);

            // 生成新的唯一文件名，保留原扩展名
            let now = Local::now().format("%Y%m%d%H%M%S").to_string();
            let uid = Uuid::new_v4().to_string();
            let new_filename = if extension.is_empty() {
                format!("{}_{}", now, uid)
            } else {
                format!("{}_{}.{}", now, uid, extension)
            };

            Ok(files_dir.join(new_filename))
        }
        _ => Err(AppError::Config("Unsupported file type".to_string())),
    }
}
