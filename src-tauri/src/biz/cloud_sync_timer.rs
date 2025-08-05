use log;
use rbatis::RBatis;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};
use tokio::time::{Duration, sleep};

use crate::api::cloud_sync_api::{CloudSyncRequest, sync_clipboard, sync_server_time};
use crate::biz::sync_time::SyncTime;
use crate::biz::system_setting::{SYNC_INTERVAL_SECONDS, check_cloud_sync_enabled};
use crate::errors::{AppError, AppResult};
use crate::utils::lock_utils::lock_utils::safe_read_lock;
use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, system_setting::Settings},
    utils::lock_utils::GlobalSyncLock,
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
            if check_cloud_sync_enabled().await {
                log::debug!("云同步未开启，跳过定时任务");
                sleep(Duration::from_secs(cloud_sync_interval as u64)).await;
                continue;
            }

            // 尝试获取锁，执行同步任务
            if let Some(_guard) = sync_lock.try_lock() {
                if let Err(e) = self.execute_sync_task().await {
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
        log::info!("开始执行云同步定时任务...");

        let last_sync_time = SyncTime::select_last_time(&self.rb).await;

        let server_time = sync_server_time()
            .await
            .map_err(|e| AppError::General(format!("获取服务器时间失败: {}", e)))?
            .unwrap_or(0);

        let unsynced_record = self.get_unsynced_records().await?;
        let ids = unsynced_record
            .iter()
            .map(|record| record.id.clone())
            .collect();

        let sync_request = CloudSyncRequest {
            clips: unsynced_record.clone(),
            timestamp: server_time,
            last_sync_time,
        };

        let response = sync_clipboard(&sync_request)
            .await
            .map_err(|e| AppError::General(format!("云同步请求失败: {}", e)))?;

        if let Some(cloud_sync_res) = response {
            let new_server_time = cloud_sync_res.timestamp;
            SyncTime::update_last_time(&self.rb, new_server_time).await?;

            if let Some(clips) = cloud_sync_res.clips {
                for clip in clips {
                    let check_res = ClipRecord::check_by_type_and_md5(
                        &self.rb,
                        &clip.r#type.clone().unwrap_or_default(),
                        &clip.md5_str.clone().unwrap_or_default(),
                    )
                    .await?;

                    if check_res.is_empty() {
                        ClipRecord::insert_by_created_sort(&self.rb, clip.to_clip_record()).await?;
                    }
                }
            }

            ClipRecord::update_sync_flag(&self.rb, &ids, 2, new_server_time).await?;
            self.notify_frontend_sync_status_batch(&ids, 1).await?;
            Ok(())
        } else {
            log::warn!("云同步请求未返回数据");
            log::info!(
                "云同步定时任务执行完成... 本次同步数据量: {}",
                unsynced_record.len()
            );
            Err(AppError::ClipSync("云同步异常".to_string()))
        }
    }

    async fn get_unsynced_records(&self) -> AppResult<Vec<ClipRecord>> {
        let records = ClipRecord::select_by_sync_flag(&self.rb, 0).await?;
        Ok(records)
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
}

pub async fn start_cloud_sync_timer(app_handle: AppHandle, rb: RBatis) {
    let timer = CloudSyncTimer::new(app_handle, rb);
    timer.start().await;
}
