use std::sync::Arc;
use serde::Serialize;
use crate::{app_context::AppContext, response::{string_result, CommandResponse}};

#[derive(Serialize)]
pub struct AppCapabilities {
    edition: &'static str,
    cloud_sync: bool,
    account: bool,
    max_records: u32,
}

#[tauri::command]
pub fn get_app_capabilities(state: tauri::State<'_, Arc<AppContext>>) -> CommandResponse<AppCapabilities> {
    let context = state.inner().as_ref();
    string_result(context.policy().max_records(context).map(|max_records| AppCapabilities {
        edition: if cfg!(feature = "commercial-extension") { "commercial" }
            else if cfg!(feature = "sync") { "self-hosted" } else { "local" },
        cloud_sync: cfg!(feature = "sync"),
        account: cfg!(feature = "commercial-extension"),
        max_records,
    }))
}
