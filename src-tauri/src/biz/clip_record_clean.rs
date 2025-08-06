use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    CONTEXT,
    biz::{
        clip_record::ClipRecord, content_search::remove_ids_from_index, system_setting::Settings,
    },
    utils::{
        file_dir::get_resources_dir, lock_utils::lock_utils::safe_read_lock,
        path_utils::to_safe_string,
    },
};
use clipboard_listener::ClipType;
use once_cell::sync::Lazy;
use rbatis::RBatis;

static IS_CLEANING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

// AtomicBool状态保护器
struct CleaningGuard;

impl Drop for CleaningGuard {
    fn drop(&mut self) {
        IS_CLEANING.store(false, Ordering::SeqCst);
    }
}

pub async fn try_clean_clip_record() {
    // 如果已有清理在运行，直接跳过
    if IS_CLEANING.swap(true, Ordering::SeqCst) {
        return;
    }

    // 创建自动清理状态的 Guard
    let _guard = CleaningGuard;

    // 执行清理逻辑
    clip_record_clean().await;
}

async fn clip_record_clean() {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    let system_settings = {
        let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
        let result = match safe_read_lock(&lock) {
            Ok(current) => current.clone(),
            Err(e) => {
                log::error!("获取系统设置锁失败: {}", e);
                return;
            }
        };
        result
    };
    let max_num = system_settings.max_records;

    // 数据清理有两个部分
    // 1. 逻辑删除超过系统设置的最大记录数的剪贴板记录，但是逻辑删除的数据需要标记为未同步，等待定时任务同步删除的数据
    // 2. 还有一部分数据就是已经同步并且被逻辑删除的数据，这部分数据可以直接物理删除

    // 查询页面会展示的有效数据数量
    let count = ClipRecord::count_effective(rb).await;
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
            let del_res = ClipRecord::tombstone_by_ids(rb, &del_ids).await;
            match del_res {
                Ok(_) => {
                    log::info!("删除超限数据成功, 数量: {}", del_ids.len());
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

    // 查询已同步并且已逻辑删除的数据数量   这些数据需要物理删除
    let invalid_count = ClipRecord::count_invalid(rb).await;
    if invalid_count > 0 {
        let invalid_data = ClipRecord::select_invalid(rb).await;
        match invalid_data {
            Ok(data) => {
                if data.len() > 0 {
                    let mut img_path_arr: Vec<String> = vec![];
                    let mut del_ids: Vec<String> = vec![];
                    for record in data {
                        if record.r#type == ClipType::Image.to_string() {
                            img_path_arr
                                .push(record.content.as_str().unwrap_or_default().to_string());
                        }
                        del_ids.push(record.id);
                    }
                    let del_res = ClipRecord::del_by_ids(rb, &del_ids).await;
                    match del_res {
                        Ok(_) => {
                            log::info!("删除超限数据成功, 数量: {}", del_ids.len());
                            // 同步删除搜索索引
                            let _ = remove_ids_from_index(&del_ids).await;
                            if img_path_arr.len() > 0 {
                                let base_path = get_resources_dir();
                                if let Some(resource_path) = base_path {
                                    // 删除图片
                                    for path in img_path_arr {
                                        let full_path = resource_path.join(path);
                                        std::fs::remove_file(full_path.clone()).unwrap_or_else(
                                            |e| {
                                                let safe_path = to_safe_string(&full_path);
                                                log::error!(
                                                    "删除图片失败: {}, 路径: {}",
                                                    e,
                                                    safe_path
                                                );
                                            },
                                        )
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("物理删除过期数据异常:{}", e)
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("查询待物理删除数据异常:{}", e)
            }
        }
    }
}
