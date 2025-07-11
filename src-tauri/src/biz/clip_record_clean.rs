use std::sync::{Arc, Mutex};

use clipboard_listener::ClipType;
use rbatis::RBatis;

use crate::{
    CONTEXT,
    biz::{
        clip_record::ClipRecord, system_setting::Settings, content_search::remove_ids_from_index,
    },
    errors::lock_utils::safe_lock,
    utils::{file_dir::get_resources_dir, path_utils::to_safe_string},
};

pub async fn clip_record_clean() {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let count = ClipRecord::count(rb).await;
    let system_settings = {
        let lock = CONTEXT.get::<Arc<Mutex<Settings>>>().clone();
        let result = match safe_lock(&lock) {
            Ok(current) => current.clone(),
            Err(e) => {
                log::error!("获取系统设置锁失败: {}", e);
                return;
            }
        };
        result
    };
    let max_num = system_settings.max_records;
    if count > max_num as i64 {
        let clip_records = ClipRecord::select_order_by_limit(rb, -1, max_num as i32)
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
            let del_res = ClipRecord::del_by_ids(rb, &del_ids).await;
            match del_res {
                Ok(_) => {
                    // 同步删除搜索索引
                    let _ = remove_ids_from_index(&del_ids).await;
                    if img_path_arr.len() > 0 {
                        let base_path = get_resources_dir();
                        if let Some(resource_path) = base_path {
                            // 删除图片
                            for path in img_path_arr {
                                let full_path = resource_path.join(path);
                                std::fs::remove_file(full_path.clone()).unwrap_or_else(|e| {
                                    let safe_path = to_safe_string(&full_path);
                                    log::error!("删除图片失败: {}, 路径: {}", e, safe_path);
                                })
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("删除过期数据异常:{}", e)
                }
            }
        }
    }
}
