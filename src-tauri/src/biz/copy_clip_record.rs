use clipboard_listener::ClipType;

use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    auto_paste,
    biz::{
        clip_async_queue::AsyncQueue,
        clip_record::ClipRecord,
        content_processor::ContentProcessor,
        content_search::remove_ids_from_index,
        system_setting::{check_cloud_sync_enabled, Settings},
    },
    utils::{
        aes_util::decrypt_content,
        lock_utils::lock_utils::safe_read_lock,
        path_utils::{generate_file_not_found_error, str_to_safe_string},
    },
    window::{WindowHideFlag, WindowHideGuard},
    CONTEXT,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopyClipRecord {
    pub record_id: String,
}

#[tauri::command]
pub async fn copy_clip_record(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data[0].clone(),
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            let content = match decrypt_content(
                ContentProcessor::process_text_content(record.content).as_str(),
            ) {
                Ok(text) => text,
                Err(e) => {
                    log::error!("解密文本内容失败: {}", e);
                    return Err("文本解密失败".to_string());
                }
            };
            let _ = clipboard.write_text(content);
        }
        ClipType::Image => {
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::utils::file_dir::get_resources_dir() {
                    let abs_path = base_path.join(path);
                    if !abs_path.exists() {
                        return Err("图片资源不存在，无法复制".to_string());
                    }
                    if let Ok(img_bytes) = std::fs::read(abs_path) {
                        let _ = clipboard.write_image_binary(img_bytes);
                    } else {
                        return Err("图片资源读取失败，无法复制".to_string());
                    }
                } else {
                    return Err("资源目录获取失败".to_string());
                }
            } else {
                return Err("图片路径无效".to_string());
            }
        }
        ClipType::File => {
            // 获取显示名称和实际路径
            let display_names = record.content.as_str().unwrap_or("");
            let actual_paths = record.local_file_path.as_deref().unwrap_or("");

            if display_names.is_empty() || actual_paths.is_empty() {
                return Err("文件信息无效".to_string());
            }

            let display_list: Vec<String> =
                display_names.split(":::").map(|s| s.to_string()).collect();
            let actual_list: Vec<String> =
                actual_paths.split(":::").map(|s| s.to_string()).collect();

            // 检查文件是否存在
            let mut not_found: Vec<String> = vec![];
            for (i, actual_path) in actual_list.iter().enumerate() {
                let actual_path = actual_path.trim();
                if actual_path.is_empty() {
                    continue;
                }
                if !std::path::Path::new(actual_path).exists() {
                    let display_name = display_list
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| actual_path.to_string());
                    not_found.push(display_name);
                }
            }
            if !not_found.is_empty() {
                return Err(generate_file_not_found_error(&not_found));
            }

            // 创建临时文件链接以使用正确的文件名
            match create_temp_files_with_correct_names(&display_list, &actual_list).await {
                Ok(temp_files) => {
                    let _ = clipboard.write_files_uris(temp_files);
                }
                Err(e) => {
                    log::warn!("创建临时文件失败，使用原始路径: {}", e);
                    // 回退到使用原始路径
                    let _ = clipboard.write_files_uris(actual_list);
                }
            }
        }
        _ => {}
    }

    // 检查是否启用自动粘贴功能
    let auto_paste_enabled = {
        let settings_lock = CONTEXT.get::<Arc<RwLock<Settings>>>();
        match safe_read_lock(&settings_lock) {
            Ok(settings) => settings.auto_paste == 1,
            Err(e) => {
                log::warn!("无法获取设置: {}", e);
                false // 如果无法获取设置，默认不启用自动粘贴
            }
        }
    };

    // 只有在启用自动粘贴时才执行
    if auto_paste_enabled {
        // 使用异步任务避免阻塞主线程
        tokio::spawn(async {
            // 等待一小段时间确保剪贴板内容已经更新
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // 尝试自动粘贴到之前获得焦点的窗口
            if let Err(e) = auto_paste::auto_paste_to_previous_window() {
                log::warn!("自动粘贴失败: {}", e);
                // 自动粘贴失败不影响复制功能，只记录警告日志
            }
        });
    }

    Ok(String::new())
}

