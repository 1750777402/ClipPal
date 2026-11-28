//! 自建同步契约。协议只传内容身份和动作，不传客户端数据库主键。
use serde::{Deserialize, Serialize};

/// 每批或每页的条数上限。正文还受字节数约束，大批历史需要分轮处理。
pub const BATCH_LIMIT: usize = 100;
/// 未配置额外限制时，单条文本原文或单个附件允许 5 MiB。
pub const DEFAULT_RECORD_BYTES: usize = 5 * 1024 * 1024;
/// AES-GCM 的 12 字节 nonce、16 字节认证标签与 Base64 的传输开销。
pub const TEXT_LIMIT: usize = (DEFAULT_RECORD_BYTES + 28).div_ceil(3) * 4;
/// 上传批次的总密文字节预算，HTTP 层另有完整响应体限制。
pub const BATCH_BYTES: usize = 8 * 1024 * 1024;

/// 部署公布的同步限制和附件能力。
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SyncLimits {
    pub max_record_bytes: usize,
    pub max_records: Option<u32>,
    pub attachments: bool,
}

impl Default for SyncLimits {
    fn default() -> Self {
        Self { max_record_bytes: DEFAULT_RECORD_BYTES, max_records: None, attachments: false }
    }
}

/// 加密后存入云端 content 的附件描述。对象引用在部署范围内长期有效。
#[derive(Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub object_id: String,
    pub name: String,
    pub sha256: String,
    pub size: usize,
}

/// 短期直传地址及必须随请求发送的签名头。
#[derive(Serialize, Deserialize)]
pub struct AttachmentUrl {
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
}

/// 一次可重试的本机动作。同一操作 ID 始终对应相同的动作、内容键和创建时间。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    pub operation_id: String,
    pub content_key: String,
    pub created_at: i64,
    #[serde(flatten)]
    pub action: Action,
}

/// 新增携带加密内容，删除只携带操作元数据和内容键。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Upsert { ciphertext: String },
    Delete,
}

/// 一个批次必须按数组顺序在服务端提交。
#[derive(Serialize, Deserialize)]
pub struct UploadRequest {
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
    pub after: i64,
    pub limit: usize,
}

/// 一页已提交变更。客户端从最后一条实际落库的动作推导新游标。
#[derive(Serialize, Deserialize)]
pub struct PullPage {
    pub changes: Vec<Change>,
    pub has_more: bool,
}
