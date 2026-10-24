//! Đo lường chung cho các lượt gọi model chữ (tóm tắt, hỏi đáp, dịch), theo đúng một trình tự:
//!
//! 1. lấy một lượt ngày của tính năng (sổ ngày riêng mỗi tính năng);
//! 2. giữ chỗ chi phí ước lượng trên sổ tháng của tài khoản (dùng chung giữa các tính năng);
//! 3. giữ chỗ trên trần tổng tháng của chép lời;
//! 4. mở lượt trong nhật ký (`summary_runs` / `ask_runs` / `translate_runs`) trước khi gọi;
//! 5. sau khi gọi: quyết toán theo token thật, đóng lượt, báo mốc trần.
//!
//! Một lượt dịch gọi model nhiều lần (mỗi lô một lần) dưới cùng một phần giữ chỗ: người gọi cộng dồn kết quả
//! các lần gọi rồi quyết toán một lần bằng [`settle_outcome`].
//!
//! Lượt ngày chỉ được trả lại khi OpenAI chắc chắn chưa xử lý gì ([`charge_of`]). Người gọi phải chạy cả
//! [`reserve`] lẫn [`settle`] bên trong task riêng (`tokio::spawn`): client ngắt kết nối không được để
//! phần giữ chỗ treo.
//!
//! Lượt chạy bằng key riêng của người dùng ([`reserve_own`]) không qua bước 1–3 và không quyết toán sổ nào: chỉ mở và
//! đóng lượt trong nhật ký (đánh dấu `byok`) để người dùng và quản trị biết token đã dùng. Lệnh đối soát bỏ qua các lượt
//! này.

use super::llm;
use crate::audit;
use crate::config::Vendor;
use crate::error::AppError;
use crate::models::summary::{self as ledger, Feature};
use crate::models::{plan, usage};
use crate::state::AppState;
use crate::transcribe;
use mongodb::bson::{doc, oid::ObjectId};

/// Token của phần hướng dẫn và schema, cộng vào ước lượng đầu vào.
const PROMPT_OVERHEAD_TOKENS: i64 = 800;

/// Chi phí token của model chữ của server (`LLM_PROVIDER`/`LLM_MODEL`, giá ở `llm::MODELS`) quy ra giây chép lời
/// tương đương (giá chép lời của nhà cung cấp đang dùng mỗi phút), làm tròn lên.
pub fn cost_seconds(input_tokens: i64, output_tokens: i64) -> i64 {
    let (vendor, model) = crate::config::llm_from_build();
    cost_seconds_at(llm::price_of(vendor, &model), input_tokens, output_tokens, crate::config::usd_per_minute())
}

/// Phần giữ chỗ (giây tương đương) cho một lượt của model chữ của server: `input_tokens` ước lượng và trần token ra
/// `max_output_tokens`, cộng phần suy nghĩ nhà cung cấp có thể tính thêm (`llm::output_cap`, Gemini): luôn là cận trên của
/// chi phí thật.
pub fn estimate_seconds(input_tokens: i64, max_output_tokens: i64) -> i64 {
    let (vendor, model) = crate::config::llm_from_build();
    let cap = llm::output_cap(vendor, max_output_tokens);
    cost_seconds_at(llm::price_of(vendor, &model), input_tokens, cap, crate::config::usd_per_minute())
}

/// Như [`cost_seconds`] với giá model `price` và giá chép lời `usd_per_minute` cho trước.
fn cost_seconds_at(price: llm::Price, input_tokens: i64, output_tokens: i64, usd_per_minute: f64) -> i64 {
    let cost_e8 = price.cost_e8(input_tokens, output_tokens);
    // Giá một giây chép lời theo cùng đơn vị: OpenAI $0,003 / 60 = 5000 × 1e-8 USD; ElevenLabs $0,22 / 3600 ≈ 6111.
    let second_e8 = ((usd_per_minute / 60.0) * 1e8).round() as i64;
    (cost_e8 + second_e8 - 1) / second_e8
}

/// Ước lượng token đầu vào cho phần giữ chỗ: chữ Việt ~2–3 ký tự mỗi token, lấy 2 cho chắc phần trên.
pub fn estimate_input_tokens(chars: usize) -> i64 {
    chars as i64 / 2 + PROMPT_OVERHEAD_TOKENS
}

