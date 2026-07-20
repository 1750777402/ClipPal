#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
/// 记录跳过云同步的业务原因，与数据库 `skip_type` 保持稳定映射。
pub enum SkipSyncReason {
    // 当前内容不支持再次同步
    Unsupported = 1,
    // VIP 权益限制，权益变化后可以重试
    VipLimit = 2,
}

impl SkipSyncReason {
    /// 返回持久化使用的稳定整数值。
    pub const fn value(self) -> i32 {
        self as i32
    }

    /// 将数据库整数转换为跳过原因；未知值不参与业务判断。
    pub const fn from_value(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Unsupported),
            2 => Some(Self::VipLimit),
            _ => None,
        }
    }

    /// 判断限制解除后是否允许重新进入同步队列。
    pub const fn can_retry(self) -> bool {
        matches!(self, Self::VipLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证只有 VIP 限制解除后允许重新尝试同步。
    fn only_vip_limit_can_retry() {
        assert!(!SkipSyncReason::Unsupported.can_retry());
        assert!(SkipSyncReason::VipLimit.can_retry());
    }
}
