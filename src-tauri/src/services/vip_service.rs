use std::collections::HashMap;

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_opener::OpenerExt;

use crate::{
    api::vip_api::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
    },
    app_context::AppContext,
    biz::vip_checker::VipChecker,
    infra::http::vip_client::{HttpVipClient, VipClient},
    utils::secure_store::{VipInfo, VipType},
};

#[derive(Serialize, Clone)]
struct VipStatusChangedPayload {
    is_vip: bool,
    vip_type: Option<VipType>,
    expire_time: Option<u64>,
    max_records: u32,
}

pub struct VipService<'a, C> {
    context: &'a AppContext,
    client: C,
}

impl<'a> VipService<'a, HttpVipClient> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            client: HttpVipClient,
        }
    }
}

impl<C> VipService<'_, C>
where
    C: VipClient,
{
    pub fn get_vip_status(&self) -> Result<Option<VipInfo>, String> {
        VipChecker::get_local_vip_info().map_err(|error| error.to_string())
    }

    pub async fn check_vip_permission(&self) -> Result<(bool, String), String> {
        VipChecker::check_cloud_sync_permission()
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn get_vip_limits(&self) -> Result<serde_json::Value, String> {
        let is_vip = VipChecker::is_vip_user()
            .await
            .map_err(|error| error.to_string())?;

        let (max_records, max_file_size) =
            if let Ok(Some(vip_info)) = VipChecker::get_local_vip_info() {
                (vip_info.max_records, vip_info.max_file_size * 1024)
            } else {
                (300, 0)
            };

        let can_cloud_sync = VipChecker::check_cloud_sync_permission_with_vip_status(Some(is_vip))
            .await
            .map_err(|error| error.to_string())?
            .0;

        Ok(serde_json::json!({
            "isVip": is_vip,
            "maxRecords": max_records,
            "maxFileSize": max_file_size,
            "canCloudSync": can_cloud_sync
        }))
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

    pub async fn refresh_vip_status(&self) -> Result<bool, String> {
        match VipChecker::refresh_vip_from_server().await {
            Ok(updated) => {
                if updated {
                    if let Ok(Some(info)) = VipChecker::get_local_vip_info() {
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
                Ok(updated)
            }
            Err(error) => {
                log::error!("刷新 VIP 状态失败: {}", error);
                Err(error.to_string())
            }
        }
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
}
