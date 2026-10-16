//! Sổ của tính năng tóm tắt (`summary_usage`): lượt mỗi ngày giờ Việt Nam của từng tài khoản (doc
//! `<userId>:<YYYY-MM-DD>`, trường `count`) và chi phí tháng của từng tài khoản (doc
//! `cost:<userId>:<YYYY-MM>`, trường `costS` — giây chép lời tương đương). Nhật ký mỗi lượt gọi model ở
//! `summary_runs`: mở (`open: true`, kèm phần đã giữ `reservedS`) trước khi gọi, đóng khi quyết toán
//! (token, `costS`). Lượt còn mở cho lệnh đối soát biết có tóm tắt đang chạy.
//!
//! Như sổ phút chép lời: Mongo prod là standalone, mỗi bước là một lệnh nguyên tử trên một doc.

use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::Database;

const USAGE_COLL: &str = "summary_usage";
pub const RUNS_COLL: &str = "summary_runs";
/// Lượt mở lâu hơn chừng này là lượt bỏ dở (máy chủ tắt giữa chừng): một lượt thật xong trong ~90 giây.
pub const OPEN_RUN_STALE_MINUTES: i64 = 10;

fn day_doc(user_id: &str, day: &str) -> String {
    format!("{user_id}:{day}")
}

fn month_doc(user_id: &str, month: &str) -> String {
    format!("cost:{user_id}:{month}")
}

fn read_i64(d: &Document, field: &str) -> i64 {
    d.get_i64(field).ok().or_else(|| d.get_i32(field).ok().map(i64::from)).unwrap_or(0)
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

/// Trả lại một lượt (OpenAI chưa xử lý gì, người dùng không nhận được gì).
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
    Ok(d.map(|d| read_i64(&d, "count")).unwrap_or(0))
}

/// Giữ `need` giây trên sổ chi phí tháng của user nếu tổng không vượt `limit`. `false` = hết hạn mức.
pub async fn reserve_monthly(db: &Database, user_id: &str, month: &str, need: i64, limit: i64) -> Result<bool, AppError> {
    let coll = db.collection::<Document>(USAGE_COLL);
    let id = month_doc(user_id, month);
    let created = coll
        .update_one(doc! { "_id": &id }, doc! { "$setOnInsert": { "costS": 0_i64, "updatedAt": DateTime::now() } })
        .upsert(true)
        .await;
    match created {
        Ok(_) => {}
        Err(e) if is_duplicate_key(&e) => {}
        Err(e) => return Err(AppError::Internal(format!("tạo sổ chi phí tóm tắt: {e}"))),
    }
    let res = coll
        .update_one(
            doc! { "_id": &id, "costS": { "$lte": limit - need } },
            doc! { "$inc": { "costS": need }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("giữ chi phí tóm tắt: {e}")))?;
    Ok(res.matched_count > 0)
}

/// Điều chỉnh sổ chi phí tháng của user (`delta` âm = hoàn phần giữ dư).
pub async fn settle_monthly(db: &Database, user_id: &str, month: &str, delta: i64) -> Result<(), AppError> {
    if delta == 0 {
        return Ok(());
    }
    db.collection::<Document>(USAGE_COLL)
        .update_one(
            doc! { "_id": month_doc(user_id, month) },
            doc! { "$inc": { "costS": delta }, "$set": { "updatedAt": DateTime::now() } },
        )
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("quyết toán chi phí tóm tắt: {e}")))
}

/// Giây tương đương user đã dùng cho tóm tắt trong tháng (để báo cho người dùng).
pub async fn used_monthly(db: &Database, user_id: &str, month: &str) -> Result<i64, AppError> {
    let d = db
        .collection::<Document>(USAGE_COLL)
        .find_one(doc! { "_id": month_doc(user_id, month) })
        .await
        .map_err(|e| AppError::Internal(format!("đọc sổ chi phí tóm tắt: {e}")))?;
    Ok(d.map(|d| read_i64(&d, "costS")).unwrap_or(0))
}

/// Một lượt sắp gọi model; `reserved_s` là phần đã giữ trên trần tổng và sổ tháng của user.
pub struct Run<'a> {
    pub user_id: &'a str,
    pub key: &'a str,
    pub model: &'a str,
    pub month: &'a str,
    pub day: &'a str,
    pub reserved_s: i64,
}

