use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

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
    }
}

async fn handle_text(
    rb: &RBatis,
    content: &str,
    sort: i32,
) -> Result<Option<ClipRecord>, AppError> {
    let encrypt_res = encrypt_content(content);
    match encrypt_res {
        Ok(encrypted) => {
            let md5_str = format!("{:x}", md5::compute(content));
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
                        let content_string = content.to_string();
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
                &content[..content.len().min(50)]
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
                    // 无论图片大小是否超过限制，都要保存到资源文件夹
                    // 大小限制只影响云同步，不影响本地存储
                    save_img_to_resource(&id, rb, data).await;

                    if is_oversized {
                        log::info!(
                            "保存超大图片记录成功（跳过云同步），记录ID: {}, 大小: {} 字节",
                            id,
                            data.len()
                        );
                    } else {
                        log::info!(
                            "保存图片记录成功，记录ID: {}, 大小: {} 字节",
                            id,
                            data.len()
                        );
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
        // 检查所有文件大小是否超过限制
        use crate::utils::config::get_max_file_size_bytes;
        let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);

        let mut oversized_files = Vec::new();
        let mut valid_files = Vec::new();

        for path in paths {
            let file_path = std::path::PathBuf::from(path);
            if file_path.exists() {
                match std::fs::metadata(&file_path) {
                    Ok(metadata) => {
                        if metadata.len() > max_file_size {
                            oversized_files.push(path.clone());
                        } else {
                            valid_files.push(path.clone());
                        }
                    }
                    Err(e) => {
                        log::warn!("读取文件元数据失败: {}, 文件: {}", e, path);
                    }
                }
            } else {
                log::warn!("文件不存在: {}", path);
            }
        }

        if !oversized_files.is_empty() {
            log::warn!(
                "发现 {} 个文件超过大小限制 {} 字节，文件: {:?}",
                oversized_files.len(),
                max_file_size,
                oversized_files
            );
        }

        // 无论文件大小如何，都保存完整的文件路径列表
        // 大小限制只影响云同步，不影响本地记录的完整性
        let files_to_save = paths.clone();

        if !oversized_files.is_empty() {
            log::warn!(
                "检测到 {} 个文件超过云同步大小限制，这些文件不会上传到云端但会保留在本地记录中",
                oversized_files.len()
            );
        }

        // 使用原始路径计算MD5，保持兼容性
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        let combined = sorted_paths.join("");
        let md5_str = format!("{:x}", md5::compute(combined.as_bytes()));

        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::File.to_string().as_str(), &md5_str)
                .await?;

        if let Some(record) = existing.first() {
            if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                log::error!("更新文件排序失败: {}", e);
                return Err(e);
            }
            Ok(None)
        } else {
            // 保存文件路径（如果所有文件都超限则保存原始路径）
            let mut record = build_clip_record(
                Uuid::new_v4().to_string(),
                0,
                ClipType::File.to_string(),
                Value::String(files_to_save.join(":::")),
                md5_str,
                sort,
            );

            // 如果包含任何超过大小限制的文件，设置为跳过同步状态
            if !oversized_files.is_empty() {
                record.sync_flag = Some(SKIP_SYNC);
            }

            match ClipRecord::insert(rb, &record).await {
                Ok(_res) => {
                    let file_paths_string = files_to_save.join(":::");
                    let record_id = record.id.clone();
                    tokio::spawn(async move {
                        if let Err(e) =
                            add_content_to_index(record_id.as_str(), file_paths_string.as_str())
                                .await
                        {
                            log::error!("搜索索引更新失败: {}", e);
                        }
                    });

                    if !oversized_files.is_empty() {
                        if valid_files.is_empty() {
                            log::info!(
                                "保存文件记录成功（完全跳过云同步），记录ID: {}, 总文件数: {}, 原因: 所有文件都超过大小限制",
                                record.id,
                                files_to_save.len()
                            );
                        } else {
                            log::info!(
                                "保存文件记录成功（部分跳过云同步），记录ID: {}, 总文件数: {}, 可同步文件数: {}, 超限文件数: {}",
                                record.id,
                                files_to_save.len(),
                                valid_files.len(),
                                oversized_files.len()
                            );
                        }
                    } else {
                        log::info!(
                            "保存文件记录成功，记录ID: {}, 文件数: {}",
                            record.id,
                            files_to_save.len()
                        );
                    }

                    Ok(Some(record))
                }
                Err(e) => {
                    log::error!("插入文件记录失败: {}", e);
                    Err(AppError::Database(e))
                }
            }
        }
    } else {
        Ok(None)
    }
}

async fn save_img_to_resource(data_id: &str, rb: &RBatis, image: &Vec<u8>) {
    if let Some(resource_path) = get_resources_dir() {
        // 生成唯一文件名
        let uid = Uuid::new_v4().to_string();
        let filename = format!("{}.png", uid);

        // 拼接完整路径
        let mut full_path: PathBuf = resource_path.clone();
        full_path.push(&filename);

        // 创建并写入图片
        match File::create(&full_path) {
            Ok(mut file) => {
                if file.write_all(image).is_ok() && file.flush().is_ok() {
                    // 写成功后，记录相对路径到数据库
                    let _ = ClipRecord::update_content(rb, data_id, &filename).await;
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
}
