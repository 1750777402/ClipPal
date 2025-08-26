use clipboard_listener::ClipType;
use rbatis::RBatis;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{FileCloudSyncParam, upload_file_clip_record};
use crate::biz::clip_record::{ClipRecord, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::biz::system_setting::check_cloud_sync_enabled;
use crate::errors::{AppError, AppResult};
use crate::utils::config::get_max_file_size_bytes;
use crate::utils::file_dir::get_resources_dir;
use crate::utils::retry_helper::{RetryConfig, retry_with_config};
use crate::utils::token_manager::has_valid_auth;

/// 这个定时任务是云同步上传记录时，文件类型的内容上传到云端的任务

/// 启动文件同步定时任务
pub fn start_upload_cloud_timer() {
    task::spawn(async move {
        log::info!("文件同步定时任务已启动");

        loop {
            // 检查云同步是否开启
            if !check_cloud_sync_enabled().await {
                log::debug!("云同步未开启，跳过文件同步任务");
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            // 检查用户登录状态
            if !has_valid_auth() {
                log::debug!("用户未登录或认证已过期，跳过文件同步任务");
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            // 执行文件同步任务
            if let Err(e) = process_one_file_sync().await {
                log::error!("文件同步任务执行失败: {}", e);
            }

            // 等待一段时间后继续下一轮
            sleep(Duration::from_secs(1)).await;
        }
    });
}

/// 处理一个文件同步任务
/// 每次只处理一条SYNCHRONIZING状态的记录
async fn process_one_file_sync() -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    // 查找一条sync_flag为SYNCHRONIZING的记录，但是需要是本地自己的记录，而不是云端同步下来的
    let pending_records = ClipRecord::select_by_sync_flag_limit(rb, SYNCHRONIZING, 0, 1).await?;

    if pending_records.is_empty() {
        log::debug!("没有发现待同步文件的记录");
        return Ok(());
    }

    // 只处理第一条记录
    let record = &pending_records[0];
    log::info!(
        "开始处理文件同步，记录ID: {}, 类型: {}",
        record.id,
        record.r#type
    );

    match record.r#type.as_str() {
        t if t == ClipType::Image.to_string() => process_image_sync(record).await,
        t if t == ClipType::File.to_string() => process_file_sync(record).await,
        _ => {
            // 其他类型不需要文件同步，直接标记为已同步
            let ids = vec![record.id.clone()];
            let current_time = current_timestamp();
            ClipRecord::update_sync_flag(rb, &ids, SKIP_SYNC, current_time).await?;
            log::info!("非文件类型记录直接标记为已同步: {}", record.id);
            Ok(())
        }
    }
}

/// 处理图片同步
async fn process_image_sync(record: &ClipRecord) -> AppResult<()> {
    // 获取图片文件名（从content字段）
    let image_filename = record
        .content
        .as_str()
        .ok_or(AppError::Config("图片记录content字段无效".to_string()))?;

    if image_filename.is_empty() {
        // 文件名为空，直接标记为已同步
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let ids = vec![record.id.clone()];
        let current_time = current_timestamp();
        ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await?;
        log::warn!("图片记录content为空，直接标记为已同步: {}", record.id);
        return Ok(());
    }

    // 拼接完整的图片文件路径（resources目录 + 文件名）
    let resources_dir =
        get_resources_dir().ok_or_else(|| AppError::Config("无法获取resources目录".to_string()))?;
    let file_path = resources_dir.join(image_filename);

    // 检查文件是否存在
    if !file_path.exists() {
        log::error!("图片文件不存在: {:?}, 记录ID: {}", file_path, record.id);
        return mark_as_skip_sync(&record.id, "图片文件不存在").await;
    }

    // 检查文件大小
    if let Err(e) = check_file_size(&file_path).await {
        log::warn!("图片文件大小检查失败: {}, 记录ID: {}", e, record.id);
        return mark_as_skip_sync(&record.id, &e).await;
    }

    // 上传文件
    let upload_param = FileCloudSyncParam {
        md5_str: record.md5_str.clone(),
        r#type: ClipType::Image.to_string(),
        file: file_path,
    };

    upload_file_with_retry(&record.id, upload_param).await
}

/// 处理文件同步
async fn process_file_sync(record: &ClipRecord) -> AppResult<()> {
    // 使用local_file_path字段获取文件路径
    if let Some(local_file_path) = &record.local_file_path {
        let file_paths: Vec<String> = local_file_path
            .split(":::")
            .map(|s| s.to_string())
            .collect();

        // 检查所有文件是否存在以及大小是否符合要求
        let mut valid_files = Vec::new();
        let mut has_oversized_file = false;

        for file_path_str in &file_paths {
            let file_path = PathBuf::from(file_path_str);

            if !file_path.exists() {
                log::warn!("文件不存在，跳过: {}", file_path_str);
                continue;
            }

            // 检查文件大小
            if let Err(e) = check_file_size(&file_path).await {
                log::warn!("文件大小检查失败: {}, 文件: {}", e, file_path_str);
                has_oversized_file = true;
                continue;
            }

            valid_files.push(file_path);
        }

        if valid_files.is_empty() {
            if has_oversized_file {
                return mark_as_skip_sync(&record.id, "所有文件都超过大小限制或不存在").await;
            } else {
                // 所有文件都不存在，直接标记为已同步
                let rb: &RBatis = CONTEXT.get::<RBatis>();
                let ids = vec![record.id.clone()];
                let current_time = current_timestamp();
                ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await?;
                log::warn!("所有文件都不存在，直接标记为已同步: {}", record.id);
                return Ok(());
            }
        }

        // 逐个上传有效文件
        for file_path in valid_files {
            let upload_param = FileCloudSyncParam {
                md5_str: record.md5_str.clone(),
                r#type: ClipType::File.to_string(),
                file: file_path.clone(),
            };

            if let Err(e) = upload_file_with_retry(&record.id, upload_param).await {
                log::error!(
                    "文件上传失败: {:?}, 记录ID: {}, 错误: {}",
                    file_path,
                    record.id,
                    e
                );
                // 上传失败后，将记录标记为跳过同步，避免死循环
                return mark_as_skip_sync(&record.id, &format!("文件上传失败: {}", e)).await;
            }

            log::info!("文件上传成功: {:?}, 记录ID: {}", file_path, record.id);
        }

        // 所有文件上传完成，标记为已同步
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let ids = vec![record.id.clone()];
        let current_time = current_timestamp();
        ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await?;
        notify_frontend_sync_status(vec![record.id.clone()], SYNCHRONIZED).await;
        log::info!("所有文件上传完成，记录标记为已同步: {}", record.id);

        Ok(())
    } else {
        // local_file_path字段为None，直接标记为已同步
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let ids = vec![record.id.clone()];
        let current_time = current_timestamp();
        ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await?;
        log::warn!(
            "文件记录local_file_path字段为None，直接标记为已同步: {}",
            record.id
        );
        Ok(())
    }
}

/// 检查文件大小是否超过限制
async fn check_file_size(file_path: &PathBuf) -> Result<(), String> {
    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);

            if metadata.len() > max_file_size {
                Err(format!(
                    "文件大小 {} 字节超过限制 {} 字节",
                    metadata.len(),
                    max_file_size
                ))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(format!("读取文件元数据失败: {}", e)),
    }
}

