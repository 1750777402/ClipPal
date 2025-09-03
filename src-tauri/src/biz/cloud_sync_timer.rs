use clipboard_listener::ClipType;
use log;
use rbatis::RBatis;
use std::sync::{Arc, OnceLock, RwLock};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::Duration;
use uuid::Uuid;

use crate::api::cloud_sync_api::{
    ClipRecordParam, CloudSyncRequest, sync_clipboard, sync_server_time,
};
use crate::biz::clip_record::{NOT_SYNCHRONIZED, SKIP_SYNC, SYNCHRONIZED, SYNCHRONIZING};
use crate::biz::clip_record_clean::try_clean_clip_record;
use crate::biz::content_search::add_content_to_index;
use crate::biz::sync_time::SyncTime;
use crate::biz::system_setting::{SYNC_INTERVAL_SECONDS, check_cloud_sync_enabled};
use crate::biz::vip_checker::VipChecker;
use crate::errors::{AppError, AppResult};
use crate::utils::config::get_max_file_size_bytes;
use crate::utils::device_info::GLOBAL_DEVICE_ID;
use crate::utils::file_dir::get_resources_dir;
use crate::utils::lock_utils::lock_utils::safe_read_lock;
use crate::utils::token_manager::has_valid_auth;
use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, system_setting::Settings},
    utils::lock_utils::GlobalSyncLock,
};
use std::path::PathBuf;

pub struct CloudSyncTimer {
    app_handle: AppHandle,
    rb: RBatis,
    trigger_receiver: Option<mpsc::UnboundedReceiver<()>>,
}

// 全局触发器发送端
static TRIGGER_SENDER: OnceLock<mpsc::UnboundedSender<()>> = OnceLock::new();

impl CloudSyncTimer {
    pub fn new(app_handle: AppHandle, rb: RBatis) -> Self {
        // 创建触发器通道
        let (trigger_sender, trigger_receiver) = mpsc::unbounded_channel();

        // 保存全局发送端
        let _ = TRIGGER_SENDER.set(trigger_sender);

        Self {
            app_handle,
            rb,
            trigger_receiver: Some(trigger_receiver),
        }
    }

