//! 单机数据库结构入口。这里只定义本地记录和资源，不创建同步或账号表。
use std::path::Path;
use rbatis::RBatis;
use rbs::to_value;
use crate::errors::AppResult;

/// 仅定义本地业务表。外键由 SQLite 连接选项对每个池连接启用。
pub async fn create_tables(executor: &dyn rbatis::executor::Executor) -> AppResult<()> {
    // 表约束保证类型、资源顺序和置顶状态有效；索引负责去重、列表排序和路径查找。
    // 资源外键级联删除行，磁盘文件的回收由应用服务在提交后执行。
    for sql in [
        "CREATE TABLE IF NOT EXISTS clip_record (id INTEGER PRIMARY KEY AUTOINCREMENT, content_key TEXT NOT NULL UNIQUE, type TEXT NOT NULL CHECK(type IN ('text','image','files','directory')), content TEXT, created_at INTEGER NOT NULL, last_copied_at INTEGER NOT NULL, sort_order INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1)))",
        "CREATE TABLE IF NOT EXISTS clip_record_resource (id INTEGER PRIMARY KEY AUTOINCREMENT, record_id INTEGER NOT NULL REFERENCES clip_record(id) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position >= 0), display_name TEXT NOT NULL, resource_kind TEXT NOT NULL CHECK(resource_kind IN ('image','file','directory')), storage_kind TEXT NOT NULL CHECK(storage_kind IN ('managed','reference')), path TEXT NOT NULL, byte_size INTEGER CHECK(byte_size IS NULL OR byte_size >= 0), content_hash TEXT, UNIQUE(record_id, position))",
        "CREATE INDEX IF NOT EXISTS idx_local_resource_path ON clip_record_resource(storage_kind,path)",
        "CREATE INDEX IF NOT EXISTS idx_local_record_order ON clip_record(pinned DESC, sort_order DESC)",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_local_single_pin ON clip_record(pinned) WHERE pinned = 1",
    ] { executor.exec(sql, vec![]).await?; }
    Ok(())
}

/// 按表名检查结构是否存在；名称使用参数绑定，不拼接进 SQL。
pub async fn has_table(db: &RBatis, name: &str) -> AppResult<bool> {
    #[derive(serde::Deserialize)] struct Row { count: i64 }
    let rows: Vec<Row> = db.query_decode("SELECT COUNT(*) AS count FROM sqlite_master WHERE type='table' AND name=?", vec![to_value!(name)]).await?;
    Ok(rows[0].count != 0)
}

/// 建立连接并按当前结构初始化本机数据库。
pub async fn connect(path: &Path) -> AppResult<RBatis> {
    let db = RBatis::new();
    // 驱动默认为每个连接设置 foreign_keys=ON、WAL 和 busy_timeout。
    db.init(rbdc_sqlite::Driver {}, &format!("sqlite://{}", path.to_string_lossy()))?;
    create_tables(&db).await?;
    Ok(db)
}
