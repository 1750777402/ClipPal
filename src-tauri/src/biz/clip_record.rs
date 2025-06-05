use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: Value,
    // 内容md5值
    pub md5_str: String,
    // 时间戳
    pub created: u64,
    // 用户id
    pub user_id: i32,
    // os类型
    pub os_type: String,
    // 排序字段
    pub sort: i32,
}

crud!(ClipRecord {}, "clip_record");
impl_select!(ClipRecord{select_order_by() =>"`order by created desc`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_content(content_type:&str, content:&str) =>"`where type = #{content_type} and content = #{content} limit 1`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_md5(content_type:&str, md5_str:&str) =>"`where type = #{content_type} and md5_str = #{md5_str} limit 1`"});

impl ClipRecord {
    pub async fn update_content(rb: &RBatis, id: &str, content: &Value) -> Result<(), Error> {
        let sql = "UPDATE clip_record SET content = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(content), to_value!(id)]).await;
        tx.commit().await
    }
}
