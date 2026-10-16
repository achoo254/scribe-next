//! Sổ của tính năng tóm tắt: lượt mỗi ngày (giờ Việt Nam) của từng tài khoản (`summary_usage`, doc
//! `<userId>:<YYYY-MM-DD>`) và nhật ký mỗi lượt đã gọi model (`summary_runs`, chỉ thêm: token, chi phí
//! quy ra giây tương đương đã cộng vào trần tổng tháng).
//!
//! Như sổ phút chép lời: Mongo prod là standalone, mỗi bước là một lệnh nguyên tử trên một doc.

use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;

const USAGE_COLL: &str = "summary_usage";
pub const RUNS_COLL: &str = "summary_runs";

fn day_doc(user_id: &str, day: &str) -> String {
    format!("{user_id}:{day}")
}

/// Lấy một lượt của ngày nếu chưa đủ `limit`. `false` = hết lượt.
pub async fn take_daily(db: &Database, user_id: &str, day: &str, limit: i64) -> Result<bool, AppError> {
    if limit <= 0 {
        return Ok(false);
    }
    let coll = db.collection::<Document>(USAGE_COLL);
    let id = day_doc(user_id, day);
    let created = coll
        .update_one(doc! { "_id": &id }, doc! { "$setOnInsert": { "count": 0_i64, "updatedAt": DateTime::now() } })
        .upsert(true)
        .await;
    match created {
        Ok(_) => {}
        // Hai request cùng tạo doc ngày: bên thua thấy doc đã có, đúng ý.
        Err(e) if is_duplicate_key(&e) => {}
        Err(e) => return Err(AppError::Internal(format!("tạo sổ tóm tắt: {e}"))),
    }
    let res = coll
        .update_one(
            doc! { "_id": &id, "count": { "$lt": limit } },
            doc! { "$inc": { "count": 1_i64 }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("lấy lượt tóm tắt: {e}")))?;
    Ok(res.matched_count > 0)
}

/// Trả lại một lượt (model lỗi, người dùng không nhận được gì).
pub async fn refund_daily(db: &Database, user_id: &str, day: &str) {
    let res = db
        .collection::<Document>(USAGE_COLL)
        .update_one(
            doc! { "_id": day_doc(user_id, day), "count": { "$gt": 0 } },
            doc! { "$inc": { "count": -1_i64 }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await;
    if let Err(e) = res {
        tracing::warn!(user = %user_id, "trả lượt tóm tắt lỗi: {e}");
    }
}

/// Số lượt đã dùng trong ngày (để báo cho người dùng).
pub async fn used_daily(db: &Database, user_id: &str, day: &str) -> Result<i64, AppError> {
    let d = db
        .collection::<Document>(USAGE_COLL)
        .find_one(doc! { "_id": day_doc(user_id, day) })
        .await
        .map_err(|e| AppError::Internal(format!("đọc sổ tóm tắt: {e}")))?;
    Ok(d.and_then(|d| d.get_i64("count").ok().or_else(|| d.get_i32("count").ok().map(i64::from))).unwrap_or(0))
}

/// Một lượt đã gọi model (thành công hay không), để đối soát trần tổng.
pub struct Run<'a> {
    pub user_id: &'a str,
    pub key: &'a str,
    pub model: &'a str,
    pub month: &'a str,
    pub day: &'a str,
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// Chi phí quy ra giây chép lời tương đương, đã cộng vào trần tổng tháng.
    pub cost_s: i64,
    pub ok: bool,
}

pub async fn record_run(db: &Database, run: &Run<'_>) {
    let d = doc! {
        "userId": run.user_id,
        "key": run.key,
        "model": run.model,
        "month": run.month,
        "day": run.day,
        "inputTokens": run.input_tokens,
        "outputTokens": run.output_tokens,
        "costS": run.cost_s,
        "ok": run.ok,
        "at": DateTime::now(),
    };
    if let Err(e) = db.collection::<Document>(RUNS_COLL).insert_one(d).await {
        tracing::error!(user = %run.user_id, "ghi summary_runs lỗi: {e}");
    }
}

/// Tổng giây tương đương của các lượt tóm tắt trong tháng (lệnh đối soát trần tổng).
pub async fn month_cost_seconds(db: &Database, month: &str) -> Result<i64, AppError> {
    use futures_util::StreamExt;
    let pipeline = vec![
        doc! { "$match": { "month": month } },
        doc! { "$group": { "_id": null, "total": { "$sum": "$costS" } } },
    ];
    let mut cur = db
        .collection::<Document>(RUNS_COLL)
        .aggregate(pipeline)
        .await
        .map_err(|e| AppError::Internal(format!("cộng summary_runs: {e}")))?;
    let Some(d) = cur.next().await else { return Ok(0) };
    let d = d.map_err(|e| AppError::Internal(format!("cộng summary_runs: {e}")))?;
    Ok(d.get_i64("total").ok().or_else(|| d.get_i32("total").ok().map(i64::from)).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// summary_daily -- --ignored`. Ngày giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn summary_daily_counter_never_exceeds_limit_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());
        let day = "test-day";
        let tasks = (0..25).map(|_| take_daily(&db, &user, day, 10));
        let ok = futures_util::future::join_all(tasks).await.into_iter().filter(|r| matches!(r, Ok(true))).count();
        assert_eq!(ok, 10);
        assert_eq!(used_daily(&db, &user, day).await.expect("đọc"), 10);
        refund_daily(&db, &user, day).await;
        assert!(take_daily(&db, &user, day, 10).await.expect("lấy lại"));
        assert!(!take_daily(&db, &user, day, 10).await.expect("hết"));
        assert!(!take_daily(&db, &user, "other-day", 0).await.expect("giới hạn 0"));
        db.collection::<Document>(USAGE_COLL).delete_many(doc! { "_id": { "$regex": format!("^{user}:") } }).await.expect("dọn");
    }
}
