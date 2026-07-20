use std::{collections::HashMap, sync::Arc};

use crate::{
    app_context::AppContext,
    domain::vip::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
        VipInfo, VipLimits, VipType,
    },
    response::{string_result, CommandResponse},
    services::vip_service::VipService,
};

#[tauri::command]
/// 读取本地缓存的 VIP 状态，用于前端快速展示账户权益。
pub async fn get_vip_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<VipInfo>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_vip_status()))
}

#[tauri::command]
/// 检查当前用户是否具备云同步权限，并返回判断原因。
pub async fn check_vip_permission(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<(bool, String)>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_vip_permission().await))
}

#[tauri::command]
/// 计算当前用户可用的记录数、文件大小和云同步限制。
pub async fn get_vip_limits(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<VipLimits>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_vip_limits().await))
}

#[tauri::command]
/// 使用系统浏览器打开 VIP 购买页面。
pub async fn open_vip_purchase_page(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<()>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.open_vip_purchase_page()))
}

#[tauri::command]
/// 从服务端刷新 VIP 状态，并在状态更新后通知前端。
pub async fn refresh_vip_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.refresh_vip_status().await))
}

#[tauri::command]
/// 获取服务端下发的各 VIP 类型价格和权益配置。
pub async fn get_server_config(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<HashMap<VipType, ServerConfigResponse>>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_server_config().await))
}

#[tauri::command]
/// 创建支付订单并获取用于展示的支付二维码地址。
pub async fn get_pay_url(
    state: tauri::State<'_, Arc<AppContext>>,
    param: PayParam,
) -> Result<CommandResponse<Option<PayCodrUrlResponse>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_pay_url(param).await))
}

#[tauri::command]
/// 根据订单号查询支付状态，供前端轮询支付结果。
pub async fn get_pay_result(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryPayParam,
) -> Result<CommandResponse<Option<QueryPayResponse>>, String> {
    let service = VipService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_pay_result(param).await))
}
