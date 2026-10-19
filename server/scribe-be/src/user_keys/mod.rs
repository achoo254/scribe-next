//! Key riêng của người dùng ("Key riêng"): ElevenLabs, OpenAI, Google Gemini, Anthropic Claude. Mọi người đăng nhập đều
//! thêm được. Việc chạy bằng key riêng không tính vào phút, lượt ngày, quỹ free, trần tổng hay sổ giờ ElevenLabs của
//! Scribe (tiền của người dùng); dung lượng, cỡ tệp và rate limit vẫn áp.
//!
//! - Lưu trên server, mã hoá AES-256-GCM ([`crypto`]) bằng bí mật trong tệp `USER_KEYS_SECRET_PATH`; thiếu tệp thì tính
//!   năng tắt ([`api`] trả `available: false`). API chỉ ghi: không bao giờ trả key, chỉ nhà cung cấp, 4 ký tự cuối, trạng
//!   thái, giờ sửa/kiểm. Key không vào log, audit hay thông báo lỗi.
//! - Việc nào dùng key nào ([`Use`]): chép lời theo tệp (ElevenLabs có người nói, OpenAI `gpt-transcribe`, Gemini), chép
//!   trực tiếp (ElevenLabs, OpenAI Realtime), model chữ cho biên bản/hỏi đáp/dịch (OpenAI, Gemini, Claude, kèm model).
//!   Người dùng chọn nhà cung cấp cho từng việc; chưa chọn (hoặc key đã chọn không còn hợp lệ) thì lấy key hợp lệ đầu tiên
//!   theo thứ tự trên; không có key hợp lệ thì dùng key server với hạn mức thường ([`pick`]).
//! - Lượt xếp hàng chỉ ghi nhà cung cấp; worker đọc và giải mã key của chủ phiên lúc chạy ([`open`]).
//! - Key lỗi lúc dùng (sai, thu hồi, thiếu quyền, hết credit): lượt đó dừng với câu "Key … của bạn không dùng được: … —
//!   sửa hoặc xoá key ở Key riêng", key bị đánh dấu ([`report`]); không bao giờ chuyển sang key server giữa chừng và không
//!   đụng cờ tạm ngưng hay dấu hết quota của key server.

pub mod api;
pub mod check;
pub mod crypto;
pub mod store;

use crate::audit;
use crate::config::Vendor;
use crate::error::AppError;
use crate::state::AppState;
use crate::summary::llm;
use crate::transcribe::{elevenlabs, openai};
use axum::http::StatusCode;
use mongodb::bson::{doc, Document};
use serde_json::{json, Value};
use store::Status;
use zeroize::Zeroizing;

/// Việc dùng key riêng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Chép lời theo tệp (cả phiên iPhone).
    Transcribe,
    /// Chép trực tiếp trên iPhone.
    Live,
    /// Biên bản, hỏi đáp, dịch.
    Text,
}

impl Use {
    /// Nhà cung cấp làm được việc này, theo thứ tự mặc định.
    pub fn providers(self) -> &'static [Vendor] {
        match self {
            Use::Transcribe => &[Vendor::ElevenLabs, Vendor::OpenAi, Vendor::Gemini],
            Use::Live => &[Vendor::ElevenLabs, Vendor::OpenAi],
            Use::Text => &[Vendor::OpenAi, Vendor::Gemini, Vendor::Anthropic],
        }
    }

    fn pref(self, prefs: &Prefs) -> Option<Vendor> {
        match self {
            Use::Transcribe => prefs.transcribe,
            Use::Live => prefs.live,
            Use::Text => prefs.text,
        }
    }
}

/// Lựa chọn của người dùng (`None` = tự chọn).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    pub transcribe: Option<Vendor>,
    pub live: Option<Vendor>,
    pub text: Option<Vendor>,
    pub text_model: Option<String>,
}

impl Prefs {
    fn from_doc(d: &Document) -> Prefs {
        let get = |name: &str, use_: Use| d.get_str(name).ok().and_then(Vendor::parse).filter(|v| use_.providers().contains(v));
        Prefs {
            transcribe: get("transcribe", Use::Transcribe),
            live: get("live", Use::Live),
            text: get("text", Use::Text),
            text_model: d.get_str("textModel").ok().map(str::to_string),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "transcribe": self.transcribe.map(Vendor::code),
            "live": self.live.map(Vendor::code),
            "text": self.text.map(Vendor::code),
            "textModel": self.text_model,
        })
    }
}

