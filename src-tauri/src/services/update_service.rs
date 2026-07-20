use crate::{app_context::AppContext, domain::update::UpdateInfo, system::updater::TauriUpdater};

/// 软件更新应用服务，负责从应用上下文取得 AppHandle 并调用更新系统适配器。
pub struct UpdateService<'a> {
    context: &'a AppContext,
}

impl<'a> UpdateService<'a> {
    /// 使用当前应用上下文创建更新服务。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self { context }
    }

    /// 检查远程更新信息，并返回领域层定义的版本结果。
    pub async fn check_soft_version(&self) -> Result<UpdateInfo, String> {
        // Updater 插件依赖 setup 后的 AppHandle，因此在执行 command 时按需取得。
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        TauriUpdater::new(app_handle).check().await
    }

    /// 下载并安装可用更新；具体插件调用和进度日志由系统适配器负责。
    pub async fn download_and_install_update(&self) -> Result<bool, String> {
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        TauriUpdater::new(app_handle).download_and_install().await
    }
}
