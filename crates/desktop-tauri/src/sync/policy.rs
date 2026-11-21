//! 自建部署策略，不调用账号或 VIP。
use async_trait::async_trait;
use crate::{app_context::AppContext, core::policy::ApplicationPolicy};

pub struct SelfhostPolicy { pub max_file_size_bytes: u64 }
#[async_trait]
impl ApplicationPolicy for SelfhostPolicy {
    fn max_records(&self, _context: &AppContext) -> Result<u32, String> { Ok(i32::MAX as u32) }
    fn cached_sync_file_limit(&self, _context: &AppContext) -> Result<u64, String> { Ok(self.max_file_size_bytes) }
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        self.cached_sync_file_limit(context)
    }
    async fn file_copy_limit(&self, _context: &AppContext) -> u64 { u64::MAX }
    async fn authorize_sync(&self, context: &AppContext) -> Result<(), String> {
        context.sync().auth().connection().await.map(|_| ())
    }
    async fn can_sync_file(&self, _context: &AppContext, size: u64) -> Result<(bool, String), String> {
        if self.max_file_size_bytes == 0 { return Ok((false, "当前部署未启用附件同步".into())); }
        if size > self.max_file_size_bytes { return Ok((false, "文件大小超过部署同步限制".into())); }
        Ok((true, "文件可以同步".into()))
    }
}
