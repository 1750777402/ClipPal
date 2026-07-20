use rbatis::RBatis;

use crate::{errors::AppResult, sqlite_storage};

/// 初始化并返回应用唯一的 RBatis/SQLite 连接池句柄。
///
/// 表结构创建和迁移由底层 SQLite 初始化流程完成；bootstrap 只依赖此统一入口。
pub async fn connect() -> AppResult<RBatis> {
    sqlite_storage::init_sqlite().await
}
