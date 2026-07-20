use crate::{domain::settings::Settings, errors::AppResult};

/// 设置仓储接口，隔离应用服务与配置文件的具体读写格式和路径。
pub trait SettingsRepository: Send + Sync {
    /// 加载当前设置；底层不可用时返回默认配置。
    fn load(&self) -> Settings;

    /// 将完整设置持久化，写入失败时返回统一应用错误。
    fn save(&self, settings: &Settings) -> AppResult<()>;
}
