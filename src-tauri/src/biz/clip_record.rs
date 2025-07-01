use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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
    // 是否置顶
    pub pinned_flag: i32,
    // 是否已同步云端  0:未同步，1:已同步
    pub sync_flag: Option<i32>,
    // 同步时间
    pub sync_time: Option<u64>,
    // 设备标识
    pub device_id: Option<String>,
}

crud!(ClipRecord {}, "clip_record");
impl_select!(ClipRecord{select_by_id(id: &str) =>"`where id = #{id}`"});
impl_select!(ClipRecord{select_by_pinned_flag(pinned_flag: i32) =>"`where pinned_flag = #{pinned_flag}`"});
impl_select!(ClipRecord{select_order_by() =>"`order by sort desc, created desc`"});
impl_select!(ClipRecord{select_where_order_by_limit(content: &str, limit:i32, offset:i32) =>"` where content like #{content} order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
//  根据limit和offset 查询   获取limit条数据(-1表示全部)   跳过前offset条数据
impl_select!(ClipRecord{select_order_by_limit(limit:i32, offset:i32) =>"`order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_content(content_type:&str, content:&str) =>"`where type = #{content_type} and content = #{content} limit 1`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_md5(content_type:&str, md5_str:&str) =>"`where type = #{content_type} and md5_str = #{md5_str} limit 1`"});
// 取出最大的sort数据
impl_select!(ClipRecord{select_max_sort(user_id: i32) =>"`where user_id = #{user_id} order by sort desc, created desc limit 1`"});

impl ClipRecord {
    pub async fn update_content(rb: &RBatis, id: &str, content: &String) -> Result<(), Error> {
        let sql = "UPDATE clip_record SET content = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(content), to_value!(id)]).await;
        tx.commit().await
    }

    pub async fn update_sort(rb: &RBatis, id: &str, sort: i32) -> Result<(), Error> {
        let sql = "UPDATE clip_record SET sort = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(sort), to_value!(id)]).await;
        tx.commit().await
    }

    pub async fn update_pinned(rb: &RBatis, id: &str, pinned_flag: i32) -> Result<(), Error> {
        let sql = "UPDATE clip_record SET pinned_flag = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        if pinned_flag == 1 {
            // 置顶某一条的时候  先把其他的置顶都取消
            let sql1 = "UPDATE clip_record SET pinned_flag = 0 WHERE pinned_flag = 1";
            let _ = tx.exec(sql1, vec![]).await;
        }
        let _ = tx
            .exec(sql, vec![to_value!(pinned_flag), to_value!(id)])
            .await;
        tx.commit().await
    }

    pub async fn count(rb: &RBatis) -> i64 {
        let count_res: Result<i64, rbs::Error> = rb
            .query_decode("SELECT COUNT(*) FROM clip_record", vec![])
            .await;
        match count_res {
            Ok(count) => return count,
            Err(_) => return 0,
        }
    }

    pub async fn del_by_ids(rb: &RBatis, ids: &Vec<String>) -> Result<(), Error> {
        let sql = format!(
            "DELETE FROM clip_record WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let tx = rb.acquire_begin().await?;
        // 转换ids为Vec<Value>
        let params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let _ = tx.exec(&sql, params).await?;
        tx.commit().await
    }

    pub async fn select_by_ids(
        rb: &RBatis,
        ids: &Vec<String>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<ClipRecord>, Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        
        let sql = format!(
            "SELECT * FROM clip_record WHERE id IN ({}) ORDER BY pinned_flag DESC, sort DESC, created DESC",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        // 转换ids为Vec<Value>
        let mut params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        params.push(to_value!(limit));
        params.push(to_value!(offset));
        let limit_sql = format!("{} LIMIT ? OFFSET ?", sql);
        let res: Vec<ClipRecord> = rb.query_decode(limit_sql.as_str(), params).await?;
        Ok(res)
    }
}
