//! 桌面记录服务的薄适配，实际查询和删除规则复用单机应用核心。
use crate::app_context::AppContext;
use desktop_app_core::records::ClipRecordService as LocalRecordService;

/// 保留桌面 IPC 的调用入口，不在此处重复实现数据库业务。
pub struct ClipRecordService<'a>{local:LocalRecordService<'a>}
impl<'a> ClipRecordService<'a>{
    /// 从桌面 AppContext 借用单机核心，保证各版本共享相同本地行为。
    pub fn from_context(context:&'a AppContext)->Self{Self{local:LocalRecordService::new(context.core())}}
}
impl<'a> std::ops::Deref for ClipRecordService<'a>{
    type Target=LocalRecordService<'a>;
    /// 将查询、全文、置顶和删除等调用交给单机服务。
    fn deref(&self)->&Self::Target{&self.local}
}
