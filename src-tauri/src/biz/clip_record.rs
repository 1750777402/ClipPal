use rbatis::crud;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecord {
    id: i32,
    // 类型
    r#type: String,
    // 内容
    content: String,
    // 时间戳
    created: i32,
    // 用户id
    user_id: i32,
}

crud!(ClipRecord {}, "clip_record");
