//! `realtime_tokens`: mỗi token tạm thời của chép trực tiếp (ElevenLabs Scribe v2 Realtime hoặc OpenAI Realtime) đã
//! cấp cho app bằng key riêng của người dùng.
//!
//! Không giữ chỗ hay quyết toán sổ nào (tiền của người dùng). App báo số giây audio đã stream bằng token đó khi đóng phiên
//! hoặc đổi token; báo trước `reportBy` thì ghi số đó (`sentS`) vào sổ phút key riêng để người dùng xem. Đóng theo một
//! lệnh nguyên tử `settled: false → true`, nên một token chỉ ghi sổ một lần dù app gửi lại.
//!
//! Token cũ thời có key server (không có `byok`, có `chargedS`, `freePool`, `costRatio`, `elRatio`, `elPeriod`) vẫn đọc
//! được: trường lạ bị bỏ qua; báo số giây của chúng chỉ đóng token.

use crate::error::AppError;
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
    /// Tháng cấp token (giờ Việt Nam).
    #[serde(default)]
    pub month: String,
    #[serde(default)]
    pub settled: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
    /// Hạn nhận báo cáo; quá hạn thì không ghi sổ nữa.
    #[serde(rename = "reportBy")]
    pub report_by: DateTime,
    /// `elevenlabs` hoặc `openai` (token cấp trước khi có trường này là OpenAI).
    #[serde(default = "openai")]
    pub provider: String,
    /// Mã nhà cung cấp của key riêng đã cấp token; token cũ thời key server không có.
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
        "settled": false,
        "createdAt": t.created_at,
        "reportBy": t.report_by,
        "provider": &t.provider,
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
