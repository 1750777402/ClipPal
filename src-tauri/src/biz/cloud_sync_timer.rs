use log;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::time::{Duration, interval};

use crate::biz::system_setting::SYNC_INTERVAL_SECONDS;
use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, system_setting::Settings},
    errors::lock_utils::safe_lock,
    utils::http_client::{ApiResponse, HttpClient, HttpError},
};

/// 云同步API响应结构
#[derive(Debug, Serialize, Deserialize)]
struct CloudSyncResponse {
    success: bool,
    message: String,
    data: Option<serde_json::Value>,
}

/// 云同步请求结构
#[derive(Debug, Serialize, Deserialize)]
struct CloudSyncRequest {
    clips: Vec<ClipRecord>,
    timestamp: u64,
}

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
    pub async fn execute_sync_task(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::info!("开始执行云同步定时任务...");
        let unsynced_record = self.get_unsynced_records().await?;
        if unsynced_record.is_empty() {
            log::info!("没有未同步的记录，跳过同步任务");
            return Ok(());
        }
        // 有需要同步的记录时，发起http请求服务端

        // 服务端返回成功后，更新记录状态
        let ids = unsynced_record
            .iter()
            .map(|record| record.id.clone())
            .collect();
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

    /// 同步单条记录
    async fn sync_single_record(
        &self,
        record: &ClipRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ids = &vec![record.id.clone()];
        // 1. 先将记录状态设置为同步中
        self.update_sync_status(ids, 1).await?;

        // 2. 通知前端更新状态
        self.notify_frontend_sync_status(&record.id, 1).await?;

        // 3. 执行实际的同步操作（这里需要你实现具体的HTTP请求逻辑）
        match self.perform_http_sync(record).await {
            Ok(_) => {
                // 同步成功，更新状态为已同步
                self.update_sync_status(ids, 2).await?;
                self.notify_frontend_sync_status(&record.id, 2).await?;
                log::info!("记录 {} 同步成功", record.id);
            }
            Err(e) => {
                // 同步失败，回滚状态为未同步
                self.update_sync_status(ids, 0).await?;
                self.notify_frontend_sync_status(&record.id, 0).await?;
                return Err(e);
            }
        }

        Ok(())
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

    /// 通知前端同步状态变化（单个，保留兼容性）
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

    /// 执行HTTP同步操作
    async fn perform_http_sync(
        &self,
        record: &ClipRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 创建HTTP客户端
        let client = HttpClient::new();

        // 准备同步请求数据
        let sync_request = CloudSyncRequest {
            clips: vec![record.clone()],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or_else(|e| {
                    log::warn!("获取系统时间失败，使用默认值: {}", e);
                    0
                }),
        };

        // 设置API认证头（这里需要从配置中获取）
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer your-api-key".to_string(),
        );
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // 发起同步请求
        let api_url = "https://your-sync-api.com/sync"; // 这里需要从配置中获取
        let response: ApiResponse<CloudSyncResponse> = client
            .request_with_headers::<CloudSyncRequest, CloudSyncResponse>(
                "POST",
                api_url,
                Some(&sync_request),
                Some(headers),
            )
            .await?;
        if response.code == 200 {
            log::info!("记录 {} 同步成功", record.id);
            Ok(())
        } else {
            log::error!("同步请求失败，状态码: {}", response.code);
            Err(format!("同步请求失败，状态码: {}", response.code).into())
        }
    }
}

/// 启动云同步定时任务
pub async fn start_cloud_sync_timer(app_handle: AppHandle, rb: RBatis) {
    let timer = CloudSyncTimer::new(app_handle, rb);
    timer.start().await;
}
