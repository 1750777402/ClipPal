//! 应用层访问外部系统的能力接口。
//!
//! Service 只依赖这里定义的 Gateway，不关心 HTTP 路径、请求参数等传输细节；
//! `infra` 层负责实现这些 Gateway，并完成实际的远程请求。

mod auth_gateway;
mod vip_gateway;

pub use auth_gateway::AuthGateway;
pub use vip_gateway::VipGateway;
