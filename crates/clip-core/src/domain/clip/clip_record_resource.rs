use serde::{Deserialize, Serialize};

/// clip_record_resource 表实体。资源隶属记录聚合，由同一仓储事务维护，
/// 不提供脱离所属记录的独立增删入口。
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ClipRecordResource {
    /// 本地自增主键；IPC 将其转换为十进制字符串。
    pub id: i64,
    /// clip_record.id 外键，删除记录时级联删除资源行。
    pub record_id: i64,
    /// 记录内从零开始的顺序；与 record_id 组合唯一。
    pub position: i32,
    /// 用户看到的原始文件名；不是文件定位或资源选择依据。
    pub display_name: String,
    /// image / file / directory。
    pub resource_kind: String,
    /// managed 为应用缓存，reference 为用户原件引用。
    pub storage_kind: String,
    /// managed 保存资源根目录内的相对路径；reference 保存绝对路径。
    pub path: String,
    /// 文件字节数；目录或缺失资源为 None。
    pub byte_size: Option<i64>,
    /// 完整文件的 SHA-256；目录或缺失旧资源为 None。
    pub content_hash: Option<String>,
}