/// 判断上传错误是否应该重试
fn should_retry_upload_error(error: &AppError) -> bool {
    match error {
        // 网络相关错误可以重试
        AppError::Http(_) => true,
        // 通用错误中的网络问题可以重试
        AppError::General(msg) => {
            let msg_lower = msg.to_lowercase();
            msg_lower.contains("网络")
                || msg_lower.contains("timeout")
                || msg_lower.contains("connection")
                || msg_lower.contains("上传")
                || msg_lower.contains("请求失败")
                || msg_lower.contains("响应为空")
        }
        // 其他错误类型不重试
        _ => false,
    }
}

/// 带重试的文件上传 - 使用 backon
async fn upload_file_with_retry(
    record_id: &str,
    upload_param: FileCloudSyncParam,
) -> AppResult<()> {
    log::info!("开始上传文件（带重试），记录ID: {}", record_id);

    // 配置文件上传的重试策略
    let retry_config = RetryConfig::new(3, 5000) // 最多重试3次，初始延迟5秒
        .with_backoff_multiplier(2.0) // 指数退避，延迟时间每次翻倍
        .with_max_delay(120000) // 最大延迟2分钟
        .with_jitter(true); // 启用抖动，避免惊群效应

    // 使用 backon 执行带重试的上传操作
    let result = retry_with_config(
        retry_config,
        || {
            let param = upload_param.clone();
            let id = record_id.to_string();
            async move { upload_file_and_update_status(&id, param).await }
        },
        should_retry_upload_error,
    )
    .await;

    // 处理结果
    match result {
        Ok(_) => {
            log::info!("文件上传最终成功，记录ID: {}", record_id);
            Ok(())
        }
        Err(e) => {
            log::error!("文件上传最终失败，记录ID: {}，错误: {}", record_id, e);
            Err(e)
        }
    }
}

