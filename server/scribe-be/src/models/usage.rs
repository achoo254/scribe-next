//! Sổ phút chép lời theo tháng (giờ Việt Nam): mỗi user một doc, cộng quỹ chung của gói free và trần
//! tổng. Tiền OpenAI là của chủ sản phẩm nên phút được **giữ chỗ** trước khi gửi audio, rồi
//! **quyết toán** theo số giây thật sự đã gửi khi lượt chép kết thúc.
//!
//! Mongo prod là standalone (không transaction): mỗi bước là một lệnh nguyên tử trên một doc; giữ chỗ
//! hỏng ở bước sau thì trả lại các bước trước.

use crate::config::TranscribeLimits;
use crate::error::AppError;
use crate::models::plan::Plan;
use crate::models::user::is_duplicate_key;
use chrono::{DateTime as ChronoDateTime, Datelike, FixedOffset, TimeZone, Utc};
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;

const COLL: &str = "transcribe_usage";
/// Sổ chi tiết từng lượt đã quyết toán (chỉ thêm) — nguồn để đối soát `COLL`.
pub const RUNS_COLL: &str = "transcribe_runs";

/// Giờ Việt Nam (+07:00, không đổi giờ mùa hè).
fn vietnam() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("offset +07:00 hợp lệ")
}

/// Tháng tính phút của thời điểm `now`, dạng `YYYY-MM` theo giờ Việt Nam.
pub fn month_key(now: ChronoDateTime<Utc>) -> String {
    let local = now.with_timezone(&vietnam());
    format!("{:04}-{:02}", local.year(), local.month())
}

/// Lúc bắt đầu tháng sau (giờ Việt Nam) — ngày quota đặt lại.
pub fn next_reset(now: ChronoDateTime<Utc>) -> ChronoDateTime<Utc> {
    let local = now.with_timezone(&vietnam());
    let (year, month) = if local.month() == 12 { (local.year() + 1, 1) } else { (local.year(), local.month() + 1) };
    vietnam()
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .expect("ngày 1 luôn hợp lệ")
        .with_timezone(&Utc)
}

fn user_doc(user_id: &str, month: &str) -> String {
    format!("u:{user_id}:{month}")
}
fn free_pool_doc(month: &str) -> String {
    format!("free:{month}")
}
fn total_doc(month: &str) -> String {
    format!("all:{month}")
}

/// Vì sao không giữ được chỗ.
#[derive(Debug, PartialEq)]
pub enum Blocked {
    /// Hết hạn mức gói của user.
    UserQuota,
    /// Quỹ chung của gói free đã hết trong tháng.
    FreePool,
    /// Trần chi phí tổng của tháng.
    TotalCap,
}

/// Một lần giữ chỗ: sổ nào bị trừ, để hoàn/quyết toán đúng chỗ.
#[derive(Debug, Clone, PartialEq)]
pub struct Charge {
    pub user_id: String,
    pub month: String,
    /// Lượt này có tính vào quỹ free không (user đang ở gói free lúc giữ chỗ).
    pub free_pool: bool,
}

async fn ensure(db: &Database, id: &str) -> Result<(), AppError> {
    let res = db
        .collection::<Document>(COLL)
        .update_one(doc! { "_id": id }, doc! { "$setOnInsert": { "seconds": 0_i64, "updatedAt": DateTime::now() } })
        .upsert(true)
        .await;
    match res {
        Ok(_) => Ok(()),
        // Hai request cùng tạo doc tháng: bên thua thấy doc đã có, đúng ý.
        Err(e) if is_duplicate_key(&e) => Ok(()),
        Err(e) => Err(AppError::Internal(format!("tạo sổ phút: {e}"))),
    }
}

/// Cộng `need` giây nếu sau khi cộng vẫn ≤ `limit`. Trả `false` khi không còn chỗ.
async fn take(db: &Database, id: &str, need: i64, limit: i64) -> Result<bool, AppError> {
    ensure(db, id).await?;
    let res = db
        .collection::<Document>(COLL)
        .update_one(
            doc! { "_id": id, "seconds": { "$lte": limit - need } },
            doc! { "$inc": { "seconds": need }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("giữ chỗ phút: {e}")))?;
    Ok(res.matched_count > 0)
}

