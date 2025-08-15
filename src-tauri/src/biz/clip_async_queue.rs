#![allow(dead_code)]

use async_channel::{Receiver, Sender, TryRecvError, bounded};
use rbatis::RBatis;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{ClipRecordParam, SingleCloudSyncParam, sync_single_clip_record};
use crate::biz::clip_record::{ClipRecord, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::errors::{AppError, AppResult};
use crate::utils::config::get_max_file_size_bytes;
use crate::utils::file_dir::get_resources_dir;
use crate::utils::lock_utils::GlobalSyncLock;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum QueueEvent<T> {
    Add(T),
    Delete(T),
}

#[derive(Clone)]
pub struct AsyncQueue<T> {
    sender: Arc<Sender<QueueEvent<T>>>,
    receiver: Arc<Receiver<QueueEvent<T>>>,
}

impl<T: Clone + Send + 'static> AsyncQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        AsyncQueue {
            sender: Arc::new(sender),
            receiver: Arc::new(receiver),
        }
    }

    pub async fn send_add(&self, item: T) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Add(item)).await
    }

    pub async fn send_delete(
        &self,
        item: T,
    ) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Delete(item)).await
    }

    pub async fn recv(&self) -> Result<QueueEvent<T>, async_channel::RecvError> {
        self.receiver.recv().await
    }

    pub fn try_recv(&self) -> Result<QueueEvent<T>, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn capacity(&self) -> Option<usize> {
        self.sender.capacity()
    }

    pub fn len(&self) -> usize {
        self.sender.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.sender.is_full()
    }
}

