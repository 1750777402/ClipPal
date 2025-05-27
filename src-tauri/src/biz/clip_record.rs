use rbatis::crud;
use serde::{Deserialize, Serialize};

use super::clip_board_sync::ClipType;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecord {
    id: i32,
    // 类型
    r#type: ClipType,
    // 内容
    content: String,
    // 文件内容
    file: Vec<u8>,
    // 时间戳
    created: i32,
    // 用户id
    user_id: i32,
}

crud!(ClipRecord {}, "clip_record");
