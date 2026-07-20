/// 数据库存储的“未同步”状态值。
pub const NOT_SYNCHRONIZED: i32 = SyncStatus::NotSynchronized as i32;
/// 数据库存储的“同步中”状态值。
pub const SYNCHRONIZING: i32 = SyncStatus::Synchronizing as i32;
/// 数据库存储的“已同步”状态值。
pub const SYNCHRONIZED: i32 = SyncStatus::Synchronized as i32;
/// 数据库存储的“跳过同步”状态值。
pub const SKIP_SYNC: i32 = SyncStatus::Skipped as i32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
/// 剪贴记录云同步生命周期，与数据库 `sync_flag` 数值保持稳定映射。
pub enum SyncStatus {
    // 未同步
    NotSynchronized = 0,
    // 同步中
    Synchronizing = 1,
    // 已同步
    Synchronized = 2,
    // 当前记录跳过同步
    Skipped = 3,
}

impl SyncStatus {
    /// 返回写入数据库和发送前端事件使用的稳定整数值。
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// 从数据库整数恢复同步状态；无法识别的值返回 `None`，避免错误解释数据。
    pub const fn from_value(value: i32) -> Option<Self> {
        match value {
            NOT_SYNCHRONIZED => Some(Self::NotSynchronized),
            SYNCHRONIZING => Some(Self::Synchronizing),
            SYNCHRONIZED => Some(Self::Synchronized),
            SKIP_SYNC => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证数据库状态数值与领域枚举之间的稳定映射。
    fn converts_persisted_values() {
        assert_eq!(SyncStatus::from_value(0), Some(SyncStatus::NotSynchronized));
        assert_eq!(SyncStatus::from_value(3), Some(SyncStatus::Skipped));
        assert_eq!(SyncStatus::from_value(4), None);
    }
}
