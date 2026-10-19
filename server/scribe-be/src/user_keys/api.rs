//! API "Key riêng" (cần đăng nhập, mọi gói):
//! - `GET /api/me/keys` — `{available, keys: [{provider, last4, status, updatedAt, lastCheckedAt}], prefs, active, models}`;
//! - `PUT /api/me/keys/{provider}` `{key}` — kiểm với nhà cung cấp rồi lưu (mã hoá); trả phần xem được của key;
//! - `DELETE /api/me/keys/{provider}` — xoá;
//! - `PUT /api/me/key-prefs` `{transcribe?, live?, text?, textModel?}` — chọn nhà cung cấp cho từng việc (`null` = tự chọn).
//!
//! Không đường nào trả key. Lưu key bị giới hạn [`SAVES_PER_MINUTE`] lần mỗi phút mỗi tài khoản (mỗi lần gọi nhà cung
//! cấp một lần).

use super::check::{self, Verdict};
use super::store::{self, KeyInfo};
use super::{crypto, Active, Prefs, Use};
use crate::audit;
use crate::auth::UserId;
use crate::config::Vendor;
use crate::error::AppError;
use crate::state::AppState;
use crate::summary::llm;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::{doc, Bson, Document};
use serde::Deserialize;
use serde_json::{json, Value};
use zeroize::Zeroizing;

/// Số lần lưu key mỗi phút mỗi tài khoản.
const SAVES_PER_MINUTE: u32 = 5;

const DISABLED: &str = "Máy chủ chưa bật Key riêng. Quản trị cần tạo tệp bí mật mã hoá rồi khởi động lại.";

fn parse_provider(raw: &str) -> Result<Vendor, AppError> {
    Vendor::parse(raw).ok_or_else(|| AppError::NotFound("nhà cung cấp không có".into()))
}

fn coded(status: StatusCode, code: &'static str, message: impl Into<String>) -> AppError {
    AppError::Coded { status, code, message: message.into() }
}

/// Danh sách model chữ chọn được, theo nhà cung cấp.
fn models_json() -> Value {
    let mut out = serde_json::Map::new();
    for (provider, models) in llm::MODELS {
        let list: Vec<Value> = models.iter().map(|m| json!({ "id": m.id, "label": m.label })).collect();
        out.insert(provider.code().into(), Value::Array(list));
    }
    Value::Object(out)
}

/// Thân của `GET /api/me/keys` (không có trường nào chứa key).
fn list_body(available: bool, keys: &[KeyInfo], prefs: &Prefs, active: &Active) -> Value {
    json!({
        "available": available,
        "keys": keys.iter().map(KeyInfo::to_json).collect::<Vec<_>>(),
        "prefs": prefs.to_json(),
        "active": active.to_json(),
        "models": models_json(),
    })
}

async fn current(state: &AppState, user_id: &str) -> Result<Value, AppError> {
    let keys = store::list(&state.db, user_id).await?;
    let prefs = super::prefs(state, user_id).await?;
    let active = super::active(state, user_id).await?;
    Ok(list_body(super::available(state), &keys, &prefs, &active))
}

/// `GET /api/me/keys`.
pub async fn list(State(state): State<AppState>, Extension(user): Extension<UserId>) -> Result<Json<Value>, AppError> {
    Ok(Json(current(&state, &user.0).await?))
}

#[derive(Deserialize)]
pub struct SaveReq {
    key: String,
}

/// Câu khi nhà cung cấp từ chối key lúc lưu.
fn rejected_message(provider: Vendor) -> String {
    let extra = if provider == Vendor::ElevenLabs { " Key ElevenLabs cần quyền Speech to Text." } else { "" };
    format!("{} từ chối key này: key sai, đã bị thu hồi hoặc thiếu quyền.{extra}", provider.label())
}

/// `PUT /api/me/keys/{provider}` `{key}`.
pub async fn save(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(provider): Path<String>,
    Json(req): Json<SaveReq>,
) -> Result<Json<Value>, AppError> {
    // Bản rõ luôn nằm trong `Zeroizing` từ đây: mọi đường ra đều ghi đè.
    let raw = Zeroizing::new(req.key);
    let provider = parse_provider(&provider)?;
    let Some(secret) = state.user_keys.as_deref() else {
        return Err(coded(StatusCode::SERVICE_UNAVAILABLE, "USER_KEYS_DISABLED", DISABLED));
    };
    if !state.rate.allow(&format!("user-key:{}", user.0), SAVES_PER_MINUTE) {
        return Err(AppError::TooManyRequests("lưu key quá nhanh, thử lại sau một phút".into()));
    }
    let key = Zeroizing::new(raw.trim().to_string());
    check::check_format(provider, &key).map_err(|m| coded(StatusCode::BAD_REQUEST, "KEY_FORMAT", m))?;
    let reason = match check::check(&state.http, &check::LIVE, provider, &key).await {
        Verdict::Valid => None,
        Verdict::Invalid => Some(("Invalid", coded(StatusCode::BAD_REQUEST, "KEY_REJECTED", rejected_message(provider)))),
        Verdict::NoCredit => Some((
            "NoCredit",
            coded(StatusCode::BAD_REQUEST, "KEY_NO_CREDIT", format!("Tài khoản {} của key này đã hết credit.", provider.label())),
        )),
        Verdict::Unreachable => Some((
            "Unreachable",
            coded(
                StatusCode::SERVICE_UNAVAILABLE,
                "KEY_CHECK_FAILED",
                format!("Không kiểm được key với {} lúc này, thử lại sau.", provider.label()),
            ),
        )),
    };
    if let Some((why, error)) = reason {
        audit::record(&state, Some(&user.0), "user_key.rejected", doc! { "provider": provider.code(), "reason": why });
        return Err(error);
    }
    let sealed = crypto::seal(secret, &user.0, provider.code(), &key).map_err(AppError::Internal)?;
    let last4: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    let info = store::save(&state.db, &user.0, provider, &sealed, &last4).await?;
    audit::record(&state, Some(&user.0), "user_key.save", doc! { "provider": provider.code() });
    Ok(Json(info.to_json()))
}

