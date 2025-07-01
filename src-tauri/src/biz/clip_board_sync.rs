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

use crate::{biz::simple_search_bin::add_content_to_index, utils::aes_util::encrypt_content};

use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, clip_record_clean::clip_record_clean},
    utils::file_dir::get_resources_dir,
};

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;

#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let next_sort = get_next_sort(rb).await;

        match event.r#type {
            ClipType::Text => handle_text(rb, &event.content, next_sort).await,
            ClipType::Image => handle_image(rb, event.file.as_ref(), next_sort).await,
            ClipType::File => handle_file(rb, event.file_path_vec.as_ref(), next_sort).await,
            _ => {}
        }

        clip_record_clean().await;
        // 通知前端粘贴板变更
        let app_handle = CONTEXT.get::<AppHandle>();
        let _ = app_handle.emit("clip_record_change", ());
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

async fn get_next_sort(rb: &RBatis) -> i32 {
    ClipRecord::select_max_sort(rb, 0)
        .await
        .ok()
        .and_then(|records| records.get(0).map(|r| r.sort + 1))
        .unwrap_or(0)
}

async fn handle_text(rb: &RBatis, content: &str, sort: i32) {
    let encrypt_res = encrypt_content(content);
    match encrypt_res {
        Ok(encrypted) => {
            // 使用md5进行内容重复检查
            // 加密后的密文不能用于重复检查   因为加密过程使用了随机nonce(随机数种子)   同一个字符串多次加密结果是不一样的
            let md5_str = format!("{:x}", md5::compute(content));
            let existing = ClipRecord::check_by_type_and_md5(
                rb,
                ClipType::Text.to_string().as_str(),
                md5_str.as_str(),
            )
            .await
            .unwrap_or_default();

            if let Some(record) = existing.first() {
                if let Err(e) = ClipRecord::update_sort(rb, &record.id, sort).await {
                    log::error!("更新排序失败: {}", e);
                }
            } else {
                let record = ClipRecord {
                    id: Uuid::new_v4().to_string(),
                    r#type: ClipType::Text.to_string(),
                    content: Value::String(encrypted),
                    md5_str: md5_str,
                    created: current_timestamp(),
                    os_type: "win".to_string(),
                    sort,
                    pinned_flag: 0,
                    ..Default::default()
                };

                match ClipRecord::insert(rb, &record).await {
                    Ok(_res) => {
                        // 将原始内容添加到搜索索引
                        let content_string = content.to_string();
                        let record_id = record.id.clone();
                        tokio::spawn(async move {
                            if let Err(e) = add_content_to_index(
                                record_id.as_str(),
                                content_string.as_str(),
                            )
                            .await
                            {
                                log::error!("搜索索引更新失败: {}", e);
                            }
                        });
                    }
                    Err(e) => log::error!("插入文本记录失败: {}", e),
                }
            }
        }
        Err(e) => {
            log::error!("文本内容加密失败，无法保存记录: {:?}", e);
            log::error!(
                "失败的文本内容前50个字符: {:?}",
                &content[..content.len().min(50)]
            );
        }
    }
}

async fn handle_image(rb: &RBatis, file_data: Option<&Vec<u8>>, sort: i32) {
    if let Some(data) = file_data {
        let md5_str = format!("{:x}", md5::compute(data));
        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::Image.to_string().as_str(), &md5_str)
                .await
                .unwrap_or_default();

        if let Some(record) = existing.first() {
            let _ = ClipRecord::update_sort(rb, &record.id, sort).await;
        } else {
            let id = Uuid::new_v4().to_string();

            let record = ClipRecord {
                id: id.clone(),
                r#type: ClipType::Image.to_string(),
                content: Value::Null,
                md5_str,
                created: current_timestamp(),
                os_type: "win".to_string(),
                sort,
                pinned_flag: 0,
                ..Default::default()
            };

            if ClipRecord::insert(rb, &record).await.is_ok() {
                save_img_to_resource(&id, rb, data).await;
            }
        }
    }
}

async fn handle_file(rb: &RBatis, file_paths: Option<&Vec<String>>, sort: i32) {
    if let Some(paths) = file_paths {
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        let combined = sorted_paths.join("");
        let md5_str = format!("{:x}", md5::compute(combined.as_bytes()));

        let existing =
            ClipRecord::check_by_type_and_md5(rb, ClipType::File.to_string().as_str(), &md5_str)
                .await
                .unwrap_or_default();

        if let Some(record) = existing.first() {
            let _ = ClipRecord::update_sort(rb, &record.id, sort).await;
        } else {
            let record = ClipRecord {
                id: Uuid::new_v4().to_string(),
                r#type: ClipType::File.to_string(),
                content: Value::String(paths.join(":::")),
                md5_str,
                created: current_timestamp(),
                os_type: "win".to_string(),
                sort,
                pinned_flag: 0,
                ..Default::default()
            };

            if let Err(e) = ClipRecord::insert(rb, &record).await {
                log::error!("insert file error: {}", e);
            }
        }
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
                log::error!("创建图片文件失败: {}", e);
            }
        }
    } else {
        log::error!("资源路径获取失败");
    }
}
