use crate::{CONTEXT, utils::file_dir::get_data_dir};
use rbatis::RBatis;

pub async fn init_sqlite() {
    // 创建sqlite链接
    let rb = RBatis::new();
    let db_path = get_data_dir()
        .unwrap()
        .join("clip_record.db")
        .to_str()
        .unwrap()
        .to_string();

    rb.init(rbdc_sqlite::Driver {}, &format!("sqlite://{}", db_path))
        .unwrap();

    // 设置加密密钥（SQLCipher）
    // rb.acquire()
    //     .await
    //     .unwrap()
    //     .exec("PRAGMA key = '123456';", vec![])
    //     .await
    //     .unwrap();

    // 手动建表和索引
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS clip_record (
            id TEXT PRIMARY KEY,
            "type" TEXT,
            content TEXT,
            md5_str TEXT,
            created INTEGER,
            user_id INTEGER,
            os_type TEXT,
            sort INTEGER,
            pinned_flag INTEGER
        );
        "#;
    let create_index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_md5_str ON clip_record(md5_str);
        CREATE INDEX IF NOT EXISTS idx_created ON clip_record(created);
        CREATE INDEX IF NOT EXISTS idx_sort ON clip_record(sort);
        "#;
    rb.acquire()
        .await
        .unwrap()
        .exec(create_table_sql, vec![])
        .await
        .unwrap();
    rb.acquire()
        .await
        .unwrap()
        .exec(create_index_sql, vec![])
        .await
        .unwrap();

    // 把sqlite链接放入全局变量中
    CONTEXT.set(rb);
}
