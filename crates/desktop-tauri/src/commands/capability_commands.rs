use std::sync::Arc;
use serde::Serialize;
use crate::{app_context::AppContext, response::{string_result, CommandResponse}};

/// 前端可用能力快照；界面按此决定显示入口，实际权限仍由后端校验。
#[derive(Serialize)]
pub struct AppCapabilities {
    /// 当前编译版本：local / self-hosted / commercial。
    edition: &'static str,
    /// 新同步实现完成前固定关闭，不能仅凭编译 feature 宣称同步可用。
    cloud_sync: bool,
    /// 只有商业扩展提供账号入口。
    account: bool,
    /// 当前策略允许的本地记录数上限。
    max_records: u32,
}

#[tauri::command]
/// 从后端构建和策略生成能力信息，前端不自行推断权益或版本。
pub fn get_app_capabilities(state: tauri::State<'_, Arc<AppContext>>) -> CommandResponse<AppCapabilities> {
    let context = state.inner().as_ref();
    string_result(context.policy().max_records(context).map(|max_records| AppCapabilities {
        edition: if cfg!(feature = "commercial-extension") { "commercial" }
            else if cfg!(feature = "sync") { "self-hosted" } else { "local" },
        // 新本地表尚未接入同步扩展，禁止旧运行时写入新库。
        cloud_sync: false,
        account: cfg!(feature = "commercial-extension"),
        max_records,
    }))
}
