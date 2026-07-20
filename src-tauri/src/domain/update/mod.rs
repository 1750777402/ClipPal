use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 软件更新检查结果，作为 service、system adapter 和前端之间的稳定契约。
pub struct UpdateInfo {
    /// 是否有新版本。
    pub has_update: bool,
    /// 当前版本。
    pub current_version: String,
    /// 最新版本。
    pub latest_version: String,
    /// 更新日志或描述。
    pub body: Option<String>,
    /// 更新包大小，单位为字节。
    pub size: Option<u64>,
    /// 发布日期。
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 软件更新下载进度。
pub struct UpdateProgress {
    /// 已下载字节数。
    pub downloaded: u64,
    /// 总字节数。
    pub total: u64,
    /// 进度百分比，范围为 0-100。
    pub percentage: u8,
}
