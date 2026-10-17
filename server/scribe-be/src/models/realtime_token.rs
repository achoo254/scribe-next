//! `realtime_tokens`: mỗi token tạm thời của chép trực tiếp (OpenAI Realtime) đã cấp cho app iPhone.
//!
//! Lúc cấp, backend giữ chỗ `chargedS` giây trên sổ phút (`usage`) — cùng hạn mức với chép lời cả file.
//! App báo số giây audio thật sự đã stream bằng token đó khi đóng phiên hoặc đổi token; báo trước
//! `reportBy` thì quyết toán theo số thật (`sentS`). Không báo (app bị tắt, mất mạng) thì phần giữ chỗ
//! được tính trọn: không cần job dọn, sổ đã có sẵn số đó. Đóng theo một lệnh nguyên tử
//! `settled: false → true`, nên một token chỉ quyết toán một lần dù app gửi lại.

use crate::error::AppError;
use crate::models::usage::Charge;
use futures_util::StreamExt;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::{Database, IndexModel};
use serde::Deserialize;

const COLL: &str = "realtime_tokens";

#[derive(Debug, Clone, Deserialize)]
pub struct TokenDoc {
    /// Id do backend sinh (UUID), không phải giá trị token của OpenAI (token không được lưu).
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    /// Phiên ghi trên iPhone dùng token này (để tra cứu), có thể thiếu.
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<String>,
    pub month: String,
    #[serde(rename = "freePool", default)]
    pub free_pool: bool,
    #[serde(default)]
    pub day: Option<String>,
    /// Giây đã giữ chỗ trên sổ.
    #[serde(rename = "chargedS")]
    pub charged_s: i64,
    #[serde(default)]
    pub settled: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
    /// Hạn nhận báo cáo; quá hạn thì phần giữ chỗ tính trọn.
    #[serde(rename = "reportBy")]
    pub report_by: DateTime,
}

impl TokenDoc {
    pub fn charge(&self) -> Charge {
        Charge { user_id: self.user_id.clone(), month: self.month.clone(), free_pool: self.free_pool, day: self.day.clone() }
    }
}

pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<Document>(COLL)
        .create_index(IndexModel::builder().keys(doc! { "month": 1, "settled": 1 }).build())
        .await
        .map(|_| ())
        .map_err(|e| format!("index realtime_tokens.month: {e}"))
}

pub async fn insert(db: &Database, t: &TokenDoc) -> Result<(), AppError> {
    let d = doc! {
        "_id": &t.id,
        "userId": &t.user_id,
        "sessionId": &t.session_id,
        "month": &t.month,
        "freePool": t.free_pool,
        "day": &t.day,
        "chargedS": t.charged_s,
        "settled": false,
        "createdAt": t.created_at,
        "reportBy": t.report_by,
    };
    db.collection::<Document>(COLL)
        .insert_one(d)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("ghi token chép trực tiếp: {e}")))
}

/// Token `id` của `user_id`, nếu có.
pub async fn find_owned(db: &Database, user_id: &str, id: &str) -> Result<Option<TokenDoc>, AppError> {
    db.collection::<TokenDoc>(COLL)
        .find_one(doc! { "_id": id, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("đọc token chép trực tiếp: {e}")))
}

/// Nhận quyền quyết toán: `settled: false → true` với `sentS`, chỉ khi còn trong hạn báo. `false` khi
/// token đã quyết toán hoặc quá hạn.
pub async fn close(db: &Database, user_id: &str, id: &str, sent_s: i64, now: DateTime) -> Result<bool, AppError> {
    let res = db
        .collection::<Document>(COLL)
        .update_one(
            doc! { "_id": id, "userId": user_id, "settled": false, "reportBy": { "$gt": now } },
            doc! { "$set": { "settled": true, "sentS": sent_s, "settledAt": now } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("quyết toán token chép trực tiếp: {e}")))?;
    Ok(res.modified_count > 0)
}

/// Token chưa quyết toán trong tháng: (userId, freePool, chargedS) — sổ đang tính trọn phần giữ chỗ.
/// Token đã quyết toán nằm trong `usage::RUNS_COLL` như lượt chép file.
pub async fn unsettled(db: &Database, month: &str) -> Result<Vec<(String, bool, i64)>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc token chép trực tiếp: {e}"));
    let mut cur = db.collection::<TokenDoc>(COLL).find(doc! { "month": month, "settled": false }).await.map_err(read)?;
    let mut out = Vec::new();
    while let Some(t) = cur.next().await {
        let t = t.map_err(read)?;
        out.push((t.user_id, t.free_pool, t.charged_s));
    }
    Ok(out)
}