/// 只复制到剪贴板，不触发自动粘贴功能
#[tauri::command]
pub async fn copy_clip_record_no_paste(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data[0].clone(),
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            let content = match decrypt_content(
                ContentProcessor::process_text_content(record.content).as_str(),
            ) {
                Ok(text) => text,
                Err(e) => {
                    log::error!("解密文本内容失败: {}", e);
                    return Err("文本解密失败".to_string());
                }
            };
            let _ = clipboard.write_text(content);
        }
        ClipType::Image => {
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::utils::file_dir::get_resources_dir() {
                    let abs_path = base_path.join(path);
                    if !abs_path.exists() {
                        return Err("图片资源不存在，无法复制".to_string());
                    }
                    if let Ok(img_bytes) = std::fs::read(abs_path) {
                        let _ = clipboard.write_image_binary(img_bytes);
                    } else {
                        return Err("图片资源读取失败，无法复制".to_string());
                    }
                } else {
                    return Err("资源目录获取失败".to_string());
                }
            } else {
                return Err("图片路径无效".to_string());
            }
        }
        ClipType::File => {
            // 获取显示名称和实际路径
            let display_names = record.content.as_str().unwrap_or("");
            let actual_paths = record.local_file_path.as_deref().unwrap_or("");

            if display_names.is_empty() || actual_paths.is_empty() {
                return Err("文件信息无效".to_string());
            }

            let display_list: Vec<String> =
                display_names.split(":::").map(|s| s.to_string()).collect();
            let actual_list: Vec<String> =
                actual_paths.split(":::").map(|s| s.to_string()).collect();

            // 检查文件是否存在
            let mut not_found: Vec<String> = vec![];
            for (i, actual_path) in actual_list.iter().enumerate() {
                let actual_path = actual_path.trim();
                if actual_path.is_empty() {
                    continue;
                }
                if !std::path::Path::new(actual_path).exists() {
                    let display_name = display_list
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| actual_path.to_string());
                    not_found.push(display_name);
                }
            }
            if !not_found.is_empty() {
                return Err(generate_file_not_found_error(&not_found));
            }

            // 创建临时文件链接以使用正确的文件名
            match create_temp_files_with_correct_names(&display_list, &actual_list).await {
                Ok(temp_files) => {
                    let _ = clipboard.write_files_uris(temp_files);
                }
                Err(e) => {
                    log::warn!("创建临时文件失败，使用原始路径: {}", e);
                    // 回退到使用原始路径
                    let _ = clipboard.write_files_uris(actual_list);
                }
            }
        }
        _ => {}
    }

    // 注意：这个函数不执行自动粘贴功能
    log::debug!("仅复制到剪贴板，不触发自动粘贴");
    Ok(String::new())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PinnedClipRecord {
    pub record_id: String,
    pub pinned_flag: i32,
}

#[tauri::command]
pub async fn set_pinned(param: PinnedClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let _ = ClipRecord::update_pinned(rb, &param.record_id, param.pinned_flag).await;
    Ok(String::new())
}

/// 删除一条记录
#[tauri::command]
pub async fn del_record(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let ids = vec![param.record_id.clone()];

    let record_result = ClipRecord::select_by_id(rb, &param.record_id).await;
    match record_result {
        Ok(records) => {
            if !records.is_empty() {
                // 逻辑删除 并标记为待同步状态
                let res = ClipRecord::update_del_by_ids(rb, &ids).await;
                if let Ok(_) = res {
                    // 如果有删除记录，发送到异步队列   前提是开启了云同步开关
                    if check_cloud_sync_enabled().await {
                        let async_queue = CONTEXT.get::<AsyncQueue<ClipRecord>>();
                        if !async_queue.is_full() {
                            let send_res = async_queue.send_delete(records[0].clone()).await;
                            if let Err(e) = send_res {
                                log::error!(
                                    "异步队列发送失败，删除的粘贴内容：{:?}, 异常:{}",
                                    records[0],
                                    e
                                );
                            }
                        }
                    }
                    // 异步从搜索索引中移除记录
                    tokio::spawn(async move {
                        if let Err(e) = remove_ids_from_index(&ids).await {
                            log::error!("从搜索索引删除记录失败: {}", e);
                        }
                    });
                }
            }
            return Ok(String::new());
        }
        Err(_) => return Err("未找到该记录".to_string()),
    };
}

