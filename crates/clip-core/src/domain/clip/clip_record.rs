use serde::{Deserialize, Serialize};
use super::ClipRecordResource;

/// 纯本地记录。自增 ID 只用于本机关联，内容标识独立负责去重。
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClipRecord {
    /// clip_record.id，本地自增主键，与跨设备内容身份无关。
    pub id: i64,
    /// 完整内容的版本化 SHA-256 唯一键，资源不通过它建立外键。
    pub content_key: String,
    /// text / image / files / directory。
    pub r#type: String,
    /// 文本的 AES-GCM 密文；非文本记录为 None。
    pub content: Option<String>,
    /// 首次采集时间，毫秒；重复采集保留原值。
    pub created_at: i64,
    /// 最近一次采集时间，毫秒。
    pub last_copied_at: i64,
    /// 事务内分配的本地排序序号。
    pub sort_order: i64,
    /// 0 / 1；数据库保证最多一条置顶记录。
    pub pinned: i32,
    /// 非 clip_record 表字段，由仓储按 record_id 加载的有序子实体。
    #[serde(default)]
    pub resources: Vec<ClipRecordResource>,
}

impl ClipRecord {
    /// 将数据库的 0/1 状态转成业务判断使用的布尔值。
    pub fn is_pinned(&self) -> bool { self.pinned == 1 }
    /// 拼接原始资源名供搜索使用，不使用缓存文件名或磁盘路径。
    pub fn search_names(&self) -> String {
        self.resources.iter().map(|resource| resource.display_name.as_str()).collect::<Vec<_>>().join("\n")
    }
}
