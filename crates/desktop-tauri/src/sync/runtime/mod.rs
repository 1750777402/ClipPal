//! 同步后台任务的实际实现及稳定入口。当前 HTTP 流程保持兼容。

#[cfg(feature = "sync")]
pub mod records;
#[cfg(feature = "sync")]
pub mod upload;
#[cfg(feature = "sync")]
pub mod download;

#[cfg(feature = "sync")]
pub use records::{start_cloud_sync_timer, trigger_immediate_sync};
#[cfg(feature = "sync")]
pub use download::start_cloud_file_download_timer;
#[cfg(feature = "sync")]
pub use upload::start_upload_cloud_timer;
