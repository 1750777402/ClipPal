use clipboard_listener::ClipType;
use rbatis::RBatis;

use crate::{CONTEXT, biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir};

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
                    img_path_arr.push(record.content.as_str().unwrap_or_default().to_string());
                }
                del_ids.push(record.id);
            }
            let del_res = ClipRecord::del_by_ids(rb, del_ids).await;
            match del_res {
                Ok(_) => {
                    if img_path_arr.len() > 0 {
                        let base_path = get_resources_dir();
                        if let Some(resource_path) = base_path {
                            // 删除图片
                            for path in img_path_arr {
                                let full_path = resource_path.join(path);
                                println!("删除图片失败图片路径:{:?}", full_path.as_os_str());
                                std::fs::remove_file(full_path).unwrap_or_else(|e| {
                                    println!("删除图片失败:{}", e);
                                })
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("删除过期数据异常:{}", e)
                }
            }
        }
    }
}