/// Nhà cung cấp dùng cho `use_`: lựa chọn của người dùng nếu key đó hợp lệ, không thì key hợp lệ đầu tiên theo thứ tự
/// mặc định; `None` = dùng key server.
pub fn pick(use_: Use, prefs: &Prefs, valid: &[Vendor]) -> Option<Vendor> {
    let allowed = use_.providers();
    use_.pref(prefs)
        .filter(|v| allowed.contains(v) && valid.contains(v))
        .or_else(|| allowed.iter().copied().find(|v| valid.contains(v)))
}

/// Model chữ của nhà cung cấp `provider`: model người dùng chọn nếu có trong danh sách, không thì model mặc định.
pub fn text_model(prefs: &Prefs, provider: Vendor) -> String {
    prefs
        .text_model
        .as_deref()
        .filter(|m| llm::is_listed(provider, m))
        .unwrap_or_else(|| llm::default_model(provider))
        .to_string()
}

/// Key riêng đang có hiệu lực cho từng việc.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Active {
    pub transcribe: Option<Vendor>,
    pub live: Option<Vendor>,
    pub text: Option<(Vendor, String)>,
}

impl Active {
    pub fn to_json(&self) -> Value {
        json!({
            "transcribe": self.transcribe.map(Vendor::code),
            "live": self.live.map(Vendor::code),
            "text": self.text.as_ref().map(|(v, _)| v.code()),
            "textModel": self.text.as_ref().map(|(_, m)| m),
        })
    }
}

/// Tính năng có bật không (máy chủ đọc được tệp bí mật).
pub fn available(state: &AppState) -> bool {
    state.user_keys.is_some()
}

pub(crate) async fn prefs(state: &AppState, user_id: &str) -> Result<Prefs, AppError> {
    Ok(store::get_prefs(&state.db, user_id).await?.as_ref().map(Prefs::from_doc).unwrap_or_default())
}

/// Key riêng có hiệu lực của người dùng (rỗng khi tính năng tắt).
pub async fn active(state: &AppState, user_id: &str) -> Result<Active, AppError> {
    if !available(state) {
        return Ok(Active::default());
    }
    let valid: Vec<Vendor> =
        store::list(&state.db, user_id).await?.into_iter().filter(|k| k.status == Status::Valid).map(|k| k.vendor).collect();
    if valid.is_empty() {
        return Ok(Active::default());
    }
    let prefs = prefs(state, user_id).await?;
    Ok(Active {
        transcribe: pick(Use::Transcribe, &prefs, &valid),
        live: pick(Use::Live, &prefs, &valid),
        text: pick(Use::Text, &prefs, &valid).map(|v| (v, text_model(&prefs, v))),
    })
}

/// Key đã giải mã, sống ngắn: bản rõ bị ghi đè khi huỷ. Không có `Debug`.
pub struct UserKey {
    pub provider: Vendor,
    user_id: String,
    value: Zeroizing<String>,
    nonce: Vec<u8>,
}

impl UserKey {
    pub fn value(&self) -> &str {
        self.value.as_str()
    }

    /// Nonce của bản key đã giải mã: nhận diện đúng bản key này khi báo lỗi về sau.
    pub fn nonce(&self) -> &[u8] {
        &self.nonce
    }
}

/// Câu người dùng thấy khi key riêng không dùng được.
pub fn failure_message(provider: Vendor, reason: &str) -> String {
    format!("Key {} của bạn không dùng được: {reason} — sửa hoặc xoá key ở Key riêng.", provider.label())
}

fn status_reason(status: Status) -> &'static str {
    match status {
        Status::Valid => "",
        Status::Invalid => "nhà cung cấp từ chối key (sai, đã thu hồi hoặc thiếu quyền)",
        Status::Quota => "tài khoản của key đã hết credit hoặc hạn mức",
        Status::Unreadable => "máy chủ không đọc được key đã lưu, hãy nhập lại",
    }
}