/// 核心上传逻辑（被重试机制调用）
async fn upload_file_and_update_status(
    record_id: &str,
    upload_param: FileCloudSyncParam,
) -> AppResult<()> {
    log::debug!(
        "执行文件上传，记录ID: {}, 文件: {:?}",
        record_id,
        upload_param.file
    );

    let res = upload_file_clip_record(&upload_param).await;

    match res {
        Ok(response) => {
            if let Some(success) = response {
                let rb: &RBatis = CONTEXT.get::<RBatis>();
                let ids = vec![record_id.to_string()];
                let update_res =
                    ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, success.timestamp).await;

                match update_res {
                    Ok(_) => {
                        notify_frontend_sync_status(vec![record_id.to_string()], SYNCHRONIZED)
                            .await;
                        log::info!("文件上传成功并更新本地同步状态，记录ID: {}", record_id);
                        Ok(())
                    }
                    Err(e) => {
                        log::error!(
                            "文件上传成功但更新本地同步状态失败，记录ID: {}, 错误: {}",
                            record_id,
                            e
                        );
                        Err(AppError::General("文件上传后更新状态失败".to_string()))
                    }
                }
            } else {
                Err(AppError::General("文件上传响应为空".to_string()))
            }
        }
        Err(e) => {
            log::warn!("文件上传请求失败，记录ID: {}, 错误: {}", record_id, e);
            Err(AppError::General(format!("文件上传请求失败: {}", e)))
        }
    }
}

/// 标记记录为跳过同步状态
async fn mark_as_skip_sync(record_id: &str, reason: &str) -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let ids = vec![record_id.to_string()];
    let current_time = current_timestamp();

    ClipRecord::update_sync_flag(rb, &ids, SKIP_SYNC, current_time).await?;
    notify_frontend_sync_status(vec![record_id.to_string()], SKIP_SYNC).await;
    log::info!(
        "记录标记为跳过同步，记录ID: {}, 原因: {}",
        record_id,
        reason
    );

    Ok(())
}

/// 通知前端同步状态更新
async fn notify_frontend_sync_status(ids: Vec<String>, sync_flag: i32) {
    let payload = serde_json::json!({
        "clip_ids": ids,
        "sync_flag": sync_flag
    });
    let app_handle = CONTEXT.get::<AppHandle>();
    let _ = app_handle
        .emit("sync_status_update_batch", payload)
        .map_err(|e| AppError::General(format!("批量通知前端文件同步状态失败: {}", e)));
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
