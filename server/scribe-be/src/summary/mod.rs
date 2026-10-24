//! Biên bản sau cuộc họp: tóm tắt, quyết định và việc cần làm của một transcript, viết bằng OpenAI
//! (`SUMMARY_MODEL`, mặc định `gpt-4.1-mini`) với key của server, khi người dùng bấm "Tóm tắt".
//!
//! Đầu vào là các câu đã chép xong của phiên, đã áp câu người dùng sửa (`docs::load`, cùng đường ghép
//! với màn hình đọc), mỗi câu kèm giây bắt đầu tính theo cả phiên. Biên bản lưu ở
//! `transcript_docs.summary` cùng băm chữ đầu vào: chữ đổi thì API báo `stale`.
//!
//! Tiền: mỗi tài khoản có số lượt mỗi ngày giờ Việt Nam theo gói (`SUMMARY_FREE_DAILY`,
//! `SUMMARY_PAID_DAILY`) và trần chi phí mỗi tháng (`SUMMARY_*_MONTHLY_MINUTES`); chi phí token quy ra
//! giây chép lời tương đương, giữ chỗ theo ước lượng trên sổ tháng của user lẫn trần tổng tháng trước khi
//! gọi rồi quyết toán theo token thật, nên cảnh báo 80/100% và chặn ở 100% của trần vẫn đúng. Lượt ngày
//! chỉ được trả lại khi OpenAI chắc chắn chưa xử lý gì. Audit không ghi nội dung.
//!
//! Phần có tiền (lấy lượt → gọi model → quyết toán → lưu biên bản) chạy trong task riêng, handler chờ
//! kết quả: client ngắt kết nối (Cloudflare cắt ở ~100 giây, người dùng rời trang) thì task vẫn chạy
//! hết, sổ không bị giữ chỗ treo và biên bản vẫn được lưu. Trình tự đo lường ở `meter`, dùng chung với
//! hỏi đáp (`crate::search::ask`).
//!
//! Model chữ chạy bằng key server (`LLM_PROVIDER`/`LLM_MODEL`), key riêng của người dùng hoặc key của chủ domain
//! ([`TextKey`], chọn ở `tenant_keys::choose`): lượt bằng key riêng hay key chủ domain không có lượt ngày hay trần nào
//! (`meter::reserve_own`), lỗi của key đó chỉ đánh dấu key đó (`USER_KEY_FAILED`, `TENANT_KEY_FAILED`) và không tạm
//! ngưng tính năng cho người khác.

pub(crate) mod llm;
pub(crate) mod meter;
pub mod share;

use crate::tenant::Tenant;
use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::transcript_doc::{self, Action, Decision, StoredSummary};
use crate::models::summary::Feature;
use crate::config::Vendor;
use crate::models::{plan, user};
use crate::state::AppState;
use crate::tenant_keys::{self, Choice, TenantKey};
use crate::transcribe::docs;
use crate::user_keys::{self, UserKey};
use llm::is_key_broken;
use crate::notify;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::{doc, DateTime};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Chữ đầu vào dài nhất (ký tự): ~3 giờ họp nói liên tục vẫn dưới mức này; chặn chi phí bất thường.
const MAX_INPUT_CHARS: usize = 400_000;
/// Trần token đầu ra của một lượt (biên bản vài nghìn chữ là đủ).
pub(crate) const MAX_OUTPUT_TOKENS: i64 = 4_000;
/// Một người bấm tối đa chừng này lượt mỗi phút (chống bấm liên tục, cộng với khoá một lượt mỗi phiên).
const PER_MINUTE: u32 = 6;

pub(crate) const PAUSED: &str = "Tóm tắt và hỏi đáp đang tạm ngưng, quản trị đã được báo. Thử lại sau.";
const FAILED: &str = "Không tạo được biên bản lúc này, thử lại sau.";

/// Trạng thái chung của các tính năng gọi model chữ (tóm tắt, hỏi đáp, dịch) trong tiến trình.
#[derive(Default)]
pub struct Summaries {
    /// Các lượt đang chạy (`<userId>|<slot>`): slot là khoá phiên khi tóm tắt (tối đa một lượt mỗi
    /// phiên), [`ASK_SLOT`] khi hỏi đáp (tối đa một câu hỏi đang chạy mỗi người), `tr:<khoá phiên>` khi dịch.
    keys: Mutex<HashSet<String>>,
    /// Lượt tóm tắt, hỏi đáp hay dịch gặp lỗi key/credit: các tính năng này (cùng một API Chat Completions)
    /// tạm ngưng tới khi khởi động lại, chép lời không bị ngắt theo (chép lời tự ngắt khi chính nó gặp lỗi đó).
    paused: AtomicBool,
}

