use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use rbatis::RBatis;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    CONTEXT,
    biz::clip_record::{ClipRecord, SKIP_SYNC},
    utils::file_dir::get_resources_dir,
};
use crate::{
    biz::{
        clip_async_queue::AsyncQueue, clip_record_clean::try_clean_clip_record,
        content_search::add_content_to_index, system_setting::check_cloud_sync_enabled,
    },
    errors::AppError,
    utils::{
        aes_util::encrypt_content,
        device_info::{GLOBAL_DEVICE_ID, GLOBAL_OS_TYPE},
        path_utils::to_safe_string,
    },
};

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;

#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let next_sort = ClipRecord::get_next_sort(rb).await;

        let record_result = match event.r#type {
            ClipType::Text => handle_text(rb, &event.content, next_sort).await,
            ClipType::Image => handle_image(rb, event.file.as_ref(), next_sort).await,
            ClipType::File => handle_file(rb, event.file_path_vec.as_ref(), next_sort).await,
            _ => Ok(None),
        };

        // 处理错误情况
        if let Err(e) = &record_result {
            log::error!("处理剪贴板事件失败: {:?}", e);
        }

        tokio::spawn(async {
            // 清理过期数据
            try_clean_clip_record().await;
        });

        // 通知前端粘贴板变更
        let app_handle = CONTEXT.get::<AppHandle>();
        let _ = app_handle.emit("clip_record_change", ());

        if let Ok(Some(item)) = record_result {
            // 如果有新增记录，发送到异步队列   前提是开启了云同步开关
            if check_cloud_sync_enabled().await {
                let async_queue = CONTEXT.get::<AsyncQueue<ClipRecord>>();
                if !async_queue.is_full() {
                    let send_res = async_queue.send_add(item.clone()).await;
                    if let Err(e) = send_res {
                        log::error!("异步队列发送失败，粘贴内容：{:?}, 异常:{}", item, e);
                    }
                }
            }
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_else(|e| {
            log::warn!("获取系统时间失败，使用默认值: {}", e);
            0
        })
}

fn build_clip_record(
    id: String,
    user_id: i32,
    r#type: String,
    content: Value,
    md5_str: String,
    sort: i32,
) -> ClipRecord {
    let cur_time = current_timestamp();
    ClipRecord {
        id,
        user_id,
        r#type,
        content,
        md5_str,
        created: cur_time,
        os_type: GLOBAL_OS_TYPE.clone(),
        sort,
        pinned_flag: 0,
        sync_flag: Some(0),
        sync_time: Some(0),
        device_id: Some(GLOBAL_DEVICE_ID.clone()),
        version: Some(1),
        del_flag: Some(0),
        cloud_source: Some(0),
    }
}

async fn handle_text(
    rb: &RBatis,
    content: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    // 过滤空文本，空文本不进行记录
    let trimmed_content = content.trim();
    if trimmed_content.is_empty() {
        log::debug!("跳过空文本记录");
        return Ok(None);
    }
    
    let encrypt_res = encrypt_content(trimmed_content);
    match encrypt_res {
        Ok(encrypted) => {
            let md5_str = format!("{:x}", md5::compute(trimmed_content));
            let existing = ClipRecord::check_by_type_and_md5(
                rb,
                ClipType::Text.to_string().as_str(),
                md5_str.as_str(),
            )
            .await?;

            if let Some(record) = existing.first() {
                if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                    log::error!("更新排序失败: {}", e);
                    return Err(e);
                }
                Ok(None)
            } else {
                let record = build_clip_record(
                    Uuid::new_v4().to_string(),
                    0,
                    ClipType::Text.to_string(),
                    Value::String(encrypted),
                    md5_str,
                    sort,
                );

                match ClipRecord::insert(rb, &record).await {
                    Ok(_res) => {
                        let content_string = trimmed_content.to_string();
                        let record_id = record.id.clone();
                        tokio::spawn(async move {
                            if let Err(e) =
                                add_content_to_index(record_id.as_str(), content_string.as_str())
                                    .await
                            {
                                log::error!("搜索索引更新失败: {}", e);
                            }
                        });
                        Ok(Some(record))
                    }
                    Err(e) => {
                        log::error!("插入文本记录失败: {}", e);
                        Err(AppError::Database(e))
                    }
                }
            }
        }
        Err(e) => {
            log::error!("文本内容加密失败，无法保存记录: {:?}", e);
            log::error!(
                "失败的文本内容前50个字符: {:?}",
                &trimmed_content[..trimmed_content.len().min(50)]
            );
            Err(AppError::Clipboard(format!("文本内容加密失败: {:?}", e)))
        }
    }
}