/// Đọc và giải mã key `provider` của `user_id` lúc chạy việc. `Err` là câu cho người dùng (key bị xoá, không còn hợp
/// lệ, không giải mã được — lúc đó key bị đánh dấu `unreadable`).
pub async fn open(state: &AppState, user_id: &str, provider: Vendor) -> Result<UserKey, String> {
    let Some(secret) = state.user_keys.as_deref() else {
        return Err(failure_message(provider, "máy chủ đang tắt tính năng Key riêng"));
    };
    let stored = match store::get(&state.db, user_id, provider).await {
        Ok(Some(stored)) => stored,
        Ok(None) => return Err(failure_message(provider, "key đã bị xoá")),
        Err(e) => {
            tracing::warn!("đọc key riêng lỗi: {e:?}");
            return Err("Không đọc được key riêng lúc này, thử lại sau.".into());
        }
    };
    if stored.info.status != Status::Valid {
        return Err(failure_message(provider, status_reason(stored.info.status)));
    }
    match crypto::open(secret, user_id, provider.code(), &stored.ciphertext, &stored.nonce) {
        Some(value) => Ok(UserKey { provider, user_id: user_id.to_string(), value, nonce: stored.nonce }),
        None => {
            tracing::error!(provider = provider.code(), "không giải mã được key riêng (bí mật trên server đã đổi?)");
            if let Err(e) = store::set_status(&state.db, user_id, provider, &stored.nonce, Status::Unreadable).await {
                tracing::warn!("đánh dấu key riêng không đọc được lỗi: {e:?}");
            }
            Err(failure_message(provider, status_reason(Status::Unreadable)))
        }
    }
}

/// Lỗi lúc dùng key riêng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    Invalid,
    NoCredit,
}

/// Lỗi của nhà cung cấp (theo các hằng lỗi của từng client) là lỗi của key không. Lỗi khác (mạng, 5xx, dữ liệu) thì
/// không đổi trạng thái key.
pub fn problem_of(e: &str) -> Option<Problem> {
    if e == elevenlabs::KEY_INVALID || e == openai::KEY_INVALID || e == llm::KEY_INVALID {
        Some(Problem::Invalid)
    } else if e == elevenlabs::QUOTA_EXHAUSTED || e == openai::NO_CREDIT || e == llm::NO_CREDIT {
        Some(Problem::NoCredit)
    } else {
        None
    }
}

/// Key riêng lỗi lúc dùng: đánh dấu đúng bản key đã dùng, ghi audit (không có key), trả câu cho người dùng.
pub async fn report(state: &AppState, key: &UserKey, problem: Problem) -> String {
    report_nonce(state, &key.user_id, key.provider, &key.nonce, problem).await
}

/// Như [`report`] khi chỉ còn nonce của bản key đã dùng (vd token chép trực tiếp cấp từ trước): key người dùng đã thay
/// từ đó (nonce khác) thì không bị đánh dấu.
pub async fn report_nonce(state: &AppState, user_id: &str, provider: Vendor, nonce: &[u8], problem: Problem) -> String {
    let status = match problem {
        Problem::Invalid => Status::Invalid,
        Problem::NoCredit => Status::Quota,
    };
    match store::set_status(&state.db, user_id, provider, nonce, status).await {
        Ok(true) => {
            audit::record(state, Some(user_id), "user_key.failed", doc! { "provider": provider.code(), "status": status.code() });
        }
        Ok(false) => {}
        Err(e) => tracing::warn!("đánh dấu key riêng lỗi: {e:?}"),
    }
    failure_message(provider, status_reason(status))
}

/// Lỗi trả cho request khi key riêng không dùng được.
pub fn user_key_error(message: String) -> AppError {
    AppError::Coded { status: StatusCode::FORBIDDEN, code: "USER_KEY_FAILED", message }
}

