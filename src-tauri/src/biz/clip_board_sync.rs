use std::{
    env::current_dir,
    fs::{self, File},
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use rbatis::RBatis;
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
        match event.r#type {
            ClipType::Text => {
                let insert_res = ClipRecord::insert(
                    rb,
                    &ClipRecord {
                        id: Uuid::new_v4().to_string(),
                        r#type: ClipType::Text.to_string(),
                        content: event.content.clone(),
                        created: timestamp,
                        user_id: 0,
                        os_type: "win".to_string(),
                    },
                )
                .await;
                if let Err(e) = insert_res {
                    println!("insert text record error {}", e);
                }
            }
            ClipType::Image => {
                let uid = Uuid::new_v4().to_string();
                if let Some(file) = &event.file {
                    let insert_res = ClipRecord::insert(
                        rb,
                        &ClipRecord {
                            id: uid.clone(),
                            r#type: ClipType::Image.to_string(),
                            content: String::new(),
                            created: timestamp,
                            user_id: 0,
                            os_type: "win".to_string(),
                        },
                    )
                    .await;
                    if let Err(e) = insert_res {
                        println!("insert img record error {}", e);
                    } else {
                        save_img_to_resource(uid, rb, file).await;
                    }
                }
            }
            ClipType::File => {
                let insert_res = ClipRecord::insert(
                    rb,
                    &ClipRecord {
                        id: Uuid::new_v4().to_string(),
                        r#type: ClipType::File.to_string(),
                        content: event.content.clone(),
                        created: timestamp,
                        user_id: 0,
                        os_type: "win".to_string(),
                    },
                )
                .await;
                if let Err(e) = insert_res {
                    println!("insert file error {}", e);
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
            let _ = ClipRecord::update_content(rb, data_id.as_str(), path.as_str()).await;
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
