//! macOS 辅助功能授权。状态只来自系统 TCC，不写入设置文件，也不尝试替用户授权。
use serde::Serialize;
use crate::{app_context::AppContext, errors::{AppError, AppResult}};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoPastePermission {
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    NotRequired,
    Required,
    Granted,
}

/// 普通查询不携带 prompt 选项，打开页面或重新聚焦不会触发系统弹窗。
pub fn status() -> AutoPastePermission {
    #[cfg(target_os = "macos")]
    {
        if macos::is_trusted() { AutoPastePermission::Granted }
        else { AutoPastePermission::Required }
    }
    #[cfg(not(target_os = "macos"))]
    { AutoPastePermission::NotRequired }
}

/// 仅由用户主动授权或权限提示的确认按钮调用。
/// Apple 的授权提示是异步的；返回 Required 仅代表此刻尚未授权，不能视为用户拒绝。
pub async fn request(context: &AppContext) -> AppResult<AutoPastePermission> {
    #[cfg(target_os = "macos")]
    {
        if status() == AutoPastePermission::Granted { return Ok(AutoPastePermission::Granted); }
        let app = context.app_handle()?;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || { let _ = sender.send(macos::request_trust()); })
            .map_err(|error| AppError::System(format!("调度辅助功能授权请求失败: {}", error)))?;
        let trusted = receiver.await
            .map_err(|error| AppError::System(format!("辅助功能授权请求回调失败: {}", error)))?;
        Ok(if trusted { AutoPastePermission::Granted } else { AutoPastePermission::Required })
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = context; Ok(AutoPastePermission::NotRequired) }
}

/// 系统不再次展示授权提示时，仍可直接进入对应设置页面手工启用。
pub fn open_settings(context: &AppContext) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_opener::OpenerExt;
        context.app_handle()?.opener()
            .open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility", None::<&str>)
            .map_err(|error| AppError::System(format!("打开辅助功能设置失败: {}", error)))
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = context; Err(AppError::Config("当前平台无需 macOS 辅助功能授权".into())) }
}

#[cfg(target_os = "macos")]
mod macos {
    use core_foundation::{base::TCFType, boolean::CFBoolean,
        dictionary::{CFDictionary, CFDictionaryRef}, string::{CFString, CFStringRef}};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    pub fn is_trusted() -> bool { unsafe { AXIsProcessTrusted() != 0 } }

    pub fn request_trust() -> bool {
        // 使用 SDK 导出的选项键与 CFBoolean，CF 对象由 RAII 正确管理生命周期。
        let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_query_is_repeatable_and_uses_stable_ipc_values() {
        // 只读系统检测，不展示原生提示，不修改真实权限。
        let result = status();
        assert_eq!(serde_json::to_value(AutoPastePermission::Required).unwrap(), "required");
        assert_eq!(serde_json::to_value(AutoPastePermission::Granted).unwrap(), "granted");
        assert_eq!(serde_json::to_value(AutoPastePermission::NotRequired).unwrap(), "not_required");
        #[cfg(target_os = "macos")]
        assert!(matches!(result, AutoPastePermission::Required | AutoPastePermission::Granted));
        #[cfg(not(target_os = "macos"))]
        assert_eq!(result, AutoPastePermission::NotRequired);
    }
}
