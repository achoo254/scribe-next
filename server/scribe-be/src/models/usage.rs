//! Sổ phút chép lời bằng key riêng của người dùng theo tháng (giờ Việt Nam): chỉ để người dùng xem ("đã chép N phút
//! bằng key của bạn"), không chặn gì. Tiền là của người dùng (trả cho nhà cung cấp của họ), Scribe không giữ chỗ hay
//! quyết toán gì.
//!
//! Collection `transcribe_usage` còn các doc sổ cũ thời có key server (`u:`, `free:`, `all:`, `d:`, `el:` …): không còn
//! được đọc hay ghi. `transcribe_runs` là sổ chi tiết từng lượt (chỉ thêm), doc cũ có thêm trường của sổ server.

use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use chrono::{DateTime as ChronoDateTime, Datelike, FixedOffset, Utc};
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;

const COLL: &str = "transcribe_usage";
/// Sổ chi tiết từng lượt đã kết thúc (chỉ thêm).
pub const RUNS_COLL: &str = "transcribe_runs";

/// Giờ Việt Nam (+07:00, không đổi giờ mùa hè).
pub fn vietnam() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("offset +07:00 hợp lệ")
}

/// Tháng của thời điểm `now`, dạng `YYYY-MM` theo giờ Việt Nam.
pub fn month_key(now: ChronoDateTime<Utc>) -> String {
    let local = now.with_timezone(&vietnam());
    format!("{:04}-{:02}", local.year(), local.month())
}

/// Ngày của thời điểm `now`, dạng `YYYY-MM-DD` theo giờ Việt Nam.
pub fn day_key(now: ChronoDateTime<Utc>) -> String {
    now.with_timezone(&vietnam()).format("%Y-%m-%d").to_string()
}

/// Doc giây chép lời (theo tệp và trực tiếp) bằng key riêng của người dùng trong tháng.
fn byok_doc(user_id: &str, month: &str) -> String {
    format!("b:{user_id}:{month}")
}

/// Cộng giây chép bằng key riêng (sau khi lượt kết thúc).
pub async fn add_byok(db: &Database, user_id: &str, month: &str, seconds: i64) -> Result<(), AppError> {
    let seconds = seconds.max(0);
    if seconds == 0 {
        return Ok(());
    }
    let id = byok_doc(user_id, month);
    let res = db
        .collection::<Document>(COLL)
        .update_one(doc! { "_id": &id }, doc! { "$inc": { "seconds": seconds }, "$set": { "updatedAt": DateTime::now() } })
        .upsert(true)
        .await;
    match res {
        Ok(_) => Ok(()),
        // Hai lượt cùng tạo doc tháng: bên thua cộng lại vào doc vừa có.
        Err(e) if is_duplicate_key(&e) => db
            .collection::<Document>(COLL)
            .update_one(doc! { "_id": &id }, doc! { "$inc": { "seconds": seconds }, "$set": { "updatedAt": DateTime::now() } })
            .await
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("ghi sổ phút key riêng: {e}"))),
        Err(e) => Err(AppError::Internal(format!("ghi sổ phút key riêng: {e}"))),
    }
}

/// Giây chép bằng key riêng của người dùng trong tháng.
pub async fn byok_seconds(db: &Database, user_id: &str, month: &str) -> Result<i64, AppError> {
    let d = db
        .collection::<Document>(COLL)
        .find_one(doc! { "_id": byok_doc(user_id, month) })
        .await
        .map_err(|e| AppError::Internal(format!("đọc sổ phút key riêng: {e}")))?;
    Ok(d.and_then(|d| d.get_i64("seconds").ok().or_else(|| d.get_i32("seconds").ok().map(i64::from))).unwrap_or(0))
}

/// Phút để hiển thị: làm tròn lên (3 giây là 1 phút).
pub fn used_minutes(seconds: i64) -> i64 {
    (seconds.max(0) + 59) / 60
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> ChronoDateTime<Utc> {
        s.parse().expect("thời điểm")
    }

    #[test]
    fn days_and_months_follow_vietnam_time() {
        assert_eq!(day_key(at("2026-10-16T16:59:59Z")), "2026-10-16");
        assert_eq!(day_key(at("2026-10-16T17:00:00Z")), "2026-10-17");
        // 31/10 23:30 giờ VN = 16:30 UTC: vẫn tháng 10. 17:00 UTC = 1/11 00:00 giờ VN.
        assert_eq!(month_key(at("2026-10-31T16:30:00Z")), "2026-10");
        assert_eq!(month_key(at("2026-10-31T17:00:00Z")), "2026-11");
    }

    #[test]
    fn minutes_round_up() {
        assert_eq!((used_minutes(0), used_minutes(3), used_minutes(60), used_minutes(61), used_minutes(-5)), (0, 1, 1, 2, 0));
    }
}
