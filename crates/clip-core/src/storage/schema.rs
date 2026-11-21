//! 单机数据库结构入口。这里只定义本地记录和资源，不创建同步或账号表。
use std::path::Path;
use rbatis::RBatis;
use rbs::to_value;
use crate::errors::{AppError,AppResult};

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

/// 旧 UUID/MD5 表没有 content_key，用这一特征区分新旧结构。
pub async fn is_legacy(db: &RBatis) -> AppResult<bool> {
    if !has_table(db, "clip_record").await? { return Ok(false); }
    #[derive(serde::Deserialize)] struct Column { name: String }
    let columns: Vec<Column> = db.query_decode("PRAGMA table_info(clip_record)", vec![]).await?;
    Ok(!columns.iter().any(|column| column.name == "content_key"))
}

/// 建立连接，不隐式迁移旧表；旧表需要由持有解密能力和资源路径的应用层迁移。
pub async fn connect(path: &Path) -> AppResult<RBatis> {
    let db = RBatis::new();
    // 驱动默认为每个连接设置 foreign_keys=ON、WAL 和 busy_timeout。
    db.init(rbdc_sqlite::Driver {}, &format!("sqlite://{}", path.to_string_lossy()))?;
    #[derive(serde::Deserialize)] struct Version { user_version:i32 }
    let version:Vec<Version>=db.query_decode("PRAGMA user_version",vec![]).await?;
    // 新版数据库不能交给旧程序降级处理，否则可能破坏尚不认识的结构。
    if version[0].user_version>2{return Err(AppError::Config("数据库版本高于当前应用支持的版本，请升级应用".into()));}
    if !is_legacy(&db).await? {
        // 新库直接建两表；旧库留给具有解密能力的应用层完整迁移。
        create_tables(&db).await?;
        db.exec("PRAGMA user_version=2",vec![]).await?;
    }
    Ok(db)
}
