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
//! hết, sổ không bị giữ chỗ treo và biên bản vẫn được lưu.

mod llm;
pub mod share;

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::transcript_doc::{self, Action, Decision, StoredSummary};
use crate::models::{plan, summary as ledger, usage, user};
use crate::state::AppState;
use crate::transcribe::{self, docs, openai::is_key_broken};
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
/// Token của phần hướng dẫn và schema, cộng vào ước lượng đầu vào.
const PROMPT_OVERHEAD_TOKENS: i64 = 800;
/// Giá `gpt-4.1-mini` theo đơn vị 1e-8 USD mỗi token ($0,40 vào, $1,60 ra mỗi triệu token), để quy chi
/// phí ra giây chép lời tương đương bằng số nguyên (không lệch làm tròn).
const INPUT_PRICE_E8: i64 = 40;
const OUTPUT_PRICE_E8: i64 = 160;
/// Một người bấm tối đa chừng này lượt mỗi phút (chống bấm liên tục, cộng với khoá một lượt mỗi phiên).
const PER_MINUTE: u32 = 6;

const PAUSED: &str = "Tóm tắt đang tạm ngưng, quản trị đã được báo. Thử lại sau.";
const FAILED: &str = "Không tạo được biên bản lúc này, thử lại sau.";

/// Trạng thái chung của tính năng tóm tắt trong tiến trình.
#[derive(Default)]
pub struct Summaries {
    /// Các phiên đang được tóm tắt (`<userId>|<key>`): tối đa một lượt đang chạy cho mỗi phiên.
    keys: Mutex<HashSet<String>>,
    /// Lượt tóm tắt gặp lỗi key/credit: chỉ tóm tắt tạm ngưng tới khi khởi động lại, chép lời không bị
    /// ngắt theo (chép lời tự ngắt khi chính nó gặp lỗi đó).
    paused: AtomicBool,
}

/// Giữ chỗ "đang chạy" tới khi lượt kết thúc. Sở hữu `Arc` nên đi được vào task nền cùng lượt.
pub struct RunningGuard {
    summaries: Arc<Summaries>,
    id: String,
}

impl Summaries {
    fn start(self: &Arc<Self>, user_id: &str, key: &str) -> Option<RunningGuard> {
        let id = format!("{user_id}|{key}");
        let mut keys = self.keys.lock().unwrap_or_else(|p| p.into_inner());
        keys.insert(id.clone()).then(|| RunningGuard { summaries: Arc::clone(self), id })
    }

    fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// `true` ở lần ngắt đầu tiên (để báo quản trị một lần).
    fn pause(&self) -> bool {
        !self.paused.swap(true, Ordering::Relaxed)
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.summaries.keys.lock().unwrap_or_else(|p| p.into_inner()).remove(&self.id);
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

/// Băm chữ đầu vào (SHA-256 hex) — so với băm đã lưu để biết biên bản đã cũ.
pub fn source_hash(input: &str) -> String {
    crate::session::hash_token(input)
}

/// Chi phí token quy ra giây chép lời tương đương (giá chép lời `USD_PER_MINUTE` mỗi phút), làm tròn lên.
pub fn cost_seconds(input_tokens: i64, output_tokens: i64) -> i64 {
    let cost_e8 = input_tokens.max(0) * INPUT_PRICE_E8 + output_tokens.max(0) * OUTPUT_PRICE_E8;
    // Giá một giây chép lời theo cùng đơn vị: $0,003 / 60 = 5000 × 1e-8 USD.
    let second_e8 = ((crate::config::USD_PER_MINUTE / 60.0) * 1e8).round() as i64;
    (cost_e8 + second_e8 - 1) / second_e8
}

/// Ước lượng token đầu vào cho phần giữ chỗ: chữ Việt ~2–3 ký tự mỗi token, lấy 2 cho chắc phần trên.
fn estimate_input_tokens(chars: usize) -> i64 {
    chars as i64 / 2 + PROMPT_OVERHEAD_TOKENS
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

fn daily_limit_error(used: i64, limit: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    AppError::Quota {
        code: "SUMMARY_DAILY_LIMIT",
        message: format!("Hôm nay đã dùng {used}/{limit} lượt tóm tắt. Mở lại lúc 0 giờ."),
        used_minutes: used,
        limit_minutes: limit,
        retry_at: Some(usage::next_day(now).to_rfc3339()),
    }
}

fn monthly_limit_error(used_s: i64, limit_s: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    AppError::Quota {
        code: "SUMMARY_MONTHLY_LIMIT",
        message: "Tháng này tài khoản đã dùng hết hạn mức tóm tắt, sẽ mở lại đầu tháng sau.".into(),
        used_minutes: usage::used_minutes(used_s),
        limit_minutes: limit_s / 60,
        retry_at: Some(usage::next_reset(now).to_rfc3339()),
    }
}

fn cap_error(now: chrono::DateTime<chrono::Utc>) -> AppError {
    AppError::Quota {
        code: "CAP_REACHED",
        message: "Tóm tắt đã dùng hết hạn mức chung của tháng này, sẽ mở lại đầu tháng sau.".into(),
        used_minutes: 0,
        limit_minutes: 0,
        retry_at: Some(usage::next_reset(now).to_rfc3339()),
    }
}

/// Chi phí ghi sổ của một lượt gọi model (giây tương đương) và có trả lại lượt ngày không. OpenAI đã
/// xử lý thì tính theo token thật, không trả lượt (kể cả khi biên bản bị cắt, bị từ chối hay sai dạng);
/// có thể đã xử lý mà không biết token thì giữ nguyên phần ước lượng; chắc chắn chưa xử lý thì không
/// tính gì và trả lượt.
fn charge_of(call: &llm::Call, estimate: i64) -> (i64, bool) {
    match &call.result {
        Ok(c) => (cost_seconds(c.input_tokens, c.output_tokens), false),
        Err(_) if call.maybe_billed => (estimate, false),
        Err(_) => (0, true),
    }
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
    api_key: String,
}

/// `POST /api/transcript-docs/{key}/summary` — tạo (hoặc tạo lại) biên bản của phiên.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.config.transcribe_allowed(&profile.email) {
        return Err(AppError::Forbidden("tài khoản này chưa được dùng tóm tắt".into()));
    }
    let api_key = state
        .transcriber
        .api_key()
        .filter(|_| !state.summaries.paused())
        .ok_or_else(|| AppError::Unavailable(PAUSED.into()))?
        .to_string();
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
    let input = build_input(&sentences);
    let chars = input.chars().count();
    if chars > MAX_INPUT_CHARS {
        return Err(AppError::TooLarge("bản chép lời quá dài để tóm tắt".into()));
    }
    let job = Job {
        user_id: user.0.clone(),
        key,
        plan: plan::effective(profile.plan.as_deref(), profile.plan_until, DateTime::now()),
        hash: source_hash(&input),
        input,
        chars,
        max_at: loaded.duration_s(),
        api_key,
    };
    // Task giữ `running` tới khi xong; handler bị huỷ thì task vẫn chạy hết.
    let task = tokio::spawn(run(state, job, running));
    task.await.unwrap_or_else(|e| {
        tracing::error!("task tóm tắt dừng bất thường: {e}");
        Err(AppError::Internal("tóm tắt dừng bất thường".into()))
    })
}

/// Trả lại phần đã giữ (lượt ngày, sổ tháng, và trần tổng khi `total`) khi lượt dừng trước lúc gọi model.
async fn undo(db: &mongodb::Database, user_id: &str, day: &str, month: &str, estimate: i64, total: bool) {
    ledger::refund_daily(db, user_id, day).await;
    if let Err(e) = ledger::settle_monthly(db, user_id, month, -estimate).await {
        tracing::error!(estimate, "trả chi phí tháng của tóm tắt lỗi: {e:?}");
    }
    if total {
        if let Err(e) = usage::settle_total(db, month, -estimate).await {
            tracing::error!(estimate, "trả trần tổng của tóm tắt lỗi: {e:?}");
        }
    }
}

/// Phần có tiền của một lượt: mọi đường ra đều quyết toán phần đã giữ.
async fn run(state: AppState, job: Job, _running: RunningGuard) -> Result<Json<Value>, AppError> {
    let (db, user_id, key) = (&state.db, job.user_id.as_str(), job.key.as_str());
    let now = chrono::Utc::now();
    let (day, month) = (usage::day_key(now), usage::month_key(now));
    let limits = state.config.summary_limits;
    let (daily, monthly) = if job.plan == plan::Plan::Free {
        (limits.free_daily, limits.free_monthly_seconds)
    } else {
        (limits.paid_daily, limits.paid_monthly_seconds)
    };
    if !ledger::take_daily(db, user_id, &day, daily).await? {
        let used = ledger::used_daily(db, user_id, &day).await?;
        audit::record(&state, Some(user_id), "summary.blocked", doc! { "reason": "DailyLimit" });
        return Err(daily_limit_error(used, daily, now));
    }
    let estimate = cost_seconds(estimate_input_tokens(job.chars), MAX_OUTPUT_TOKENS);
    match ledger::reserve_monthly(db, user_id, &month, estimate, monthly).await {
        Ok(true) => {}
        Ok(false) => {
            ledger::refund_daily(db, user_id, &day).await;
            let used = ledger::used_monthly(db, user_id, &month).await?;
            audit::record(&state, Some(user_id), "summary.blocked", doc! { "reason": "MonthlyLimit" });
            return Err(monthly_limit_error(used, monthly, now));
        }
        Err(e) => {
            ledger::refund_daily(db, user_id, &day).await;
            return Err(e);
        }
    }
    match usage::reserve_total(db, &month, estimate, &state.config.transcribe_limits).await {
        Ok(true) => {}
        Ok(false) => {
            undo(db, user_id, &day, &month, estimate, false).await;
            audit::record(&state, Some(user_id), "summary.blocked", doc! { "reason": "TotalCap" });
            return Err(cap_error(now));
        }
        Err(e) => {
            undo(db, user_id, &day, &month, estimate, false).await;
            return Err(e);
        }
    }
    let model = state.config.summary_model.as_str();
    let opened = ledger::Run { user_id, key, model, month: &month, day: &day, reserved_s: estimate };
    let run_id = match ledger::open_run(db, &opened).await {
        Ok(id) => id,
        Err(e) => {
            undo(db, user_id, &day, &month, estimate, true).await;
            return Err(e);
        }
    };

    // Từ đây phần giữ chỗ phải được quyết toán ở mọi đường ra.
    let call = llm::summarize(&state.http, &job.api_key, model, &job.input).await;
    let (cost_s, refund) = charge_of(&call, estimate);
    if let Err(e) = usage::settle_total(db, &month, cost_s - estimate).await {
        tracing::error!(cost_s, estimate, "quyết toán trần cho tóm tắt lỗi: {e:?}");
    }
    if let Err(e) = ledger::settle_monthly(db, user_id, &month, cost_s - estimate).await {
        tracing::error!(cost_s, estimate, "quyết toán chi phí tháng của tóm tắt lỗi: {e:?}");
    }
    if refund {
        ledger::refund_daily(db, user_id, &day).await;
    }
    let (input_tokens, output_tokens) = call.result.as_ref().map_or((0, 0), |c| (c.input_tokens, c.output_tokens));
    let draft = call.result.and_then(|c| c.draft);
    let closing = ledger::Closing { input_tokens, output_tokens, cost_s, ok: draft.is_ok() };
    ledger::close_run(db, run_id, &closing).await;
    transcribe::note_cap_levels(&state, &month).await;

    let draft = match draft {
        Ok(draft) => draft,
        Err(e) => {
            if is_key_broken(&e) {
                if state.summaries.pause() {
                    tracing::error!("key OpenAI không dùng được cho tóm tắt ({e}) — tạm ngưng tóm tắt tới khi khởi động lại");
                    audit::record(&state, None, "summary.key_invalid", doc! { "reason": &e });
                    notify::send(&state, format!("Tóm tắt tạm ngưng: key OpenAI của server không dùng được ({e}). Kiểm tra key rồi khởi động lại."));
                }
                return Err(AppError::Unavailable(PAUSED.into()));
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
    fn token_cost_converts_to_transcription_seconds() {
        // 1M token vào = $0,40 = 133,3 phút chép lời = 8000 giây; 1M token ra = $1,60 = 32000 giây.
        assert_eq!(cost_seconds(1_000_000, 0), 8_000);
        assert_eq!(cost_seconds(0, 1_000_000), 32_000);
        // Lượt thường (20k vào, 1,5k ra) = $0,0104 ⟹ 208 giây.
        assert_eq!(cost_seconds(20_000, 1_500), 208);
        assert_eq!(cost_seconds(1, 0), 1, "làm tròn lên");
        assert_eq!(cost_seconds(-5, 0), 0);
        // Ước lượng giữ chỗ không thấp hơn chi phí thật của một bản tiếng Việt thông thường.
        let chars = 60_000;
        assert!(cost_seconds(estimate_input_tokens(chars), MAX_OUTPUT_TOKENS) >= cost_seconds(chars as i64 / 3, 2_000));
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
    fn pause_reports_only_the_first_trip() {
        let s = Summaries::default();
        assert!(!s.paused());
        assert!(s.pause());
        assert!(!s.pause());
        assert!(s.paused());
    }

    #[test]
    fn daily_turn_is_refunded_only_when_openai_processed_nothing() {
        let completion = |draft: Result<llm::Draft, String>| llm::Call {
            result: Ok(llm::Completion { input_tokens: 20_000, output_tokens: 1_500, draft }),
            maybe_billed: true,
        };
        // Bị cắt / từ chối / sai dạng: đã tính tiền theo token, không trả lượt.
        assert_eq!(charge_of(&completion(Err("cắt".into())), 900), (208, false));
        let ok = llm::Draft { summary: "x".into(), decisions: vec![], actions: vec![] };
        assert_eq!(charge_of(&completion(Ok(ok)), 900), (208, false));
        // Có thể đã xử lý (quá giờ sau khi gửi): giữ ước lượng, không trả lượt.
        let maybe = llm::Call { result: Err("quá giờ".into()), maybe_billed: true };
        assert_eq!(charge_of(&maybe, 900), (900, false));
        // Chưa xử lý gì (không kết nối được, 401, 5xx hết lượt thử): không tính, trả lượt.
        let none = llm::Call { result: Err("401".into()), maybe_billed: false };
        assert_eq!(charge_of(&none, 900), (0, true));
    }

    #[test]
    fn monthly_limit_error_reopens_next_month() {
        let now: chrono::DateTime<chrono::Utc> = "2026-10-16T04:00:00Z".parse().expect("giờ");
        let AppError::Quota { code, retry_at, used_minutes, limit_minutes, .. } = monthly_limit_error(3_590, 3_600, now) else {
            panic!("phải là Quota");
        };
        assert_eq!((code, used_minutes, limit_minutes), ("SUMMARY_MONTHLY_LIMIT", 60, 60));
        assert_eq!(retry_at.as_deref(), Some("2026-10-31T17:00:00+00:00"));
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

    #[test]
    fn daily_limit_error_reopens_at_vietnam_midnight() {
        let now: chrono::DateTime<chrono::Utc> = "2026-10-16T04:00:00Z".parse().expect("giờ");
        let AppError::Quota { code, retry_at, used_minutes, limit_minutes, .. } = daily_limit_error(10, 10, now) else {
            panic!("phải là Quota");
        };
        assert_eq!((code, used_minutes, limit_minutes), ("SUMMARY_DAILY_LIMIT", 10, 10));
        assert_eq!(retry_at.as_deref(), Some("2026-10-16T17:00:00+00:00"));
    }
}
