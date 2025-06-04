use rbatis::RBatis;

use crate::{CONTEXT, biz::clip_record::ClipRecord};

#[tauri::command]
pub async fn get_clip_records() -> Vec<ClipRecord> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let all_data = ClipRecord::select_all(rb).await;
    all_data.unwrap()
}