/// Chi phí ghi sổ của một lượt gọi model (giây tương đương) và có trả lại lượt ngày không. OpenAI đã
/// xử lý thì tính theo token thật, không trả lượt (kể cả khi kết quả bị cắt, bị từ chối hay sai dạng);
/// có thể đã xử lý mà không biết token thì giữ nguyên phần ước lượng; chắc chắn chưa xử lý thì không
/// tính gì và trả lượt.
pub fn charge_of<T>(call: &llm::Call<T>, estimate: i64) -> (i64, bool) {
    match &call.result {
        Ok(c) => (cost_seconds(c.input_tokens, c.output_tokens), false),
        Err(_) if call.maybe_billed => (estimate, false),
        Err(_) => (0, true),
    }
}

/// Tên tính năng trong câu báo cho người dùng.
fn label(feature: Feature) -> &'static str {
    match feature {
        Feature::Summary => "tóm tắt",
        Feature::Ask => "hỏi đáp",
        Feature::Translate => "dịch",
    }
}

pub(crate) fn daily_limit_error(feature: Feature, used: i64, limit: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    let (code, unit) = match feature {
        Feature::Summary => ("SUMMARY_DAILY_LIMIT", "lượt tóm tắt"),
        Feature::Ask => ("ASK_DAILY_LIMIT", "lượt hỏi"),
        Feature::Translate => ("TRANSLATE_DAILY_LIMIT", "lượt dịch"),
    };
    AppError::Quota {
        code,
        message: format!("Hôm nay đã dùng {used}/{limit} {unit}. Mở lại lúc 0 giờ."),
        used_minutes: used,
        limit_minutes: limit,
        retry_at: Some(usage::next_day(now).to_rfc3339()),
    }
}

pub(crate) fn monthly_limit_error(feature: Feature, used_s: i64, limit_s: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    let code = match feature {
        Feature::Summary => "SUMMARY_MONTHLY_LIMIT",
        Feature::Ask => "ASK_MONTHLY_LIMIT",
        Feature::Translate => "TRANSLATE_MONTHLY_LIMIT",
    };
    AppError::Quota {
        code,
        message: "Tháng này tài khoản đã dùng hết hạn mức tóm tắt, hỏi đáp và dịch, sẽ mở lại đầu tháng sau.".into(),
        used_minutes: usage::used_minutes(used_s),
        limit_minutes: limit_s / 60,
        retry_at: Some(usage::next_reset(now).to_rfc3339()),
    }
}

fn cap_error(feature: Feature, now: chrono::DateTime<chrono::Utc>) -> AppError {
    let what = match feature {
        Feature::Summary => "Tóm tắt",
        Feature::Ask => "Hỏi đáp",
        Feature::Translate => "Dịch",
    };
    AppError::Quota {
        code: "CAP_REACHED",
        message: format!("{what} đã dùng hết hạn mức chung của tháng này, sẽ mở lại đầu tháng sau."),
        used_minutes: 0,
        limit_minutes: 0,
        retry_at: Some(usage::next_reset(now).to_rfc3339()),
    }
}

/// Lượt ngày và trần chi phí tháng (giây tương đương) của một tài khoản cho một tính năng.
fn limits_of(state: &AppState, feature: Feature, plan: plan::Plan) -> (i64, i64) {
    let summary = state.config.summary_limits;
    let (ask, translate) = (state.config.ask_limits, state.config.translate_limits);
    let free = plan == plan::Plan::Free;
    let daily = match (feature, free) {
        (Feature::Summary, true) => summary.free_daily,
        (Feature::Summary, false) => summary.paid_daily,
        (Feature::Ask, true) => ask.free_daily,
        (Feature::Ask, false) => ask.paid_daily,
        (Feature::Translate, true) => translate.free_daily,
        (Feature::Translate, false) => translate.paid_daily,
    };
    let monthly = if free { summary.free_monthly_seconds } else { summary.paid_monthly_seconds };
    (daily, monthly)
}

/// Phần đã giữ của một lượt, đủ để quyết toán sau khi gọi model.
pub(crate) struct Reservation {
    feature: Feature,
    /// Lượt chạy bằng key riêng của người dùng (nhà cung cấp): không có sổ nào để quyết toán.
    byok: Option<Vendor>,
    user_id: String,
    day: String,
    month: String,
    pub estimate: i64,
    run_id: ObjectId,
}

