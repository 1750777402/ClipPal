#![allow(dead_code)]

use async_channel::{Receiver, Sender, TryRecvError, bounded};
use rbatis::RBatis;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{ClipRecordParam, SingleCloudSyncParam, sync_single_clip_record};
use crate::biz::clip_record::{ClipRecord, NOT_SYNCHRONIZED, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::errors::{AppError, AppResult};
use crate::biz::vip_checker::VipChecker;
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
                log::debug!("开始处理同步队列");

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
                                    let res = handle_sync_inner(param.clone()).await;
                                    if let Ok(final_status) = res {
                                        // 根据实际处理结果通知前端
                                        notify_frontend_sync_status_with_flag(
                                            vec![item.id],
                                            final_status,
                                        )
                                        .await;
                                    }
                                }
                                QueueEvent::Delete(item) => {
                                    let param = SingleCloudSyncParam {
                                        r#type: 2,
                                        clip: item.clone().into(),
                                    };
                                    let rb: &RBatis = CONTEXT.get::<RBatis>();
                                    let record = ClipRecord::select_by_id(rb, &item.id).await;
                                    match record {
                                        Ok(rec) => {
                                            if !rec.is_empty() && rec[0].del_flag == Some(0) {
                                                // 说明这个记录现在不是已删除状态了
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "同步已删除记录时，检查已删除记录状态出现异常：{}",
                                                e
                                            )
                                        }
                                    };
                                    let _ = handle_sync_inner(param).await;
                                }
                            };
                        }
                        Err(TryRecvError::Empty) => {
                            // 队列空了，跳出内层循环，释放锁
                            log::debug!("同步队列处理完成");
                            break;
                        }
                        Err(e) => {
                            log::error!("队列消息处理错误: {}", e);
                            break;
                        }
                    }
                }
            } else {
                // 锁被占用，短暂休眠避免忙等
                log::debug!("同步锁被占用，等待重试");
            }
            sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn handle_sync_inner(param: SingleCloudSyncParam) -> AppResult<i32> {
    let record_id = param.clip.id.clone().unwrap_or_default();
    let record_type = param.clip.r#type.clone().unwrap_or_default();

    // 先检查文件类型是否应该跳过同步（技术限制）
    if should_skip_sync(&param.clip, &record_type).await {
        log::debug!("记录 {} ({}) 不支持云同步", record_id, record_type);
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        update_sync_status(rb, &record_id, SKIP_SYNC, 0).await?;
        return Ok(SKIP_SYNC);
    }
    
    // 检查文件大小限制（所有用户都需要检查，根据VIP等级有不同限制）
    if record_type == "file" || record_type == "image" {
        // 获取文件大小
        let file_size = get_file_size_from_param(&param.clip).await;
        // 获取当前用户的文件大小限制（根据VIP等级）
        let max_file_size = VipChecker::get_cached_max_file_size().unwrap_or(0);
        
        if max_file_size == 0 {
            log::info!(
                "用户不支持文件同步，暂不同步: 记录ID={}, 类型={}",
                record_id, record_type
            );
            // 注意：不修改sync_flag，保持为0，等用户升级VIP后可以同步
            return Ok(NOT_SYNCHRONIZED);
        }
        
        if file_size > max_file_size {
            log::info!(
                "文件超过大小限制，暂不同步: 记录ID={}, 大小={}, 限制={}",
                record_id, file_size, max_file_size
            );
            // 注意：不修改sync_flag，保持为0，等用户升级VIP后可以同步
            return Ok(NOT_SYNCHRONIZED);
        }
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
            Ok(final_status)
        }
        Ok(None) => {
            log::error!("同步返回空结果: {}", record_id);
            Err(AppError::General("同步返回空结果".to_string()))
        }
        Err(e) => {
            log::error!("同步失败: {}, 错误: {}", record_id, e);
            Err(AppError::General(format!("同步失败: {}", e)))
        }
    }
}

async fn notify_frontend_sync_status(ids: Vec<String>) {
    notify_frontend_sync_status_with_flag(ids, SYNCHRONIZED).await;
}

async fn notify_frontend_sync_status_with_flag(ids: Vec<String>, sync_flag: i32) {
    let payload = serde_json::json!({
        "clip_ids": ids,
        "sync_flag": sync_flag
    });
    let app_handle = CONTEXT.get::<AppHandle>();
    let _ = app_handle
        .emit("sync_status_update_batch", payload)
        .map_err(|e| AppError::General(format!("批量通知前端失败: {}", e)));
}

/// 判断记录是否应该跳过同步
// 从参数中获取文件大小
async fn get_file_size_from_param(clip: &ClipRecordParam) -> u64 {
    // 如果是图片，从resources目录获取
    if let Some(content_str) = clip.content.as_str() {
        if let Some(resource_path) = get_resources_dir() {
            let mut file_path = resource_path;
            file_path.push(content_str);
            if file_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    return metadata.len();
                }
            }
        }
    }
    
    // 如果是文件，从local_file_path获取
    if let Some(local_path) = &clip.local_file_path {
        let paths: Vec<&str> = local_path.split(":::").collect();
        if let Some(first_path) = paths.first() {
            if let Ok(metadata) = std::fs::metadata(first_path) {
                return metadata.len();
            }
        }
    }
    
    0
}

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
            log::error!("更新同步状态失败: {}, 错误: {}", record_id, e);
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
                check_single_file_size(&file_path).await
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
async fn check_file_size_for_files(clip: &ClipRecordParam) -> Result<(), String> {
    if let Some(local_file_path_str) = &clip.local_file_path {
        // 使用 local_file_path 而不是 content，因为 content 存储的是显示用的文件名
        let file_paths: Vec<String> = local_file_path_str
            .split(":::")
            .map(|s| s.to_string())
            .collect();

        // 检查是否是多文件
        if file_paths.len() > 1 {
            return Err("多文件不支持云同步".to_string());
        }

        // 单文件处理
        if let Some(file_path_str) = file_paths.first() {
            let file_path = PathBuf::from(file_path_str);
            if file_path.exists() {
                check_single_file_size(&file_path).await
            } else {
                // 文件不存在，跳过同步
                Err(format!("文件不存在: {}", file_path_str))
            }
        } else {
            Err("缺少文件路径信息".to_string())
        }
    } else {
        // 如果没有 local_file_path，不支持同步
        Err("缺少文件路径信息".to_string())
    }
}

/// 检查单个文件大小是否超过限制
async fn check_single_file_size(file_path: &PathBuf) -> Result<(), String> {
    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            let file_size = metadata.len();
            match VipChecker::can_sync_file(file_size).await {
                Ok((can_sync, message)) => {
                    if can_sync {
                        Ok(())
                    } else {
                        Err(message)
                    }
                }
                Err(e) => Err(format!("检查VIP文件权限失败: {}", e))
            }
        }
        Err(e) => Err(format!("读取文件元数据失败: {}", e)),
    }
}
