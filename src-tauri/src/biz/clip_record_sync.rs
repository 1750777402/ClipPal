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

use crate::{CONTEXT, biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir};
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

        if let Ok(Some(item)) = record_result {
            // 通知前端粘贴板变更
            let app_handle = CONTEXT.get::<AppHandle>();
            let _ = app_handle.emit("clip_record_change", ());

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
                    return Err(AppError::Database(e));
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
        let md5_str = format!("{:x}", md5::compute(data));
        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::Image.to_string().as_str(), &md5_str)
                .await?;

        if let Some(record) = existing.first() {
            if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                log::error!("更新图片排序失败: {}", e);
                return Err(AppError::Database(e));
            }
            Ok(None)
        } else {
            let id = Uuid::new_v4().to_string();
            let record = build_clip_record(
                id.clone(),
                0,
                ClipType::Image.to_string(),
                Value::Null,
                md5_str,
                sort,
            );

            match ClipRecord::insert(rb, &record).await {
                Ok(_) => {
                    save_img_to_resource(&id, rb, data).await;
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
                return Err(AppError::Database(e));
            }
            Ok(None)
        } else {
            let record = build_clip_record(
                Uuid::new_v4().to_string(),
                0,
                ClipType::File.to_string(),
                Value::String(paths.join(":::")),
                md5_str,
                sort,
            );

            match ClipRecord::insert(rb, &record).await {
                Ok(_res) => {
                    let file_paths_string = paths.join(":::");
                    let record_id = record.id.clone();
                    tokio::spawn(async move {
                        if let Err(e) =
                            add_content_to_index(record_id.as_str(), file_paths_string.as_str())
                                .await
                        {
                            log::error!("搜索索引更新失败: {}", e);
                        }
                    });
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