/// Trả lại phần đã giữ (lượt ngày, sổ tháng, và trần tổng khi `total`) khi lượt dừng trước lúc gọi model.
async fn undo(db: &mongodb::Database, feature: Feature, user_id: &str, day: &str, month: &str, estimate: i64, total: bool) {
    ledger::refund_daily(db, feature, user_id, day).await;
    if let Err(e) = ledger::settle_monthly(db, user_id, month, -estimate).await {
        tracing::error!(estimate, "trả chi phí tháng của {} lỗi: {e:?}", label(feature));
    }
    if total {
        if let Err(e) = usage::settle_total(db, month, -estimate).await {
            tracing::error!(estimate, "trả trần tổng của {} lỗi: {e:?}", label(feature));
        }
    }
}

/// Bước 1–4: lấy lượt ngày, giữ chỗ `estimate` giây trên sổ tháng và trần tổng, mở lượt. Lỗi ở bước nào
/// thì trả lại phần đã giữ trước đó.
pub(crate) async fn reserve(
    state: &AppState,
    feature: Feature,
    user_id: &str,
    key: Option<&str>,
    plan: plan::Plan,
    estimate: i64,
) -> Result<Reservation, AppError> {
    let db = &state.db;
    let now = chrono::Utc::now();
    let (day, month) = (usage::day_key(now), usage::month_key(now));
    let (daily, monthly) = limits_of(state, feature, plan);
    let blocked = format!("{}.blocked", feature.name());
    if !ledger::take_daily(db, feature, user_id, &day, daily).await? {
        let used = ledger::used_daily(db, feature, user_id, &day).await?;
        audit::record(state, Some(user_id), &blocked, doc! { "reason": "DailyLimit" });
        return Err(daily_limit_error(feature, used, daily, now));
    }
    match ledger::reserve_monthly(db, user_id, &month, estimate, monthly).await {
        Ok(true) => {}
        Ok(false) => {
            ledger::refund_daily(db, feature, user_id, &day).await;
            let used = ledger::used_monthly(db, user_id, &month).await?;
            audit::record(state, Some(user_id), &blocked, doc! { "reason": "MonthlyLimit" });
            return Err(monthly_limit_error(feature, used, monthly, now));
        }
        Err(e) => {
            ledger::refund_daily(db, feature, user_id, &day).await;
            return Err(e);
        }
    }
    match usage::reserve_total(db, &month, estimate, &state.config.transcribe_limits).await {
        Ok(true) => {}
        Ok(false) => {
            undo(db, feature, user_id, &day, &month, estimate, false).await;
            audit::record(state, Some(user_id), &blocked, doc! { "reason": "TotalCap" });
            return Err(cap_error(feature, now));
        }
        Err(e) => {
            undo(db, feature, user_id, &day, &month, estimate, false).await;
            return Err(e);
        }
    }
    let model = state.config.summary_model.as_str();
    let opened = ledger::Run { user_id, key, model, month: &month, day: &day, reserved_s: estimate, byok: None };
    match ledger::open_run(db, feature, &opened).await {
        Ok(run_id) => Ok(Reservation { feature, byok: None, user_id: user_id.to_string(), day, month, estimate, run_id }),
        Err(e) => {
            undo(db, feature, user_id, &day, &month, estimate, true).await;
            Err(e)
        }
    }
}

/// Lượt bằng key riêng của người dùng (`vendor`, `model`): không lượt ngày, không trần tháng, không trần tổng; chỉ mở
/// lượt trong nhật ký. Lỗi ghi nhật ký thì không gọi model (như lượt bằng key server).
pub(crate) async fn reserve_own(
    state: &AppState,
    feature: Feature,
    user_id: &str,
    key: Option<&str>,
    vendor: Vendor,
    model: &str,
) -> Result<Reservation, AppError> {
    let now = chrono::Utc::now();
    let (day, month) = (usage::day_key(now), usage::month_key(now));
    let opened = ledger::Run { user_id, key, model, month: &month, day: &day, reserved_s: 0, byok: Some(vendor.code()) };
    let run_id = ledger::open_run(&state.db, feature, &opened).await?;
    Ok(Reservation { feature, byok: Some(vendor), user_id: user_id.to_string(), day, month, estimate: 0, run_id })
}

