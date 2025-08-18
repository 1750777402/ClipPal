use clipboard_listener::ClipType;
use log;
use rbatis::RBatis;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

use crate::api::cloud_sync_api::{
    ClipRecordParam, CloudSyncRequest, sync_clipboard, sync_server_time,
};
use crate::biz::clip_record::{NOT_SYNCHRONIZED, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::biz::clip_record_clean::try_clean_clip_record;
use crate::biz::content_search::add_content_to_index;
use crate::biz::sync_time::SyncTime;
use crate::biz::system_setting::{SYNC_INTERVAL_SECONDS, check_cloud_sync_enabled};
use crate::errors::{AppError, AppResult};
use crate::utils::config::get_max_file_size_bytes;
use crate::utils::device_info::GLOBAL_DEVICE_ID;
use crate::utils::file_dir::get_resources_dir;
use crate::utils::lock_utils::lock_utils::safe_read_lock;
use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, system_setting::Settings},
    utils::lock_utils::GlobalSyncLock,
};
use std::path::PathBuf;

pub struct CloudSyncTimer {
    app_handle: AppHandle,
    rb: RBatis,
}

impl CloudSyncTimer {
    pub fn new(app_handle: AppHandle, rb: RBatis) -> Self {
        Self { app_handle, rb }
    }

