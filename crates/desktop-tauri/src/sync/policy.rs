//! 自建部署策略，不调用账号或 VIP。
use async_trait::async_trait;
use crate::{app_context::AppContext, core::policy::ApplicationPolicy};

/// 自建版能力限制只取决于部署配置，不要求商业账号。
pub struct SelfhostPolicy { pub max_file_size_bytes: u64 }
#[async_trait]
impl ApplicationPolicy for SelfhostPolicy {
    /// 返回当前版本策略允许的本地记录数上限。
    fn max_records(&self, _context: &AppContext) -> Result<u32, String> { Ok(i32::MAX as u32) }
    /// 只读本地配置或权益缓存，返回同步附件的字节上限。
    fn cached_sync_file_limit(&self, _context: &AppContext) -> Result<u64, String> { Ok(self.max_file_size_bytes) }
    /// 取得当前同步文件大小上限，具体是否联网由策略实现决定。
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        self.cached_sync_file_limit(context)
    }
    /// 检查当前凭据或部署配置是否允许同步；不直接读写记录。
    async fn authorize_sync(&self, context: &AppContext) -> Result<(), String> {
        context.sync().auth().connection().await.map(|_| ())
    }
    /// 根据当前策略判断单个附件大小是否允许同步，并返回原因。
    async fn can_sync_file(&self, _context: &AppContext, size: u64) -> Result<(bool, String), String> {
        if self.max_file_size_bytes == 0 { return Ok((false, "当前部署未启用附件同步".into())); }
        if size > self.max_file_size_bytes { return Ok((false, "文件大小超过部署同步限制".into())); }
        Ok((true, "文件可以同步".into()))
    }
}