/// Slot "đang chạy" của hỏi đáp; khoá phiên luôn có tiền tố `s:`/`f:` nên không trùng.
pub(crate) const ASK_SLOT: &str = "ask";

/// Giữ chỗ "đang chạy" tới khi lượt kết thúc. Sở hữu `Arc` nên đi được vào task nền cùng lượt.
pub struct RunningGuard {
    summaries: Arc<Summaries>,
    id: String,
}

impl Summaries {
    pub(crate) fn start(self: &Arc<Self>, user_id: &str, slot: &str) -> Option<RunningGuard> {
        let id = format!("{user_id}|{slot}");
        let mut keys = self.keys.lock().unwrap_or_else(|p| p.into_inner());
        keys.insert(id.clone()).then(|| RunningGuard { summaries: Arc::clone(self), id })
    }

    /// Lượt `slot` của user đang chạy trong tiến trình này.
    pub(crate) fn is_running(&self, user_id: &str, slot: &str) -> bool {
        self.keys.lock().unwrap_or_else(|p| p.into_inner()).contains(&format!("{user_id}|{slot}"))
    }

    pub(crate) fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// `true` ở lần ngắt đầu tiên (để báo quản trị một lần).
    pub(crate) fn pause(&self) -> bool {
        !self.paused.swap(true, Ordering::Relaxed)
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.summaries.keys.lock().unwrap_or_else(|p| p.into_inner()).remove(&self.id);
    }
}

/// Key của một lượt gọi model chữ.
pub(crate) enum TextKey {
    /// Key server của `LLM_PROVIDER`, model `LLM_MODEL`.
    Server { provider: Vendor, key: String, model: String },
    /// Key riêng của người dùng và model họ chọn.
    Own { key: UserKey, model: String },
    /// Key của chủ domain và model mặc định của nhà cung cấp.
    Tenant { key: TenantKey, model: String },
}

impl TextKey {
    pub(crate) fn llm(&self) -> llm::Llm<'_> {
        match self {
            TextKey::Server { provider, key, model } => llm::Llm { vendor: *provider, model, api_key: key },
            TextKey::Own { key, model } => llm::Llm { vendor: key.provider, model, api_key: key.value() },
            TextKey::Tenant { key, model } => llm::Llm { vendor: key.vendor, model, api_key: key.value() },
        }
    }

    pub(crate) fn model(&self) -> &str {
        match self {
            TextKey::Server { model, .. } | TextKey::Own { model, .. } | TextKey::Tenant { model, .. } => model,
        }
    }

    /// Mã nhà cung cấp khi lượt không chạy bằng key server (`byok` của audit và nhật ký).
    pub(crate) fn byok(&self) -> Option<Vendor> {
        match self {
            TextKey::Server { .. } => None,
            TextKey::Own { key, .. } => Some(key.provider),
            TextKey::Tenant { key, .. } => Some(key.vendor),
        }
    }

    /// `keySource` của audit và nhật ký: `tenant` khi chạy bằng key của chủ domain.
    pub(crate) fn key_source(&self) -> Option<&'static str> {
        matches!(self, TextKey::Tenant { .. }).then_some(crate::models::transcript::TENANT_KEY_SOURCE)
    }
}

