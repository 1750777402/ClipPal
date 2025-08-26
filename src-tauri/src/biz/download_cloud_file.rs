use std::path::PathBuf;

use chrono::Local;
use clipboard_listener::ClipType;
use tauri::{AppHandle, Emitter};
use tokio::time::Duration;
use uuid::Uuid;

use crate::{
    CONTEXT,
    api::cloud_sync_api::{DownloadCloudFileParam, get_dowload_url},
    biz::clip_record::{ClipRecord, SYNCHRONIZING, SKIP_SYNC},
    errors::{AppError, AppResult},
    utils::{
        file_dir::get_resources_dir, file_ext::extract_full_extension_from_str, http_client,
        retry_helper::{retry_with_config, RetryConfig},
        token_manager::has_valid_auth,
    },
};
use rbatis::RBatis;

/// 判断下载错误是否应该重试
fn should_retry_download_error(error: &AppError) -> bool {
    match error {
        // 网络相关错误可以重试
        AppError::Network(msg) => {
            let msg_lower = msg.to_lowercase();
            // 排除明确不应该重试的情况
            !(msg_lower.contains("404") || msg_lower.contains("not found") || 
              msg_lower.contains("403") || msg_lower.contains("forbidden") ||
              msg_lower.contains("401") || msg_lower.contains("unauthorized"))
        },
        // HTTP客户端错误
        AppError::Http(_) => true,
        // 通用错误中的网络问题可以重试
        AppError::General(msg) => {
            let msg_lower = msg.to_lowercase();
            (msg_lower.contains("网络") || 
             msg_lower.contains("timeout") || 
             msg_lower.contains("connection") ||
             msg_lower.contains("下载失败")) &&
            // 排除不应重试的情况
            !(msg_lower.contains("404") || msg_lower.contains("not found") ||
              msg_lower.contains("403") || msg_lower.contains("forbidden"))
        },
        // IO错误中的网络相关问题可以重试
        AppError::Io(io_err) => {
            matches!(io_err.kind(), 
                std::io::ErrorKind::TimedOut | 
                std::io::ErrorKind::Interrupted |
                std::io::ErrorKind::ConnectionRefused |
                std::io::ErrorKind::ConnectionAborted |
                std::io::ErrorKind::UnexpectedEof
            )
        },
        // 云同步错误 - 部分可以重试
        AppError::ClipSync(msg) => {
            let msg_lower = msg.to_lowercase();
            msg_lower.contains("网络") || msg_lower.contains("timeout") || msg_lower.contains("connection")
        },
        // 其他错误类型不重试
        _ => false,
    }
}

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
        "Starting cloud file download with retry: record_id={}, type={}, md5={}",
        record.id,
        record.r#type,
        record.md5_str
    );

    // 配置下载重试策略 - 下载任务相对轻量，可以更频繁重试
    let retry_config = RetryConfig::new(3, 3000)  // 最多重试3次，初始延迟3秒
        .with_backoff_multiplier(1.5)             // 较温和的退避策略，避免对服务器压力过大
        .with_max_delay(30000)                    // 最大延迟30秒
        .with_jitter(true);                       // 启用抖动

    // 使用重试机制执行下载
    let result = retry_with_config(
        retry_config,
        || {
            let record_clone = record.clone();
            let app_handle_clone = app_handle.clone();
            async move {
                download_cloud_file_core(app_handle_clone, record_clone).await
            }
        },
        should_retry_download_error,
    ).await;

    match result {
        Ok(_) => {
            log::info!("Cloud file download ultimately succeeded: record_id={}", record.id);
            Ok(())
        }
        Err(e) => {
            log::error!(
                "Cloud file download ultimately failed: record_id={}, error={}",
                record.id,
                e
            );
            
            // 下载失败达到最大重试次数后，标记为跳过同步，避免一直重试
            if let Err(mark_err) = mark_download_as_skip_sync(&record.id, &format!("下载失败: {}", e)).await {
                log::warn!("Failed to mark record as skip sync: {}", mark_err);
            }
            
            Err(e)
        }
    }
}

/// 核心下载逻辑（被重试机制调用）
async fn download_cloud_file_core(
    app_handle: AppHandle,
    record: ClipRecord,
) -> AppResult<()> {
    let download_param = DownloadCloudFileParam {
        md5_str: record.md5_str.clone(),
        r#type: record.r#type.clone(),
    };

    // 获取下载URL
    let download_response = match get_dowload_url(&download_param).await {
        Ok(Some(response)) => response,
        Ok(None) => {
            log::warn!("No download URL received for record_id: {}", record.id);
            return Err(AppError::ClipSync("No download URL received".to_string()));
        }
        Err(e) => {
            let error_msg = format!("Failed to get download URL: {}", e);
            log::warn!("Download URL error for record_id {}: {}", record.id, error_msg);
            return Err(AppError::ClipSync(error_msg));
        }
    };

    // 下载文件到本地
    let (filename, absolute_path) = download_cloud_file_to_local(
        &download_response.url,
        &download_response.file_name,
        &record.r#type,
        &record.id,
    ).await?;

    // 更新数据库记录
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    ClipRecord::update_after_cloud_download(rb, &record.id, &filename, &absolute_path)
        .await?;

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

    Ok(())
}

/// 标记下载记录为跳过同步状态
async fn mark_download_as_skip_sync(record_id: &str, reason: &str) -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let ids = vec![record_id.to_string()];
    let current_time = current_timestamp();

    ClipRecord::update_sync_flag(rb, &ids, SKIP_SYNC, current_time).await?;
    
    log::info!(
        "标记下载记录为跳过同步，记录ID: {}, 原因: {}",
        record_id,
        reason
    );

    Ok(())
}

/// 获取当前时间戳
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_else(|e| {
            log::warn!("获取系统时间失败，使用默认值: {}", e);
            0
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_retry_download_error() {
        // 网络错误应该重试
        assert!(should_retry_download_error(&AppError::Network("Connection timeout".to_string())));
        assert!(should_retry_download_error(&AppError::Http("HTTP error".to_string())));
        assert!(should_retry_download_error(&AppError::General("网络超时".to_string())));
        
        // IO错误中的网络相关问题应该重试
        assert!(should_retry_download_error(&AppError::Io(
            std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")
        )));
        
        // 404, 403等错误不应该重试
        assert!(!should_retry_download_error(&AppError::Network("404 not found".to_string())));
        assert!(!should_retry_download_error(&AppError::Network("403 forbidden".to_string())));
        assert!(!should_retry_download_error(&AppError::General("404 file not found".to_string())));
        
        // 数据库错误不应该重试
        assert!(!should_retry_download_error(&AppError::Database(rbatis::Error::from("db error"))));
        
        // 配置错误不应该重试
        assert!(!should_retry_download_error(&AppError::Config("config error".to_string())));
        
        // 云同步错误中只有网络相关的才重试
        assert!(should_retry_download_error(&AppError::ClipSync("网络连接失败".to_string())));
        assert!(!should_retry_download_error(&AppError::ClipSync("文件格式错误".to_string())));
    }
    
    #[test]
    fn test_current_timestamp() {
        let timestamp = current_timestamp();
        // 时间戳应该是一个合理的值（大于2020年的时间戳）
        assert!(timestamp > 1577836800000); // 2020-01-01 00:00:00 UTC in milliseconds
    }
}
