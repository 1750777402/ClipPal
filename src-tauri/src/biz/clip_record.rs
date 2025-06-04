use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: String,
    // 时间戳
    pub created: u64,
    // 用户id
    pub user_id: i32,
    // os类型
    pub os_type: String,
}

crud!(ClipRecord {}, "clip_record");
impl_select!(ClipRecord{select_order_by() =>"`order by created desc`"});

impl ClipRecord {
    pub async fn update_content(rb: &RBatis, id: &str, content: &str) -> Result<(), Error> {
        let sql = "UPDATE clip_record SET content = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(content), to_value!(id)]).await;
        tx.commit().await
    }
}