/// Key cho một lượt theo `tenant_keys::choose`: key riêng của người dùng (không mở được ⟹ `USER_KEY_FAILED`), key của
/// chủ domain (không mở được ⟹ `TENANT_KEY_FAILED`), hay key server — không bao giờ dùng key server thay key khác. Key
/// server thiếu hoặc đang tạm ngưng ⟹ 503 với câu `paused`; chỉ tenant chính có key server: tenant sổ không có key cho
/// việc này ⟹ 403 `TENANT_KEY_MISSING`.
pub(crate) async fn text_key(
    state: &AppState,
    tenant: &crate::tenant::TenantCtx,
    user_id: &str,
    paused: &str,
) -> Result<TextKey, AppError> {
    match tenant_keys::choose(state, tenant, user_id, user_keys::Use::Text).await? {
        Choice::Own(provider) => {
            // Key vừa bị xoá giữa hai lần đọc: báo như key riêng không mở được.
            let (key, model) = user_keys::text_key(state, user_id)
                .await?
                .ok_or_else(|| user_keys::user_key_error(user_keys::failure_message(provider, "key đã bị xoá")))?;
            return Ok(TextKey::Own { key, model });
        }
        Choice::Tenant(provider) => {
            let key = tenant_keys::open(state, &tenant.id, provider).await.map_err(tenant_keys::tenant_key_error)?;
            return Ok(TextKey::Tenant { key, model: llm::default_model(provider).to_string() });
        }
        Choice::Server => {}
    }
    let (provider, key) = state
        .transcriber
        .text_key()
        .filter(|_| !state.summaries.paused())
        .ok_or_else(|| AppError::Unavailable(paused.into()))?;
    Ok(TextKey::Server { provider, key: key.to_string(), model: state.config.summary_model.clone() })
}

/// Bước giữ chỗ theo key: key server qua mọi hạn mức (`meter::reserve`), key riêng hay key chủ domain chỉ mở lượt
/// (`meter::reserve_own`).
pub(crate) async fn reserve_for(
    state: &AppState,
    feature: Feature,
    user_id: &str,
    key: Option<&str>,
    plan: plan::Plan,
    estimate: i64,
    text_key: &TextKey,
) -> Result<meter::Reservation, AppError> {
    match text_key.byok() {
        None => meter::reserve(state, feature, user_id, key, plan, estimate).await,
        Some(vendor) => {
            let byok = meter::Byok { vendor, key_source: text_key.key_source() };
            meter::reserve_own(state, feature, user_id, key, byok, text_key.model()).await
        }
    }
}

/// Lỗi key/credit của một lượt: key server ⟹ tạm ngưng tính năng (báo quản trị) và 503 `paused`; key riêng ⟹ đánh dấu
/// key đó và 403 `USER_KEY_FAILED`; key chủ domain ⟹ báo IdP và 403 `TENANT_KEY_FAILED`. Lỗi khác ⟹ `None`.
pub(crate) async fn key_failure(state: &AppState, text_key: &TextKey, e: &str, paused: &str) -> Option<AppError> {
    match (key_error_action(text_key.byok().is_some(), e), text_key) {
        (KeyErrorAction::None, _) => None,
        (KeyErrorAction::PauseServer, _) => {
            report_key_broken(state, e);
            Some(AppError::Unavailable(paused.into()))
        }
        (KeyErrorAction::MarkOwnKey(problem), TextKey::Own { key, .. }) => {
            Some(user_keys::user_key_error(user_keys::report(state, key, problem).await))
        }
        (KeyErrorAction::MarkOwnKey(problem), TextKey::Tenant { key, .. }) => {
            Some(tenant_keys::tenant_key_error(tenant_keys::report(state, key, problem).await))
        }
        (KeyErrorAction::MarkOwnKey(_), TextKey::Server { .. }) => None,
    }
}

/// Việc phải làm khi một lượt gặp lỗi `e`.
#[derive(Debug, PartialEq, Eq)]
enum KeyErrorAction {
    None,
    /// Key server hỏng: tạm ngưng tính năng cho mọi người.
    PauseServer,
    /// Key riêng hay key chủ domain hỏng: chỉ đánh dấu key đó, không bao giờ đụng cờ tạm ngưng của server.
    MarkOwnKey(user_keys::Problem),
}

fn key_error_action(byok: bool, e: &str) -> KeyErrorAction {
    if !is_key_broken(e) {
        return KeyErrorAction::None;
    }
    if byok {
        KeyErrorAction::MarkOwnKey(user_keys::problem_of(e).unwrap_or(user_keys::Problem::Invalid))
    } else {
        KeyErrorAction::PauseServer
    }
}

/// Đầu vào của model: mỗi câu một dòng `[giây] câu`, giây làm tròn xuống tính theo cả phiên.
pub fn build_input(sentences: &[(f64, &str)]) -> String {
    let mut out = String::new();
    for (at, text) in sentences {
        let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(&format!("[{}] {line}\n", at.max(0.0).floor() as i64));
    }
    out
}

