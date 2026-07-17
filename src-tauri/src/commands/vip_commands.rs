use std::{collections::HashMap, sync::Arc};

use crate::{
    api::vip_api::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
    },
    app_context::AppContext,
    response::{string_result, CommandResponse},
    services::vip_service::VipService,
    utils::secure_store::{VipInfo, VipType},
};

#[tauri::command]
pub async fn get_vip_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<VipInfo>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_vip_status()))
}

#[tauri::command]
pub async fn check_vip_permission(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<(bool, String)>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_vip_permission().await))
}

#[tauri::command]
pub async fn get_vip_limits(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<serde_json::Value>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_vip_limits().await))
}

#[tauri::command]
pub async fn open_vip_purchase_page(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<()>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.open_vip_purchase_page()))
}

#[tauri::command]
pub async fn refresh_vip_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.refresh_vip_status().await))
}

#[tauri::command]
pub async fn get_server_config(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<HashMap<VipType, ServerConfigResponse>>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_server_config().await))
}

#[tauri::command]
pub async fn get_pay_url(
    state: tauri::State<'_, Arc<AppContext>>,
    param: PayParam,
) -> Result<CommandResponse<Option<PayCodrUrlResponse>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_pay_url(param).await))
}

#[tauri::command]
pub async fn get_pay_result(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryPayParam,
) -> Result<CommandResponse<Option<QueryPayResponse>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_pay_result(param).await))
}
