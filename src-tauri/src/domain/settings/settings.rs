use serde::{Deserialize, Serialize};

/// 大内容可以直接信任布隆过滤器结果的默认字节阈值。
pub static DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD: usize = 1024 * 1024;
/// 小内容直接执行字符串包含查询的默认字节阈值。
pub static DEFAULT_DIRECT_CONTAINS_THRESHOLD: usize = 128 * 1024;
/// 云同步任务默认检查间隔，单位为秒。
pub static SYNC_INTERVAL_SECONDS: u32 = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
/// 应用设置领域模型，同时作为配置文件和前端设置表单的数据契约。
pub struct Settings {
    /// 本地允许保留的最大剪贴记录数。
    pub max_records: u32,
    /// 是否开机自启动，`1` 表示开启。
    pub auto_start: u32,
    /// 唤起主窗口的全局快捷键字符串。
    pub shortcut_key: String,
    /// 是否启用云同步，`1` 表示开启。
    pub cloud_sync: u32,
    /// 复制后是否自动向前台窗口粘贴，`1` 表示开启。
    pub auto_paste: u32,
    /// 新手教程是否已完成，`1` 表示完成。
    pub tutorial_completed: u32,
    /// 大内容直接信任布隆过滤器的字节阈值。
    pub bloom_filter_trust_threshold: Option<usize>,
    /// 小内容直接执行字符串包含查询的字节阈值。
    pub direct_contains_threshold: Option<usize>,
    /// 云同步定时检查间隔，单位为秒。
    pub cloud_sync_interval: u32,
}

impl Default for Settings {
    /// 构造首次启动和配置文件损坏时使用的安全默认设置。
    fn default() -> Self {
        Self {
            max_records: 200,
            auto_start: 0,
            shortcut_key: String::from("Ctrl+`"),
            cloud_sync: 0,
            auto_paste: 1,
            tutorial_completed: 0,
            bloom_filter_trust_threshold: Some(DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD),
            direct_contains_threshold: Some(DEFAULT_DIRECT_CONTAINS_THRESHOLD),
            cloud_sync_interval: SYNC_INTERVAL_SECONDS,
        }
    }
}
