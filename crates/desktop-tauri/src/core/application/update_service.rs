use {
    crate::app_context::AppContext,
    desktop_app_core::update::{UpdateInfo, UpdateProgress},
    crate::desktop::system::updater::TauriUpdater,
};

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
        TauriUpdater::new(app_handle, self.context.updater()).check().await
    }

    /// 下载并安装可用更新；具体插件调用和进度日志由系统适配器负责。
    pub async fn download_and_install_update(
        &self,
        version: &str,
        on_progress: impl Fn(UpdateProgress),
    ) -> Result<(), String> {
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        // 安装程序或系统权限对话框可能夺取焦点，期间保留更新窗口。
        let _guard = crate::desktop::shell::window::WindowHideGuard::new(self.context.window_hide_flag());
        TauriUpdater::new(app_handle.clone(), self.context.updater())
            .download_and_install(version, on_progress)
            .await?;

        // Windows 由插件交给安装器并退出旧进程。其他平台请求按应用生命周期重启。
        #[cfg(not(target_os = "windows"))]
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            app_handle.request_restart();
        });
        Ok(())
    }
}
