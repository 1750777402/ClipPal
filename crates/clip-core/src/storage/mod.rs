//! 本地 SQLite 适配，无同步表、云端合并或同步状态操作。
mod clip_record_repository;
mod schema;
pub use clip_record_repository::{begin_record_write, load_record_in, delete_in, persist_record, save_in, RecordWriteHook, SqliteClipRecordRepository};
pub use schema::{connect, create_tables, has_table};
