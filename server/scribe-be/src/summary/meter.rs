//! Nhật ký chung cho các lượt gọi model chữ (tóm tắt, hỏi đáp, dịch): mở lượt trong nhật ký (`summary_runs` /
//! `ask_runs` / `translate_runs`) trước khi gọi, đóng với số token thật sau khi gọi. Mọi lượt chạy bằng key riêng của
//! người dùng (tiền của họ): không có lượt ngày, trần tháng hay trần tổng nào để giữ chỗ hay quyết toán.
//!
//! Một lượt dịch gọi model nhiều lần (mỗi lô một lần) dưới cùng một lượt nhật ký: người gọi cộng dồn token các lần gọi
//! rồi đóng một lần bằng [`settle_outcome`].

use super::llm;
use crate::config::Vendor;
use crate::error::AppError;
use crate::models::summary::{self as ledger, Feature};
use crate::models::usage;
use crate::state::AppState;
use mongodb::bson::oid::ObjectId;

/// Lượt đã mở trong nhật ký, đủ để đóng sau khi gọi model.
pub(crate) struct Reservation {
    feature: Feature,
    run_id: ObjectId,
}

/// Mở lượt bằng key riêng `vendor`, model `model`. Lỗi ghi nhật ký thì không gọi model.
pub(crate) async fn open(
    state: &AppState,
    feature: Feature,
    user_id: &str,
    key: Option<&str>,
    vendor: Vendor,
    model: &str,
) -> Result<Reservation, AppError> {
    let now = chrono::Utc::now();
    let (day, month) = (usage::day_key(now), usage::month_key(now));
    let opened = ledger::Run { user_id, key, model, month: &month, day: &day, byok: vendor.code() };
    let run_id = ledger::open_run(&state.db, feature, &opened).await?;
    Ok(Reservation { feature, run_id })
}

/// Kết quả của một lượt (một hay nhiều lần gọi model).
pub(crate) struct Outcome {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub ok: bool,
}

/// Đóng lượt theo kết quả một lần gọi model.
pub(crate) async fn settle<T>(state: &AppState, r: &Reservation, call: &llm::Call<T>) {
    let (input_tokens, output_tokens) = call.tokens();
    let ok = call.result.as_ref().is_ok_and(|c| c.draft.is_ok());
    settle_outcome(state, r, &Outcome { input_tokens, output_tokens, ok }).await
}

/// Đóng lượt với kết quả đã cộng dồn (lượt gọi model nhiều lần).
pub(crate) async fn settle_outcome(state: &AppState, r: &Reservation, outcome: &Outcome) {
    let closing = ledger::Closing { input_tokens: outcome.input_tokens, output_tokens: outcome.output_tokens, ok: outcome.ok };
    ledger::close_run(&state.db, r.feature, r.run_id, &closing).await;
}
