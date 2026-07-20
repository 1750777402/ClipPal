use std::sync::Arc;

use super::{AuthRepository, ClipRecordRepository, SettingsRepository, VipRepository};

#[derive(Clone)]
/// 应用仓储容器，集中持有各领域的数据访问接口并隐藏具体实现类型。
pub struct AppRepositories {
    clip_records: Arc<dyn ClipRecordRepository>,
    settings: Arc<dyn SettingsRepository>,
    auth: Arc<dyn AuthRepository>,
    vip: Arc<dyn VipRepository>,
}

impl AppRepositories {
    /// 组装应用运行期使用的全部仓储；具体实现由 bootstrap 创建并注入。
    pub fn new(
        clip_records: Arc<dyn ClipRecordRepository>,
        settings: Arc<dyn SettingsRepository>,
        auth: Arc<dyn AuthRepository>,
        vip: Arc<dyn VipRepository>,
    ) -> Self {
        Self {
            clip_records,
            settings,
            auth,
            vip,
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

    /// 获取认证仓储，用于认证 HTTP 请求和安全存储访问。
    pub fn auth(&self) -> &dyn AuthRepository {
        self.auth.as_ref()
    }

    /// 获取 VIP 仓储的借用引用，适合当前异步调用链直接使用。
    pub fn vip(&self) -> &dyn VipRepository {
        self.vip.as_ref()
    }

    /// 克隆 VIP 仓储共享句柄，供需要 `'static` 生命周期的后台任务使用。
    pub fn vip_shared(&self) -> Arc<dyn VipRepository> {
        self.vip.clone()
    }
}
