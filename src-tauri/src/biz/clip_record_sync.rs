use std::{
    fs::File,
    io::{Read, Write},
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
    biz::clip_record::{ClipRecord, NOT_SYNCHRONIZED, SKIP_SYNC},
    utils::{file_dir::get_resources_dir, file_ext::extract_full_extension},
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
            if item.sync_flag != Some(SKIP_SYNC) && check_cloud_sync_enabled().await {
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

/// 计算文件内容的MD5值（智能策略：小文件全读，大文件采样）
async fn compute_file_content_md5(file_path: &std::path::Path) -> Result<String, std::io::Error> {
    const SMALL_FILE_THRESHOLD: u64 = 10 * 1024 * 1024; // 10MB

    let metadata = std::fs::metadata(file_path)?;
    let file_size = metadata.len();

    if file_size <= SMALL_FILE_THRESHOLD {
        // 小文件：读取完整内容计算MD5
        compute_full_file_md5(file_path).await
    } else {
        // 大文件：采样计算MD5（文件头+中间+尾部+文件大小）
        compute_sampled_file_md5(file_path, file_size).await
    }
}

/// 计算完整文件内容的MD5
async fn compute_full_file_md5(file_path: &std::path::Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(file_path)?;
    let mut buffer = [0; 8192]; // 8KB缓冲区
    let mut context = md5::Context::new();

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        context.consume(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", context.compute()))
}

/// 计算大文件采样MD5（文件头+中间+尾部+文件大小）
async fn compute_sampled_file_md5(
    file_path: &std::path::Path,
    file_size: u64,
) -> Result<String, std::io::Error> {
    use std::io::{Seek, SeekFrom};

    const SAMPLE_SIZE: usize = 1024 * 1024; // 1MB
    let mut file = std::fs::File::open(file_path)?;
    let mut context = md5::Context::new();
    let sample_len = SAMPLE_SIZE.min(file_size as usize / 3);
    let mut buffer = vec![0u8; sample_len];

    // 读取文件头
    file.read_exact(&mut buffer)?;
    context.consume(&buffer);

    // 读取文件中间
    if file_size > (sample_len * 2) as u64 {
        let mid_pos = file_size / 2 - (sample_len / 2) as u64;
        file.seek(SeekFrom::Start(mid_pos))?;
        file.read_exact(&mut buffer)?;
        context.consume(&buffer);
    }

    // 读取文件尾
    if file_size > sample_len as u64 {
        file.seek(SeekFrom::End(-(sample_len as i64)))?;
        file.read_exact(&mut buffer)?;
        context.consume(&buffer);
    }

    // 包含文件大小信息防止大小相同但内容不同的文件冲突
    context.consume(&file_size.to_le_bytes());

    Ok(format!("{:x}", context.compute()))
}

/// 计算多文件内容的组合MD5（基于文件名和内容，不包含路径）
async fn compute_multiple_files_md5(file_paths: &[String]) -> Result<String, std::io::Error> {
    let mut context = md5::Context::new();

    // 创建文件信息列表：(文件名, 文件路径)
    let mut file_info: Vec<(String, String)> = Vec::new();

    for file_path in file_paths {
        let path = std::path::Path::new(file_path);
        if path.exists() {
            // 提取文件名（不包含路径）
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(file_path)
                .to_string();

            file_info.push((filename, file_path.clone()));
        }
    }

    // 按文件名排序确保一致性（不是按路径排序）
    file_info.sort_by(|a, b| a.0.cmp(&b.0));

    for (filename, file_path) in file_info {
        let path = std::path::Path::new(&file_path);

        // 只包含文件名信息（不包含路径，确保相同文件产生相同MD5）
        context.consume(filename.as_bytes());

        // 包含文件内容MD5
        match compute_file_content_md5(&path).await {
            Ok(content_md5) => {
                context.consume(content_md5.as_bytes());
            }
            Err(e) => {
                log::warn!(
                    "无法读取文件内容生成MD5，跳过文件: {}, 错误: {}",
                    file_path,
                    e
                );
            }
        }
    }

    Ok(format!("{:x}", context.compute()))
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
        local_file_path: None,
        created: cur_time,
        os_type: GLOBAL_OS_TYPE.clone(),
        sort,
        pinned_flag: 0,
        sync_flag: Some(NOT_SYNCHRONIZED),
        sync_time: Some(0),
        device_id: Some(GLOBAL_DEVICE_ID.clone()),
        version: Some(1),
        del_flag: Some(0),
        cloud_source: Some(0),
    }
}

fn build_oversized_file_record(id: &str, file_path: &str, md5_str: &str, sort: i32) -> ClipRecord {
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path);

    let mut record = build_clip_record(
        id.to_string(),
        0,
        ClipType::File.to_string(),
        Value::String(filename.to_string()),
        md5_str.to_string(),
        sort,
    );

    record.sync_flag = Some(SKIP_SYNC);
    record.local_file_path = Some(file_path.to_string());
    record
}

fn build_sync_eligible_file_record(
    id: &str,
    file_path: &str,
    md5_str: &str,
    sort: i32,
) -> ClipRecord {
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path);

    build_clip_record(
        id.to_string(),
        0,
        ClipType::File.to_string(),
        Value::String(filename.to_string()),
        md5_str.to_string(),
        sort,
    )
}