/// `DELETE /api/me/keys/{provider}`.
pub async fn remove(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(provider): Path<String>,
) -> Result<Json<Value>, AppError> {
    let provider = parse_provider(&provider)?;
    let deleted = store::delete(&state.db, &user.0, provider).await?;
    if deleted {
        audit::record(&state, Some(&user.0), "user_key.delete", doc! { "provider": provider.code() });
    }
    Ok(Json(json!({ "deleted": deleted })))
}

#[derive(Deserialize)]
pub struct PrefsReq {
    #[serde(default)]
    transcribe: Option<String>,
    #[serde(default)]
    live: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(rename = "textModel", default)]
    text_model: Option<String>,
}

/// Lựa chọn một việc: rỗng/`null` = tự chọn; nhà cung cấp phải làm được việc đó.
fn pref_value(raw: Option<&str>, use_: Use) -> Result<Bson, AppError> {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(Bson::Null),
        Some(code) => match Vendor::parse(code).filter(|v| use_.providers().contains(v)) {
            Some(v) => Ok(Bson::String(v.code().into())),
            None => Err(AppError::BadRequest(format!("nhà cung cấp “{code}” không dùng được cho việc này"))),
        },
    }
}

/// Kiểm và dựng doc lựa chọn (model chữ phải có trong danh sách của nhà cung cấp nào đó).
fn prefs_doc(req: &PrefsReq) -> Result<Document, AppError> {
    let model = match req.text_model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        None => Bson::Null,
        Some(m) if Use::Text.providers().iter().any(|v| llm::is_listed(*v, m)) => Bson::String(m.into()),
        Some(_) => return Err(AppError::BadRequest("model không có trong danh sách".into())),
    };
    Ok(doc! {
        "transcribe": pref_value(req.transcribe.as_deref(), Use::Transcribe)?,
        "live": pref_value(req.live.as_deref(), Use::Live)?,
        "text": pref_value(req.text.as_deref(), Use::Text)?,
        "textModel": model,
    })
}

/// `PUT /api/me/key-prefs` — trả lại toàn bộ như `GET /api/me/keys`.
pub async fn save_prefs(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<PrefsReq>,
) -> Result<Json<Value>, AppError> {
    let prefs = prefs_doc(&req)?;
    store::save_prefs(&state.db, &user.0, prefs.clone()).await?;
    audit::record(&state, Some(&user.0), "user_key.prefs", prefs);
    Ok(Json(current(&state, &user.0).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::DateTime;

    /// Không trường nào của phản hồi chứa key hay bản mã, dù đi sâu tới đâu.
    #[test]
    fn list_never_returns_key_material() {
        let keys = vec![KeyInfo {
            vendor: Vendor::OpenAi,
            last4: "9xyz".into(),
            status: store::Status::Valid,
            updated_at: DateTime::from_millis(0),
            last_checked_at: Some(DateTime::from_millis(0)),
        }];
        let active = Active { text: Some((Vendor::OpenAi, "gpt-4.1-mini".into())), ..Active::default() };
        let body = list_body(true, &keys, &Prefs::default(), &active);
        fn walk(v: &Value, path: &str) {
            match v {
                Value::Object(map) => {
                    for (k, v) in map {
                        assert!(!["key", "ciphertext", "nonce", "secret", "apiKey"].contains(&k.as_str()), "{path}.{k}");
                        walk(v, &format!("{path}.{k}"));
                    }
                }
                Value::Array(items) => items.iter().for_each(|v| walk(v, path)),
                Value::String(s) => assert!(!s.starts_with("sk-"), "{path}: {s}"),
                _ => {}
            }
        }
        walk(&body, "");
        assert_eq!(body["keys"][0]["last4"], "9xyz");
        assert_eq!(body["active"]["text"], "openai");
        assert_eq!(body["models"]["anthropic"][0]["id"], "claude-haiku-5-5");
    }

    #[test]
    fn prefs_are_validated_per_job() {
        let req = |transcribe: Option<&str>, live: Option<&str>, text: Option<&str>, model: Option<&str>| PrefsReq {
            transcribe: transcribe.map(str::to_string),
            live: live.map(str::to_string),
            text: text.map(str::to_string),
            text_model: model.map(str::to_string),
        };
        let ok = prefs_doc(&req(Some("gemini"), Some(""), Some("anthropic"), Some("claude-sonnet-5-5"))).expect("hợp lệ");
        assert_eq!(ok.get_str("transcribe"), Ok("gemini"));
        assert_eq!(ok.get("live"), Some(&Bson::Null));
        assert!(prefs_doc(&req(Some("anthropic"), None, None, None)).is_err(), "Claude không chép lời");
        assert!(prefs_doc(&req(None, Some("gemini"), None, None)).is_err(), "Gemini không chép trực tiếp");
        assert!(prefs_doc(&req(None, None, Some("elevenlabs"), None)).is_err());
        assert!(prefs_doc(&req(None, None, None, Some("gpt-9"))).is_err());
    }

    #[test]
    fn rejection_names_the_provider_and_permission() {
        assert!(rejected_message(Vendor::ElevenLabs).contains("Speech to Text"));
        assert!(rejected_message(Vendor::Gemini).starts_with("Google Gemini từ chối"));
    }
}
