//! 桌面生命周期只负责启停会话和转发列表变化。
use crate::app_context::AppContext;
use crate::errors::{AppError, AppResult};
use std::{io, path::{Path, PathBuf}, sync::Arc};
use tauri::Emitter;

/// 用户开关与实际后台状态分别返回，连接故障不会把用户的开关拨回去。
#[derive(serde::Serialize)]
pub(crate) struct SyncSettingsStatus {
    requested_enabled: bool,
    #[serde(flatten)]
    status: selfhost_app_core::SyncStatus,
}

/// 自建同步由安装包明确启用，旧配置默认保持关闭。
pub(crate) fn enabled() -> bool {
    crate::desktop::config::get_cloud_sync().is_ok_and(|config| config.enabled)
}

/// 开关只保存一个停用标记。安装包已启用、旧版本没有该标记时沿用自动同步行为。
fn disabled_marker() -> AppResult<PathBuf> {
    crate::desktop::paths::get_config_dir().map(|dir| dir.join("cloud-sync-disabled"))
        .ok_or_else(|| AppError::Config("无法获取本机设置目录".into()))
}

fn is_requested(marker: &Path) -> AppResult<bool> {
    Ok(!marker.try_exists()?)
}

fn save_requested(marker: &Path, requested: bool) -> AppResult<()> {
    if requested {
        match std::fs::remove_file(marker) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    } else {
        // 只关心文件是否存在，创建空文件即可持久化关闭选择。
        std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(marker)?;
    }
    Ok(())
}

/// 启动时先应用本机偏好。无法读取时保留本地功能，并暂停同步。
pub(crate) async fn start(context: &Arc<AppContext>) {
    if !enabled() {
        log::info!("自建版已启动，但 cloud_sync.enabled 未开启，同步保持关闭");
        return;
    }
    let _switch = context.sync_switch_lock().lock().await;
    match disabled_marker().and_then(|path| is_requested(&path)) {
        Ok(true) => start_session(context).await,
        Ok(false) => log::info!("本机已关闭云同步"),
        Err(error) => log::error!("读取本机同步开关失败：{error}"),
    }
}

/// 恢复本机队列。读取同步能力和连接后续在后台执行。
async fn start_session(context: &Arc<AppContext>) {
    log::info!("自建版开始连接同步服务，握手与传输状态可在设置中查看");
    let weak = Arc::downgrade(context);
    let notify = Arc::new(move || {
        if let Some(app) = weak.upgrade().and_then(|context| context.try_app_handle()) {
            let _ = app.emit("clip_record_change", ());
        }
    });
    let encoder = Arc::new(crate::desktop::security::encrypt_content);
    if let Err(error) = context.sync().enable(notify, encoder).await {
        log::error!("启动自建同步失败：{error}");
    }
}

/// 返回与启停操作一致的快照，避免界面轮询覆盖刚切换的状态。
pub(crate) async fn settings_status(context: &AppContext) -> AppResult<SyncSettingsStatus> {
    let _switch = context.sync_switch_lock().lock().await;
    let requested_enabled = enabled() && is_requested(&disabled_marker()?)?;
    Ok(SyncSettingsStatus { requested_enabled, status: context.sync().status().await })
}

/// 保存本机选择并启停会话。停止时现有队列和游标继续保存在 SQLite。
pub(crate) async fn set_enabled(context: &Arc<AppContext>, requested_enabled: bool) -> AppResult<SyncSettingsStatus> {
    if !enabled() {
        return Err(AppError::Config("当前自建包未启用云同步部署配置".into()));
    }
    let _switch = context.sync_switch_lock().lock().await;
    save_requested(&disabled_marker()?, requested_enabled)?;
    if requested_enabled {
        start_session(context).await;
    } else {
        context.sync().disable().await;
    }
    Ok(SyncSettingsStatus { requested_enabled, status: context.sync().status().await })
}

/// 请求退出。已发送但未确认的动作仍保存在 SQLite，下一次启动会重试。
pub(crate) fn stop(context: &Arc<AppContext>) {
    let context = context.clone();
    tauri::async_runtime::spawn(async move {
        let _switch = context.sync_switch_lock().lock().await;
        context.sync().disable().await;
    });
}

#[cfg(test)]
mod tests {
    use super::{is_requested, save_requested};

    /// 自建包升级时沿用原有启动行为，关闭后可从磁盘恢复选择。
    #[test]
    fn persists_sync_switch() {
        let marker = std::env::temp_dir().join(format!("clippal-sync-switch-{}", uuid::Uuid::new_v4()));
        assert!(is_requested(&marker).unwrap());
        save_requested(&marker, false).unwrap();
        save_requested(&marker, false).unwrap();
        assert!(!is_requested(&marker).unwrap());
        save_requested(&marker, true).unwrap();
        save_requested(&marker, true).unwrap();
        assert!(is_requested(&marker).unwrap());
    }
}