async fn handle_sync_eligible_file_update(
    rb: &RBatis,
    record_id: &str,
    file_path: &str,
    record: &mut ClipRecord,
) {
    let file_path_buf = std::path::PathBuf::from(file_path);

    // 复制文件到resources/files目录
    if let Some((_relative_path, absolute_path)) =
        copy_file_to_resources(record_id, &file_path_buf).await
    {
        let _ = ClipRecord::update_local_file_path(rb, record_id, &absolute_path).await;
        record.local_file_path = Some(absolute_path);
    } else {
        // 复制失败，设置为不支持云同步
        log::warn!("文件复制失败，设置为不支持同步: {}", file_path);
        record.sync_flag = Some(SKIP_SYNC);
        record.local_file_path = Some(file_path.to_string());
        let _ = ClipRecord::update_local_file_path(rb, record_id, file_path).await;
    }
}

fn build_multiple_files_record(
    id: &str,
    paths: &Vec<String>,
    md5_str: &str,
    sort: i32,
) -> ClipRecord {
    // content存储文件名列表（显示用）
    let filenames: Vec<String> = paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_string()
        })
        .collect();
    let content_display = filenames.join(":::");

    let mut record = build_clip_record(
        id.to_string(),
        0,
        ClipType::File.to_string(),
        Value::String(content_display),
        md5_str.to_string(),
        sort,
    );

    // 多文件不支持云同步
    record.sync_flag = Some(SKIP_SYNC);
    record.local_file_path = Some(paths.join(":::"));
    record
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
            // 单次查询检查是否有相同内容的记录
            let existing = ClipRecord::check_by_type_and_md5(
                rb,
                ClipType::Text.to_string().as_str(),
                md5_str.as_str(),
            )
            .await?;

            if let Some(record) = existing.first() {
                if record.del_flag == Some(1) {
                    // 已删除的记录，更新为新记录的所有字段
                    let new_record = build_clip_record(
                        record.id.clone(), // 保持原ID
                        0,
                        ClipType::Text.to_string(),
                        Value::String(encrypted),
                        md5_str,
                        sort,
                    );

                    if let Err(e) =
                        ClipRecord::update_deleted_record_as_new(rb, &record.id, &new_record).await
                    {
                        log::error!("更新已删除文本记录失败: {}", e);
                        return Err(e);
                    }

                    // 更新搜索索引
                    let record_id_copy = record.id.clone();
                    let content_copy = trimmed_content.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = add_content_to_index(&record_id_copy, &content_copy).await {
                            log::error!("搜索索引更新失败: {}", e);
                        }
                    });

                    log::info!("更新已删除的文本记录为新数据: {}", record.id);
                    return Ok(Some(new_record));
                } else {
                    // 活跃记录，只更新排序
                    if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                        log::error!("更新排序失败: {}", e);
                        return Err(e);
                    }
                    return Ok(None);
                }
            }

            // 创建新记录
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
                            add_content_to_index(record_id.as_str(), content_string.as_str()).await
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
                "图片大小 {} 字节超过限制 {} 字节，将创建记录但标记为不支持同步",
                data.len(),
                max_file_size
            );
        }

        let md5_str = format!("{:x}", md5::compute(data));

        // 单次查询检查是否有相同内容的记录
        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::Image.to_string().as_str(), &md5_str)
                .await?;

        if let Some(record) = existing.first() {
            if record.del_flag == Some(1) {
                // 已删除的记录，更新为新记录的所有字段
                let id = record.id.clone();

                // 先生成文件名，然后保存图片
                let filename = generate_unique_filename("png");
                if save_image_with_filename(&filename, data).await {
                    let mut new_record = build_clip_record(
                        id.clone(),
                        0,
                        ClipType::Image.to_string(),
                        Value::String(filename.clone()), // 直接设置为生成的文件名
                        md5_str,
                        sort,
                    );

                    // 如果图片大小超过限制，设置为不支持同步状态
                    if is_oversized {
                        new_record.sync_flag = Some(SKIP_SYNC);
                    }

                    if let Err(e) =
                        ClipRecord::update_deleted_record_as_new(rb, &id, &new_record).await
                    {
                        log::error!("更新已删除图片记录失败: {}", e);
                        // 保存图片失败时删除已创建的文件
                        delete_image_file(&filename).await;
                        return Err(e);
                    }

                    log::info!("更新已删除的图片记录为新数据: {}", id);
                    return Ok(Some(new_record));
                } else {
                    log::error!("保存图片失败，无法更新记录");
                    return Err(AppError::Clipboard("保存图片失败".to_string()));
                }
            } else {
                // 活跃记录，只更新排序
                if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                    log::error!("更新图片排序失败: {}", e);
                    return Err(e);
                }
                return Ok(None);
            }
        }

        // 创建新记录 - 先生成文件名，然后保存图片
        let id = Uuid::new_v4().to_string();
        let filename = generate_unique_filename("png");

        if save_image_with_filename(&filename, data).await {
            let mut record = build_clip_record(
                id.clone(),
                0,
                ClipType::Image.to_string(),
                Value::String(filename.clone()), // 直接设置为生成的文件名
                md5_str,
                sort,
            );

            // 如果图片大小超过限制，设置为不支持同步状态
            if is_oversized {
                record.sync_flag = Some(SKIP_SYNC);
            }

            match ClipRecord::insert(rb, &record).await {
                Ok(_) => {
                    log::info!("新增图片记录成功，ID: {}, 文件名: {}", id, filename);
                    Ok(Some(record))
                }
                Err(e) => {
                    log::error!("插入图片记录失败: {}", e);
                    // 数据库插入失败时删除已创建的文件
                    delete_image_file(&filename).await;
                    Err(AppError::Database(e))
                }
            }
        } else {
            log::error!("保存图片失败，无法创建记录");
            Err(AppError::Clipboard("保存图片失败".to_string()))
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

        // 多文件不支持云同步
        if paths.len() > 1 {
            log::info!(
                "检测到多文件复制({} 个文件)，不支持云同步，仅保留本地记录",
                paths.len()
            );
            return handle_multiple_files(rb, paths, sort).await;
        }

        // 单文件处理
        if let Some(file_path) = paths.first() {
            let path = std::path::Path::new(file_path);

            if !path.exists() {
                log::warn!("文件不存在: {}", file_path);
                return Ok(None);
            }

            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(e) => {
                    log::warn!("读取文件元数据失败: {}, 文件: {}", e, file_path);
                    return Ok(None);
                }
            };

            // 使用文件内容计算MD5
            let md5_str = match compute_file_content_md5(path).await {
                Ok(hash) => hash,
                Err(e) => {
                    log::error!("无法读取文件内容生成MD5: {}, 文件: {}", e, file_path);
                    return Ok(None); // 无法读取文件则跳过
                }
            };

            // 单次查询检查是否有相同内容的记录
            let existing = ClipRecord::check_by_type_and_md5(
                rb,
                ClipType::File.to_string().as_str(),
                &md5_str,
            )
            .await?;

            if let Some(record) = existing.first() {
                if record.del_flag == Some(1) {
                    // 已删除的记录，根据文件大小决定处理方式
                    if metadata.len() > max_file_size {
                        // 超大文件：更新为不支持同步的记录
                        let new_record =
                            build_oversized_file_record(&record.id, file_path, &md5_str, sort);
                        if let Err(e) =
                            ClipRecord::update_deleted_record_as_new(rb, &record.id, &new_record)
                                .await
                        {
                            log::error!("更新已删除超大文件记录失败: {}", e);
                            return Err(e);
                        }

                        // 更新搜索索引
                        let record_id_copy = record.id.clone();
                        let filename = std::path::Path::new(file_path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(file_path)
                            .to_string();
                        tokio::spawn(async move {
                            if let Err(e) = add_content_to_index(&record_id_copy, &filename).await {
                                log::error!("搜索索引更新失败: {}", e);
                            }
                        });

                        log::info!("更新已删除的超大文件记录为新数据: {}", record.id);
                        return Ok(Some(new_record));
                    } else {
                        // 小文件：更新为支持同步的记录
                        let new_record =
                            build_sync_eligible_file_record(&record.id, file_path, &md5_str, sort);
                        if let Err(e) =
                            ClipRecord::update_deleted_record_as_new(rb, &record.id, &new_record)
                                .await
                        {
                            log::error!("更新已删除小文件记录失败: {}", e);
                            return Err(e);
                        }

                        // 处理文件复制和路径更新
                        let mut updated_record = new_record;
                        handle_sync_eligible_file_update(
                            rb,
                            &record.id,
                            file_path,
                            &mut updated_record,
                        )
                        .await;

                        // 更新搜索索引
                        let record_id_copy = record.id.clone();
                        let filename = std::path::Path::new(file_path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(file_path)
                            .to_string();
                        tokio::spawn(async move {
                            if let Err(e) = add_content_to_index(&record_id_copy, &filename).await {
                                log::error!("搜索索引更新失败: {}", e);
                            }
                        });

                        log::info!("更新已删除的小文件记录为新数据: {}", record.id);
                        return Ok(Some(updated_record));
                    }
                } else {
                    // 活跃记录，只更新排序
                    if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                        log::error!("更新文件排序失败: {}", e);
                        return Err(e);
                    }
                    return Ok(None);
                }
            }

            // 检查文件大小
            if metadata.len() > max_file_size {
                log::warn!(
                    "单文件大小 {} 字节超过限制 {} 字节，设置为不支持同步: {}",
                    metadata.len(),
                    max_file_size,
                    file_path
                );
                return handle_oversized_single_file(rb, file_path, &md5_str, sort).await;
            }

            // 小文件：复制到resources目录并支持云同步
            return handle_sync_eligible_file(rb, file_path, &md5_str, sort).await;
        }
    }
    Ok(None)
}

