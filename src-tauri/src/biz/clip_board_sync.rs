use std::{
    env::current_dir,
    fs::File,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use rbatis::RBatis;
use uuid::Uuid;

use crate::{biz::clip_record::ClipRecord, CONTEXT};

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
                        r#type: "Text".to_string(),
                        content: event.content.clone(),
                        created: timestamp,
                        user_id: 1,
                    },
                )
                .await;
                if let Err(e) = insert_res {
                    println!("insert text record error {}", e);
                }
            }
            ClipType::Img => {
                let uid = Uuid::new_v4().to_string();
                if let Some(file) = &event.file {
                    let insert_res = ClipRecord::insert(
                        rb,
                        &ClipRecord {
                            id: uid.clone(),
                            r#type: "Img".to_string(),
                            content: String::new(),
                            created: timestamp,
                            user_id: 1,
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
                        r#type: "File".to_string(),
                        content: event.content.clone(),
                        created: timestamp,
                        user_id: 1,
                    },
                )
                .await;
                if let Err(e) = insert_res {
                    println!("insert file error {}", e);
                }
            }
            _ => {}
        }
    }
}

async fn save_img_to_resource(data_id: String, rb: &RBatis, image: &Vec<u8>) {
    let uid = Uuid::new_v4().to_string();
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

async fn save_file_to_resource(data_id: String, rb: &RBatis, file_path: Vec<String>) {}