/// Model chữ bằng key riêng của người dùng, khi có: key đã giải mã và model. Key đang có hiệu lực mà không mở được
/// ⟹ lỗi `USER_KEY_FAILED` (không lặng lẽ dùng key server).
pub(crate) async fn text_key(state: &AppState, user_id: &str) -> Result<Option<(UserKey, String)>, AppError> {
    let Some((provider, model)) = active(state, user_id).await?.text else { return Ok(None) };
    match open(state, user_id, provider).await {
        Ok(key) => Ok(Some((key, model))),
        Err(message) => Err(user_key_error(message)),
    }
}

/// Phần `ownKeys` của `GET /api/me`: việc nào đang chạy bằng key riêng và số phút chép lời bằng key riêng tháng này.
pub async fn me_summary(state: &AppState, user_id: &str) -> Result<Value, AppError> {
    let active = active(state, user_id).await?;
    let month = crate::models::usage::month_key(chrono::Utc::now());
    let seconds = crate::models::usage::byok_seconds(&state.db, user_id, &month).await?;
    let mut out = active.to_json();
    out["available"] = json!(available(state));
    out["minutes"] = json!(crate::models::usage::used_minutes(seconds));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_prefers_the_chosen_valid_key_then_the_default_order() {
        let none = Prefs::default();
        assert_eq!(pick(Use::Transcribe, &none, &[]), None);
        assert_eq!(pick(Use::Transcribe, &none, &[Vendor::Gemini, Vendor::OpenAi]), Some(Vendor::OpenAi));
        let chosen = Prefs { transcribe: Some(Vendor::Gemini), text: Some(Vendor::Anthropic), ..Prefs::default() };
        assert_eq!(pick(Use::Transcribe, &chosen, &[Vendor::Gemini, Vendor::OpenAi]), Some(Vendor::Gemini));
        // Key đã chọn không còn hợp lệ: key hợp lệ khác.
        assert_eq!(pick(Use::Transcribe, &chosen, &[Vendor::ElevenLabs]), Some(Vendor::ElevenLabs));
        // Claude không chép lời, Gemini không chép trực tiếp.
        assert_eq!(pick(Use::Transcribe, &none, &[Vendor::Anthropic]), None);
        assert_eq!(pick(Use::Live, &none, &[Vendor::Gemini]), None);
        assert_eq!(pick(Use::Text, &chosen, &[Vendor::OpenAi, Vendor::Anthropic]), Some(Vendor::Anthropic));
        assert_eq!(pick(Use::Text, &none, &[Vendor::ElevenLabs]), None);
    }

    #[test]
    fn text_model_must_be_listed_for_the_provider() {
        let prefs = Prefs { text_model: Some("claude-sonnet-5-5".into()), ..Prefs::default() };
        assert_eq!(text_model(&prefs, Vendor::Anthropic), "claude-sonnet-5-5");
        assert_eq!(text_model(&prefs, Vendor::Gemini), "gemini-3.8-flash");
        assert_eq!(text_model(&Prefs::default(), Vendor::OpenAi), "gpt-4.1-mini");
    }

    #[test]
    fn provider_errors_map_to_key_problems_and_messages_name_the_fix() {
        assert_eq!(problem_of(elevenlabs::KEY_INVALID), Some(Problem::Invalid));
        assert_eq!(problem_of(elevenlabs::QUOTA_EXHAUSTED), Some(Problem::NoCredit));
        assert_eq!(problem_of(llm::NO_CREDIT), Some(Problem::NoCredit));
        assert_eq!(problem_of(openai::KEY_INVALID), Some(Problem::Invalid));
        assert_eq!(problem_of("Dịch vụ chép lời đang lỗi (HTTP 500), thử lại sau."), None);
        let m = failure_message(Vendor::ElevenLabs, status_reason(Status::Quota));
        assert!(m.starts_with("Key ElevenLabs của bạn không dùng được: "), "{m}");
        assert!(m.ends_with("sửa hoặc xoá key ở Key riêng."), "{m}");
    }

    #[test]
    fn prefs_ignore_providers_that_cannot_do_the_job() {
        let d = doc! { "transcribe": "anthropic", "live": "gemini", "text": "gemini", "textModel": "x" };
        let p = Prefs::from_doc(&d);
        assert_eq!((p.transcribe, p.live, p.text), (None, None, Some(Vendor::Gemini)));
        assert_eq!(p.to_json()["text"], "gemini");
    }
}
