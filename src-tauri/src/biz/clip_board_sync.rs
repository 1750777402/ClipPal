use std::time::{SystemTime, UNIX_EPOCH};

use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use rbatis::{rbdc::DateTime, RBatis};
use uuid::Uuid;

use crate::{biz::clip_record::ClipRecord, CONTEXT};

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;
#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        let rb = CONTEXT.get::<RBatis>();
        println!("触发了粘贴板监听器，内容：{:?}", &event.r#type);
        let timestamp_u32 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap_or_else(|_| {
                u32::MAX // 返回最大允许值
            });
        match event.r#type {
            ClipType::Text => {
                println!("文本内容：{}", event.content);
                let insert_res = ClipRecord::insert(
                    rb,
                    &ClipRecord {
                        id: Uuid::new_v4().to_string(),
                        r#type: "text".to_string(),
                        content: event.content.clone(),
                        created: timestamp_u32,
                        user_id: 1,
                    },
                )
                .await;
                if let Err(e) = insert_res {
                    println!("insert error {}", e);
                }
            }
            ClipType::Img => {
                println!("图片内容：{}", event.content);
            }
            ClipType::File => {
                println!("文件内容：{}", event.content);
            }
            _ => {}
        }
    }
}
