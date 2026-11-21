//! 自建版在代码层面依赖并扩展完整单机版应用核心。
//! 此包不依赖商业账号、VIP、支付或具体部署认证。
pub use desktop_app_core;
pub use clip_sync_client;
use desktop_app_core::{LocalContext, clip_core::domain::clip::ClipRecord};
use clip_sync_client::{queue::AsyncQueue, lock::{GlobalSyncLock, create_global_sync_lock}};
use std::sync::Arc;
use clip_sync_client::auth::{DisabledSyncAuth, SyncAuthProvider};
pub mod deployment;

const DEFAULT_CLIP_RECORD_QUEUE_CAPACITY: usize = 1000;

pub struct SelfhostContext {
    local: LocalContext,
    sync: SyncContext,
}
impl SelfhostContext {
    pub fn new(local: LocalContext) -> Self {
        Self { local, sync: SyncContext::default() }
    }

    pub fn with_auth(local: LocalContext, auth: Arc<dyn SyncAuthProvider>) -> Self {
        Self { local, sync: SyncContext { auth, ..SyncContext::default() } }
    }

    pub fn local(&self) -> &LocalContext {
        &self.local
    }

    pub fn sync(&self) -> &SyncContext {
        &self.sync
    }
}

/// 云同步相关运行资源。
pub struct SyncContext {
    auth: Arc<dyn SyncAuthProvider>,
    /// 云同步全局互斥锁。
    ///
    /// 用来避免多个同步任务同时执行。
    /// 比如定时同步、手动触发同步、队列消费同步之间应该共享同一把锁。
    lock: GlobalSyncLock,

    /// 剪贴记录内存同步队列（后续迁移为持久化 Outbox）。
    ///
    /// 队列是同步领域的运行资源，而不是剪贴板监听器本身的状态。
    record_queue: AsyncQueue<ClipRecord>,
}

impl Default for SyncContext {
    /// 创建供全部云同步任务共享的全局互斥锁。
    fn default() -> Self {
        Self {
            auth: Arc::new(DisabledSyncAuth),
            lock: create_global_sync_lock(),
            record_queue: AsyncQueue::new(DEFAULT_CLIP_RECORD_QUEUE_CAPACITY),
        }
    }
}

impl SyncContext {
    pub fn auth(&self) -> &dyn SyncAuthProvider { self.auth.as_ref() }

    /// 获取同步任务共用的互斥锁。
    pub fn lock(&self) -> &GlobalSyncLock {
        &self.lock
    }

    /// 获取同步内存队列的共享句柄。
    pub fn record_queue(&self) -> AsyncQueue<ClipRecord> {
        self.record_queue.clone()
    }
}
