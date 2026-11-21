mod clip_record;
mod clip_record_resource;
pub use clip_record::ClipRecord;
pub use clip_record_resource::ClipRecordResource;

/// 核心类型使用稳定的小写值，桌面 DTO 单独适配原有页面类型。
pub const TEXT_TYPE: &str = "text";
pub const FILE_TYPE: &str = "files";
pub const IMAGE_TYPE: &str = "image";
pub const DIRECTORY_TYPE: &str = "directory";
