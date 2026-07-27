use std::collections::HashMap;

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_opener::OpenerExt;

use crate::{
    app_context::AppContext,
    domain::{
        clip::{NOT_SYNCHRONIZED, SKIP_SYNC},
        vip::{
            PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
            UserVipInfoResponse, VipInfo, VipLimits, VipType,
        },
    },
    infra::{http::VipClient, security::AuthStore, storage::VipStore},
    utils::file_dir::get_resources_dir,
};

const DEFAULT_FREE_MAX_RECORDS: u32 = 300;
const DEFAULT_VIP_MAX_RECORDS: u32 = 1000;
const DEFAULT_VIP_FILE_SIZE_KB: u64 = 5120;
const FALLBACK_COPY_SIZE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Serialize, Clone)]
struct VipStatusChangedPayload {
    is_vip: bool,
    vip_type: Option<VipType>,
    expire_time: Option<u64>,
    max_records: u32,
}

/// VIP 应用服务，统一编排权益刷新、缓存、限制执行、支付和状态通知。
pub struct VipService<'a> {
    context: &'a AppContext,
    client: &'a dyn VipClient,
    auth_store: &'a dyn AuthStore,
    store: &'a dyn VipStore,
}

impl<'a> VipService<'a> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            client: context.vip_client(),
            auth_store: context.auth_store(),
            store: context.vip_store(),
        }
    }

    /// 读取本地缓存的 VIP 信息，不触发网络刷新。
    pub fn get_vip_status(&self) -> Result<Option<VipInfo>, String> {
        self.store.get_info()
    }

    /// 实时校验 VIP 状态；网络失败时回退到本地缓存。
    pub async fn is_vip_user(&self) -> Result<bool, String> {
        if !self.auth_store.has_access_token() {
            log::debug!("用户未登录，跳过 VIP 状态检查");
            return Ok(false);
        }

        match self.client.fetch_current_info().await {
            Ok(Some(response)) => {
                self.apply_remote_info(&response).await?;
                Ok(response.vip_flag)
            }
            Ok(None) => {
                log::warn!("服务端返回空的 VIP 信息");
                Ok(false)
            }
            Err(error) => {
                log::error!("VIP 状态检查失败: {}", error);
                let cached = self.store.get_info()?;
                if let Some(info) = cached {
                    log::warn!("网络错误，使用本地缓存的 VIP 状态: {}", info.vip_flag);
                    Ok(info.vip_flag)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// 检查当前账号是否允许使用云同步，并返回判断原因。
    pub async fn check_vip_permission(&self) -> Result<(bool, String), String> {
        self.check_cloud_sync_permission_with_status(None).await
    }

    async fn check_cloud_sync_permission_with_status(
        &self,
        known_vip_status: Option<bool>,
    ) -> Result<(bool, String), String> {
        if !self.auth_store.has_access_token() {
            return Ok((false, "需要登录后才能使用云同步功能".to_string()));
        }

        let is_vip = match known_vip_status {
            Some(status) => status,
            None => self.is_vip_user().await?,
        };

        if is_vip {
            return Ok((true, "VIP用户，享受完整云同步功能".to_string()));
        }

        Ok((true, "云同步功能已启用".to_string()))
    }

    /// 获取当前权益限制，包括记录数、文件大小和云同步能力。
    pub async fn get_vip_limits(&self) -> Result<VipLimits, String> {
        let is_vip = self.is_vip_user().await?;
        let (max_records, max_file_size) = match self.store.get_info()? {
            Some(info) => (info.max_records, info.max_file_size_bytes()),
            None => (DEFAULT_FREE_MAX_RECORDS, 0),
        };
        let can_cloud_sync = self
            .check_cloud_sync_permission_with_status(Some(is_vip))
            .await?
            .0;

        Ok(VipLimits {
            is_vip,
            max_records,
            max_file_size,
            can_cloud_sync,
        })
    }

    /// 从服务端刷新权益，成功应用快照后通知前端。
    pub async fn refresh_vip_status(&self) -> Result<bool, String> {
        log::info!("从服务器刷新 VIP 状态");
        match self.client.fetch_current_info().await {
            Ok(Some(response)) => {
                self.apply_remote_info(&response).await?;
                self.emit_status_changed();
                log::info!("VIP 状态已从服务器更新");
                Ok(true)
            }
            Ok(None) => {
                log::warn!("服务器返回空的 VIP 信息，使用本地缓存");
                Ok(false)
            }
            Err(error) => {
                log::warn!("从服务器获取 VIP 状态失败，将使用本地缓存: {}", error);
                Ok(false)
            }
        }
    }

    /// 判断本地权益缓存是否需要刷新。
    pub fn should_refresh_vip_status(&self) -> Result<bool, String> {
        self.store.should_refresh()
    }

    /// 基于服务端状态获取最大记录条数。
    pub async fn get_max_records_limit(&self) -> Result<u32, String> {
        if self.is_vip_user().await? {
            return Ok(self
                .store
                .get_info()?
                .map(|info| info.max_records)
                .unwrap_or(DEFAULT_VIP_MAX_RECORDS));
        }

        if let Some(config) = self.client.get_server_config().await? {
            if let Some(free) = config.get(&VipType::Free) {
                return Ok(free.record_limit);
            }
        }
        Ok(DEFAULT_FREE_MAX_RECORDS)
    }

    /// 仅使用本地缓存计算记录条数限制。
    pub fn get_cached_max_records_limit(&self) -> Result<u32, String> {
        if let Some(info) = self.store.get_info()? {
            if info.vip_flag {
                return Ok(info.max_records);
            }
        }
        Ok(DEFAULT_FREE_MAX_RECORDS)
    }

    /// 实时校验后返回同步文件大小限制，单位为字节。
    pub async fn get_max_file_size(&self) -> Result<u64, String> {
        if self.is_vip_user().await? {
            return Ok(self
                .store
                .get_info()?
                .map(|info| info.max_file_size_bytes())
                .unwrap_or(DEFAULT_VIP_FILE_SIZE_KB * 1024));
        }
        Ok(0)
    }

    /// 仅使用本地缓存返回文件大小限制，单位为字节。
    pub fn get_cached_max_file_size(&self) -> Result<u64, String> {
        Ok(self
            .store
            .get_info()?
            .map(|info| info.max_file_size_bytes())
            .unwrap_or(0))
    }

    /// 获取本地文件复制限制；无缓存时从套餐配置取最大值。
    pub async fn get_file_copy_size_limit(&self) -> u64 {
        if let Ok(Some(info)) = self.store.get_info() {
            if info.max_file_size > 0 {
                log::debug!("从本地 VIP 缓存获取文件复制限制: {}KB", info.max_file_size);
                return info.max_file_size_bytes();
            }
        }

        match self.client.get_server_config().await {
            Ok(Some(configs)) => {
                let max_kb = configs
                    .values()
                    .map(|config| config.max_file_size)
                    .max()
                    .unwrap_or(DEFAULT_VIP_FILE_SIZE_KB);
                log::debug!("从服务器配置获取文件复制限制: {}KB", max_kb);
                max_kb * 1024
            }
            Ok(None) | Err(_) => {
                log::debug!("无法获取服务器配置，使用本地文件复制限制");
                FALLBACK_COPY_SIZE_BYTES
            }
        }
    }

    /// 检查文件是否满足当前账号的云同步限制。
    pub async fn can_sync_file(&self, file_size: u64) -> Result<(bool, String), String> {
        let max_file_size = self.get_cached_max_file_size()?;
        if max_file_size == 0 {
            return Ok((false, "免费用户不支持文件云同步".to_string()));
        }
        if file_size > max_file_size {
            let size_mb = file_size as f64 / 1024.0 / 1024.0;
            let max_mb = max_file_size as f64 / 1024.0 / 1024.0;
            return Ok((
                false,
                format!("文件大小 {:.2}MB 超过 {:.2}MB 限制", size_mb, max_mb),
            ));
        }
        Ok((true, "文件可以同步".to_string()))
    }

    /// 启动或登录后初始化 VIP 状态并执行本地限制。
    pub async fn initialize_and_enforce_limits(&self) -> Result<(), String> {
        log::info!("初始化 VIP 状态并执行权益限制检查");
        if let Err(error) = self.is_vip_user().await {
            log::warn!("VIP 状态检查失败（静默处理）: {}", error);
        }
        if let Err(error) = self.enforce_local_records_limit_from_db().await {
            log::warn!("数据库记录数限制检查失败（静默处理）: {}", error);
        }
        log::info!("VIP 状态初始化和权益限制检查完成");
        Ok(())
    }

    pub fn open_vip_purchase_page(&self) -> Result<(), String> {
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        app_handle
            .opener()
            .open_url("https://jingchuanyuexiang.com", None::<&str>)
            .map_err(|error| format!("打开浏览器失败: {}", error))
    }

    pub async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        self.client.get_server_config().await
    }

    pub async fn get_pay_url(&self, param: PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        self.client.get_pay_url(&param).await
    }

    pub async fn get_pay_result(
        &self,
        param: QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        self.client.get_pay_result(&param).await
    }

    async fn apply_remote_info(&self, response: &UserVipInfoResponse) -> Result<(), String> {
        let previous = self.store.get_info()?;
        let current = VipInfo::from(response);
        let changed = previous
            .as_ref()
            .map(|old| current.materially_differs_from(old))
            .unwrap_or(true);

        self.store.save_checked(&current)?;
        self.enforce_local_records_limit(response.max_records)
            .await?;
        self.update_skipped_records_after_vip_change(current.max_file_size_bytes())
            .await?;

        if changed {
            log::info!("检测到 VIP 权益关键字段变化");
        }
        Ok(())
    }

    async fn enforce_local_records_limit(&self, max_records: u32) -> Result<(), String> {
        let mut settings = self
            .context
            .with_settings(Clone::clone)
            .map_err(|error| error.to_string())?;
        if settings.max_records <= max_records {
            return Ok(());
        }

        log::warn!(
            "本地记录条数({})超过服务端限制({})，自动调整",
            settings.max_records,
            max_records
        );
        settings.max_records = max_records;
        self.context
            .repositories()
            .settings()
            .save(&settings)
            .map_err(|error| error.to_string())?;
        self.context
            .update_settings(|current| *current = settings)
            .map_err(|error| error.to_string())
    }

    async fn update_skipped_records_after_vip_change(
        &self,
        max_file_size: u64,
    ) -> Result<(), String> {
        let records = self
            .context
            .repositories()
            .clip_records()
            .list_skipped(SKIP_SYNC, 2)
            .await
            .map_err(|error| error.to_string())?;
        if records.is_empty() {
            return Ok(());
        }

        let mut updated_count = 0;
        for record in records {
            let should_update = match record.r#type.as_str() {
                "text" => record
                    .content
                    .as_str()
                    .map(|content| max_file_size > 0 && content.len() as u64 <= max_file_size)
                    .unwrap_or(false),
                "image" => record
                    .content
                    .as_str()
                    .and_then(|content| get_resources_dir().map(|base| base.join(content)))
                    .and_then(|path| std::fs::metadata(path).ok())
                    .map(|metadata| max_file_size > 0 && metadata.len() <= max_file_size)
                    .unwrap_or(false),
                "file" => record
                    .local_file_path
                    .as_ref()
                    .and_then(|path| std::fs::metadata(path).ok())
                    .map(|metadata| max_file_size > 0 && metadata.len() <= max_file_size)
                    .unwrap_or(false),
                _ => false,
            };

            if should_update {
                match self
                    .context
                    .repositories()
                    .clip_records()
                    .update_sync_state(&record.id, NOT_SYNCHRONIZED, None)
                    .await
                {
                    Ok(_) => updated_count += 1,
                    Err(error) => {
                        log::error!("恢复记录{}的同步状态失败: {}", record.id, error)
                    }
                }
            }
        }

        if updated_count > 0 {
            log::info!("VIP 状态更新后，已将{}条记录恢复为待同步", updated_count);
        }
        Ok(())
    }

    async fn enforce_local_records_limit_from_db(&self) -> Result<(), String> {
        let max_allowed = self.get_max_records_limit().await?;
        self.enforce_local_records_limit(max_allowed).await?;

        let current_count = self
            .context
            .repositories()
            .clip_records()
            .count_active()
            .await
            .map_err(|error| format!("查询记录总数失败: {}", error))?;
        if current_count > max_allowed as i64 {
            let excess_count = current_count - max_allowed as i64;
            self.context
                .repositories()
                .clip_records()
                .delete_oldest_unpinned(excess_count as i32)
                .await
                .map_err(|error| format!("清理超出记录失败: {}", error))?;
        }
        Ok(())
    }

    fn emit_status_changed(&self) {
        let Ok(Some(info)) = self.store.get_info() else {
            return;
        };
        let payload = VipStatusChangedPayload {
            is_vip: info.vip_flag,
            vip_type: Some(info.vip_type),
            expire_time: info.expire_time,
            max_records: info.max_records,
        };
        if let Some(app_handle) = self.context.try_app_handle() {
            let _ = app_handle.emit("vip-status-changed", payload);
        }
    }
}