/// Kết quả quyết toán của một lượt.
pub struct Closing {
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// Chi phí quy ra giây chép lời tương đương, đã cộng vào trần tổng tháng.
    pub cost_s: i64,
    pub ok: bool,
}

/// Mở lượt trước khi gọi model. Lỗi thì không gọi (lệnh đối soát phải thấy mọi lượt đang chạy).
pub async fn open_run(db: &Database, run: &Run<'_>) -> Result<ObjectId, AppError> {
    let id = ObjectId::new();
    let d = doc! {
        "_id": id,
        "userId": run.user_id,
        "key": run.key,
        "model": run.model,
        "month": run.month,
        "day": run.day,
        "open": true,
        "reservedS": run.reserved_s,
        "startedAt": DateTime::now(),
    };
    db.collection::<Document>(RUNS_COLL)
        .insert_one(d)
        .await
        .map(|_| id)
        .map_err(|e| AppError::Internal(format!("mở lượt tóm tắt: {e}")))
}

/// Đóng lượt với số thật. Lỗi chỉ log: sổ đã quyết toán; lượt còn mở được đối soát tính theo `reservedS`.
pub async fn close_run(db: &Database, id: ObjectId, closing: &Closing) {
    let update = doc! {
        "$set": {
            "open": false,
            "inputTokens": closing.input_tokens,
            "outputTokens": closing.output_tokens,
            "costS": closing.cost_s,
            "ok": closing.ok,
            "at": DateTime::now(),
        },
    };
    if let Err(e) = db.collection::<Document>(RUNS_COLL).update_one(doc! { "_id": id }, update).await {
        tracing::error!(run = %id, "đóng lượt summary_runs lỗi: {e}");
    }
}

/// Số lượt tóm tắt của tháng đang chạy (mở chưa quá [`OPEN_RUN_STALE_MINUTES`] phút).
pub async fn running_count(db: &Database, month: &str) -> Result<u64, AppError> {
    let since = DateTime::from_millis(DateTime::now().timestamp_millis() - OPEN_RUN_STALE_MINUTES * 60_000);
    db.collection::<Document>(RUNS_COLL)
        .count_documents(doc! { "month": month, "open": true, "startedAt": { "$gte": since } })
        .await
        .map_err(|e| AppError::Internal(format!("đếm lượt tóm tắt đang chạy: {e}")))
}

/// Tổng giây tương đương của các lượt tóm tắt trong tháng (lệnh đối soát trần tổng). Lượt còn mở tính
/// theo phần đã giữ, đúng như sổ đang ghi.
pub async fn month_cost_seconds(db: &Database, month: &str) -> Result<i64, AppError> {
    use futures_util::StreamExt;
    let charged = doc! { "$cond": [{ "$eq": ["$open", true] }, { "$ifNull": ["$reservedS", 0] }, { "$ifNull": ["$costS", 0] }] };
    let pipeline = vec![
        doc! { "$match": { "month": month } },
        doc! { "$group": { "_id": null, "total": { "$sum": charged } } },
    ];
    let mut cur = db
        .collection::<Document>(RUNS_COLL)
        .aggregate(pipeline)
        .await
        .map_err(|e| AppError::Internal(format!("cộng summary_runs: {e}")))?;
    let Some(d) = cur.next().await else { return Ok(0) };
    let d = d.map_err(|e| AppError::Internal(format!("cộng summary_runs: {e}")))?;
    Ok(read_i64(&d, "total"))
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

        // Sổ chi phí tháng: giữ đồng thời không vượt trần; quyết toán trả phần dư.
        let month = "test-month";
        let tasks = (0..10).map(|_| reserve_monthly(&db, &user, month, 100, 450));
        let ok = futures_util::future::join_all(tasks).await.into_iter().filter(|r| matches!(r, Ok(true))).count();
        assert_eq!(ok, 4);
        settle_monthly(&db, &user, month, -350).await.expect("hoàn");
        assert_eq!(used_monthly(&db, &user, month).await.expect("đọc"), 50);
        assert!(reserve_monthly(&db, &user, month, 400, 450).await.expect("giữ"));
        assert!(!reserve_monthly(&db, &user, month, 1, 450).await.expect("đầy"));
        db.collection::<Document>(USAGE_COLL).delete_many(doc! { "_id": { "$regex": format!("{user}:") } }).await.expect("dọn");
    }
}
