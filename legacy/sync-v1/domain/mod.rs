mod clip_record;
mod skip_sync_reason;
mod sync_status;

pub use clip_record::ClipRecord;
pub use skip_sync_reason::SkipSyncReason;
pub use sync_status::{SyncStatus, NOT_SYNCHRONIZED, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};

/// 与现有数据库和剪贴板事件的类型字符串保持一致。
pub const TEXT_TYPE: &str = "Text";
pub const FILE_TYPE: &str = "File";
pub const IMAGE_TYPE: &str = "Image";
