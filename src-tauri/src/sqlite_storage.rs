use rbatis::{RBatis, table_sync::SqliteTableMapper};
use std::{
    env::current_dir,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    CONTEXT,
    biz::clip_record::ClipRecord,
    utils::file_dir::{get_data_dir},
};

pub async fn init_sqlite() {
    // 创建sqlite链接
    let rb = RBatis::new();
    let build_dir = current_dir().unwrap().parent().unwrap().join("build");
    if let false = build_dir.exists() {
        fs::create_dir_all(build_dir).unwrap();
    }
    let db_path = get_data_dir().unwrap().join("clip_record.db").to_str().unwrap().to_string();
    rb.init(rbdc_sqlite::Driver {}, &format!("sqlite://{}", db_path))
        .unwrap();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let table = ClipRecord {
        id: "".to_string(),
        r#type: "".to_string(),
        content: "".to_string(),
        md5_str: "".to_string(),
        created: timestamp,
        user_id: 0,
        os_type: "win".to_string(),
        sort: 1,
    };
    let _ = RBatis::sync(&rb, &SqliteTableMapper {}, &table, "clip_record").await;
    // 把sqlite链接放入全局变量中
    CONTEXT.set(rb);
}
