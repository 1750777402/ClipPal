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
            let mut resource_files_to_delete: Vec<String> = vec![];
            let mut del_ids: Vec<String> = vec![];

            for record in clip_records {
                // 收集需要删除的resources目录下的文件
                collect_resource_files_to_delete(&record, &mut resource_files_to_delete);
                del_ids.push(record.id);
            }

            let del_res = ClipRecord::tombstone_by_ids(rb, &del_ids).await;
            match del_res {
                Ok(_) => {
                    log::info!("删除超限数据成功, 数量: {}", del_ids.len());
                    // 同步删除搜索索引
                    let _ = remove_ids_from_index(&del_ids).await;

                    // 删除resources目录下的文件
                    delete_resource_files(&resource_files_to_delete).await;
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
                    let mut resource_files_to_delete: Vec<String> = vec![];
                    let mut del_ids: Vec<String> = vec![];

                    for record in data {
                        // 收集需要删除的resources目录下的文件
                        collect_resource_files_to_delete(&record, &mut resource_files_to_delete);
                        del_ids.push(record.id);
                    }

                    let del_res = ClipRecord::del_by_ids(rb, &del_ids).await;
                    match del_res {
                        Ok(_) => {
                            log::info!("物理删除数据成功, 数量: {}", del_ids.len());
                            // 同步删除搜索索引
                            let _ = remove_ids_from_index(&del_ids).await;

                            // 删除resources目录下的文件
                            delete_resource_files(&resource_files_to_delete).await;
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

/// 收集需要删除的resources目录下的文件
fn collect_resource_files_to_delete(record: &ClipRecord, resource_files: &mut Vec<String>) {
    let content_str = record.content.as_str().unwrap_or_default();

    if content_str.is_empty() || content_str == "null" {
        return;
    }

    match record.r#type.as_str() {
        x if x == ClipType::Image.to_string() => {
            // 图片文件都存储在resources根目录下，直接添加
            resource_files.push(content_str.to_string());
        }
        x if x == ClipType::File.to_string() => {
            // 文件类型需要判断是否为相对路径（resources中的文件）
            if content_str.starts_with("files/") {
                // 这是复制到resources/files/下的文件，需要删除
                resource_files.push(content_str.to_string());
            } else if content_str.contains(":::") {
                // 多文件不删除（原本就是绝对路径）
                log::debug!("跳过多文件记录的文件删除: {}", content_str);
            } else {
                // 单文件绝对路径，不删除用户原文件
                log::debug!("跳过绝对路径文件的删除: {}", content_str);
            }
        }
        _ => {
            // 文本类型等其他类型无需删除文件
        }
    }
}

/// 删除resources目录下的文件
async fn delete_resource_files(resource_files: &[String]) {
    if resource_files.is_empty() {
        return;
    }

    let base_path = get_resources_dir();
    if let Some(resource_path) = base_path {
        for relative_path in resource_files {
            let full_path = resource_path.join(relative_path);

            if full_path.exists() {
                match std::fs::remove_file(&full_path) {
                    Ok(_) => {
                        log::debug!("删除文件成功: {:?}", full_path);
                    }
                    Err(e) => {
                        let safe_path = to_safe_string(&full_path);
                        log::error!("删除文件失败: {}, 路径: {}", e, safe_path);
                    }
                }
            } else {
                log::debug!("文件已不存在，跳过删除: {:?}", full_path);
            }
        }

        log::info!(
            "完成resources目录文件清理，处理了 {} 个文件",
            resource_files.len()
        );
    } else {
        log::error!("无法获取resources目录路径，跳过文件删除");
    }
}
