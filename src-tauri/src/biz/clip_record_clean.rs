use clipboard_listener::ClipType;
use rbatis::RBatis;

use crate::{CONTEXT, biz::clip_record::ClipRecord};

pub async fn clip_record_clean() {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let count = ClipRecord::count(rb).await;
    if count > 200 {
        // 获取200条之后的数据
        let clip_records = ClipRecord::select_order_by_limit(rb, -1, 200)
            .await
            .unwrap_or(vec![]);
        if clip_records.len() > 0 {
            let mut img_path_arr: Vec<String> = vec![];
            let mut del_ids: Vec<String> = vec![];
            for record in clip_records {
                if record.r#type == ClipType::Image.to_string() {
                    img_path_arr.push(record.content);
                }
                del_ids.push(record.id);
            }
            ClipRecord::del_by_ids(rb, del_ids)
                .await
                .unwrap_or_else(|e| println!("删除过期数据异常:{}", e));
        }
    }
}
