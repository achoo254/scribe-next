//! Nhật ký các lượt gọi model chữ (tóm tắt, hỏi đáp, dịch — [`Feature`]) ở `summary_runs` / `ask_runs` /
//! `translate_runs`: mở (`open: true`) trước khi gọi, đóng khi có kết quả (token vào/ra, thành công hay không). Mọi lượt
//! chạy bằng key riêng của người dùng (`byok` = nhà cung cấp): không có lượt ngày, trần tháng hay trần tổng nào.
//!
//! Collection `summary_usage` (lượt ngày, chi phí tháng thời có key server) và các trường `reservedS`/`costS` của lượt
//! cũ không còn được đọc hay ghi.

use crate::error::AppError;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::Database;

/// Tính năng gọi model chữ, mỗi tính năng một nhật ký.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Summary,
    Ask,
    Translate,
}

impl Feature {
    /// Nhật ký lượt gọi model của tính năng.
    pub fn runs_coll(self) -> &'static str {
        match self {
            Feature::Summary => "summary_runs",
            Feature::Ask => "ask_runs",
            Feature::Translate => "translate_runs",
        }
    }
}

/// Một lượt sắp gọi model.
pub struct Run<'a> {
    pub user_id: &'a str,
    /// Phiên của lượt; hỏi đáp trên cả kho thì không có.
    pub key: Option<&'a str>,
    pub model: &'a str,
    pub month: &'a str,
    pub day: &'a str,
    /// Mã nhà cung cấp của key riêng chạy lượt.
    pub byok: &'a str,
}

/// Kết quả của một lượt.
pub struct Closing {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub ok: bool,
}

/// Mở lượt trước khi gọi model. Lỗi thì không gọi.
pub async fn open_run(db: &Database, feature: Feature, run: &Run<'_>) -> Result<ObjectId, AppError> {
    let id = ObjectId::new();
    let d = doc! {
        "_id": id,
        "userId": run.user_id,
        "key": run.key,
        "model": run.model,
        "month": run.month,
        "day": run.day,
        "open": true,
        "byok": run.byok,
        "startedAt": DateTime::now(),
    };
    db.collection::<Document>(feature.runs_coll())
        .insert_one(d)
        .await
        .map(|_| id)
        .map_err(|e| AppError::Internal(format!("mở lượt {}: {e}", feature.runs_coll())))
}

/// Đóng lượt với số token thật. Lỗi chỉ log.
pub async fn close_run(db: &Database, feature: Feature, id: ObjectId, closing: &Closing) {
    let update = doc! {
        "$set": {
            "open": false,
            "inputTokens": closing.input_tokens,
            "outputTokens": closing.output_tokens,
            "ok": closing.ok,
            "at": DateTime::now(),
        },
    };
    if let Err(e) = db.collection::<Document>(feature.runs_coll()).update_one(doc! { "_id": id }, update).await {
        tracing::error!(run = %id, "đóng lượt {} lỗi: {e}", feature.runs_coll());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// text_runs_are_logged -- --ignored`. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn text_runs_are_logged_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());
        let run = Run { user_id: &user, key: None, model: "m", month: "2026-11", day: "2026-11-02", byok: "anthropic" };
        let id = open_run(&db, Feature::Ask, &run).await.expect("mở");
        close_run(&db, Feature::Ask, id, &Closing { input_tokens: 9_000, output_tokens: 900, ok: true }).await;
        let stored = db.collection::<Document>(Feature::Ask.runs_coll()).find_one(doc! { "_id": id }).await.expect("đọc").expect("có");
        assert_eq!((stored.get_bool("open"), stored.get_str("byok"), stored.get_i64("inputTokens")), (Ok(false), Ok("anthropic"), Ok(9_000)));
        db.collection::<Document>(Feature::Ask.runs_coll()).delete_many(doc! { "userId": &user }).await.expect("dọn");
    }
}