    /// 启动云同步定时任务
    pub async fn start(&self) {
        let cloud_sync_interval = {
            let settings_lock = CONTEXT.get::<Arc<RwLock<Settings>>>();
            match safe_read_lock(&settings_lock) {
                Ok(settings) => settings.cloud_sync_interval,
                Err(e) => {
                    log::warn!("无法获取设置: {}", e);
                    SYNC_INTERVAL_SECONDS
                }
            }
        };
        log::info!("云同步定时任务已启动，间隔: {}秒", cloud_sync_interval);

        let sync_lock: &GlobalSyncLock = CONTEXT.get::<GlobalSyncLock>();
        loop {
            // 检查云同步是否开启
            if !check_cloud_sync_enabled().await {
                log::debug!("云同步未开启，跳过定时任务");
                sleep(Duration::from_secs(cloud_sync_interval as u64)).await;
                continue;
            }

            // 尝试获取锁，执行同步任务
            if let Some(guard) = sync_lock.try_lock() {
                let result = self.execute_sync_task().await;
                drop(guard); // 显式释放锁

                if let Err(e) = result {
                    log::error!("云同步定时任务执行失败: {}", e);
                }
                // 执行完后等待
                sleep(Duration::from_secs(cloud_sync_interval as u64)).await;
            } else {
                // 获取不到锁，不等待直接下一轮尝试
                log::debug!("跳过本轮云同步执行，等待下次尝试...");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    /// 执行同步任务
    pub async fn execute_sync_task(&self) -> AppResult<()> {
        log::debug!("开始执行云同步定时任务...");

        let last_sync_time = SyncTime::select_last_time(&self.rb).await;

        // 获取一次服务器时间，代表了本次同步的时间戳版本号
        let server_time = sync_server_time()
            .await
            .map_err(|e| AppError::General(format!("获取服务器时间失败: {}", e)))?
            .unwrap_or(0);

        let unsynced_record = self.get_unsynced_records().await?;
        let _ids: Vec<String> = unsynced_record
            .iter()
            .map(|record| record.id.clone())
            .collect();

        let mut params: Vec<ClipRecordParam> = Vec::new();
        let records = unsynced_record.clone();
        if !records.is_empty() {
            records.iter().for_each(|record| {
                let param: ClipRecordParam = record.clone().into();
                params.push(param);
            });
        }

        let sync_request = CloudSyncRequest {
            clips: params, // 本次需要同步的数据
            timestamp: server_time,
            last_sync_time,
            device_id: GLOBAL_DEVICE_ID.clone(),
        };

        let response = sync_clipboard(&sync_request)
            .await
            .map_err(|e| AppError::General(format!("云同步请求失败: {}", e)))?;

        if let Some(cloud_sync_res) = response {
            if let Some(clips) = cloud_sync_res.clips {
                log::info!(
                    "云同步定时任务执行完成... 本次上传数据量: {}，拉取数据量：{}",
                    unsynced_record.len(),
                    clips.len()
                );
                for clip in clips {
                    // 遍历每一条记录  查看是不是在本地已经存在了
                    let check_res = ClipRecord::check_by_type_and_md5(
                        &self.rb,
                        &clip.r#type.clone().unwrap_or_default(),
                        &clip.md5_str.clone().unwrap_or_default(),
                    )
                    .await?;
                    if check_res.is_empty() && matches!(clip.del_flag, Some(0)) {
                        // 如果本地没有这条记录 并且这条记录不是已经删除的 那么就插入新记录
                        let new_id = Uuid::new_v4().to_string();
                        let content = clip.content.clone();
                        let mut obj = clip.to_clip_record();
                        obj.sort = 0;
                        obj.id = new_id.clone();
                        obj.sync_flag = Some(SYNCHRONIZED); // 设置为已同步
                        if obj.r#type == ClipType::Image.to_string()
                            || obj.r#type == ClipType::File.to_string()
                        {
                            // 如果从云端拉取下来的是图片或者文件类型   设置为同步中  等待拉取文件数据
                            obj.sync_flag = Some(SYNCHRONIZING);
                        }
                        obj.pinned_flag = 0; // 默认不置顶
                        obj.cloud_source = Some(1); // 云端同步下来的设置为1
                        let _ = ClipRecord::insert_by_created_sort(&self.rb, obj.clone()).await?;
                        log::info!("同步数据后拉取到云端新数据，插入新记录: {:?}", obj);
                        // 插入成功后，更新搜索索引
                        tokio::spawn(async move {
                            if let Err(e) =
                                add_content_to_index(&new_id, content.as_str().unwrap_or_default())
                                    .await
                            {
                                log::error!("搜索索引更新失败: {}", e);
                            }
                        });
                    } else {
                        // 如果本地有这条记录，那么查看是不是云端同步的是被删除的，如果是那么本地也逻辑删除  并且把同步状态设置为已同步
                        if clip.del_flag.unwrap_or_default() == 1 {
                            log::info!(
                                "同步数据后拉取到云端已删除数据，逻辑删除记录: {}",
                                clip.md5_str.clone().unwrap_or_default()
                            );
                            // 如果是删除操作，逻辑删除记录
                            ClipRecord::sync_del_by_ids(
                                &self.rb,
                                &vec![clip.id.unwrap_or_default()],
                                server_time,
                            )
                            .await?;
                        }
                    }
                }
            }

            // 根据记录类型分别处理同步状态
            self.update_sync_status_by_type(&unsynced_record, server_time)
                .await?;

            // 在最后的位置更新本次同步的服务器时间版本号   防止上面哪一步出现异常导致数据没同步成功
            SyncTime::update_last_time(&self.rb, server_time).await?;
            // 同步完数据之后，检查是否需要删除过期数据
            tokio::spawn(async {
                try_clean_clip_record().await;
            });

            Ok(())
        } else {
            log::warn!("云同步请求未返回数据");
            Err(AppError::ClipSync("云同步异常".to_string()))
        }
    }

    async fn get_unsynced_records(&self) -> AppResult<Vec<ClipRecord>> {
        let records = ClipRecord::select_by_sync_flag(&self.rb, NOT_SYNCHRONIZED).await?;
        Ok(records)
    }

    /// 根据记录类型更新同步状态
    async fn update_sync_status_by_type(
        &self,
        records: &Vec<ClipRecord>,
        server_time: u64,
    ) -> AppResult<()> {
        // 分类处理不同类型的记录
        let mut text_ids = Vec::new();
        let mut image_records = Vec::new();
        let mut file_records = Vec::new();

        for record in records {
            match record.r#type.as_str() {
                t if t == ClipType::Text.to_string() => {
                    text_ids.push(record.id.clone());
                }
                t if t == ClipType::Image.to_string() => {
                    image_records.push(record);
                }
                t if t == ClipType::File.to_string() => {
                    file_records.push(record);
                }
                _ => {
                    text_ids.push(record.id.clone());
                }
            }
        }

        // 文本类型直接标记为已同步
        if !text_ids.is_empty() {
            ClipRecord::update_sync_flag(&self.rb, &text_ids, SYNCHRONIZED, server_time).await?;
            self.notify_frontend_sync_status_batch(&text_ids, SYNCHRONIZED)
                .await?;
            log::info!("批量更新 {} 条文本记录为已同步", text_ids.len());
        }

        // 图片类型：检查文件大小，超过限制的跳过同步，否则标记为同步中
        let (image_sync_ids, image_skip_ids) = self.categorize_image_records(image_records).await;

        self.batch_update_sync_status(
            &image_sync_ids,
            SYNCHRONIZING,
            server_time,
            "图片",
            "同步中，等待文件上传队列处理",
        )
        .await?;
        self.batch_update_sync_status(
            &image_skip_ids,
            SKIP_SYNC,
            server_time,
            "图片",
            "跳过同步（文件大小超过限制）",
        )
        .await?;

        // 文件类型：检查文件大小，超过限制的跳过同步，否则标记为同步中
        let (file_sync_ids, file_skip_ids) = self.categorize_file_records(file_records).await;

        self.batch_update_sync_status(
            &file_sync_ids,
            SYNCHRONIZING,
            server_time,
            "文件",
            "同步中，等待文件上传队列处理",
        )
        .await?;
        self.batch_update_sync_status(
            &file_skip_ids,
            SKIP_SYNC,
            server_time,
            "文件",
            "跳过同步（文件大小超过限制）",
        )
        .await?;

        Ok(())
    }

    async fn notify_frontend_sync_status_batch(
        &self,
        record_ids: &Vec<String>,
        sync_flag: i32,
    ) -> AppResult<()> {
        let payload = serde_json::json!({
            "clip_ids": record_ids,
            "sync_flag": sync_flag
        });

        self.app_handle
            .emit("sync_status_update_batch", payload)
            .map_err(|e| AppError::General(format!("批量通知前端失败: {}", e)))
    }

    #[allow(dead_code)]
    async fn notify_frontend_sync_status(&self, record_id: &str, sync_flag: i32) -> AppResult<()> {
        let payload = serde_json::json!({
            "clip_id": record_id,
            "sync_flag": sync_flag
        });

        self.app_handle
            .emit("sync_status_update", payload)
            .map_err(|e| AppError::General(format!("通知前端失败: {}", e)))
    }

    /// 分类图片记录
    async fn categorize_image_records(
        &self,
        records: Vec<&ClipRecord>,
    ) -> (Vec<String>, Vec<String>) {
        let mut sync_ids = Vec::new();
        let mut skip_ids = Vec::new();

        for record in records {
            if self.check_image_file_size(record).await.is_err() {
                skip_ids.push(record.id.clone());
            } else {
                sync_ids.push(record.id.clone());
            }
        }

        (sync_ids, skip_ids)
    }

    /// 分类文件记录
    async fn categorize_file_records(
        &self,
        records: Vec<&ClipRecord>,
    ) -> (Vec<String>, Vec<String>) {
        let mut sync_ids = Vec::new();
        let mut skip_ids = Vec::new();

        for record in records {
            if self.check_files_size(record).await.is_err() {
                skip_ids.push(record.id.clone());
            } else {
                sync_ids.push(record.id.clone());
            }
        }

        (sync_ids, skip_ids)
    }

    /// 批量更新同步状态
    async fn batch_update_sync_status(
        &self,
        ids: &Vec<String>,
        sync_flag: i32,
        server_time: u64,
        record_type: &str,
        action_desc: &str,
    ) -> AppResult<()> {
        if !ids.is_empty() {
            ClipRecord::update_sync_flag(&self.rb, ids, sync_flag, server_time).await?;
            self.notify_frontend_sync_status_batch(ids, sync_flag)
                .await?;
            log::info!(
                "批量更新 {} 条{}记录为{}",
                ids.len(),
                record_type,
                action_desc
            );
        }
        Ok(())
    }

    /// 检查图片文件大小是否超过限制
    async fn check_image_file_size(&self, record: &ClipRecord) -> Result<(), String> {
        if let Some(content_str) = record.content.as_str() {
            if content_str.is_empty() || content_str == "null" {
                return Ok(()); // 无内容，不检查大小
            }

            // 构造图片文件路径
            if let Some(resource_path) = get_resources_dir() {
                let mut file_path = resource_path.clone();
                file_path.push(content_str);

                if file_path.exists() {
                    self.check_single_file_size(&file_path)
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
    async fn check_files_size(&self, record: &ClipRecord) -> Result<(), String> {
        if let Some(content_str) = record.content.as_str() {
            let file_paths: Vec<String> = content_str.split(":::").map(|s| s.to_string()).collect();

            for file_path_str in &file_paths {
                let file_path = PathBuf::from(file_path_str);
                if file_path.exists() {
                    if let Err(e) = self.check_single_file_size(&file_path) {
                        return Err(format!("文件 {}: {}", file_path_str, e));
                    }
                }
            }
        }
        Ok(())
    }

    /// 检查单个文件大小是否超过限制
    fn check_single_file_size(&self, file_path: &PathBuf) -> Result<(), String> {
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
}

pub async fn start_cloud_sync_timer(app_handle: AppHandle, rb: RBatis) {
    let timer = CloudSyncTimer::new(app_handle, rb);
    timer.start().await;
}
