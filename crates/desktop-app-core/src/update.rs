use serde::{Deserialize, Serialize};

/// 检查结果明确区分个人包、当前平台无产物和正常版本检查。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// 当前安装包通过手动获取新版更新。
    Manual,
    /// 有新版，但当前系统或架构没有对应的更新包。
    Unavailable,
    /// 当前版本已是最新。
    UpToDate,
    /// 找到适用于当前系统和架构的新版。
    Available,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 软件更新检查结果，作为 service、system adapter 和前端之间的稳定契约。
pub struct UpdateInfo {
    /// 当前安装包对应的更新状态。
    pub status: UpdateStatus,
    /// 当前版本。
    pub current_version: String,
    /// 可安装的新版本，仅在发现更新时提供。
    pub latest_version: Option<String>,
    /// 更新日志或描述。
    pub body: Option<String>,
    /// 发布日期。
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 当前安装请求的进度，按阶段发送给发起请求的界面。
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum UpdateProgress {
    /// 正在下载，服务端未返回文件大小时 total 为空。
    Downloading { downloaded: u64, total: Option<u64> },
    /// 下载完成，等待 Tauri 验证签名。
    Verifying,
    /// 签名已通过，正在安装或启动系统安装程序。
    Installing,
}