pub fn consume_clip_record_queue(queue: AsyncQueue<ClipRecord>) {
    task::spawn(async move {
        let sync_lock: &GlobalSyncLock = CONTEXT.get::<GlobalSyncLock>();

        loop {
            // 先尝试拿锁，拿不到就等待一会儿再重试
            if let Some(_guard) = sync_lock.try_lock() {
                log::debug!("获取到同步锁，开始处理队列数据...");

                // 循环接收并处理队列数据
                loop {
                    match queue.try_recv() {
                        Ok(event) => {
                            // 处理数据
                            match event {
                                QueueEvent::Add(item) => {
                                    let param = SingleCloudSyncParam {
                                        r#type: 1,
                                        clip: item.clone().into(),
                                    };
                                    let res = handle_sync_inner(param).await;
                                    if let Ok(_) = res {
                                        // 新增的数据需要通知前端同步完成
                                        notify_frontend_sync_status(vec![item.id]).await;
                                    }
                                }
                                QueueEvent::Delete(item) => {
                                    let param = SingleCloudSyncParam {
                                        r#type: 2,
                                        clip: item.clone().into(),
                                    };
                                    let _ = handle_sync_inner(param).await;
                                }
                            };
                        }
                        Err(TryRecvError::Empty) => {
                            // 队列空了，跳出内层循环，释放锁
                            log::debug!("队列已空，释放锁，等待下一轮处理");
                            break;
                        }
                        Err(e) => {
                            log::error!("接收队列消息错误: {}", e);
                            break;
                        }
                    }
                }
            } else {
                // 锁被占用，短暂休眠避免忙等
                log::debug!("同步锁被占用，等待后重试...");
            }
            sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn handle_sync_inner(param: SingleCloudSyncParam) -> AppResult<()> {
    let res = sync_single_clip_record(&param).await;
    log::info!(
        "同步单个剪贴板记录，粘贴记录：{:?}，结果：{:?}",
        param.clip.md5_str,
        res
    );
    match res {
        Ok(response) => {
            if let Some(success) = response {
                let record_id = param.clip.id.clone().unwrap_or_default();
                let ids = vec![record_id.clone()];
                let rb: &RBatis = CONTEXT.get::<RBatis>();

                // 检查记录类型，决定同步策略
                let record_type = param.clip.r#type.clone().unwrap_or_default();
                match record_type.as_str() {
                    "Image" => {
                        handle_file_type_sync(
                            rb,
                            &ids,
                            &record_id,
                            success.timestamp,
                            check_file_size_for_image(&param.clip).await,
                            "图片",
                        )
                        .await
                    }
                    "File" => {
                        handle_file_type_sync(
                            rb,
                            &ids,
                            &record_id,
                            success.timestamp,
                            check_file_size_for_files(&param.clip).await,
                            "文件",
                        )
                        .await
                    }
                    _ => {
                        // 文本类型：直接标记为SYNCHRONIZED
                        update_sync_flag_with_log(
                            rb,
                            &ids,
                            SYNCHRONIZED,
                            success.timestamp,
                            &record_id,
                            "文本记录已同步",
                            "同步单个剪贴板记录失败",
                        )
                        .await
                    }
                }
            } else {
                Err(AppError::General("同步单个剪贴板记录失败".to_string()))
            }
        }
        Err(e) => {
            log::error!(
                "同步单个剪贴板记录失败，粘贴记录：{:?}，错误：{}",
                param.clip.id,
                e
            );
            Err(AppError::General("同步单个剪贴板记录失败".to_string()))
        }
    }
}

async fn notify_frontend_sync_status(ids: Vec<String>) {
    let payload = serde_json::json!({
        "clip_ids": ids,
        "sync_flag": SYNCHRONIZED
    });
    let app_handle = CONTEXT.get::<AppHandle>();
    let _ = app_handle
        .emit("sync_status_update_batch", payload)
        .map_err(|e| AppError::General(format!("批量通知前端失败: {}", e)));
}

/// 处理文件类型同步（图片和文件共用）
async fn handle_file_type_sync(
    rb: &RBatis,
    ids: &Vec<String>,
    record_id: &str,
    timestamp: u64,
    size_check_result: Result<(), String>,
    file_type: &str,
) -> AppResult<()> {
    match size_check_result {
        Err(_) => {
            // 文件大小超过限制，直接标记为SKIP_SYNC
            update_sync_flag_with_log(
                rb,
                ids,
                SKIP_SYNC,
                timestamp,
                record_id,
                &format!("{}文件大小超过限制，标记为跳过同步", file_type),
                &format!("标记{}记录跳过同步失败", file_type),
            )
            .await
        }
        Ok(_) => {
            // 文件大小正常，标记为SYNCHRONIZING
            update_sync_flag_with_log(
                rb,
                ids,
                SYNCHRONIZING,
                timestamp,
                record_id,
                &format!("{}记录标记为同步中，等待文件上传队列处理", file_type),
                &format!("同步{}记录失败", file_type),
            )
            .await
        }
    }
}

/// 更新同步状态并记录日志
async fn update_sync_flag_with_log(
    rb: &RBatis,
    ids: &Vec<String>,
    sync_flag: i32,
    timestamp: u64,
    record_id: &str,
    success_msg: &str,
    error_msg: &str,
) -> AppResult<()> {
    let update_res = ClipRecord::update_sync_flag(rb, ids, sync_flag, timestamp).await;
    match update_res {
        Ok(_) => {
            log::info!("{}, 记录ID: {}", success_msg, record_id);
            Ok(())
        }
        Err(e) => {
            log::error!("{}: {}", error_msg, e);
            Err(AppError::General(error_msg.to_string()))
        }
    }
}

/// 检查图片文件大小是否超过限制
async fn check_file_size_for_image(clip: &ClipRecordParam) -> Result<(), String> {
    if let Some(content_str) = clip.content.as_str() {
        if content_str.is_empty() || content_str == "null" {
            return Ok(()); // 无内容，不检查大小
        }

        // 构造图片文件路径
        if let Some(resource_path) = get_resources_dir() {
            let mut file_path = resource_path.clone();
            file_path.push(content_str);

            if file_path.exists() {
                check_single_file_size(&file_path)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

/// 检查文件大小是否超过限制
async fn check_file_size_for_files(clip: &ClipRecordParam) -> Result<(), String> {
    if let Some(content_str) = clip.content.as_str() {
        let file_paths: Vec<String> = content_str.split(":::").map(|s| s.to_string()).collect();

        for file_path_str in &file_paths {
            let file_path = PathBuf::from(file_path_str);
            if file_path.exists() {
                if let Err(e) = check_single_file_size(&file_path) {
                    return Err(format!("文件 {}: {}", file_path_str, e));
                }
            }
        }
    }
    Ok(())
}

/// 检查单个文件大小是否超过限制
fn check_single_file_size(file_path: &PathBuf) -> Result<(), String> {
    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);
            if metadata.len() > max_file_size {
                Err(format!(
                    "文件大小 {} 字节超过限制 {} 字节",
                    metadata.len(),
                    max_file_size
                ))
            } else {
                Ok(())
            }
        }
        Err(e) => Err(format!("读取文件元数据失败: {}", e)),
    }
}
