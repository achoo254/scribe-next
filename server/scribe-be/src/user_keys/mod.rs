//! Key riêng của người dùng ("Key riêng"): ElevenLabs, OpenAI, Google Gemini, Anthropic Claude. Từ 02/11/2026 (chủ dự án
//! chốt) Scribe không trả tiền AI thay người dùng: mọi việc AI — chép lời theo tệp (cả phiên iPhone), chép trực tiếp,
//! biên bản, hỏi đáp (cả `scribe_ask` của MCP), dịch — chỉ chạy bằng key riêng; server không có key AI nào. Mọi người
//! đăng nhập đều thêm được key; không có key hợp lệ cho việc đó thì request bị từ chối 403 `NO_USER_KEY` với câu bảo thêm
//! key ở Key riêng ([`no_key_error`]). Không có phút, lượt ngày hay trần AI (tiền của người dùng); dung lượng, cỡ tệp và
//! rate limit vẫn áp.
//!
//! - Lưu trên server, mã hoá AES-256-GCM ([`crypto`]) bằng bí mật trong tệp `USER_KEYS_SECRET_PATH`; thiếu tệp thì tính
//!   năng tắt ([`api`] trả `available: false`) và mọi việc AI bị từ chối. API chỉ ghi: không bao giờ trả key, chỉ nhà
//!   cung cấp, 4 ký tự cuối, trạng thái, giờ sửa/kiểm. Key không vào log, audit hay thông báo lỗi.
//! - Việc nào dùng key nào ([`Use`]): chép lời theo tệp (ElevenLabs có người nói, OpenAI `gpt-transcribe`, Gemini), chép
//!   trực tiếp (ElevenLabs, OpenAI Realtime), model chữ cho biên bản/hỏi đáp/dịch (OpenAI, Gemini, Claude, kèm model).
//!   Người dùng chọn nhà cung cấp cho từng việc; chưa chọn (hoặc key đã chọn không còn hợp lệ) thì lấy key hợp lệ đầu tiên
//!   theo thứ tự trên ([`pick`]).
//! - Lượt xếp hàng chỉ ghi nhà cung cấp; worker đọc và giải mã key của chủ phiên lúc chạy ([`open`]).
//! - Key lỗi lúc dùng (sai, thu hồi, thiếu quyền, hết credit): lượt đó dừng với câu "Key … của bạn không dùng được: … —
//!   sửa hoặc xoá key ở Key riêng", key bị đánh dấu ([`report`]).

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
/// mặc định; `None` = không có key nào làm được việc này.
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

/// Câu người dùng thấy khi chưa có key riêng hợp lệ cho việc `use_` (`enabled` = máy chủ bật tính năng Key riêng).
pub fn missing_key_message(use_: Use, enabled: bool) -> String {
    if !enabled {
        return "Máy chủ đang tắt tính năng Key riêng nên chưa dùng được các việc AI, thử lại sau.".into();
    }
    let what = match use_ {
        Use::Transcribe => "Chép lời",
        Use::Live => "Chép trực tiếp",
        Use::Text => "Biên bản, hỏi đáp và dịch",
    };
    let labels: Vec<&str> = use_.providers().iter().map(|v| v.label()).collect();
    let providers = match labels.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} hoặc {last}", rest.join(", ")),
        _ => labels.concat(),
    };
    format!("{what} chạy bằng key riêng của bạn ({providers}). Thêm key ở Key riêng để dùng.")
}

/// Lỗi 403 `NO_USER_KEY`: người dùng chưa có key riêng hợp lệ cho việc `use_`.
pub fn no_key_error(use_: Use, enabled: bool) -> AppError {
    AppError::Coded { status: StatusCode::FORBIDDEN, code: "NO_USER_KEY", message: missing_key_message(use_, enabled) }
}

/// Nhà cung cấp của key riêng đang có hiệu lực cho việc `use_` (`Use::Text` trả cả model); không có ⟹ `NO_USER_KEY`.
pub fn require(use_: Use, active: &Active, enabled: bool) -> Result<(Vendor, Option<String>), AppError> {
    let found = match use_ {
        Use::Transcribe => active.transcribe.map(|v| (v, None)),
        Use::Live => active.live.map(|v| (v, None)),
        Use::Text => active.text.clone().map(|(v, m)| (v, Some(m))),
    };
    found.ok_or_else(|| no_key_error(use_, enabled))
}

/// Nhà cung cấp key riêng của `user_id` cho việc `use_`; không có ⟹ `NO_USER_KEY`. Tính năng Key riêng tắt thì trả lỗi
/// mà không đọc Mongo.
pub async fn required(state: &AppState, user_id: &str, use_: Use) -> Result<(Vendor, Option<String>), AppError> {
    require(use_, &active(state, user_id).await?, available(state))
}

/// Model chữ bằng key riêng của người dùng: key đã giải mã và model. Không có key ⟹ `NO_USER_KEY`; key đang có hiệu lực
/// mà không mở được ⟹ `USER_KEY_FAILED`.
pub(crate) async fn text_key(state: &AppState, user_id: &str) -> Result<(UserKey, String), AppError> {
    let (provider, model) = required(state, user_id, Use::Text).await?;
    let model = model.unwrap_or_else(|| llm::default_model(provider).to_string());
    match open(state, user_id, provider).await {
        Ok(key) => Ok((key, model)),
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
    fn without_a_key_for_the_use_the_request_is_refused_with_a_pointer_to_key_rieng() {
        let none = Active::default();
        for use_ in [Use::Transcribe, Use::Live, Use::Text] {
            let Err(AppError::Coded { status, code, message }) = require(use_, &none, true) else { panic!("phải từ chối") };
            assert_eq!((status, code), (StatusCode::FORBIDDEN, "NO_USER_KEY"));
            assert!(message.ends_with("Thêm key ở Key riêng để dùng."), "{message}");
        }
        assert_eq!(
            missing_key_message(Use::Transcribe, true),
            "Chép lời chạy bằng key riêng của bạn (ElevenLabs, OpenAI hoặc Google Gemini). Thêm key ở Key riêng để dùng."
        );
        assert!(missing_key_message(Use::Live, true).contains("(ElevenLabs hoặc OpenAI)"));
        assert!(missing_key_message(Use::Text, false).contains("tắt tính năng Key riêng"));
        // Key chép lời không mở được biên bản và ngược lại.
        let only_transcribe = Active { transcribe: Some(Vendor::ElevenLabs), ..Active::default() };
        assert_eq!(require(Use::Transcribe, &only_transcribe, true).ok(), Some((Vendor::ElevenLabs, None)));
        assert!(require(Use::Text, &only_transcribe, true).is_err());
        assert!(require(Use::Live, &only_transcribe, true).is_err());
        let text = Active { text: Some((Vendor::Anthropic, "claude-haiku-5-5".into())), ..Active::default() };
        assert_eq!(require(Use::Text, &text, true).ok(), Some((Vendor::Anthropic, Some("claude-haiku-5-5".into()))));
    }

    #[test]
    fn prefs_ignore_providers_that_cannot_do_the_job() {
        let d = doc! { "transcribe": "anthropic", "live": "gemini", "text": "gemini", "textModel": "x" };
        let p = Prefs::from_doc(&d);
        assert_eq!((p.transcribe, p.live, p.text), (None, None, Some(Vendor::Gemini)));
        assert_eq!(p.to_json()["text"], "gemini");
    }
}