async fn handle_image(
    rb: &RBatis,
    file_data: Option<&Vec<u8>>,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    if let Some(data) = file_data {
        // 检查图片大小是否超过限制
        use crate::utils::config::get_max_file_size_bytes;
        let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);

        let is_oversized = data.len() as u64 > max_file_size;
        if is_oversized {
            log::warn!(
                "图片大小 {} 字节超过限制 {} 字节，将创建记录但标记为跳过同步",
                data.len(),
                max_file_size
            );
        }

        let md5_str = format!("{:x}", md5::compute(data));
        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::Image.to_string().as_str(), &md5_str)
                .await?;

        if let Some(record) = existing.first() {
            if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                log::error!("更新图片排序失败: {}", e);
                return Err(e);
            }
            Ok(None)
        } else {
            let id = Uuid::new_v4().to_string();
            let mut record = build_clip_record(
                id.clone(),
                0,
                ClipType::Image.to_string(),
                Value::Null, // 初始为空，保存图片后会更新为文件名
                md5_str,
                sort,
            );

            // 如果图片大小超过限制，设置为跳过同步状态
            if is_oversized {
                record.sync_flag = Some(SKIP_SYNC);
            }

            match ClipRecord::insert(rb, &record).await {
                Ok(_) => {
                    // 保存图片到资源目录并生成文件名
                    if let Some(filename) = save_img_to_resource(&id, rb, data).await {
                        // 更新record的content字段为文件名
                        record.content = Value::String(filename);
                    }

                    Ok(Some(record))
                }
                Err(e) => {
                    log::error!("插入图片记录失败: {}", e);
                    Err(AppError::Database(e))
                }
            }
        }
    } else {
        Ok(None)
    }
}

async fn handle_file(
    rb: &RBatis,
    file_paths: Option<&Vec<String>>,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    if let Some(paths) = file_paths {
        use crate::utils::config::get_max_file_size_bytes;
        let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);

        // 多文件直接跳过云同步
        if paths.len() > 1 {
            log::info!(
                "检测到多文件复制({} 个文件)，跳过云同步，仅保留本地记录",
                paths.len()
            );
            return handle_multiple_files(rb, paths, sort).await;
        }

        // 单文件处理
        if let Some(file_path) = paths.first() {
            let path = std::path::PathBuf::from(file_path);

            if !path.exists() {
                log::warn!("文件不存在: {}", file_path);
                return Ok(None);
            }

            let metadata = match std::fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(e) => {
                    log::warn!("读取文件元数据失败: {}, 文件: {}", e, file_path);
                    return Ok(None);
                }
            };

            // 使用文件路径计算MD5
            let md5_str = format!("{:x}", md5::compute(file_path.as_bytes()));

            let existing = ClipRecord::check_by_type_and_md5(
                rb,
                ClipType::File.to_string().as_str(),
                &md5_str,
            )
            .await?;

            if let Some(record) = existing.first() {
                if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                    log::error!("更新文件排序失败: {}", e);
                    return Err(e);
                }
                return Ok(None);
            }

            // 检查文件大小
            if metadata.len() > max_file_size {
                log::warn!(
                    "单文件大小 {} 字节超过限制 {} 字节，跳过云同步: {}",
                    metadata.len(),
                    max_file_size,
                    file_path
                );
                return handle_oversized_single_file(rb, file_path, &md5_str, sort).await;
            }

            // 小文件：复制到resources目录并支持云同步
            return handle_sync_eligible_file(rb, &path, file_path, &md5_str, sort).await;
        }
    }
    Ok(None)
}

/// 处理多文件情况（跳过云同步）
async fn handle_multiple_files(
    rb: &RBatis,
    paths: &Vec<String>,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    // 使用所有路径计算MD5
    let mut sorted_paths = paths.clone();
    sorted_paths.sort();
    let combined = sorted_paths.join("");
    let md5_str = format!("{:x}", md5::compute(combined.as_bytes()));

    let mut record = build_clip_record(
        Uuid::new_v4().to_string(),
        0,
        ClipType::File.to_string(),
        Value::String(paths.join(":::")),
        md5_str,
        sort,
    );

    // 多文件直接跳过云同步
    record.sync_flag = Some(SKIP_SYNC);

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            let file_paths_string = paths.join(":::");
            let record_id = record.id.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    add_content_to_index(record_id.as_str(), file_paths_string.as_str()).await
                {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            log::info!(
                "保存多文件记录成功（跳过云同步），记录ID: {}, 文件数: {}",
                record.id,
                paths.len()
            );
            Ok(Some(record))
        }
        Err(e) => {
            log::error!("插入多文件记录失败: {}", e);
            Err(AppError::Database(e))
        }
    }
}

