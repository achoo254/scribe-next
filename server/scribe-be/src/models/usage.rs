//! Sổ phút chép lời theo tháng (giờ Việt Nam): mỗi user một doc, cộng quỹ chung của gói free và trần
//! tổng. Tiền OpenAI là của chủ sản phẩm nên phút được **giữ chỗ** trước khi gửi audio, rồi
//! **quyết toán** theo số giây thật sự đã gửi khi lượt chép kết thúc.
//!
//! Gói free còn hai sổ theo ngày (giờ Việt Nam) chống một người mở nhiều tài khoản: mỗi tài khoản, và
//! mọi tài khoản cùng IP (IPv6 gộp theo /64).
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
pub fn vietnam() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("offset +07:00 hợp lệ")
}

/// Tháng tính phút của thời điểm `now`, dạng `YYYY-MM` theo giờ Việt Nam.
pub fn month_key(now: ChronoDateTime<Utc>) -> String {
    let local = now.with_timezone(&vietnam());
    format!("{:04}-{:02}", local.year(), local.month())
}

/// Ngày tính phút của thời điểm `now`, dạng `YYYY-MM-DD` theo giờ Việt Nam.
pub fn day_key(now: ChronoDateTime<Utc>) -> String {
    now.with_timezone(&vietnam()).format("%Y-%m-%d").to_string()
}

/// Lúc bắt đầu ngày mai (giờ Việt Nam) — giới hạn ngày đặt lại.
pub fn next_day(now: ChronoDateTime<Utc>) -> ChronoDateTime<Utc> {
    let tomorrow = now.with_timezone(&vietnam()).date_naive() + chrono::Days::new(1);
    vietnam()
        .from_local_datetime(&tomorrow.and_hms_opt(0, 0, 0).expect("nửa đêm hợp lệ"))
        .single()
        .expect("+07:00 không có giờ trùng")
        .with_timezone(&Utc)
}

/// Khoá sổ IP: IPv4 nguyên địa chỉ, IPv6 theo /64 (một máy thường có cả dải). IP không đọc được
/// (`unknown`) thì không có sổ IP — chỉ xảy ra khi request không qua nginx.
pub fn ip_bucket(ip: &str) -> Option<String> {
    match ip.trim().parse::<std::net::IpAddr>().ok()? {
        std::net::IpAddr::V4(v4) => Some(v4.to_string()),
        std::net::IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => Some(v4.to_string()),
            None => {
                let s = v6.segments();
                Some(format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3]))
            }
        },
    }
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
fn day_doc(user_id: &str, day: &str) -> String {
    format!("d:{user_id}:{day}")
}
fn ip_doc(ip: &str, day: &str) -> String {
    format!("ip:{ip}:{day}")
}

/// Vì sao không giữ được chỗ.
#[derive(Debug, PartialEq)]
pub enum Blocked {
    /// Hết hạn mức gói của user.
    UserQuota,
    /// Tài khoản free đã dùng hết phút của hôm nay.
    DailyLimit,
    /// Các tài khoản free cùng IP đã dùng hết phút của hôm nay.
    IpLimit,
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
    /// Gói free: ngày của sổ ngày (`YYYY-MM-DD`).
    pub day: Option<String>,
    /// Gói free: khoá sổ IP (`ip_bucket`), không có khi không biết IP.
    pub ip: Option<String>,
}

impl Charge {
    /// Các sổ một lượt chép của `user_id` tính vào. Sổ ngày và IP chỉ áp cho gói free.
    pub fn new(user_id: &str, plan: Plan, now: ChronoDateTime<Utc>, ip: Option<String>) -> Charge {
        let free = plan == Plan::Free;
        Charge {
            user_id: user_id.to_string(),
            month: month_key(now),
            free_pool: free,
            day: free.then(|| day_key(now)),
            ip: if free { ip } else { None },
        }
    }

    /// Mọi sổ của lượt, mỗi sổ kèm lý do chặn khi nó đầy, theo thứ tự giữ chỗ: tháng của user → ngày của
    /// user → ngày của IP → quỹ free → trần tổng.
    fn books(&self) -> Vec<(String, Blocked)> {
        let mut books = vec![(user_doc(&self.user_id, &self.month), Blocked::UserQuota)];
        if let Some(day) = &self.day {
            books.push((day_doc(&self.user_id, day), Blocked::DailyLimit));
            if let Some(ip) = &self.ip {
                books.push((ip_doc(ip, day), Blocked::IpLimit));
            }
        }
        if self.free_pool {
            books.push((free_pool_doc(&self.month), Blocked::FreePool));
        }
        books.push((total_doc(&self.month), Blocked::TotalCap));
        books
    }
}

