use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 本地缓存的 VIP 权益信息，用于离线展示和业务限制判断。
pub struct VipInfo {
    pub vip_flag: bool,
    pub vip_type: VipType,
    /// 到期时间戳，单位为毫秒。
    pub expire_time: Option<u64>,
    /// 最大记录数限制。
    pub max_records: u32,
    /// 最大文件大小限制，单位为 KB。
    pub max_file_size: u64,
    /// VIP 功能列表。
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// 服务端支持的 VIP 套餐类型，同时作为配置映射的键。
pub enum VipType {
    /// 免费用户。
    Free,
    /// 月付费用户。
    Monthly,
    /// 季度付费用户。
    Quarterly,
    /// 年付费用户。
    Yearly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 本地保存的云同步通用限制配置。
pub struct ServerConfig {
    /// 服务器控制的文件大小限制。
    pub max_file_size: u64,
    /// 免费用户云同步限制。
    pub free_sync_limit: u32,
    /// VIP 用户云同步限制。
    pub vip_sync_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 服务端 VIP 校验接口返回的数据。
pub struct UserVipInfoResponse {
    /// 服务端用户标识。
    pub user_id: u32,
    /// 当前是否具备 VIP 权益。
    pub vip_flag: bool,
    /// 当前套餐类型；服务端未设置时为空。
    pub vip_type: Option<VipType>,
    /// VIP 到期时间戳。
    pub expire_time: Option<u64>,
    /// 当前账号允许的最大本地记录数。
    pub max_records: u32,
    /// 当前账号允许的最大同步文件大小，单位为 KB。
    pub max_file_size: u64,
    /// 服务端下发的功能标识列表。
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 服务端针对单个 VIP 套餐下发的价格和限制。
pub struct ServerConfigResponse {
    /// 价格，单位为分。
    pub price: i32,
    /// 有效期，单位为天。
    pub period: i32,
    /// 用户文件大小限制。
    pub max_file_size: u64,
    /// 用户本地记录条数限制。
    pub record_limit: u32,
    /// 同步检查间隔，单位为秒。
    pub sync_check_interval: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 创建支付订单时提交的套餐和支付方式。
pub struct PayParam {
    /// 选择的 VIP 类型。
    pub vip_type: String,
    /// 支付方式。
    pub pay_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 支付订单创建成功后返回的二维码和订单号。
pub struct PayCodrUrlResponse {
    /// 支付二维码 URL。
    pub code_url: String,
    /// 业务订单号。
    pub order_no: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 查询支付状态所需的订单参数。
pub struct QueryPayParam {
    /// 业务订单号。
    pub order_no: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 支付状态查询结果。
pub struct QueryPayResponse {
    /// 支付状态。
    pub order_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 汇总给前端使用的当前账户权益限制。
pub struct VipLimits {
    /// 当前是否为 VIP 用户。
    pub is_vip: bool,
    /// 允许保留的最大记录数。
    pub max_records: u32,
    /// 允许同步的最大文件字节数。
    pub max_file_size: u64,
    /// 当前账号是否可以开启云同步。
    pub can_cloud_sync: bool,
}