/// Cộng (hoặc trừ, `delta < 0`) không điều kiện — quyết toán số giây thật.
async fn add(db: &Database, id: &str, delta: i64) -> Result<(), AppError> {
    if delta == 0 {
        return Ok(());
    }
    ensure(db, id).await?;
    db.collection::<Document>(COLL)
        .update_one(doc! { "_id": id }, doc! { "$inc": { "seconds": delta }, "$set": { "updatedAt": DateTime::now() } })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("quyết toán phút: {e}")))
}

/// Giữ `need` giây cho user theo thứ tự: hạn mức gói → quỹ free (chỉ gói free) → trần tổng. Một bước
/// hỏng thì trả lại các bước đã giữ. Tổng hai request đồng thời không bao giờ vượt giới hạn nào.
pub async fn reserve(
    db: &Database,
    user_id: &str,
    plan: Plan,
    month: &str,
    need: i64,
    limits: &TranscribeLimits,
) -> Result<Result<Charge, Blocked>, AppError> {
    let free_pool = plan == Plan::Free;
    let mut steps = vec![(user_doc(user_id, month), plan.limit_seconds(limits), Blocked::UserQuota)];
    if free_pool {
        steps.push((free_pool_doc(month), limits.free_pool_seconds, Blocked::FreePool));
    }
    steps.push((total_doc(month), limits.monthly_cap_seconds, Blocked::TotalCap));

    let mut taken: Vec<String> = Vec::new();
    for (id, limit, blocked) in steps {
        let outcome = take(db, &id, need, limit).await;
        if !matches!(outcome, Ok(true)) {
            // Bị chặn hoặc lỗi Mongo: trả lại mọi bước đã giữ (lỗi khi trả chỉ log, đối soát sửa sau).
            for done in &taken {
                if let Err(e) = add(db, done, -need).await {
                    tracing::error!(doc = %done, need, "trả lại phút đã giữ lỗi: {e:?}");
                }
            }
            return outcome.map(|_| Err(blocked));
        }
        taken.push(id);
    }
    Ok(Ok(Charge { user_id: user_id.to_string(), month: month.to_string(), free_pool }))
}

/// Điều chỉnh mọi sổ của một lần giữ chỗ (`delta` âm = hoàn, dương = thu thêm).
pub async fn settle(db: &Database, charge: &Charge, delta: i64) -> Result<(), AppError> {
    add(db, &user_doc(&charge.user_id, &charge.month), delta).await?;
    if charge.free_pool {
        add(db, &free_pool_doc(&charge.month), delta).await?;
    }
    add(db, &total_doc(&charge.month), delta).await
}

async fn seconds_of(db: &Database, id: &str) -> Result<i64, AppError> {
    let d = db
        .collection::<Document>(COLL)
        .find_one(doc! { "_id": id })
        .await
        .map_err(|e| AppError::Internal(format!("đọc sổ phút: {e}")))?;
    Ok(d.and_then(|d| d.get_i64("seconds").ok().or_else(|| d.get_i32("seconds").ok().map(i64::from))).unwrap_or(0))
}

/// Phút đã dùng để hiển thị: làm tròn lên (dùng 3 giây là đã dùng 1 phút).
pub fn used_minutes(seconds: i64) -> i64 {
    (seconds.max(0) + 59) / 60
}

/// Số giây đã dùng của user, quỹ free và tổng trong tháng.
pub struct MonthUsage {
    pub user_seconds: i64,
    pub free_pool_seconds: i64,
    pub total_seconds: i64,
}

pub async fn month_usage(db: &Database, user_id: &str, month: &str) -> Result<MonthUsage, AppError> {
    Ok(MonthUsage {
        user_seconds: seconds_of(db, &user_doc(user_id, month)).await?,
        free_pool_seconds: seconds_of(db, &free_pool_doc(month)).await?,
        total_seconds: seconds_of(db, &total_doc(month)).await?,
    })
}

/// Đánh dấu một mốc (vd "đã báo 80% trần tháng 10") — `true` chỉ ở lần đầu.
pub async fn mark_once(db: &Database, marker: &str) -> bool {
    db.collection::<Document>("transcribe_marks")
        .insert_one(doc! { "_id": marker, "at": DateTime::now() })
        .await
        .is_ok()
}

