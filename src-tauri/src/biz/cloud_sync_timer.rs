use log;
use rbatis::RBatis;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::time::{Duration, interval};

use crate::api::cloud_sync_api::CloudSyncRequest;
use crate::api::cloud_sync_api::sync_clipboard;
use crate::api::cloud_sync_api::sync_server_time;
use crate::biz::sync_time::SyncTime;
use crate::biz::system_setting::SYNC_INTERVAL_SECONDS;
use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, system_setting::Settings},
    errors::lock_utils::safe_lock,
};

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
            let settings_lock = CONTEXT.get::<Arc<Mutex<Settings>>>();
            match safe_lock(&settings_lock) {
                Ok(settings) => settings.cloud_sync_interval,
                Err(e) => {
                    log::warn!("无法获取设置: {}", e);
                    SYNC_INTERVAL_SECONDS
                }
            }
        };
        log::info!("云同步定时任务已启动，间隔: {}秒", cloud_sync_interval);

        let mut interval_timer = interval(Duration::from_secs(cloud_sync_interval as u64));

        loop {
            interval_timer.tick().await;

            // 检查云同步是否开启
            if !self.is_cloud_sync_enabled().await {
                log::debug!("云同步未开启，跳过定时任务");
                continue;
            }

            // 执行同步任务
            if let Err(e) = self.execute_sync_task().await {
                log::error!("云同步定时任务执行失败: {}", e);
            }
        }
    }

    /// 检查云同步是否开启
    async fn is_cloud_sync_enabled(&self) -> bool {
        let settings_lock = CONTEXT.get::<Arc<Mutex<Settings>>>();
        if let Ok(settings) = safe_lock(&settings_lock) {
            return settings.cloud_sync == 1;
        }
        false
    }

    /// 执行同步任务
    /// 这个同步任务主要是在用户有未同步数据并且没有加入向云端同步的队列时，主动向云端进行同步，同时拉取云端新的数据到本地
    pub async fn execute_sync_task(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("开始执行云同步定时任务...");

        // 本地上次同步时间的时间戳
        let last_sync_time = SyncTime::select_last_time(&self.rb).await;

        // 获取服务器时间戳  以这个时间作为同步的基准时间版本
        let server_time = sync_server_time().await?.unwrap_or(0);

        // 获取所有未同步的数据记录
        let unsynced_record = self.get_unsynced_records().await?;
        if unsynced_record.is_empty() {
            log::info!("没有未同步的记录，跳过同步任务");
            return Ok(());
        }
        let ids = unsynced_record
            .iter()
            .map(|record| record.id.clone())
            .collect();
        // 有需要同步的记录时，发起http请求服务端
        let sync_request = CloudSyncRequest {
            clips: unsynced_record,
            timestamp: server_time,         // 服务器时间戳
            last_sync_time: last_sync_time, // 本地上次同步时服务端返回的时间戳
        };

        let response = sync_clipboard(&sync_request).await?;
        if let Some(cloud_sync_res) = response {
            // 服务器返回的同步时间 这个时间戳需要更新到本地做记录，下次同步需要带上这个时间戳作为版本号
            let new_server_time = cloud_sync_res.timestamp;
            SyncTime::update_last_time(&self.rb, new_server_time).await?;
            if let Some(clips) = cloud_sync_res.clips {
                // 如果服务端返回了数据，说明云端有新数据，就需要更新本地记录
                if !clips.is_empty() {
                    for clip in clips {
                        let check_res = ClipRecord::check_by_type_and_md5(
                            &self.rb,
                            &clip.r#type,
                            &clip.md5_str,
                        )
                        .await?;
                        if check_res.is_empty() {
                            // 本地没有这条记录  那么就新增一条，新增时需要注意按照时间戳排序
                            ClipRecord::insert_by_created_sort(&self.rb, clip).await?
                        }
                    }
                }
            }
            // 更新已同步的数据的状态为已同步并设置同步时间为new_server_time
            
        } else {
            log::warn!("云同步请求未返回数据");
        }
        // 服务端返回成功后，更新记录状态
        let update_res = self.update_sync_status(&ids, 1).await;
        match update_res {
            Ok(_) => {
                // 批量通知前端
                self.notify_frontend_sync_status_batch(&ids, 1).await?;
            }
            Err(e) => log::error!("更新同步状态失败: {}", e),
        }
        Ok(())
    }

    /// 获取所有未同步的记录
    async fn get_unsynced_records(
        &self,
    ) -> Result<Vec<ClipRecord>, Box<dyn std::error::Error + Send + Sync>> {
        // 查询 sync_flag = 0 的记录
        let records = ClipRecord::select_by_sync_flag(&self.rb, 0)
            .await
            .map_err(|e| format!("查询未同步记录失败: {}", e))?;

        Ok(records)
    }

    /// 更新记录的同步状态
    async fn update_sync_status(
        &self,
        record_ids: &Vec<String>,
        sync_flag: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ClipRecord::update_sync_flag(&self.rb, &record_ids, sync_flag)
            .await
            .map_err(|e| format!("更新同步状态失败: {}", e).into())
    }

    /// 批量通知前端同步状态变化
    async fn notify_frontend_sync_status_batch(
        &self,
        record_ids: &Vec<String>,
        sync_flag: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = serde_json::json!({
            "clip_ids": record_ids,
            "sync_flag": sync_flag
        });

        self.app_handle
            .emit("sync_status_update_batch", payload)
            .map_err(|e| format!("批量通知前端失败: {}", e).into())
    }

    /// 通知前端同步状态变化
    async fn notify_frontend_sync_status(
        &self,
        record_id: &str,
        sync_flag: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = serde_json::json!({
            "clip_id": record_id,
            "sync_flag": sync_flag
        });

        self.app_handle
            .emit("sync_status_update", payload)
            .map_err(|e| format!("通知前端失败: {}", e).into())
    }
}

/// 启动云同步定时任务
pub async fn start_cloud_sync_timer(app_handle: AppHandle, rb: RBatis) {
    let timer = CloudSyncTimer::new(app_handle, rb);
    timer.start().await;
}