#[tauri::command]
pub async fn image_save_as(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record_res = ClipRecord::select_by_id(rb, param.record_id.as_str()).await;
    match record_res {
        Ok(records) => {
            let record = records.first().ok_or("未找到指定的剪贴板记录")?;
            if record.r#type != ClipType::Image.to_string() {
                return Err("仅支持图片类型另存为".to_string());
            }
            let rel_path = record.content.as_str().ok_or("图片路径无效")?;
            let base_path =
                crate::utils::file_dir::get_resources_dir().ok_or("资源目录获取失败")?;
            let abs_path = base_path.join(rel_path);
            if !abs_path.exists() {
                return Err("图片资源丢失".to_string());
            }

            let window_hide_flag = CONTEXT.get::<WindowHideFlag>();
            // 用Arc包裹WindowHideGuard，延长生命周期到回调闭包
            let guard = Arc::new(WindowHideGuard::new(window_hide_flag));
            let app_handle = CONTEXT.get::<AppHandle>();
            let abs_path_clone = abs_path.clone();
            let guard_clone = guard.clone();
            app_handle
                .dialog()
                .file()
                .add_filter("图片", &["png"])
                .set_file_name(format!("clip_{}", record.id))
                .save_file(move |file_path| {
                    // guard_clone在闭包内，作用域结束时自动drop，恢复窗口可隐藏
                    let _guard = guard_clone;
                    if let Some(select_path) = file_path {
                        let select_path = select_path.as_path();
                        if let Some(select_path) = select_path {
                            if let Err(e) = std::fs::copy(&abs_path_clone, &select_path) {
                                let source_path = abs_path_clone.to_string_lossy();
                                let dest_path = select_path.to_string_lossy();
                                log::error!(
                                    "复制图片失败: {}, 源文件: {}, 目标文件: {}",
                                    e,
                                    source_path,
                                    dest_path
                                );
                            }
                        }
                    }
                });
            Ok("图片已成功保存".to_string())
        }
        Err(_) => Err("未找到该记录".to_string()),
    }
}

/// 复制单个文件
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopySingleFileRecord {
    pub record_id: String,
    pub file_path: String,
}

#[tauri::command]
pub async fn copy_single_file(param: CopySingleFileRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data.get(0).cloned().ok_or("记录不存在".to_string())?,
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    // 只处理文件类型
    if record.r#type != ClipType::File.to_string() {
        return Err("只支持文件类型的单个文件复制".to_string());
    }

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();

    // 获取显示名称列表和实际路径列表
    let display_names = record.content.as_str().unwrap_or("");
    let actual_paths = record.local_file_path.as_deref().unwrap_or("");

    if display_names.is_empty() || actual_paths.is_empty() {
        return Err("文件信息无效".to_string());
    }

    let display_list: Vec<String> = display_names.split(":::").map(|s| s.to_string()).collect();
    let actual_list: Vec<String> = actual_paths.split(":::").map(|s| s.to_string()).collect();

    // 验证指定的显示名称是否在记录中，并找到对应的实际路径
    let file_index = display_list
        .iter()
        .position(|name| name == &param.file_path);
    let actual_file_path = match file_index {
        Some(index) if index < actual_list.len() => &actual_list[index],
        _ => return Err("指定的文件不在此记录中".to_string()),
    };

    // 检查实际文件是否存在
    if !std::path::Path::new(actual_file_path).exists() {
        let safe_path = str_to_safe_string(&param.file_path);
        return Err(format!("文件不存在: {}", safe_path));
    }

    // 创建临时文件使用正确的文件名
    match create_temp_files_with_correct_names(
        &[param.file_path.clone()],
        &[actual_file_path.clone()],
    )
    .await
    {
        Ok(temp_files) => {
            let _ = clipboard.write_files_uris(temp_files);
        }
        Err(e) => {
            log::warn!("创建临时文件失败，使用原始路径: {}", e);
            // 回退到使用原始路径
            let _ = clipboard.write_files_uris(vec![actual_file_path.clone()]);
        }
    }

    log::debug!("已复制单个文件到剪贴板");
    Ok(String::new())
}

