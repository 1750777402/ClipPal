use rbatis::crud;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: String,
    // 时间戳
    pub created: u32,
    // 用户id
    pub user_id: i32,
}

crud!(ClipRecord {}, "clip_record");