/// Kết quả ghi sổ của một lượt (một hay nhiều lần gọi model).
pub(crate) struct Outcome {
    /// Chi phí ghi sổ (giây tương đương).
    pub cost_s: i64,
    /// Trả lại lượt ngày: OpenAI chắc chắn chưa xử lý lần gọi nào.
    pub refund: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub ok: bool,
}

/// Bước 5: quyết toán phần đã giữ theo kết quả gọi model. Trả chi phí đã ghi sổ (giây tương đương).
pub(crate) async fn settle<T>(state: &AppState, r: &Reservation, call: &llm::Call<T>) -> i64 {
    let (cost_s, refund) = charge_of(call, r.estimate);
    let (input_tokens, output_tokens) = call.tokens();
    let ok = call.result.as_ref().is_ok_and(|c| c.draft.is_ok());
    settle_outcome(state, r, &Outcome { cost_s, refund, input_tokens, output_tokens, ok }).await
}

/// Bước 5 với kết quả đã cộng dồn (lượt gọi model nhiều lần). Trả chi phí đã ghi sổ.
pub(crate) async fn settle_outcome(state: &AppState, r: &Reservation, outcome: &Outcome) -> i64 {
    let db = &state.db;
    let Outcome { cost_s, refund, input_tokens, output_tokens, ok } = *outcome;
    if r.byok.is_some() {
        // Key của người dùng: tiền của họ, không sổ nào của Scribe đổi. Chỉ ghi token.
        let closing = ledger::Closing { input_tokens, output_tokens, cost_s: 0, ok };
        ledger::close_run(db, r.feature, r.run_id, &closing).await;
        return 0;
    }
    if let Err(e) = usage::settle_total(db, &r.month, cost_s - r.estimate).await {
        tracing::error!(cost_s, estimate = r.estimate, "quyết toán trần cho {} lỗi: {e:?}", label(r.feature));
    }
    if let Err(e) = ledger::settle_monthly(db, &r.user_id, &r.month, cost_s - r.estimate).await {
        tracing::error!(cost_s, estimate = r.estimate, "quyết toán chi phí tháng của {} lỗi: {e:?}", label(r.feature));
    }
    if refund {
        ledger::refund_daily(db, r.feature, &r.user_id, &r.day).await;
    }
    let closing = ledger::Closing { input_tokens, output_tokens, cost_s, ok };
    ledger::close_run(db, r.feature, r.run_id, &closing).await;
    transcribe::note_cap_levels(state, &r.month).await;
    cost_s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_cost_converts_to_transcription_seconds() {
        // Giá OpenAI ($0,003/phút): 1M token vào = $0,40 = 133,3 phút = 8000 giây; 1M token ra = $1,60 = 32000 giây.
        let openai = crate::config::Provider::OpenAi.usd_per_minute();
        let mini = llm::price_of(Vendor::OpenAi, "gpt-4.1-mini");
        assert_eq!(cost_seconds_at(mini, 1_000_000, 0, openai), 8_000);
        assert_eq!(cost_seconds_at(mini, 0, 1_000_000, openai), 32_000);
        // Lượt thường (20k vào, 1,5k ra) = $0,0104 ⟹ 208 giây OpenAI, 171 giây ElevenLabs ($0,22/giờ).
        assert_eq!(cost_seconds_at(mini, 20_000, 1_500, openai), 208);
        assert_eq!(cost_seconds_at(mini, 20_000, 1_500, crate::config::Provider::ElevenLabs.usd_per_minute()), 171);
        // Gemini: phần giữ chỗ cộng cả phần suy nghĩ tối đa, nên luôn ≥ chi phí của lượt dùng hết trần token ra.
        let flash = llm::price_of(Vendor::Gemini, "gemini-3.8-flash");
        let reserved = cost_seconds_at(flash, 20_000, llm::output_cap(Vendor::Gemini, 1_500), openai);
        let worst = cost_seconds_at(flash, 20_000, 1_500 + llm::GEMINI_THINKING_HEADROOM, openai);
        assert!(reserved >= worst && reserved > cost_seconds_at(flash, 20_000, 1_500, openai));
        // Model khác của server theo bảng giá: Claude Haiku 5.5 ($0,10 / $0,50) rẻ bằng một phần tư.
        let haiku = llm::price_of(Vendor::Anthropic, "claude-haiku-5-5");
        assert_eq!(cost_seconds_at(haiku, 20_000, 1_500, openai), 55);
        assert_eq!(cost_seconds(1, 0), 1, "làm tròn lên");
        assert_eq!(cost_seconds(-5, 0), 0);
        // Ước lượng giữ chỗ không thấp hơn chi phí thật của một bản tiếng Việt thông thường.
        let chars = 60_000;
        assert!(cost_seconds(estimate_input_tokens(chars), super::super::MAX_OUTPUT_TOKENS) >= cost_seconds(chars as i64 / 3, 2_000));
    }

    #[test]
    fn daily_turn_is_refunded_only_when_openai_processed_nothing() {
        let completion = |draft: Result<llm::Draft, String>| llm::Call {
            result: Ok(llm::Completion { input_tokens: 20_000, output_tokens: 1_500, draft }),
            maybe_billed: true,
        };
        // Bị cắt / từ chối / sai dạng: đã tính tiền theo token, không trả lượt.
        assert_eq!(charge_of(&completion(Err("cắt".into())), 900), (cost_seconds(20_000, 1_500), false));
        let ok = llm::Draft { summary: "x".into(), decisions: vec![], actions: vec![] };
        assert_eq!(charge_of(&completion(Ok(ok)), 900), (cost_seconds(20_000, 1_500), false));
        // Có thể đã xử lý (quá giờ sau khi gửi): giữ ước lượng, không trả lượt.
        let maybe: llm::Call<llm::Draft> = llm::Call { result: Err("quá giờ".into()), maybe_billed: true };
        assert_eq!(charge_of(&maybe, 900), (900, false));
        // Chưa xử lý gì (không kết nối được, 401, 5xx hết lượt thử): không tính, trả lượt.
        let none: llm::Call<llm::Draft> = llm::Call { result: Err("401".into()), maybe_billed: false };
        assert_eq!(charge_of(&none, 900), (0, true));
    }

    #[test]
    fn limit_errors_carry_the_feature_code_and_reopen_time() {
        let now: chrono::DateTime<chrono::Utc> = "2026-10-16T04:00:00Z".parse().expect("giờ");
        let AppError::Quota { code, retry_at, used_minutes, limit_minutes, .. } =
            monthly_limit_error(Feature::Summary, 3_590, 3_600, now)
        else {
            panic!("phải là Quota");
        };
        assert_eq!((code, used_minutes, limit_minutes), ("SUMMARY_MONTHLY_LIMIT", 60, 60));
        assert_eq!(retry_at.as_deref(), Some("2026-10-31T17:00:00+00:00"));
        let AppError::Quota { code, retry_at, used_minutes, limit_minutes, .. } = daily_limit_error(Feature::Summary, 10, 10, now) else {
            panic!("phải là Quota");
        };
        assert_eq!((code, used_minutes, limit_minutes), ("SUMMARY_DAILY_LIMIT", 10, 10));
        assert_eq!(retry_at.as_deref(), Some("2026-10-16T17:00:00+00:00"));
        let AppError::Quota { code, message, .. } = daily_limit_error(Feature::Ask, 30, 30, now) else {
            panic!("phải là Quota");
        };
        assert_eq!(code, "ASK_DAILY_LIMIT");
        assert!(message.contains("30/30 lượt hỏi"));
        let AppError::Quota { code, .. } = monthly_limit_error(Feature::Ask, 1, 2, now) else { panic!("phải là Quota") };
        assert_eq!(code, "ASK_MONTHLY_LIMIT");
        let AppError::Quota { code, message, .. } = daily_limit_error(Feature::Translate, 3, 3, now) else {
            panic!("phải là Quota");
        };
        assert_eq!(code, "TRANSLATE_DAILY_LIMIT");
        assert!(message.contains("3/3 lượt dịch"));
        let AppError::Quota { code, .. } = monthly_limit_error(Feature::Translate, 1, 2, now) else { panic!("phải là Quota") };
        assert_eq!(code, "TRANSLATE_MONTHLY_LIMIT");
        let AppError::Quota { code, .. } = cap_error(Feature::Translate, now) else { panic!("phải là Quota") };
        assert_eq!(code, "CAP_REACHED");
    }
}
