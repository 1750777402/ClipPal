use std::collections::HashMap;

use serde::Serialize;
use tauri::Emitter;
use tauri_plugin_opener::OpenerExt;

use crate::{
    app_context::AppContext,
    domain::vip::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
        VipInfo, VipLimits, VipType,
    },
    infra::repositories::VipRepository,
};

#[derive(Serialize, Clone)]
struct VipStatusChangedPayload {
    is_vip: bool,
    vip_type: Option<VipType>,
    expire_time: Option<u64>,
    max_records: u32,
}

/// VIP 应用服务，负责编排权益仓储、购买页面和前端状态变更事件。
pub struct VipService<'a> {
    context: &'a AppContext,
    repository: &'a dyn VipRepository,
}

impl<'a> VipService<'a> {
    /// 从应用上下文取得 VIP 仓储，创建轻量服务实例。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: context.repositories().vip(),
        }
    }
    /// 读取本地缓存的 VIP 信息，不触发网络刷新。
    pub fn get_vip_status(&self) -> Result<Option<VipInfo>, String> {
        self.repository.get_local_info()
    }

    /// 检查当前账号是否允许使用云同步，并返回判断原因。
    pub async fn check_vip_permission(&self) -> Result<(bool, String), String> {
        self.repository.check_cloud_sync_permission().await
    }

    /// 获取当前权益限制，包括记录数、文件大小和云同步能力。
    pub async fn get_vip_limits(&self) -> Result<VipLimits, String> {
        self.repository.get_limits().await
    }

    /// 使用系统浏览器打开固定的 VIP 购买页面。
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

    /// 从服务端刷新 VIP 状态；刷新成功且数据变化时向前端发送状态事件。
    pub async fn refresh_vip_status(&self) -> Result<bool, String> {
        // 仓储负责服务端校验、本地加密缓存和权益限制更新。
        match self.repository.refresh().await {
            Ok(updated) => {
                if updated {
                    // 仅在能够读取到最新本地状态时构造完整事件载荷。
                    if let Ok(Some(info)) = self.repository.get_local_info() {
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

    /// 获取服务端配置的各类 VIP 价格和限制。
    pub async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        self.repository.get_server_config().await
    }

    /// 创建支付订单并返回二维码信息。
    pub async fn get_pay_url(&self, param: PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        self.repository.get_pay_url(&param).await
    }

    /// 查询指定支付订单的当前状态。
    pub async fn get_pay_result(
        &self,
        param: QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        self.repository.get_pay_result(&param).await
    }
}