    /// 启动云同步定时任务
    pub async fn start(mut self) {
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
        log::info!("云同步服务已启动，间隔: {}秒", cloud_sync_interval);

        let sync_lock: &GlobalSyncLock = CONTEXT.get::<GlobalSyncLock>();
        let mut trigger_receiver = self.trigger_receiver.take().unwrap();

        // 创建定时器
        let mut timer = tokio::time::interval(Duration::from_secs(cloud_sync_interval as u64));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // 定时器触发
                _ = timer.tick() => {
                    self.try_execute_sync(sync_lock, "定时任务").await;
                }
                // 立即同步触发
                _ = trigger_receiver.recv() => {
                    log::debug!("收到立即同步信号");
                    self.try_execute_sync(sync_lock, "立即同步").await;
                }
            }
        }
    }

    /// 尝试执行同步任务
    async fn try_execute_sync(&self, sync_lock: &GlobalSyncLock, source: &str) {
        // 检查云同步是否开启
        if !check_cloud_sync_enabled().await {
            log::debug!("云同步未开启，跳过{}同步", source);
            return;
        }

        // 检查用户登录状态
        if !has_valid_auth() {
            log::debug!("用户未登录，跳过{}同步", source);
            return;
        }

        // 检查VIP云同步权限
        match VipChecker::check_cloud_sync_permission().await {
            Ok((allowed, message)) => {
                if !allowed {
                    log::warn!("{}同步权限检查失败: {}", source, message);
                    return;
                }
                log::debug!("{}同步权限检查通过: {}", source, message);
            }
            Err(e) => {
                log::error!("{}同步权限检查出错: {}", source, e);
                return;
            }
        }

        // 检查是否需要刷新VIP状态
        if let Ok(should_refresh) = VipChecker::should_refresh_vip_status() {
            if should_refresh {
                log::info!("检测到需要刷新VIP状态");

                match VipChecker::refresh_vip_from_server().await {
                    Ok(true) => log::info!("VIP状态已更新"),
                    Ok(false) => log::warn!("VIP状态无更新"),
                    Err(e) => log::error!("VIP状态刷新失败: {}", e),
                }

                // 重新检查权限
                match VipChecker::check_cloud_sync_permission().await {
                    Ok((still_allowed, _)) => {
                        if !still_allowed {
                            log::warn!("刷新后{}同步权限检查失败", source);
                            return;
                        }
                    }
                    Err(e) => {
                        log::error!("刷新后{}同步权限检查出错: {}", source, e);
                        return;
                    }
                }
            }
        }

        // 尝试获取锁，执行同步任务
        if let Some(guard) = sync_lock.try_lock() {
            log::info!("开始{}云同步", source);
            let result = self.execute_sync_task_with_source(source).await;
            drop(guard); // 显式释放锁

            if let Err(e) = result {
                log::error!("{}云同步失败: {}", source, e);
            }
        } else {
            // 获取不到锁，说明已有同步任务在执行
            log::info!("{}云同步在执行中，跳过", source);
        }
    }

    /// 执行同步任务（带来源标识）
    pub async fn execute_sync_task_with_source(&self, source: &str) -> AppResult<()> {
        let last_sync_time = SyncTime::select_last_time(&self.rb).await;

        // 获取一次服务器时间，代表了本次同步的时间戳版本号
        let server_time = match sync_server_time().await {
            Ok(Some(time)) => time,
            Ok(None) => {
                log::warn!("服务器时间为空，使用默认值");
                0
            }
            Err(e) => {
                log::error!("获取服务器时间失败: {}", e);
                return Err(AppError::General(format!("云服务不可用: {}", e)));
            }
        };

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

        let response = match sync_clipboard(&sync_request).await {
            Ok(resp) => resp,
            Err(e) => {
                log::error!(
                    "云同步数据传输失败: {} (待同步记录数: {})",
                    e,
                    unsynced_record.len()
                );
                return Err(AppError::General(format!("云服务异常: {}", e)));
            }
        };

        if let Some(cloud_sync_res) = response {
            let mut has_data_changed = false; // 标记是否有数据变化

            if let Some(clips) = cloud_sync_res.clips {
                log::info!(
                    "{}云同步完成 - 上传{}条记录，拉取{}条记录",
                    source,
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
                        log::debug!("新增云记录: {} ({})", new_id, obj.r#type);
                        has_data_changed = true; // 标记数据已变化

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
                            log::debug!(
                                "云同步删除记录: {}",
                                clip.md5_str.clone().unwrap_or_default()
                            );
                            // 如果是删除操作，逻辑删除记录
                            ClipRecord::sync_del_by_ids(
                                &self.rb,
                                &vec![clip.id.unwrap_or_default()],
                                server_time,
                            )
                            .await?;
                            has_data_changed = true; // 标记数据已变化
                        }
                    }
                }
            }

            // 根据记录类型分别处理同步状态
            self.update_sync_status_by_type(&unsynced_record, server_time)
                .await?;

            // 在最后的位置更新本次同步的服务器时间版本号   防止上面哪一步出现异常导致数据没同步成功
            SyncTime::update_last_time(&self.rb, server_time).await?;

            // 如果有数据变化，通知前端刷新
            if has_data_changed {
                log::debug!("检测到数据变化，通知前端刷新");
                if let Err(e) = self.app_handle.emit("clip_record_change", ()) {
                    log::warn!("通知前端失败: {}", e);
                }
            }

            // 同步完数据之后，检查是否需要删除过期数据
            tokio::spawn(async {
                try_clean_clip_record().await;
            });

            Ok(())
        } else {
            log::error!("云同步异常: 服务器数据无效");
            Err(AppError::ClipSync("云服务返回异常数据".to_string()))
        }
    }

    async fn get_unsynced_records(&self) -> AppResult<Vec<ClipRecord>> {
        let all_records = ClipRecord::select_by_sync_flag(&self.rb, NOT_SYNCHRONIZED).await?;

        // 获取当前用户的文件大小限制
        let max_file_size = VipChecker::get_cached_max_file_size().unwrap_or(0);

        // 有文件大小限制的用户（各级VIP），检查每个文件的大小
        let mut filtered_records = Vec::new();

        for record in &all_records {
            match record.r#type.as_str() {
                t if t == ClipType::Text.to_string() => {
                    // 文本类型：检查内容大小（加密后的字节大小）
                    if let Some(content_str) = record.content.as_str() {
                        // 获取加密后文本的实际字节大小
                        let content_size = content_str.as_bytes().len() as u64;
                        // 对于VIP用户，检查文本大小是否超限
                        if content_size <= max_file_size {
                            filtered_records.push(record.clone());
                        } else {
                            // 文本内容超过VIP限制，更新为跳过状态
                            if let Err(e) = ClipRecord::update_sync_flag_and_skip_type(
                                &self.rb,
                                &record.id,
                                SKIP_SYNC,
                                Some(2),
                            )
                            .await
                            {
                                log::error!("更新文本记录为VIP限制跳过失败: {}", e);
                            } else {
                                log::info!(
                                    "文本超限，设置为VIP限制跳过: ID={}, 大小={}字节, 限制={}字节",
                                    record.id,
                                    content_size,
                                    max_file_size
                                );
                            }
                        }
                    } else {
                        // 无内容的文本记录，直接跳过
                        log::debug!("跳过无内容的文本记录: ID={}", record.id);
                    }
                }
                t if t == ClipType::Image.to_string() => {
                    // 图片类型：检查文件大小
                    if let Some(content_str) = record.content.as_str() {
                        if let Some(resource_path) = get_resources_dir() {
                            let mut file_path = resource_path;
                            file_path.push(content_str);
                            if file_path.exists() {
                                if let Ok(metadata) = std::fs::metadata(&file_path) {
                                    if metadata.len() <= max_file_size {
                                        filtered_records.push(record.clone());
                                    } else {
                                        // 图片超过VIP限制，更新为跳过状态
                                        if let Err(e) = ClipRecord::update_sync_flag_and_skip_type(
                                            &self.rb,
                                            &record.id,
                                            SKIP_SYNC,
                                            Some(2),
                                        )
                                        .await
                                        {
                                            log::error!("更新图片记录为VIP限制跳过失败: {}", e);
                                        } else {
                                            log::info!(
                                                "图片超限，设置为VIP限制跳过: ID={}, 大小={}, 限制={}",
                                                record.id,
                                                metadata.len(),
                                                max_file_size
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                t if t == ClipType::File.to_string() => {
                    // 文件类型：检查文件大小
                    if let Some(local_path) = &record.local_file_path {
                        let paths: Vec<&str> = local_path.split(":::").collect();
                        if let Some(first_path) = paths.first() {
                            if let Ok(metadata) = std::fs::metadata(first_path) {
                                if metadata.len() <= max_file_size {
                                    filtered_records.push(record.clone());
                                } else {
                                    // 文件超过VIP限制，更新为跳过状态
                                    if let Err(e) = ClipRecord::update_sync_flag_and_skip_type(
                                        &self.rb,
                                        &record.id,
                                        SKIP_SYNC,
                                        Some(2),
                                    )
                                    .await
                                    {
                                        log::error!("更新文件记录为VIP限制跳过失败: {}", e);
                                    } else {
                                        log::info!(
                                            "文件超限，设置为VIP限制跳过: ID={}, 大小={}, 限制={}",
                                            record.id,
                                            metadata.len(),
                                            max_file_size
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    // 其他类型：默认允许同步
                    filtered_records.push(record.clone());
                }
            }
        }

        if filtered_records.len() != all_records.len() {
            log::info!(
                "同步过滤（大小限制）: 总记录={}, 符合条件={}, 限制={}字节",
                all_records.len(),
                filtered_records.len(),
                max_file_size
            );
        }

        Ok(filtered_records)
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
            log::debug!("文本记录同步完成: {}条", text_ids.len());
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
                    self.check_single_file_size(&file_path).await
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
        if let Some(local_file_path_str) = &record.local_file_path {
            // 使用 local_file_path 而不是 content，因为 content 存储的是显示用的文件名
            let file_paths: Vec<String> = local_file_path_str
                .split(":::")
                .map(|s| s.to_string())
                .collect();

            // 检查是否是多文件
            if file_paths.len() > 1 {
                return Err("多文件不支持云同步".to_string());
            }

            for file_path_str in &file_paths {
                let file_path = PathBuf::from(file_path_str);
                if file_path.exists() {
                    if let Err(e) = self.check_single_file_size(&file_path).await {
                        return Err(format!("文件 {}: {}", file_path_str, e));
                    }
                } else {
                    // 如果文件不存在，也认为需要跳过同步
                    return Err(format!("文件不存在: {}", file_path_str));
                }
            }
        } else {
            // 如果没有 local_file_path，跳过检查（可能是旧数据或异常情况）
            return Err("缺少文件路径信息".to_string());
        }
        Ok(())
    }

    /// 检查单个文件大小是否超过限制
    async fn check_single_file_size(&self, file_path: &PathBuf) -> Result<(), String> {
        match std::fs::metadata(file_path) {
            Ok(metadata) => {
                // 使用VIP检查器获取文件大小限制
                let max_file_size = match VipChecker::get_max_file_size().await {
                    Ok(size) => size,
                    Err(_) => get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024), // fallback
                };

                if metadata.len() > max_file_size {
                    if max_file_size == 0 {
                        Err("免费用户不支持文件同步，请升级VIP".to_string())
                    } else {
                        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                        let max_mb = max_file_size as f64 / (1024.0 * 1024.0);
                        Err(format!(
                            "文件大小 {:.1}MB 超过限制 {:.1}MB，请升级VIP以支持更大文件",
                            size_mb, max_mb
                        ))
                    }
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(format!("读取文件元数据失败: {}", e)),
        }
    }
}

/// 触发立即同步
pub fn trigger_immediate_sync() -> Result<(), &'static str> {
    if let Some(sender) = TRIGGER_SENDER.get() {
        match sender.send(()) {
            Ok(()) => Ok(()),
            Err(_) => {
                log::warn!("立即同步触发信号发送失败，接收端已关闭");
                Err("同步任务未启动")
            }
        }
    } else {
        log::warn!("立即同步触发器未初始化");
        Err("同步任务未启动")
    }
}

/// 开始云同步定时任务（供外部调用）
pub async fn start_cloud_sync_timer(app_handle: AppHandle, rb: RBatis) {
    let timer = CloudSyncTimer::new(app_handle, rb);
    timer.start().await;
}
