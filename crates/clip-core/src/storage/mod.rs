//! 本地 SQLite 适配，无同步表、云端合并或同步状态操作。
mod clip_record_repository;
mod schema;
pub use clip_record_repository::{SqliteClipRecordRepository, persist_record};
pub use schema::{connect, create_tables, has_table, is_legacy};
