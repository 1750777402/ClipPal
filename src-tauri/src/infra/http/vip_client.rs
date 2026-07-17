use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    api::vip_api::{
        self, PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
    },
    utils::secure_store::VipType,
};

#[async_trait]
pub trait VipClient: Send + Sync {
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String>;

    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String>;

    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String>;
}

#[derive(Default)]
pub struct HttpVipClient;

#[async_trait]
impl VipClient for HttpVipClient {
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        vip_api::get_server_config()
            .await
            .map_err(|error| error.to_string())
    }

    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        vip_api::get_pay_url(param)
            .await
            .map_err(|error| error.to_string())
    }

    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        vip_api::get_pay_result(param)
            .await
            .map_err(|error| error.to_string())
    }
}