/// Giới hạn của sổ chặn bằng `blocked`; `user_limit` là hạn mức tháng của gói.
pub fn limit_of(blocked: &Blocked, user_limit: i64, limits: &TranscribeLimits) -> i64 {
    match blocked {
        Blocked::UserQuota => user_limit,
        Blocked::DailyLimit => limits.free_daily_seconds,
        Blocked::IpLimit => limits.free_ip_daily_seconds,
        Blocked::FreePool => limits.free_pool_seconds,
        Blocked::TotalCap => limits.monthly_cap_seconds,
    }
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

/// Giữ `need` giây trên mọi sổ của `charge` (`Charge::books`); `user_limit` là hạn mức tháng của gói.
/// Một bước hỏng thì trả lại các bước đã giữ. Tổng hai request đồng thời không bao giờ vượt giới hạn nào.
pub async fn reserve(
    db: &Database,
    charge: &Charge,
    user_limit: i64,
    need: i64,
    limits: &TranscribeLimits,
) -> Result<Result<(), Blocked>, AppError> {
    let mut taken: Vec<String> = Vec::new();
    for (id, blocked) in charge.books() {
        let outcome = take(db, &id, need, limit_of(&blocked, user_limit, limits)).await;
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
    Ok(Ok(()))
}

/// Điều chỉnh mọi sổ của một lần giữ chỗ (`delta` âm = hoàn, dương = thu thêm).
pub async fn settle(db: &Database, charge: &Charge, delta: i64) -> Result<(), AppError> {
    // Lượt đã đánh dấu quyết toán thì không thử lại: một sổ lỗi không được làm các sổ sau bỏ lỡ.
    let mut first_err = None;
    for (id, _) in charge.books() {
        if let Err(e) = add(db, &id, delta).await {
            tracing::error!(doc = %id, delta, "quyết toán sổ lỗi: {e:?}");
            first_err.get_or_insert(e);
        }
    }
    first_err.map_or(Ok(()), Err)
}

/// Giây đã dùng trên sổ đã chặn lượt `charge` (để báo số cho người dùng).
pub async fn blocked_usage(db: &Database, charge: &Charge, blocked: &Blocked) -> Result<i64, AppError> {
    match charge.books().into_iter().find(|(_, b)| b == blocked) {
        Some((id, _)) => seconds_of(db, &id).await,
        None => Ok(0),
    }
}

/// Đếm số lần một khoá bị chặn trong ngày (vd `daily:<userId>:<ngày>`); trả số lần sau khi cộng.
pub async fn count_hit(db: &Database, key: &str) -> i64 {
    let res = db
        .collection::<Document>("transcribe_marks")
        .find_one_and_update(doc! { "_id": format!("hit:{key}") }, doc! { "$inc": { "count": 1_i64 }, "$set": { "at": DateTime::now() } })
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .await;
    match res {
        Ok(Some(d)) => d.get_i64("count").or_else(|_| d.get_i32("count").map(i64::from)).unwrap_or(0),
        Ok(None) => 0,
        Err(e) => {
            tracing::warn!(key, "đếm lần chặn lỗi: {e}");
            0
        }
    }
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
    fn days_follow_vietnam_time_and_ips_group_by_network() {
        assert_eq!(day_key(at("2026-10-16T16:59:59Z")), "2026-10-16");
        assert_eq!(day_key(at("2026-10-16T17:00:00Z")), "2026-10-17");
        assert_eq!(next_day(at("2026-10-16T04:00:00Z")), at("2026-10-16T17:00:00Z"));
        assert_eq!(ip_bucket("203.0.113.9").as_deref(), Some("203.0.113.9"));
        assert_eq!(ip_bucket("2001:db8:1:2:aaaa::1").as_deref(), Some("2001:db8:1:2::/64"));
        assert_eq!(ip_bucket("2001:db8:1:2:bbbb::9"), ip_bucket("2001:db8:1:2:aaaa::1"));
        assert_eq!(ip_bucket("::ffff:198.51.100.7").as_deref(), Some("198.51.100.7"));
        assert_eq!(ip_bucket("unknown"), None);
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
        // Sổ giả: ngày dùng chính chuỗi `month`, để một regex dọn hết.
        let charge = |user: &str, plan: Plan, ip: &str| Charge {
            user_id: user.to_string(),
            month: month.clone(),
            free_pool: plan == Plan::Free,
            day: (plan == Plan::Free).then(|| month.clone()),
            ip: (plan == Plan::Free).then(|| format!("{ip}-{month}")),
        };
        let mut limits = TranscribeLimits {
            free_seconds: 100,
            plus_seconds: 1_000,
            free_pool_seconds: 150,
            monthly_cap_seconds: 400,
            free_daily_seconds: 1_000,
            free_ip_daily_seconds: 1_000,
            free_delay_hours: 0,
        };
        let (alice, bob, carol) = (charge("alice", Plan::Free, "a"), charge("bob", Plan::Free, "b"), charge("carol", Plan::Plus, "c"));

        // 20 lượt song song, mỗi lượt 10 giây, user free hạn mức 100 ⟹ đúng 10 lượt được giữ.
        let tasks = (0..20).map(|_| reserve(&db, &alice, 100, 10, &limits));
        let results = futures_util::future::join_all(tasks).await;
        let ok = results.iter().filter(|r| matches!(r, Ok(Ok(_)))).count();
        assert_eq!(ok, 10);
        let usage = month_usage(&db, "alice", &month).await.expect("đọc");
        assert_eq!((usage.user_seconds, usage.free_pool_seconds, usage.total_seconds), (100, 100, 100));

        // Quỹ free còn 50: bob (free) giữ 60 bị chặn bởi quỹ, mọi sổ của bob (tháng, ngày, IP) trả về 0.
        assert_eq!(reserve(&db, &bob, 100, 60, &limits).await.expect("bob"), Err(Blocked::FreePool));
        assert_eq!(month_usage(&db, "bob", &month).await.expect("đọc").user_seconds, 0);
        assert_eq!(blocked_usage(&db, &bob, &Blocked::DailyLimit).await.expect("ngày"), 0);
        assert_eq!(blocked_usage(&db, &bob, &Blocked::IpLimit).await.expect("ip"), 0);
        // carol (plus) không bị quỹ free chặn, chỉ bị trần tổng (400 − 100 = 300).
        reserve(&db, &carol, 1_000, 300, &limits).await.expect("carol").expect("giữ");
        assert_eq!(reserve(&db, &carol, 1_000, 1, &limits).await.expect("carol"), Err(Blocked::TotalCap));
        assert_eq!(month_usage(&db, "carol", &month).await.expect("đọc").user_seconds, 300);

        // Quyết toán: carol chỉ gửi 250 giây ⟹ hoàn 50 ở sổ của carol và trần tổng.
        settle(&db, &carol, -50).await.expect("hoàn");
        let usage = month_usage(&db, "carol", &month).await.expect("đọc");
        assert_eq!((usage.user_seconds, usage.total_seconds), (250, 350));

        // Giới hạn ngày 30 giây mỗi tài khoản, 50 giây mỗi IP; quỹ và trần rộng.
        limits.free_daily_seconds = 30;
        limits.free_ip_daily_seconds = 50;
        limits.free_pool_seconds = 10_000;
        limits.monthly_cap_seconds = 10_000;
        let (dave, erin, frank, grace) =
            (charge("dave", Plan::Free, "x"), charge("erin", Plan::Free, "x"), charge("frank", Plan::Free, "y"), charge("grace", Plan::Plus, "x"));
        let tasks = (0..6).map(|_| reserve(&db, &dave, 100, 10, &limits));
        let ok = futures_util::future::join_all(tasks).await.iter().filter(|r| matches!(r, Ok(Ok(_)))).count();
        assert_eq!(ok, 3, "dave chỉ được 30 giây trong ngày");
        assert_eq!(reserve(&db, &dave, 100, 10, &limits).await.expect("dave"), Err(Blocked::DailyLimit));
        // erin cùng IP: còn 20 giây của IP.
        reserve(&db, &erin, 100, 10, &limits).await.expect("erin").expect("giữ");
        assert_eq!(reserve(&db, &erin, 100, 20, &limits).await.expect("erin"), Err(Blocked::IpLimit));
        assert_eq!(month_usage(&db, "erin", &month).await.expect("đọc").user_seconds, 10);
        assert_eq!(blocked_usage(&db, &erin, &Blocked::IpLimit).await.expect("ip"), 40);
        // IP khác không bị ảnh hưởng; gói plus cùng IP không có sổ ngày/IP.
        reserve(&db, &frank, 100, 20, &limits).await.expect("frank").expect("giữ");
        reserve(&db, &grace, 1_000, 500, &limits).await.expect("grace").expect("giữ");
        assert_eq!(blocked_usage(&db, &erin, &Blocked::IpLimit).await.expect("ip"), 40);
        // Hoàn 10 giây của dave trả cả sổ ngày lẫn sổ IP.
        settle(&db, &dave, -10).await.expect("hoàn");
        assert_eq!(blocked_usage(&db, &dave, &Blocked::DailyLimit).await.expect("ngày"), 20);
        assert_eq!(blocked_usage(&db, &dave, &Blocked::IpLimit).await.expect("ip"), 30);

        db.collection::<Document>(COLL)
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
    }
}