/// Mọi doc sổ của user trong tháng (để đối soát cả user không còn lượt nào).
pub async fn user_ids_with_ledger(db: &Database, month: &str) -> Result<Vec<String>, AppError> {
    use futures_util::StreamExt;
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "_id": { "$regex": format!("^u:.*:{}$", regex_month(month)) } })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("đọc sổ phút: {e}")))?;
    let mut out = Vec::new();
    while let Some(d) = cur.next().await {
        let d = d.map_err(|e| AppError::Internal(format!("đọc sổ phút: {e}")))?;
        if let Some(user) = d.get_str("_id").ok().and_then(|id| id.strip_prefix("u:")).and_then(|r| r.rsplit_once(':')) {
            out.push(user.0.to_string());
        }
    }
    Ok(out)
}

/// Tháng chỉ gồm chữ số và `-`; mọi ký tự khác bị bỏ trước khi đưa vào regex.
fn regex_month(month: &str) -> String {
    month.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect()
}

/// Cộng chênh lệch vào sổ (đối soát): `$inc` để không xoá thay đổi chạy song song.
pub async fn adjust(db: &Database, id: &str, delta: i64) -> Result<(), AppError> {
    add(db, id, delta).await
}

pub fn doc_ids(user_id: Option<&str>, month: &str) -> (Option<String>, String, String) {
    (user_id.map(|u| user_doc(u, month)), free_pool_doc(month), total_doc(month))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> ChronoDateTime<Utc> {
        s.parse().expect("thời điểm")
    }

    #[test]
    fn months_follow_vietnam_time() {
        // 31/10 23:30 giờ VN = 16:30 UTC: vẫn tháng 10. 17:00 UTC = 1/11 00:00 giờ VN.
        assert_eq!(month_key(at("2026-10-31T16:30:00Z")), "2026-10");
        assert_eq!(month_key(at("2026-10-31T17:00:00Z")), "2026-11");
        assert_eq!(next_reset(at("2026-10-16T04:00:00Z")), at("2026-10-31T17:00:00Z"));
        assert_eq!(next_reset(at("2026-12-20T00:00:00Z")), at("2026-12-31T17:00:00Z"));
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// usage_ledger -- --ignored`. Dùng tháng giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn usage_ledger_never_exceeds_limits_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let month = format!("test-{}", uuid::Uuid::new_v4().simple());
        let limits =
            TranscribeLimits { free_seconds: 100, plus_seconds: 1_000, free_pool_seconds: 150, monthly_cap_seconds: 400 };
        let (alice, bob, carol) = ("alice", "bob", "carol");

        // 20 lượt song song, mỗi lượt 10 giây, user free hạn mức 100 ⟹ đúng 10 lượt được giữ.
        let tasks = (0..20).map(|_| reserve(&db, alice, Plan::Free, &month, 10, &limits));
        let results = futures_util::future::join_all(tasks).await;
        let ok = results.iter().filter(|r| matches!(r, Ok(Ok(_)))).count();
        assert_eq!(ok, 10);
        let usage = month_usage(&db, alice, &month).await.expect("đọc");
        assert_eq!((usage.user_seconds, usage.free_pool_seconds, usage.total_seconds), (100, 100, 100));

        // Quỹ free còn 50: bob (free) giữ 60 bị chặn bởi quỹ, sổ của bob trả về 0.
        assert_eq!(reserve(&db, bob, Plan::Free, &month, 60, &limits).await.expect("bob"), Err(Blocked::FreePool));
        assert_eq!(month_usage(&db, bob, &month).await.expect("đọc").user_seconds, 0);
        // carol (plus) không bị quỹ free chặn, chỉ bị trần tổng (400 − 100 = 300).
        let carol_charge = reserve(&db, carol, Plan::Plus, &month, 300, &limits).await.expect("carol").expect("giữ");
        assert!(!carol_charge.free_pool);
        assert_eq!(reserve(&db, carol, Plan::Plus, &month, 1, &limits).await.expect("carol"), Err(Blocked::TotalCap));
        assert_eq!(month_usage(&db, carol, &month).await.expect("đọc").user_seconds, 300);

        // Quyết toán: carol chỉ gửi 250 giây ⟹ hoàn 50 ở sổ của carol và trần tổng.
        settle(&db, &carol_charge, -50).await.expect("hoàn");
        let usage = month_usage(&db, carol, &month).await.expect("đọc");
        assert_eq!((usage.user_seconds, usage.total_seconds), (250, 350));

        db.collection::<Document>(COLL)
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
    }
}
