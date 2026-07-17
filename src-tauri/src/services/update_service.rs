use crate::{app_context::AppContext, system::updater::TauriUpdater, updater::UpdateInfo};

pub struct UpdateService<'a> {
    context: &'a AppContext,
}

impl<'a> UpdateService<'a> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self { context }
    }

    pub async fn check_soft_version(&self) -> Result<UpdateInfo, String> {
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        TauriUpdater::new(app_handle).check().await
    }

    pub async fn download_and_install_update(&self) -> Result<bool, String> {
        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        TauriUpdater::new(app_handle).download_and_install().await
    }
}
