//! Dấu mốc của một transcript: người dùng bấm "Đánh dấu" lúc đang ghi, mốc hiện trong bản chép lời.
//!
//! Lúc đang ghi phiên có thể chưa có file nào trên server, nên doc tạo lười bằng upsert và quyền sở hữu
//! kiểm theo trạng thái của phiên:
//! - phiên đã có bản ghi (`file::list_session`): như mọi route `/api/transcript-docs`;
//! - `f:<fileId>` chưa có bản ghi: phải có phiên ghi trình duyệt đang mở của chính user giữ trước id đó
//!   (`recordings::owns_pending_file`, cùng cách kiểm `userId` của `recordings`);
//! - `s:<sessionId>` (phiên iPhone) chưa tải đoạn nào: id phiên do app sinh, server chưa có gì để kiểm;
//!   doc nằm trong phạm vi `userId` của chính người gọi nên không đọc/ghi được dữ liệu của ai khác, và
//!   sweep mồ côi dọn doc không bao giờ có file.

use super::docs::check_key;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file;
use crate::models::transcript_doc::{self, Bookmark, MAX_BOOKMARKS};
use crate::recordings;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;
use serde_json::{json, Value};

const MAX_LABEL_CHARS: usize = 200;
/// Mốc lớn nhất (giây): phiên dài nhất vài giờ, trần này chỉ chặn số vô lý.
const MAX_AT_S: f64 = 48.0 * 3600.0;

#[derive(Deserialize)]
pub struct AddReq {
    at: f64,
    #[serde(default)]
    label: Option<String>,
}

/// Mốc làm tròn 0,1 giây và nhãn đã cắt khoảng trắng (rỗng = không nhãn).
fn validate(at: f64, label: Option<&str>) -> Result<(f64, Option<String>), AppError> {
    if !at.is_finite() || !(0.0..=MAX_AT_S).contains(&at) {
        return Err(AppError::BadRequest("mốc giây không hợp lệ".into()));
    }
    let label = label.map(str::trim).filter(|l| !l.is_empty());
    if label.is_some_and(|l| l.chars().count() > MAX_LABEL_CHARS) {
        return Err(AppError::BadRequest(format!("nhãn tối đa {MAX_LABEL_CHARS} ký tự")));
    }
    Ok(((at * 10.0).round() / 10.0, label.map(str::to_string)))
}

async fn check_owner(state: &AppState, user_id: &str, key: &str) -> Result<(), AppError> {
    check_key(key)?;
    if !file::list_session(&state.db, user_id, key).await?.is_empty() || key.starts_with("s:") {
        return Ok(());
    }
    let pending = match key.strip_prefix("f:").and_then(|hex| ObjectId::parse_str(hex).ok()) {
        Some(id) => recordings::owns_pending_file(state, user_id, id).await?,
        None => false,
    };
    if pending {
        Ok(())
    } else {
        Err(AppError::NotFound("transcript không tồn tại".into()))
    }
}

/// `POST /api/transcript-docs/{key}/bookmarks` `{at, label?}` → `{id, at, label}`.
pub async fn add(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
    Json(req): Json<AddReq>,
) -> Result<Json<Value>, AppError> {
    let (at, label) = validate(req.at, req.label.as_deref())?;
    check_owner(&state, &user.0, &key).await?;
    let bookmark = Bookmark { id: ObjectId::new().to_hex(), at, label };
    if !transcript_doc::add_bookmark(&state.db, &user.0, &key, &bookmark).await? {
        return Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "BOOKMARKS_FULL",
            message: format!("Mỗi bản ghi có tối đa {MAX_BOOKMARKS} dấu mốc."),
        });
    }
    Ok(Json(json!(bookmark)))
}

/// `DELETE /api/transcript-docs/{key}/bookmarks/{id}`.
pub async fn remove(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path((key, id)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    check_key(&key)?;
    ObjectId::parse_str(&id).map_err(|_| AppError::BadRequest("id dấu mốc không hợp lệ".into()))?;
    if !transcript_doc::remove_bookmark(&state.db, &user.0, &key, &id).await? {
        return Err(AppError::NotFound("dấu mốc không tồn tại".into()));
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_input_is_bounded() {
        assert_eq!(validate(12.345, Some("  Chốt giá  ")).expect("ok"), (12.3, Some("Chốt giá".into())));
        assert_eq!(validate(0.0, Some("   ")).expect("ok"), (0.0, None));
        assert_eq!(validate(5.0, None).expect("ok"), (5.0, None));
        assert!(validate(-0.1, None).is_err());
        assert!(validate(f64::NAN, None).is_err());
        assert!(validate(f64::INFINITY, None).is_err());
        assert!(validate(MAX_AT_S + 1.0, None).is_err());
        assert!(validate(1.0, Some(&"ữ".repeat(200))).is_ok());
        assert!(validate(1.0, Some(&"ữ".repeat(201))).is_err());
    }
}
