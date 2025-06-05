use std::{
    env::current_dir,
    fs::{self, File},
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use rbatis::RBatis;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{CONTEXT, biz::clip_record::ClipRecord};

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;

#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut max_sort = 0;
        let max_sort_record = ClipRecord::select_max_sort(rb, 0).await.unwrap_or(vec![]);
        if max_sort_record.len() > 0 {
            max_sort = max_sort_record[0].sort + 1;
        }
        match event.r#type {
            ClipType::Text => {
                let old_text_record = ClipRecord::check_by_type_and_content(
                    rb,
                    ClipType::Text.to_string().as_str(),
                    event.content.clone().as_str(),
                )
                .await
                .unwrap_or(vec![]);
                if old_text_record.len() > 0 {
                    // 存在相同的文本   把这个文本排序到最前面
                    let _ =
                        ClipRecord::update_sort(rb, old_text_record[0].id.as_str(), max_sort).await;
                } else {
                    // 新增一条记录
                    let insert_res = ClipRecord::insert(
                        rb,
                        &ClipRecord {
                            id: Uuid::new_v4().to_string(),
                            r#type: ClipType::Text.to_string(),
                            content: serde_json::Value::String(event.content.clone()),
                            md5_str: "".to_string(),
                            created: timestamp,
                            user_id: 0,
                            os_type: "win".to_string(),
                            sort: max_sort,
                        },
                    )
                    .await;
                    if let Err(e) = insert_res {
                        println!("insert text record error {}", e);
                    }
                }
            }
            ClipType::Image => {
                let file_data = &event.file.clone().unwrap_or(vec![]);
                // 计算图片的md5
                let img_md5 = format!("{:x}", md5::compute(file_data));
                let old_image_record = ClipRecord::check_by_type_and_md5(
                    rb,
                    ClipType::Image.to_string().as_str(),
                    img_md5.as_str(),
                )
                .await
                .unwrap_or(vec![]);
                if old_image_record.len() > 0 {
                    // 存在相同的图片   把这个图片排序到最前面
                    let _ = ClipRecord::update_sort(rb, old_image_record[0].id.as_str(), max_sort)
                        .await;
                } else {
                    let uid = Uuid::new_v4().to_string();
                    if let Some(file) = &event.file {
                        let insert_res = ClipRecord::insert(
                            rb,
                            &ClipRecord {
                                id: uid.clone(),
                                r#type: ClipType::Image.to_string(),
                                content: serde_json::Value::String("".to_string()),
                                md5_str: img_md5,
                                created: timestamp,
                                user_id: 0,
                                os_type: "win".to_string(),
                                sort: max_sort,
                            },
                        )
                        .await;
                        if let Err(e) = insert_res {
                            println!("insert img record error {}", e);
                        } else {
                            // 把图片存到resource文件夹
                            save_img_to_resource(uid, rb, file).await;
                        }
                    }
                }
            }
            ClipType::File => {
                let file_path = &event.file_path_vec.clone().unwrap_or(vec![]);
                let mut new_file_path = file_path.to_vec();
                // 根据字典序排序
                new_file_path.sort();
                let file_path_md5 =
                    format!("{:x}", md5::compute(new_file_path.join("").as_bytes()));
                let old_file_record = ClipRecord::check_by_type_and_md5(
                    rb,
                    ClipType::File.to_string().as_str(),
                    file_path_md5.as_str(),
                )
                .await
                .unwrap_or(vec![]);
                if old_file_record.len() > 0 {
                    // 存在相同的文件   把这个文件排序到最前面
                    let _ =
                        ClipRecord::update_sort(rb, old_file_record[0].id.as_str(), max_sort).await;
                } else {
                    let insert_res = ClipRecord::insert(
                        rb,
                        &ClipRecord {
                            id: Uuid::new_v4().to_string(),
                            r#type: ClipType::File.to_string(),
                            content: serde_json::to_value(file_path)
                                .unwrap_or(Value::String("".to_string())),
                            md5_str: file_path_md5,
                            created: timestamp,
                            user_id: 0,
                            os_type: "win".to_string(),
                            sort: max_sort,
                        },
                    )
                    .await;
                    if let Err(e) = insert_res {
                        println!("insert file error {}", e);
                    }
                }
            }
            _ => {}
        }
        // 触发粘贴板变化事件通知前端
        let app_handle = CONTEXT.get::<AppHandle>();
        let _ = app_handle.emit("clip_record_change", ());
    }
}

async fn save_img_to_resource(data_id: String, rb: &RBatis, image: &Vec<u8>) {
    let uid = Uuid::new_v4().to_string();
    check_resource_dir().await;
    let path = &format!("resources\\{}.png", uid);
    let resource_path = current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .join(path)
        .to_str()
        .unwrap()
        .to_string();
    let file = File::create(resource_path);
    if let Ok(mut file) = file {
        if let Ok(_) = file.write_all(image) {
            let _ = file.flush();
            let _ = ClipRecord::update_content(
                rb,
                data_id.as_str(),
                &serde_json::Value::String(path.clone()),
            )
            .await;
        }
    }
}

async fn check_resource_dir() {
    // 1. 准备资源目录路径
    let resources_dir = current_dir().unwrap().parent().unwrap().join("resources");

    // 2. 检查并创建目录（如果不存在）
    if !resources_dir.exists() {
        fs::create_dir(&resources_dir).unwrap();
    }
}
