use std::time::{SystemTime, UNIX_EPOCH};

use rbatis::{table_sync::SqliteTableMapper, RBatis};

use crate::{biz::clip_record::ClipRecord, CONTEXT};

pub async fn init_sqlite() {
    // 创建sqlite链接
    let rb = RBatis::new();
    rb.init(rbdc_sqlite::Driver {}, &format!("sqlite://clip_pal.db"))
        .unwrap();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let table = ClipRecord {
        id: "".to_string(),
        r#type: "".to_string(),
        content: "".to_string(),
        created: timestamp,
        user_id: 0,
    };
    let _ = RBatis::sync(&rb, &SqliteTableMapper {}, &table, "clip_record").await;
    // 把sqlite链接放入全局变量中
    CONTEXT.set(rb);
}
