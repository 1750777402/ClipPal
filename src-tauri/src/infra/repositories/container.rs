use std::sync::Arc;

use super::{ClipRecordRepository, SettingsRepository};

#[derive(Clone)]
/// 应用仓储容器，集中持有各领域的数据访问接口并隐藏具体实现类型。
pub struct AppRepositories {
    clip_records: Arc<dyn ClipRecordRepository>,
    settings: Arc<dyn SettingsRepository>,
}

impl AppRepositories {
    /// 组装应用运行期使用的全部仓储；具体实现由 bootstrap 创建并注入。
    pub fn new(
        clip_records: Arc<dyn ClipRecordRepository>,
        settings: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self {
            clip_records,
            settings,
        }
    }

    /// 获取剪贴记录仓储，用于查询、置顶和逻辑删除记录。
    pub fn clip_records(&self) -> &dyn ClipRecordRepository {
        self.clip_records.as_ref()
    }

    /// 获取设置仓储，用于加载和持久化配置文件。
    pub fn settings(&self) -> &dyn SettingsRepository {
        self.settings.as_ref()
    }
}
