//! 可复用的同步客户端。SQLite、剪贴板和桌面 UI 由宿主注入。
mod http;
mod session;

pub use clip_sync_protocol::v2 as protocol;
pub use http::HttpsTransport;
pub use session::{SessionStatus, SyncSession, SyncTriggers};

use async_trait::async_trait;
use protocol::{Operation, PullPage, PullRequest, Scope, UploadRequest, UploadResponse};

/// 可重试错误按指数退避处理。协议错误会保留队列并在状态中报告。
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("{0}")]
    Retry(String),
    #[error("{0}")]
    Blocked(String),
}

/// 只公开连接成功和收到变更两种事件。SSE 载荷不参与游标计算。
#[derive(Clone, Copy)]
pub enum StreamEvent {
    Connected,
    Changed,
}

/// 传输端口也供测试替身使用，便于独立测试断网、重连和丢失确认。
#[async_trait]
pub trait SyncTransport: Send + Sync {
    /// 获取当前凭证对应的身份与数据集。
    async fn scope(&self) -> Result<Scope, SyncError>;
    /// 按请求顺序提交一个批次。
    async fn upload(&self, request: &UploadRequest) -> Result<UploadResponse, SyncError>;
    /// 返回指定游标之后的一页动作。
    async fn pull(&self, request: &PullRequest) -> Result<PullPage, SyncError>;
    /// 读取一次 SSE 连接直到断开，重连由会话管理。
    async fn listen(
        &self,
        scope: &Scope,
        notify: &(dyn Fn(StreamEvent) + Send + Sync),
    ) -> Result<(), SyncError>;
}

/// 已绑定到一个同步范围的本机存储。会话从存储读游标，不在内存推进游标。
#[async_trait]
pub trait SyncStore: Send + Sync {
    /// 获取下一批尚未确认的动作，遵守本机顺序和批次限制。
    async fn pending(&self) -> Result<Vec<Operation>, SyncError>;
    /// 收到完整确认后，按操作 ID 清理本批。
    async fn confirm(&self, operations: &[Operation]) -> Result<(), SyncError>;
    /// 获取已经落库的游标。
    async fn cursor(&self) -> Result<i64, SyncError>;
    /// 应用整页并提交游标，失败则整页回滚。
    async fn apply(&self, after: i64, page: PullPage) -> Result<(), SyncError>;
}
