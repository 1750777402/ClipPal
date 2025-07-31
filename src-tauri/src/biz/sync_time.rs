use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SyncTime {
    pub id: String,
    // 时间戳
    pub last_time: u64,
}

pub const TABLE_KEY: &str = "last_sync_ts";

crud!(SyncTime {}, "sync_time");
impl_select!(SyncTime{select_by_id(id: &str) =>"`where id = #{id}`"});
impl_select!(SyncTime{select_last() =>"`order by last_time desc`"});

impl SyncTime {
    pub async fn update_last_time(rb: &RBatis, last_time: u64) -> Result<(), Error> {
        let sql = format!(
            "UPDATE sync_time SET last_time = ? WHERE id = {}",
            TABLE_KEY
        );
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql.as_str(), vec![to_value!(last_time)]).await;
        tx.commit().await
    }

    pub async fn insert_last_time(rb: &RBatis, last_time: u64) -> Result<(), Error> {
        let sql = format!(
            "INSERT INTO sync_time (id, last_time) VALUES ('{}', ?)",
            TABLE_KEY
        );
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql.as_str(), vec![to_value!(last_time)]).await;
        tx.commit().await
    }

    pub async fn select_last_time(rb: &RBatis) -> u64 {
        let res = SyncTime::select_by_id(rb, TABLE_KEY)
            .await
            .map_err(|e| format!("获取最后同步时间失败: {}", e));
        match res {
            Ok(sync_time) => {
                if sync_time.is_empty() {
                    let _ = SyncTime::insert_last_time(rb, 0).await;
                    0
                } else {
                    sync_time[0].last_time
                }
            }
            Err(_) => 0, // 如果没有记录，返回0
        }
    }
}