/// 处理多文件情况（不支持云同步）
async fn handle_multiple_files(
    rb: &RBatis,
    paths: &Vec<String>,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    // 使用文件内容组合计算MD5
    let md5_str = match compute_multiple_files_md5(paths).await {
        Ok(hash) => hash,
        Err(e) => {
            log::error!("无法计算多文件组合MD5: {}", e);
            // 回退到文件名组合MD5（不包含路径信息）
            let mut filenames: Vec<String> = paths
                .iter()
                .map(|path| {
                    std::path::Path::new(path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(path)
                        .to_string()
                })
                .collect();
            filenames.sort();
            let combined = filenames.join(":::");
            format!("{:x}", md5::compute(combined.as_bytes()))
        }
    };

    // 单次查询检查是否有相同内容的记录
    let existing =
        ClipRecord::check_by_type_and_md5(rb, ClipType::File.to_string().as_str(), &md5_str)
            .await?;

    if let Some(record) = existing.first() {
        if record.del_flag == Some(1) {
            // 已删除的记录，更新为新记录
            let new_record = build_multiple_files_record(&record.id, paths, &md5_str, sort);
            if let Err(e) =
                ClipRecord::update_deleted_record_as_new(rb, &record.id, &new_record).await
            {
                log::error!("更新已删除多文件记录失败: {}", e);
                return Err(e);
            }

            // 更新搜索索引
            let record_id_copy = record.id.clone();
            let content_copy = new_record.content.as_str().unwrap_or_default().to_string();
            tokio::spawn(async move {
                if let Err(e) = add_content_to_index(&record_id_copy, &content_copy).await {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            log::info!("更新已删除的多文件记录为新数据: {}", record.id);
            return Ok(Some(new_record));
        } else {
            // 活跃记录，只更新排序
            if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                log::error!("更新多文件排序失败: {}", e);
                return Err(e);
            }
            return Ok(None);
        }
    }

    let record_id = Uuid::new_v4().to_string();

    // content存储文件名列表（显示用）
    let filenames: Vec<String> = paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_string()
        })
        .collect();
    let content_display = filenames.join(":::");

    let mut record = build_clip_record(
        record_id.clone(),
        0,
        ClipType::File.to_string(),
        Value::String(content_display.clone()),
        md5_str,
        sort,
    );

    // 多文件不支持云同步
    record.sync_flag = Some(SKIP_SYNC);
    record.local_file_path = Some(paths.join(":::"));

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            let record_id_copy = record_id.clone();
            let content_copy = content_display.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    add_content_to_index(record_id_copy.as_str(), content_copy.as_str()).await
                {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            log::info!(
                "保存多文件记录成功（不支持同步），记录ID: {}, 文件数: {}, 文件名: {}",
                record.id,
                paths.len(),
                content_display
            );
            Ok(Some(record))
        }
        Err(e) => {
            log::error!("插入多文件记录失败: {}", e);
            Err(AppError::Database(e))
        }
    }
}