/// 处理超过大小限制的单文件（跳过云同步）
async fn handle_oversized_single_file(
    rb: &RBatis,
    file_path: &str,
    md5_str: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    let mut record = build_clip_record(
        Uuid::new_v4().to_string(),
        0,
        ClipType::File.to_string(),
        Value::String(file_path.to_string()),
        md5_str.to_string(),
        sort,
    );

    // 超过大小限制，跳过云同步
    record.sync_flag = Some(SKIP_SYNC);

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            let record_id = record.id.clone();
            let file_path_copy = file_path.to_string();
            tokio::spawn(async move {
                if let Err(e) =
                    add_content_to_index(record_id.as_str(), file_path_copy.as_str()).await
                {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            log::info!(
                "保存超大单文件记录成功（跳过云同步），记录ID: {}, 文件: {}",
                record.id,
                file_path
            );
            Ok(Some(record))
        }
        Err(e) => {
            log::error!("插入超大单文件记录失败: {}", e);
            Err(AppError::Database(e))
        }
    }
}

/// 处理符合云同步条件的单文件（复制到resources目录）
async fn handle_sync_eligible_file(
    rb: &RBatis,
    file_path: &std::path::PathBuf,
    original_path: &str,
    md5_str: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    let record_id = Uuid::new_v4().to_string();

    // 先创建记录，content初始为空
    let mut record = build_clip_record(
        record_id.clone(),
        0,
        ClipType::File.to_string(),
        Value::Null, // 初始为空，复制成功后会更新为相对路径
        md5_str.to_string(),
        sort,
    );

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            // 复制文件到resources/files目录
            if let Some(relative_path) = copy_file_to_resources(&record_id, file_path).await {
                // 更新record的content字段为相对路径
                let _ = ClipRecord::update_content(rb, &record_id, &relative_path).await;
                record.content = Value::String(relative_path.clone());

                log::info!(
                    "保存小文件记录成功（支持云同步），记录ID: {}, 原路径: {}, 新路径: {}",
                    record_id,
                    original_path,
                    relative_path
                );
            } else {
                // 复制失败，回退到跳过云同步
                log::warn!("文件复制失败，回退到跳过云同步: {}", original_path);
                record.sync_flag = Some(SKIP_SYNC);
                record.content = Value::String(original_path.to_string());
                let _ =
                    ClipRecord::update_content(rb, &record_id, &original_path.to_string()).await;
            }

            // 添加到搜索索引
            let record_id_copy = record_id.clone();
            let content_for_index = if let Value::String(content) = &record.content {
                content.clone()
            } else {
                original_path.to_string()
            };

            tokio::spawn(async move {
                if let Err(e) =
                    add_content_to_index(record_id_copy.as_str(), content_for_index.as_str()).await
                {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            Ok(Some(record))
        }
        Err(e) => {
            log::error!("插入小文件记录失败: {}", e);
            Err(AppError::Database(e))
        }
    }
}

/// 复制文件到resources/files目录
async fn copy_file_to_resources(
    _record_id: &str,
    file_path: &std::path::PathBuf,
) -> Option<String> {
    if let Some(resources_dir) = get_resources_dir() {
        let files_dir = resources_dir.join("files");

        // 确保files目录存在
        if let Err(e) = std::fs::create_dir_all(&files_dir) {
            log::error!("创建files目录失败: {}", e);
            return None;
        }

        // 生成新文件名：保留原扩展名
        let original_extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let now = Local::now().format("%Y%m%d%H%M%S").to_string();
        let uid = Uuid::new_v4().to_string();
        let new_filename = if original_extension.is_empty() {
            format!("{}_{}", now, uid)
        } else {
            format!("{}_{}.{}", now, uid, original_extension)
        };

        let target_path = files_dir.join(&new_filename);
        let relative_path = format!("files/{}", new_filename);

        // 复制文件
        match std::fs::copy(file_path, &target_path) {
            Ok(_) => {
                log::debug!("文件复制成功: {:?} -> {:?}", file_path, target_path);
                Some(relative_path)
            }
            Err(e) => {
                log::error!(
                    "文件复制失败: {:?} -> {:?}, 错误: {}",
                    file_path,
                    target_path,
                    e
                );
                None
            }
        }
    } else {
        log::error!("获取resources目录失败");
        None
    }
}

async fn save_img_to_resource(data_id: &str, rb: &RBatis, image: &Vec<u8>) -> Option<String> {
    if let Some(resource_path) = get_resources_dir() {
        // 生成唯一文件名
        let uid = Uuid::new_v4().to_string();
        let now = Local::now().format("%Y%m%d%H%M%S").to_string();
        let filename = format!("{}_{}.png", now, uid);

        // 拼接完整路径
        let mut full_path: PathBuf = resource_path.clone();
        full_path.push(&filename);

        // 创建并写入图片
        match File::create(&full_path) {
            Ok(mut file) => {
                if file.write_all(image).is_ok() && file.flush().is_ok() {
                    // 写成功后，记录相对路径到数据库
                    let _ = ClipRecord::update_content(rb, data_id, &filename).await;
                    return Some(filename);
                } else {
                    log::error!("写入图片失败");
                }
            }
            Err(e) => {
                let safe_path = to_safe_string(&full_path);
                log::error!("创建图片文件失败: {}, 路径: {}", e, safe_path);
            }
        }
    } else {
        log::error!("资源路径获取失败");
    }
    None
}