/// Đầu vào gửi model: như [`build_input`], thêm tên người nói trước câu (`[giây] Tên: câu`) khi bản chép có người
/// nói. Băm nguồn vẫn tính trên [`build_input`] (không gồm tên) để đổi tên người nói không làm biên bản "cũ".
pub fn build_named_input(sentences: &[(f64, Option<String>, &str)]) -> String {
    let mut out = String::new();
    for (at, name, text) in sentences {
        let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let at = at.max(0.0).floor() as i64;
        match name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => out.push_str(&format!("[{at}] {name}: {line}\n")),
            None => out.push_str(&format!("[{at}] {line}\n")),
        }
    }
    out
}

/// Băm chữ đầu vào (SHA-256 hex) — so với băm đã lưu để biết biên bản đã cũ.
pub fn source_hash(input: &str) -> String {
    crate::session::hash_token(input)
}

/// Biên bản trả cho client.
pub fn to_json(stored: &StoredSummary, current_hash: &str) -> Value {
    json!({
        "model": stored.model,
        "createdAt": stored.created_at.try_to_rfc3339_string().ok(),
        "summary": stored.summary,
        "decisions": stored.decisions,
        "actions": stored.actions,
        "stale": stored.source_hash != current_hash,
    })
}

/// Giới hạn độ dài và mốc của bản model trả về: bỏ mục rỗng, mốc ngoài phiên kéo về trong `[0, max_at]`.
fn sanitize(draft: llm::Draft, max_at: f64) -> (String, Vec<Decision>, Vec<Action>) {
    const MAX_ITEMS: usize = 50;
    const MAX_TEXT: usize = 1_000;
    const MAX_SHORT: usize = 200;
    const MAX_SUMMARY: usize = 8_000;
    let clip = |s: &str, max: usize| -> String { s.trim().chars().take(max).collect::<String>().trim_end().to_string() };
    let short = |s: Option<String>| s.map(|s| clip(&s, MAX_SHORT)).filter(|s| !s.is_empty());
    let at = |a: f64| {
        let a = if a.is_finite() { a.max(0.0) } else { 0.0 };
        let a = if max_at > 0.0 { a.min(max_at) } else { a };
        (a * 10.0).round() / 10.0
    };
    let decisions = draft
        .decisions
        .into_iter()
        .map(|d| Decision { text: clip(&d.text, MAX_TEXT), at: at(d.at) })
        .filter(|d| !d.text.is_empty())
        .take(MAX_ITEMS)
        .collect();
    let actions = draft
        .actions
        .into_iter()
        .map(|a| Action { text: clip(&a.text, MAX_TEXT), owner: short(a.owner), due: short(a.due), at: at(a.at) })
        .filter(|a| !a.text.is_empty())
        .take(MAX_ITEMS)
        .collect();
    (clip(&draft.summary, MAX_SUMMARY), decisions, actions)
}

/// Phần đã kiểm xong của một lượt, đủ để task nền chạy tiếp mà không cần request.
struct Job {
    user_id: String,
    key: String,
    plan: plan::Plan,
    input: String,
    chars: usize,
    hash: String,
    max_at: f64,
    text_key: TextKey,
}

/// `POST /api/transcript-docs/{key}/summary` — tạo (hoặc tạo lại) biên bản của phiên.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Extension(tenant): Extension<Tenant>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.config.transcribe_allowed(&tenant, &profile.email) {
        return Err(AppError::Forbidden("tài khoản này chưa được dùng tóm tắt".into()));
    }
    let text_key = text_key(&state, &tenant, &user.0, PAUSED).await?;
    if !state.rate.allow(&format!("summary:{}", user.0), PER_MINUTE) {
        return Err(AppError::TooManyRequests("bấm tóm tắt quá nhanh, thử lại sau một phút".into()));
    }
    let running = state
        .summaries
        .start(&user.0, &key)
        .ok_or_else(|| AppError::TooManyRequests("bản này đang được tóm tắt, chờ một chút".into()))?;

    let loaded = docs::load(&state, &user.0, &key).await?;
    let sentences = loaded.timed_sentences();
    if sentences.is_empty() {
        return Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "NOT_TRANSCRIBED",
            message: "Bản ghi chưa có đoạn nào chép lời xong. Chép lời trước rồi tóm tắt.".into(),
        });
    }
    let hash = source_hash(&build_input(&sentences));
    let input = build_named_input(&loaded.timed_named_sentences());
    let chars = input.chars().count();
    if chars > MAX_INPUT_CHARS {
        return Err(AppError::TooLarge("bản chép lời quá dài để tóm tắt".into()));
    }
    let job = Job {
        user_id: user.0.clone(),
        key,
        plan: plan::effective(profile.plan.as_deref(), profile.plan_until, DateTime::now()),
        hash,
        input,
        chars,
        max_at: loaded.duration_s(),
        text_key,
    };
    // Task giữ `running` tới khi xong; handler bị huỷ thì task vẫn chạy hết.
    let task = tokio::spawn(run(state, job, running));
    task.await.unwrap_or_else(|e| {
        tracing::error!("task tóm tắt dừng bất thường: {e}");
        Err(AppError::Internal("tóm tắt dừng bất thường".into()))
    })
}

