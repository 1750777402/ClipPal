//! 自建文本同步契约。协议只传内容身份和动作，不传客户端数据库主键。
use serde::{Deserialize, Serialize};

/// 每批或每页的条数上限。正文还受字节数约束，大批历史需要分轮处理。
pub const BATCH_LIMIT: usize = 100;
/// 首期单条密文的 UTF-8 字节上限。超出上限的文本仍可保存到本地。
pub const TEXT_LIMIT: usize = 4 * 1024 * 1024;
/// 上传批次的总密文字节预算，HTTP 层另有完整响应体限制。
pub const BATCH_BYTES: usize = 8 * 1024 * 1024;

/// 由服务端对当前认证身份返回。恢复旧备份时必须更换数据集标识。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub scope_id: String,
    pub dataset_id: String,
}

impl Scope {
    /// 识别缺失或异常标识，避免创建无法安全复用的本机状态。
    pub fn is_valid(&self) -> bool {
        [&self.scope_id, &self.dataset_id].into_iter()
            .all(|id| !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_graphic()))
    }
}

/// 一次可重试的本机动作。同一操作 ID 始终对应同一份数据。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub operation_id: String,
    pub content_key: String,
    pub created_at: i64,
    #[serde(flatten)]
    pub action: Action,
}

/// 新增携带不变的加密快照，删除只携带操作元数据和内容键。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Upsert { ciphertext: String },
    Delete,
}

/// 一个批次必须按数组顺序在服务端提交。范围用于校验，服务端从凭证确定身份。
#[derive(Serialize, Deserialize)]
pub struct UploadRequest {
    pub scope: Scope,
    pub operations: Vec<Operation>,
}

/// 服务端为首次提交分配序号，重复请求返回原序号。
#[derive(Clone, Serialize, Deserialize)]
pub struct Ack {
    pub operation_id: String,
    pub sync_seq: i64,
}

/// 响应须确认整个批次，客户端收到完整确认后才会清理本地队列。
#[derive(Serialize, Deserialize)]
pub struct UploadResponse {
    pub scope: Scope,
    pub acknowledgements: Vec<Ack>,
}

/// 一条已提交的云端变更。提交时间仅用于本机展示。
#[derive(Clone, Serialize, Deserialize)]
pub struct Change {
    pub sync_seq: i64,
    pub committed_at: i64,
    pub content_key: String,
    #[serde(flatten)]
    pub action: Action,
}

/// 从 after 之后向前分页，序号允许留空，由服务端保证提交顺序。
#[derive(Serialize, Deserialize)]
pub struct PullRequest {
    #[serde(flatten)]
    pub scope: Scope,
    pub after: i64,
    pub limit: usize,
}

/// 一页已提交变更。客户端从最后一条实际落库的动作推导新游标。
#[derive(Serialize, Deserialize)]
pub struct PullPage {
    pub scope: Scope,
    pub changes: Vec<Change>,
    pub has_more: bool,
}
