use clipboard_listener::ClipType;
use rbatis::RBatis;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{FileCloudSyncParam, get_upload_file_url, sync_upload_success};
use crate::biz::clip_record::{ClipRecord, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::biz::system_setting::check_cloud_sync_enabled;
use crate::biz::vip_checker::VipChecker;
use crate::errors::{AppError, AppResult};
use crate::utils::file_dir::get_resources_dir;
use crate::utils::retry_helper::{RetryConfig, retry_with_config};
use crate::utils::token_manager::has_valid_auth;

/// 这个定时任务是云同步上传记录时，文件类型的内容上传到云端的任务

/// 内部文件上传参数（包含文件路径）
#[derive(Debug, Clone)]
struct InternalFileUploadParam {
    pub md5_str: String,
    pub r#type: String,
    pub file: PathBuf,
}

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
            ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await?;
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

    // 上传文件 - 注意：upload_file_with_retry 内部已经处理了上传成功后的状态更新
    // 这里只需要调用上传函数，状态更新在 upload_file_and_update_status 中处理
    let upload_param = InternalFileUploadParam {
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

        // 逐个上传有效文件，确保所有文件都成功后才更新状态
        let mut uploaded_files = Vec::new();
        let mut upload_success = true;

        for file_path in valid_files {
            let upload_param = InternalFileUploadParam {
                md5_str: record.md5_str.clone(),
                r#type: ClipType::File.to_string(),
                file: file_path.clone(),
            };

            match upload_file_with_retry(&record.id, upload_param).await {
                Ok(_) => {
                    uploaded_files.push(file_path.clone());
                    log::info!("文件上传成功: {:?}, 记录ID: {}", file_path, record.id);
                }
                Err(e) => {
                    log::error!(
                        "文件上传失败: {:?}, 记录ID: {}, 错误: {}",
                        file_path,
                        record.id,
                        e
                    );
                    upload_success = false;
                    break; // 任何一个文件上传失败都中止整个上传过程
                }
            }
        }

        // 只有所有文件都上传成功后，才更新记录状态为已同步
        if upload_success && !uploaded_files.is_empty() {
            let rb: &RBatis = CONTEXT.get::<RBatis>();
            let ids = vec![record.id.clone()];
            let current_time = current_timestamp();

            match ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await {
                Ok(_) => {
                    notify_frontend_sync_status(vec![record.id.clone()], SYNCHRONIZED).await;
                    log::info!("所有文件上传完成，记录标记为已同步: {}", record.id);
                }
                Err(e) => {
                    log::error!(
                        "所有文件上传成功但状态更新失败，记录ID: {}, 错误: {}",
                        record.id,
                        e
                    );
                    // 虽然状态更新失败，但文件已上传成功，不返回错误避免重复上传
                    // 这个问题会在下次全量同步时得到修复
                }
            }
        } else if !upload_success {
            // 有文件上传失败，将整个记录标记为跳过同步
            return mark_as_skip_sync(&record.id, "部分文件上传失败").await;
        } else {
            log::warn!("没有有效文件可上传，记录ID: {}", record.id);
        }

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

/// 检查文件大小是否超过VIP限制
async fn check_file_size(file_path: &PathBuf) -> Result<(), String> {
    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            let file_size = metadata.len();
            match VipChecker::can_sync_file(file_size).await {
                Ok((can_sync, message)) => {
                    if can_sync {
                        Ok(())
                    } else {
                        Err(message)
                    }
                }
                Err(e) => Err(format!("检查VIP文件权限失败: {}", e)),
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
    upload_param: InternalFileUploadParam,
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

/// 核心上传逻辑（被重试机制调用）- 使用预签名URL上传
async fn upload_file_and_update_status(
    record_id: &str,
    upload_param: InternalFileUploadParam,
) -> AppResult<()> {
    log::debug!(
        "执行预签名URL文件上传，记录ID: {}, 文件: {:?}",
        record_id,
        upload_param.file
    );

    // 步骤1: 获取预签名上传URL
    let sync_param = FileCloudSyncParam {
        md5_str: upload_param.md5_str.clone(),
        r#type: upload_param.r#type.clone(),
    };

    let upload_url_response = match get_upload_file_url(&sync_param).await {
        Ok(Some(response)) => response,
        Ok(None) => {
            log::warn!("获取上传URL失败：服务端返回空响应，记录ID: {}", record_id);
            return Err(AppError::General("获取上传URL响应为空".to_string()));
        }
        Err(e) => {
            log::warn!("获取上传URL失败，记录ID: {}, 错误: {}", record_id, e);
            return Err(AppError::General(format!("获取上传URL失败: {}", e)));
        }
    };

    // 步骤2: 直接上传文件到OSS
    if let Err(e) = upload_file_to_oss(&upload_url_response.url, &upload_param.file).await {
        log::error!("上传文件到OSS失败，记录ID: {}, 错误: {}", record_id, e);
        return Err(AppError::General(format!("上传文件到OSS失败: {}", e)));
    }

    log::info!("文件上传到OSS成功，记录ID: {}", record_id);

    // 步骤3: 通知服务端上传完成
    match sync_upload_success(&sync_param).await {
        Ok(Some(true)) => {
            log::info!("通知服务端上传完成成功，记录ID: {}", record_id);
        }
        Ok(Some(false)) | Ok(None) => {
            log::warn!("通知服务端上传完成失败，记录ID: {}", record_id);
            return Err(AppError::General("通知服务端上传完成失败".to_string()));
        }
        Err(e) => {
            log::error!(
                "通知服务端上传完成请求失败，记录ID: {}, 错误: {}",
                record_id,
                e
            );
            return Err(AppError::General(format!("通知服务端上传完成失败: {}", e)));
        }
    }

    // 步骤4: 只有所有步骤都成功后，才更新本地状态
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let ids = vec![record_id.to_string()];
    let current_time = current_timestamp();

    match ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await {
        Ok(_) => {
            notify_frontend_sync_status(vec![record_id.to_string()], SYNCHRONIZED).await;
            log::info!("预签名URL上传完整流程成功，记录ID: {}", record_id);
            Ok(())
        }
        Err(e) => {
            log::error!(
                "严重错误：文件已上传并通知服务端成功，但本地状态更新失败，记录ID: {}, 错误: {}. 
                文件已在云端，但本地状态不一致！",
                record_id,
                e
            );

            // 尝试重新更新状态，最多重试2次
            let mut retry_count = 0;
            let max_retries = 2;

            while retry_count < max_retries {
                retry_count += 1;
                log::warn!(
                    "尝试重新更新本地状态，第{}次重试，记录ID: {}",
                    retry_count,
                    record_id
                );

                tokio::time::sleep(tokio::time::Duration::from_millis(1000 * retry_count)).await;

                match ClipRecord::update_sync_flag(rb, &ids, SYNCHRONIZED, current_time).await {
                    Ok(_) => {
                        notify_frontend_sync_status(vec![record_id.to_string()], SYNCHRONIZED)
                            .await;
                        log::info!("状态更新重试成功，记录ID: {}", record_id);
                        return Ok(());
                    }
                    Err(retry_e) => {
                        log::warn!("状态更新重试失败，记录ID: {}, 错误: {}", record_id, retry_e);
                    }
                }
            }

            // 所有重试都失败了，但上传已经成功，避免重复上传
            log::error!(
                "文件上传成功但本地状态更新多次重试失败，记录ID: {}. 
                建议检查数据库连接或在下次全量同步时修复状态",
                record_id
            );

            // 虽然状态不一致，但不阻塞其他记录的处理
            Ok(())
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

/// 直接上传文件到OSS（使用预签名URL）
async fn upload_file_to_oss(upload_url: &str, file_path: &PathBuf) -> AppResult<()> {
    // 检查文件是否存在
    if !file_path.exists() {
        return Err(AppError::General(format!("文件不存在: {:?}", file_path)));
    }

    // 使用现有的http_client进行上传，但采用PUT方法
    use tauri_plugin_http::reqwest;

    let file_content = std::fs::read(file_path).map_err(|e| AppError::Io(e))?;

    // 使用tauri内置的reqwest客户端直接上传到OSS
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 5分钟超时
        .user_agent("ClipPal-OSS/1.0")
        .build()
        .map_err(|e| AppError::General(format!("创建OSS客户端失败: {}", e)))?;

    let response = client
        .put(upload_url)
        .body(file_content)
        .send()
        .await
        .map_err(|e| AppError::General(format!("OSS上传请求失败: {}", e)))?;

    let status = response.status();

    if status.is_success() {
        log::info!("文件上传到OSS成功: {:?}, 状态码: {}", file_path, status);
        Ok(())
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "无法读取错误响应".to_string());

        log::error!("OSS详细错误响应: {}", error_text);

        let error_message = format!("OSS上传失败，状态码: {} - {}", status, error_text);

        Err(AppError::General(error_message))
    }
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