/// Bị key/credit chặn ở lượt tóm tắt hay hỏi đáp: tạm ngưng cả hai tính năng, báo quản trị một lần.
pub(crate) fn report_key_broken(state: &AppState, e: &str) {
    if state.summaries.pause() {
        let provider = state.config.llm_provider;
        tracing::error!(provider = provider.code(), "key model chữ của server không dùng được ({e}) — tạm ngưng tới khi khởi động lại");
        audit::record(state, None, "summary.key_invalid", doc! { "reason": e, "provider": provider.code() });
        notify::send(state, pause_alert(provider, e));
    }
}

/// Câu báo chủ sản phẩm khi key model chữ của server hỏng, nêu đúng nhà cung cấp `LLM_PROVIDER`.
fn pause_alert(provider: Vendor, e: &str) -> String {
    format!(
        "Tóm tắt, hỏi đáp và dịch tạm ngưng: key {} của server không dùng được ({e}). Kiểm tra key rồi khởi động lại.",
        provider.label()
    )
}

/// Phần có tiền của một lượt: mọi đường ra đều quyết toán phần đã giữ.
async fn run(state: AppState, job: Job, _running: RunningGuard) -> Result<Json<Value>, AppError> {
    let (db, user_id, key) = (&state.db, job.user_id.as_str(), job.key.as_str());
    let estimate = meter::estimate_seconds(meter::estimate_input_tokens(job.chars), MAX_OUTPUT_TOKENS);
    let reservation = reserve_for(&state, Feature::Summary, user_id, Some(key), job.plan, estimate, &job.text_key).await?;

    // Từ đây phần giữ chỗ phải được quyết toán ở mọi đường ra.
    let model = job.text_key.model();
    let call = llm::summarize(&state.http, &job.text_key.llm(), &job.input).await;
    let cost_s = meter::settle(&state, &reservation, &call).await;
    let (input_tokens, output_tokens) = call.tokens();
    let draft = call.result.and_then(|c| c.draft);

    let draft = match draft {
        Ok(draft) => draft,
        Err(e) => {
            if let Some(error) = key_failure(&state, &job.text_key, &e, PAUSED).await {
                return Err(error);
            }
            tracing::warn!(key = %key, "tóm tắt lỗi: {e}");
            audit::record(&state, Some(user_id), "summary.failed", doc! { "key": key, "error": &e });
            return Err(AppError::Unavailable(FAILED.into()));
        }
    };
    let (summary, decisions, actions) = sanitize(draft, job.max_at);
    let stored = StoredSummary {
        model: model.to_string(),
        created_at: DateTime::now(),
        source_hash: job.hash.clone(),
        summary,
        decisions,
        actions,
    };
    transcript_doc::set_summary(db, user_id, key, &stored).await?;
    let details = doc! {
        "key": key,
        "inputTokens": input_tokens,
        "outputTokens": output_tokens,
        "costS": cost_s,
        "decisions": stored.decisions.len() as i64,
        "actions": stored.actions.len() as i64,
        "byok": job.text_key.byok().is_some(),
        "keySource": job.text_key.key_source(),
    };
    audit::record(&state, Some(user_id), "summary.create", details);
    Ok(Json(to_json(&stored, &job.hash)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_lines_carry_whole_session_seconds() {
        let input = build_input(&[(0.4, "Xin chào."), (61.9, "Chốt  ngày\n thứ Hai."), (-1.0, "Lạ.")]);
        assert_eq!(input, "[0] Xin chào.\n[61] Chốt ngày thứ Hai.\n[0] Lạ.\n");
        assert_eq!(source_hash(&input), source_hash(&input.clone()));
        assert_ne!(source_hash(&input), source_hash("[0] Xin chào.\n"));
    }

    #[test]
    fn named_input_adds_speakers_without_changing_the_hash_source() {
        let named = build_named_input(&[(1.2, Some("Lan".into()), "Chốt  giá."), (5.0, None, "Xong."), (7.0, Some(" ".into()), "Ừ.")]);
        assert_eq!(named, "[1] Lan: Chốt giá.\n[5] Xong.\n[7] Ừ.\n");
        assert_eq!(build_named_input(&[(1.2, None, "Chốt giá.")]), build_input(&[(1.2, "Chốt giá.")]));
    }

    #[test]
    fn one_run_per_session_at_a_time() {
        let running = Arc::new(Summaries::default());
        let first = running.start("u", "s:abc").expect("lượt đầu");
        assert!(running.start("u", "s:abc").is_none());
        assert!(running.start("u", "s:other").is_some());
        assert!(running.start("v", "s:abc").is_some());
        // Guard sống trong task khác (lượt chạy nền) vẫn giữ chỗ tới khi task xong.
        let moved = std::thread::spawn(move || drop(first));
        moved.join().expect("thread");
        assert!(running.start("u", "s:abc").is_some());
    }

    #[test]
    fn the_pause_alert_names_the_server_text_provider() {
        assert!(pause_alert(Vendor::Anthropic, "x").contains("key Anthropic Claude của server"));
        assert!(pause_alert(Vendor::Gemini, "x").contains("key Google Gemini của server"));
        assert!(pause_alert(Vendor::OpenAi, "x").contains("key OpenAI của server"));
    }

    #[test]
    fn an_own_key_failure_never_pauses_the_server() {
        use user_keys::Problem;
        assert_eq!(key_error_action(true, llm::KEY_INVALID), KeyErrorAction::MarkOwnKey(Problem::Invalid));
        assert_eq!(key_error_action(true, llm::NO_CREDIT), KeyErrorAction::MarkOwnKey(Problem::NoCredit));
        assert_eq!(key_error_action(false, llm::NO_CREDIT), KeyErrorAction::PauseServer);
        assert_eq!(key_error_action(true, "Dịch vụ OpenAI đang lỗi (HTTP 500)"), KeyErrorAction::None);
        assert_eq!(key_error_action(false, "Dịch vụ OpenAI đang lỗi (HTTP 500)"), KeyErrorAction::None);
    }

    #[test]
    fn pause_reports_only_the_first_trip() {
        let s = Summaries::default();
        assert!(!s.paused());
        assert!(s.pause());
        assert!(!s.pause());
        assert!(s.paused());
    }

    #[test]
    fn model_output_is_clipped_and_clamped() {
        let draft = llm::Draft {
            summary: format!("  {}  ", "a".repeat(9_000)),
            decisions: vec![
                Decision { text: " Chốt A ".into(), at: 12.345 },
                Decision { text: "   ".into(), at: 1.0 },
                Decision { text: "Ngoài phiên".into(), at: 9_999.0 },
                Decision { text: "Âm".into(), at: f64::NAN },
            ],
            actions: vec![Action { text: "Gửi báo giá".into(), owner: Some("  ".into()), due: Some(" thứ Sáu ".into()), at: -3.0 }],
        };
        let (summary, decisions, actions) = sanitize(draft, 600.0);
        assert_eq!(summary.chars().count(), 8_000);
        assert_eq!(decisions.iter().map(|d| (d.text.as_str(), d.at)).collect::<Vec<_>>(), vec![
            ("Chốt A", 12.3),
            ("Ngoài phiên", 600.0),
            ("Âm", 0.0)
        ]);
        assert_eq!(actions[0], Action { text: "Gửi báo giá".into(), owner: None, due: Some("thứ Sáu".into()), at: 0.0 });
    }

    #[test]
    fn stale_follows_the_source_hash() {
        let stored = StoredSummary {
            model: "gpt-4.1-mini".into(),
            created_at: DateTime::from_millis(0),
            source_hash: "abc".into(),
            summary: "Tóm tắt".into(),
            decisions: vec![],
            actions: vec![Action { text: "Việc".into(), owner: None, due: None, at: 3.0 }],
        };
        assert_eq!(to_json(&stored, "abc")["stale"], json!(false));
        let v = to_json(&stored, "xyz");
        assert_eq!(v["stale"], json!(true));
        assert_eq!(v["actions"][0], json!({ "text": "Việc", "owner": null, "due": null, "at": 3.0 }));
        assert_eq!(v["createdAt"], json!("1970-01-01T00:00:00Z"));
    }

}
