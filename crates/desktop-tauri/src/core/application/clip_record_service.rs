use crate::{app_context::AppContext, errors::AppResult};
use desktop_app_core::records::ClipRecordService as LocalRecordService;

pub struct ClipRecordService<'a> { context: &'a AppContext, local: LocalRecordService<'a> }
impl<'a> ClipRecordService<'a> {
    pub fn from_context(context: &'a AppContext) -> Self { Self { context, local: LocalRecordService::new(context.core()) } }
    pub async fn delete_record(&self, record_id: String) -> AppResult<String> {
        let record = LocalRecordService::new(self.context.core()).delete_record(record_id, cfg!(feature = "sync")).await?;
        #[cfg(feature = "sync")]
        if let Some(record) = record {
            if self.context.cloud_sync_enabled().unwrap_or(false) {
                let queue = self.context.sync().record_queue();
                if !queue.is_full() { let _ = queue.send_delete(record).await; }
            }
        }
        #[cfg(not(feature = "sync"))]
        let _ = record;
        Ok(String::new())
    }
}
impl<'a> std::ops::Deref for ClipRecordService<'a> {
    type Target = LocalRecordService<'a>;
    fn deref(&self) -> &Self::Target { &self.local }
}
