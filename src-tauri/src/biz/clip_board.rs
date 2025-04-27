enum ClipType {
    Text,
    Img,
    File,
}
struct ClipBoardInfo {
    id: String,
    // 类型
    r#type: ClipType,
    // 内容
    content: String,
    // 时间戳
    timestamp: i32,
}

impl ClipBoardInfo {}

fn sync_clip_board_data() {}
