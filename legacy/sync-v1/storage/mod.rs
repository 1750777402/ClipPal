//! SQLite 适配器。领域模型不再实现数据库方法。
mod clip_record_repository;
mod merge;
mod schema;
pub use clip_record_repository::SqliteClipRecordRepository;
pub use schema::connect;

use crate::{domain::clip::ClipRecord, errors::AppResult};
use rbatis::executor::Executor;
use rbs::to_value;

/// 所有值均采用参数绑定，保留原记录字段和内容存储格式。
async fn insert_record(executor: &dyn Executor, record: &ClipRecord) -> AppResult<()> {
    executor.exec(
        "INSERT INTO clip_record (id, type, content, md5_str, local_file_path, created, os_type, sort, pinned_flag, sync_flag, sync_time, device_id, version, del_flag, cloud_source, skip_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![to_value!(&record.id), to_value!(&record.r#type), to_value!(&record.content),
             to_value!(&record.md5_str), to_value!(&record.local_file_path), to_value!(record.created),
             to_value!(&record.os_type), to_value!(record.sort), to_value!(record.pinned_flag),
             to_value!(&record.sync_flag), to_value!(&record.sync_time), to_value!(&record.device_id),
             to_value!(&record.version), to_value!(&record.del_flag), to_value!(&record.cloud_source),
             to_value!(&record.skip_type)],
    ).await?;
    Ok(())
}
