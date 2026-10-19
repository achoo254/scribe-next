//! `realtime_tokens`: mỗi token tạm thời của chép trực tiếp (ElevenLabs Scribe v2 Realtime, hoặc OpenAI Realtime khi
//! `REALTIME_PROVIDER=openai`) đã cấp cho app iPhone.
//!
//! Lúc cấp, backend giữ chỗ `chargedS` giây trên sổ phút (`usage`) — cùng hạn mức với chép lời cả file. Sổ quy theo USD
//! (quỹ free, trần tổng) giữ số giây quy đổi theo giá (`costRatio`), sổ giờ ElevenLabs giữ theo `elRatio`.
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
    /// Id do backend sinh (UUID), không phải giá trị token của nhà cung cấp (token không được lưu).
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
    /// `elevenlabs` hoặc `openai` (token cấp trước khi có trường này là OpenAI).
    #[serde(default = "openai")]
    pub provider: String,
    /// Giây trên sổ quy theo USD cho mỗi giây audio: giá chép trực tiếp / giá chép lời theo tệp của bản build.
    #[serde(rename = "costRatio", default = "one")]
    pub cost_ratio: f64,
    /// Giây trên sổ giờ ElevenLabs cho mỗi giây audio (giá Realtime / giá Scribe v2); 0 khi không qua ElevenLabs.
    #[serde(rename = "elRatio", default)]
    pub el_ratio: f64,
    /// Token cấp bằng key riêng của người dùng (mã nhà cung cấp): không giữ chỗ, không quyết toán sổ nào.
    #[serde(default)]
    pub byok: Option<String>,
    /// Nonce (hex) của đúng bản key riêng đã cấp token: báo hết quota trễ chỉ đánh dấu bản key đó, không đụng key người
    /// dùng vừa nhập lại. Không phải bí mật (nonce nằm cạnh bản mã).
    #[serde(rename = "keyNonce", default)]
    pub key_nonce: Option<String>,
}

/// Nonce ra hex (ghi vào token).
pub fn nonce_hex(nonce: &[u8]) -> String {
    nonce.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex ra nonce; sai dạng thì `None`.
pub fn nonce_from_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

fn openai() -> String {
    "openai".into()
}

fn one() -> f64 {
    1.0
}

/// Giây quy đổi của `seconds` giây audio theo `ratio`, làm tròn lên. Bỏ phần lẻ do sai số dấu phẩy động (600 × 2,1666…
/// ra 1300,0000000002 vẫn là 1300).
pub fn weighted(seconds: i64, ratio: f64) -> i64 {
    (seconds.max(0) as f64 * ratio - 1e-6).ceil().max(0.0) as i64
}

impl TokenDoc {
    pub fn charge(&self) -> Charge {
        Charge { user_id: self.user_id.clone(), month: self.month.clone(), free_pool: self.free_pool, day: self.day.clone() }
    }

    /// Giây trên sổ quy theo USD của `seconds` giây audio.
    pub fn cost_s(&self, seconds: i64) -> i64 {
        weighted(seconds, self.cost_ratio)
    }

    /// Giây trên sổ giờ ElevenLabs của `seconds` giây audio.
    pub fn el_s(&self, seconds: i64) -> i64 {
        weighted(seconds, self.el_ratio)
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
        "provider": &t.provider,
        "costRatio": t.cost_ratio,
        "elRatio": t.el_ratio,
        "byok": &t.byok,
        "keyNonce": &t.key_nonce,
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

/// Token chưa quyết toán trong tháng: (userId, freePool, chargedS, giây trên sổ quy theo USD) — sổ đang tính trọn phần
/// giữ chỗ. Token đã quyết toán nằm trong `usage::RUNS_COLL` như lượt chép file.
pub async fn unsettled(db: &Database, month: &str) -> Result<Vec<(String, bool, i64, i64)>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc token chép trực tiếp: {e}"));
    let mut cur = db.collection::<TokenDoc>(COLL).find(doc! { "month": month, "settled": false, "byok": null }).await.map_err(read)?;
    let mut out = Vec::new();
    while let Some(t) = cur.next().await {
        let t = t.map_err(read)?;
        let cost = t.cost_s(t.charged_s);
        out.push((t.user_id, t.free_pool, t.charged_s, cost));
    }
    Ok(out)
}
