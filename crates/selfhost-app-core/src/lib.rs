//! 自建版复用完整的单机应用核心，部署同步在此组装。
pub use clip_sync_client;
pub use desktop_app_core;
pub mod deployment;
mod sync_store;
mod sync_runtime;
mod attachment;
mod transfers;
pub use transfers::{CardSyncStatus, IncomingAttachment};
pub use attachment::skip_reason;

use clip_sync_client::auth::{DisabledSyncAuth, SyncAuthProvider};
use desktop_app_core::{
    capture::{CaptureInput, TextEncoder},
    clip_core::{domain::clip::ClipRecord, errors::AppResult},
    LocalContext,
};
use std::sync::Arc;
pub use sync_runtime::{SyncCardStates, SyncContext, SyncStatus};

/// 自建应用上下文。一个进程共用同一份本地数据、索引和会话。
pub struct SelfhostContext {
    local: Arc<LocalContext>,
    sync: SyncContext,
}

impl SelfhostContext {
    /// 未注入有效部署认证的实例只运行单机功能。
    pub fn new(local: LocalContext) -> Self {
        Self::with_auth(local, Arc::new(DisabledSyncAuth))
    }

    /// 注入部署认证，构造本身不会启动同步或读取历史。
    pub fn with_auth(local: LocalContext, auth: Arc<dyn SyncAuthProvider>) -> Self {
        let local = Arc::new(local);
        Self { local: local.clone(), sync: SyncContext::new(local, auth) }
    }

    /// 获取下层单机应用资源，保持查询、设置和剪贴板行为共用。
    pub fn local(&self) -> &LocalContext {
        &self.local
    }

    /// 获取自建同步控制与状态。
    pub fn sync(&self) -> &SyncContext {
        &self.sync
    }

    /// 系统采集统一经此入口，启用时在本地保存事务内追加同步动作。
    pub async fn capture(
        &self, input: CaptureInput<'_>, encoder: &dyn TextEncoder,
    ) -> AppResult<Option<ClipRecord>> {
        self.sync.capture(input, encoder).await
    }
}