/// 创建临时文件，使用正确的文件名，以便粘贴时显示用户期望的文件名
async fn create_temp_files_with_correct_names(
    display_names: &[String],
    actual_paths: &[String],
) -> Result<Vec<String>, String> {
    use std::path::Path;

    if display_names.len() != actual_paths.len() {
        return Err("显示名称和实际路径数量不匹配".to_string());
    }

    let temp_dir = std::env::temp_dir().join("clip_pal_temp");

    // 创建临时目录
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        return Err(format!("创建临时目录失败: {}", e));
    }

    let mut temp_file_paths = Vec::new();

    for (display_name, actual_path) in display_names.iter().zip(actual_paths.iter()) {
        let actual_path = actual_path.trim();
        let display_name = display_name.trim();

        if actual_path.is_empty() || display_name.is_empty() {
            continue;
        }

        let source_path = Path::new(actual_path);
        if !source_path.exists() {
            return Err(format!("源文件不存在: {}", actual_path));
        }

        // 在临时目录中创建目标文件路径，使用显示名称
        let temp_file_path = temp_dir.join(display_name);

        // 如果临时文件已存在，先删除它
        if temp_file_path.exists() {
            if let Err(e) = std::fs::remove_file(&temp_file_path) {
                log::warn!(
                    "删除已存在的临时文件失败: {:?}, 错误: {}",
                    temp_file_path,
                    e
                );
            }
        }

        // 创建硬链接（Windows和Unix都支持）
        match std::fs::hard_link(source_path, &temp_file_path) {
            Ok(_) => {
                log::debug!("创建硬链接成功: {:?} -> {:?}", source_path, temp_file_path);
                temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
            }
            Err(e) => {
                log::warn!("创建硬链接失败: {}, 尝试复制文件", e);
                // 硬链接失败时，复制文件（适用于跨文件系统的情况）
                match std::fs::copy(source_path, &temp_file_path) {
                    Ok(_) => {
                        log::debug!(
                            "复制临时文件成功: {:?} -> {:?}",
                            source_path,
                            temp_file_path
                        );
                        temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
                    }
                    Err(e) => {
                        return Err(format!("创建临时文件失败: {}", e));
                    }
                }
            }
        }
    }

    if temp_file_paths.is_empty() {
        return Err("没有创建任何临时文件".to_string());
    }

    // 启动后台任务清理临时文件（延迟清理以确保文件复制操作完成）
    let temp_dir_for_cleanup = temp_dir.clone();
    tokio::spawn(async move {
        // 等待一段时间，确保文件操作完成
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        if let Err(e) = cleanup_temp_files(&temp_dir_for_cleanup).await {
            log::warn!("清理临时文件失败: {}", e);
        }
    });

    Ok(temp_file_paths)
}

/// 清理临时文件
async fn cleanup_temp_files(temp_dir: &std::path::Path) -> Result<(), String> {
    if !temp_dir.exists() {
        return Ok(());
    }

    match std::fs::read_dir(temp_dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        if let Err(e) = std::fs::remove_file(&path) {
                            log::debug!("删除临时文件失败: {:?}, 错误: {}", path, e);
                        } else {
                            log::debug!("删除临时文件成功: {:?}", path);
                        }
                    }
                }
            }

            // 尝试删除临时目录（只有在空的情况下才会成功）
            if let Err(e) = std::fs::remove_dir(temp_dir) {
                log::debug!("删除临时目录失败: {:?}, 错误: {}", temp_dir, e);
            } else {
                log::debug!("删除临时目录成功: {:?}", temp_dir);
            }
        }
        Err(e) => {
            return Err(format!("读取临时目录失败: {}", e));
        }
    }

    Ok(())
}
