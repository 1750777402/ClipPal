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
use clipboard_listener::ClipType;
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
    let record_id = param.clip.id.clone().unwrap_or_default();
    let record_type = param.clip.r#type.clone().unwrap_or_default();

    // 先检查文件类型是否应该跳过同步
    if should_skip_sync(&param.clip, &record_type).await {
        log::info!("记录 {} ({}) 不支持云同步", record_id, record_type);
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        return update_sync_status(rb, &record_id, SKIP_SYNC, 0).await;
    }

    // 执行实际同步
    match sync_single_clip_record(&param).await {
        Ok(Some(success)) => {
            let rb: &RBatis = CONTEXT.get::<RBatis>();
            let final_status = determine_final_sync_status(&record_type, &param.clip).await;

            update_sync_status(rb, &record_id, final_status, success.timestamp).await?;

            log::info!(
                "同步成功: 记录ID={}, 类型={}, 状态={}",
                record_id,
                record_type,
                sync_status_name(final_status)
            );
            Ok(())
        }
        Ok(None) => {
            log::error!("同步返回空结果: 记录ID={}", record_id);
            Err(AppError::General("同步返回空结果".to_string()))
        }
        Err(e) => {
            log::error!("同步失败: 记录ID={}, 错误={}", record_id, e);
            Err(AppError::General(format!("同步失败: {}", e)))
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

/// 判断记录是否应该跳过同步
async fn should_skip_sync(clip: &ClipRecordParam, record_type: &str) -> bool {
    match record_type {
        x if x == ClipType::Image.to_string() => check_file_size_for_image(clip).await.is_err(),
        x if x == ClipType::File.to_string() => check_file_size_for_files(clip).await.is_err(),
        _ => false, // 文本类型不跳过
    }
}

/// 确定最终的同步状态
async fn determine_final_sync_status(record_type: &str, _clip: &ClipRecordParam) -> i32 {
    match record_type {
        x if x == ClipType::Image.to_string() || x == ClipType::File.to_string() => {
            // 文件类型：同步成功后标记为SYNCHRONIZING，等待文件上传
            SYNCHRONIZING
        }
        _ => {
            // 文本类型：直接标记为SYNCHRONIZED
            SYNCHRONIZED
        }
    }
}

/// 更新同步状态
async fn update_sync_status(
    rb: &RBatis,
    record_id: &str,
    sync_flag: i32,
    timestamp: u64,
) -> AppResult<()> {
    let ids = vec![record_id.to_string()];
    ClipRecord::update_sync_flag(rb, &ids, sync_flag, timestamp)
        .await
        .map_err(|e| {
            log::error!("更新同步状态失败: 记录ID={}, 错误={}", record_id, e);
            e
        })
}

/// 获取同步状态的可读名称
fn sync_status_name(status: i32) -> &'static str {
    if status == SYNCHRONIZED {
        "已同步"
    } else if status == SYNCHRONIZING {
        "同步中"
    } else if status == SKIP_SYNC {
        "不支持同步"
    } else {
        "未知状态"
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
                Err(format!("图片文件不存在: {:?}", file_path))
            }
        } else {
            Err("无法获取resources目录".to_string())
        }
    } else {
        Ok(())
    }
}

/// 检查文件是否应该跳过云同步
/// 按照新的策略：
/// 1. 多文件 -> 跳过同步
/// 2. 单文件但超过大小限制 -> 跳过同步
/// 3. 单文件且在resources/files/下（相对路径） -> 允许同步
/// 4. 单文件但绝对路径 -> 跳过同步（向后兼容）
async fn check_file_size_for_files(clip: &ClipRecordParam) -> Result<(), String> {
    if let Some(content_str) = clip.content.as_str() {
        // 检查是否是多文件
        if content_str.contains(":::") {
            return Err("多文件不支持云同步".to_string());
        }

        // 单文件处理
        let file_path_str = content_str.trim();

        // 检查是否是相对路径（表示文件已复制到resources目录）
        if file_path_str.starts_with("files/") {
            // 这是已经复制到resources/files/的文件，检查大小
            if let Some(resource_path) = get_resources_dir() {
                let full_path = resource_path.join(file_path_str);
                if full_path.exists() {
                    check_single_file_size(&full_path)
                } else {
                    Err(format!("resources中的文件不存在: {:?}", full_path))
                }
            } else {
                Err("无法获取resources目录".to_string())
            }
        } else {
            // 绝对路径的文件，检查文件是否存在
            let absolute_path = std::path::PathBuf::from(file_path_str);
            if absolute_path.exists() {
                // 文件存在但是绝对路径，跳过同步（向后兼容）
                Err("绝对路径文件不支持云同步".to_string())
            } else {
                // 文件不存在，跳过同步
                Err(format!("绝对路径文件不存在: {:?}", absolute_path))
            }
        }
    } else {
        Ok(())
    }
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
