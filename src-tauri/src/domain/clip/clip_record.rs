use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SkipSyncReason, SyncStatus};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
/// 剪贴记录领域模型，保存内容、排序、同步和逻辑删除等业务状态。
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: Value,
    // 内容 md5 值
    pub md5_str: String,
    // 本地文件地址
    pub local_file_path: Option<String>,
    // 时间戳
    pub created: u64,
    // 操作系统类型
    pub os_type: String,
    // 排序字段
    pub sort: i32,
    // 是否置顶
    pub pinned_flag: i32,
    // 云同步状态
    pub sync_flag: Option<i32>,
    // 同步时间
    pub sync_time: Option<u64>,
    // 设备标识
    pub device_id: Option<String>,
    // 云同步版本号
    pub version: Option<i32>,
    // 是否逻辑删除
    pub del_flag: Option<i32>,
    // 是否来自云端同步
    pub cloud_source: Option<i32>,
    // 跳过云同步的原因
    pub skip_type: Option<i32>,
}

impl ClipRecord {
    /// 将持久化的同步状态数值转换为类型安全的 `SyncStatus`；未知数值返回 `None`。
    pub fn sync_status(&self) -> Option<SyncStatus> {
        self.sync_flag.and_then(SyncStatus::from_value)
    }

    /// 将跳过同步原因转换为领域枚举，供业务层判断记录是否能够再次同步。
    pub fn skip_sync_reason(&self) -> Option<SkipSyncReason> {
        self.skip_type.and_then(SkipSyncReason::from_value)
    }

    /// 判断记录当前是否处于置顶状态。
    pub fn is_pinned(&self) -> bool {
        self.pinned_flag == 1
    }

    /// 判断记录是否已被逻辑删除。
    pub fn is_deleted(&self) -> bool {
        self.del_flag == Some(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证持久化数值可以通过领域模型暴露为类型安全状态。
    fn exposes_typed_record_state() {
        let record = ClipRecord {
            pinned_flag: 1,
            sync_flag: Some(SyncStatus::Synchronized.value()),
            skip_type: Some(SkipSyncReason::VipLimit.value()),
            ..Default::default()
        };

        assert!(record.is_pinned());
        assert!(!record.is_deleted());
        assert_eq!(record.sync_status(), Some(SyncStatus::Synchronized));
        assert_eq!(record.skip_sync_reason(), Some(SkipSyncReason::VipLimit));
    }
}