/// 处理超过大小限制的单文件（不支持云同步）
async fn handle_oversized_single_file(
    rb: &RBatis,
    file_path: &str,
    md5_str: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    let record_id = Uuid::new_v4().to_string();

    // content存储原文件名（显示用）
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path);

    let mut record = build_clip_record(
        record_id.clone(),
        0,
        ClipType::File.to_string(),
        Value::String(filename.to_string()),
        md5_str.to_string(),
        sort,
    );

    // 超过大小限制，不支持云同步
    record.sync_flag = Some(SKIP_SYNC);
    record.local_file_path = Some(file_path.to_string());

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            let record_id_copy = record_id.clone();
            let filename_copy = filename.to_string();
            tokio::spawn(async move {
                if let Err(e) =
                    add_content_to_index(record_id_copy.as_str(), filename_copy.as_str()).await
                {
                    log::error!("搜索索引更新失败: {}", e);
                }
            });

            log::info!(
                "保存超大单文件记录成功（不支持同步），记录ID: {}, 文件路径: {}, 文件名: {}",
                record.id,
                file_path,
                filename
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
    file_path: &str,
    md5_str: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    let record_id = Uuid::new_v4().to_string();

    // 先创建记录，content初始为空
    let mut record = build_clip_record(
        record_id.clone(),
        0,
        ClipType::File.to_string(),
        Value::Null, // 初始为空，复制成功后会更新为文件名
        md5_str.to_string(),
        sort,
    );

    match ClipRecord::insert(rb, &record).await {
        Ok(_) => {
            let file_path_buf = std::path::PathBuf::from(file_path);

            // 复制文件到resources/files目录
            if let Some((_relative_path, absolute_path)) =
                copy_file_to_resources(&record_id, &file_path_buf).await
            {
                // content存储原文件名（显示用），而不是生成的新文件名
                let original_filename = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(file_path);

                let _ = ClipRecord::update_content(rb, &record_id, original_filename).await;
                let _ = ClipRecord::update_local_file_path(rb, &record_id, &absolute_path).await;

                record.content = Value::String(original_filename.to_string());
                record.local_file_path = Some(absolute_path);

                log::info!(
                    "保存小文件记录成功（支持云同步），记录ID: {}, 原路径: {}, 新路径: {}, 显示文件名: {}",
                    record_id,
                    file_path,
                    record.local_file_path.as_ref().unwrap(),
                    original_filename
                );
            } else {
                // 复制失败，设置为不支持云同步
                log::warn!("文件复制失败，设置为不支持同步: {}", file_path);
                record.sync_flag = Some(SKIP_SYNC);

                // content存储原文件名（显示用）
                let original_filename = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(file_path);

                record.content = Value::String(original_filename.to_string());
                record.local_file_path = Some(file_path.to_string());

                let _ = ClipRecord::update_content(rb, &record_id, original_filename).await;
                let _ = ClipRecord::update_local_file_path(rb, &record_id, file_path).await;
            }

            // 添加到搜索索引
            let record_id_copy = record_id.clone();
            let content_for_index = if let Value::String(content) = &record.content {
                content.clone()
            } else {
                file_path.to_string()
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

/// 复制文件到resources/files目录，返回(相对路径, 绝对路径)
async fn copy_file_to_resources(
    _record_id: &str,
    file_path: &std::path::PathBuf,
) -> Option<(String, String)> {
    if let Some(resources_dir) = get_resources_dir() {
        let files_dir = resources_dir.join("files");

        // 确保files目录存在
        if let Err(e) = std::fs::create_dir_all(&files_dir) {
            log::error!("创建files目录失败: {}", e);
            return None;
        }

        // 生成新文件名：保留完整的扩展名（支持复合扩展名如tar.gz）
        let original_extension = extract_full_extension(file_path);

        let now = Local::now().format("%Y%m%d%H%M%S").to_string();
        let uid = Uuid::new_v4().to_string();
        let new_filename = if original_extension.is_empty() {
            format!("{}_{}", now, uid)
        } else {
            format!("{}_{}.{}", now, uid, original_extension)
        };

        let target_path = files_dir.join(&new_filename);
        let relative_path = format!("files/{}", new_filename);
        let absolute_path = target_path.to_string_lossy().to_string();

        // 复制文件
        match std::fs::copy(file_path, &target_path) {
            Ok(_) => {
                log::debug!("文件复制成功: {:?} -> {:?}", file_path, target_path);
                Some((relative_path, absolute_path))
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

/// 生成唯一的文件名
fn generate_unique_filename(extension: &str) -> String {
    let uid = Uuid::new_v4().to_string();
    let now = Local::now().format("%Y%m%d%H%M%S").to_string();
    format!("{}_{}.{}", now, uid, extension)
}

/// 使用指定的文件名保存图片
async fn save_image_with_filename(filename: &str, image: &Vec<u8>) -> bool {
    if let Some(resource_path) = get_resources_dir() {
        // 拼接完整路径
        let mut full_path: PathBuf = resource_path.clone();
        full_path.push(filename);

        // 创建并写入图片
        match File::create(&full_path) {
            Ok(mut file) => {
                if file.write_all(image).is_ok() && file.flush().is_ok() {
                    log::debug!("图片保存成功: {}", filename);
                    true
                } else {
                    log::error!("写入图片失败: {}", filename);
                    false
                }
            }
            Err(e) => {
                let safe_path = to_safe_string(&full_path);
                log::error!("创建图片文件失败: {}, 路径: {}", e, safe_path);
                false
            }
        }
    } else {
        log::error!("资源路径获取失败");
        false
    }
}

/// 删除图片文件
async fn delete_image_file(filename: &str) {
    if let Some(resource_path) = get_resources_dir() {
        let mut full_path: PathBuf = resource_path.clone();
        full_path.push(filename);

        if let Err(e) = std::fs::remove_file(&full_path) {
            let safe_path = to_safe_string(&full_path);
            log::warn!("删除图片文件失败: {}, 路径: {}", e, safe_path);
        } else {
            log::debug!("删除图片文件成功: {}", filename);
        }
    }
}
