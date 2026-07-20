use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    biz::vip_checker::VipChecker,
    domain::vip::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
        VipInfo, VipLimits, VipType,
    },
    infra::{
        http::vip_client::{HttpVipClient, VipClient},
        repositories::VipRepository,
    },
};

#[derive(Default)]
/// 默认 VIP 仓储实现，组合权益检查器与 VIP HTTP Client。
pub struct DefaultVipRepository {
    client: HttpVipClient,
}

#[async_trait]
impl VipRepository for DefaultVipRepository {
    /// 初始化服务端 VIP 状态，并执行本地记录数量限制。
    async fn initialize_and_enforce_limits(&self) -> Result<(), String> {
        VipChecker::initialize_vip_and_enforce_limits()
            .await
            .map_err(|error| error.to_string())
    }

    /// 从加密存储读取本地 VIP 缓存。
    fn get_local_info(&self) -> Result<Option<VipInfo>, String> {
        VipChecker::get_local_vip_info().map_err(|error| error.to_string())
    }

    /// 使用当前登录和 VIP 状态判断云同步权限。
    async fn check_cloud_sync_permission(&self) -> Result<(bool, String), String> {
        VipChecker::check_cloud_sync_permission()
            .await
            .map_err(|error| error.to_string())
    }

    /// 组合实时 VIP 判断、本地限制缓存和云同步权限，生成前端权益摘要。
    async fn get_limits(&self) -> Result<VipLimits, String> {
        // 实时状态检查会在成功时同步更新本地 VIP 缓存。
        let is_vip = VipChecker::is_vip_user()
            .await
            .map_err(|error| error.to_string())?;
        // 本地没有权益缓存时使用免费用户默认限制。
        let (max_records, max_file_size) = match self.get_local_info()? {
            Some(info) => (info.max_records, info.max_file_size * 1024),
            None => (300, 0),
        };
        let can_cloud_sync = VipChecker::check_cloud_sync_permission_with_vip_status(Some(is_vip))
            .await
            .map_err(|error| error.to_string())?
            .0;

        Ok(VipLimits {
            is_vip,
            max_records,
            max_file_size,
            can_cloud_sync,
        })
    }

    /// 从服务端刷新 VIP 状态，具体缓存和记录限制更新由 VipChecker 完成。
    async fn refresh(&self) -> Result<bool, String> {
        VipChecker::refresh_vip_from_server()
            .await
            .map_err(|error| error.to_string())
    }

    /// 通过 HTTP Client 获取全部服务端 VIP 配置。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        self.client.get_server_config().await
    }

    /// 通过 HTTP Client 创建支付订单。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        self.client.get_pay_url(param).await
    }

    /// 通过 HTTP Client 查询支付订单结果。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        self.client.get_pay_result(param).await
    }
}
