//! Bản niêm phong key AI của từng tenant, đọc từ `GET /internal/tenants/:id/scribe-keys` và giữ [`FRESH`]. Chỉ giữ bản
//! niêm phong, không bao giờ giữ bản rõ: mỗi lần dùng mở lại ([`super::open`]). Mỗi tenant chỉ một lần hỏi IdP chạy cùng
//! lúc. Bản quá [`FRESH`] mà chưa quá [`STALE_MAX`] được trả ngay và hỏi lại nền (lượt của người dùng không chờ IdP);
//! chỉ chờ khi chưa có bản nào dùng được. IdP lỗi ⟹ giữ bản đã có trong [`STALE_MAX`] kể từ lần đọc được (hỏi lại sau
//! [`RETRY`]), quá hạn đó hay chưa có bản nào ⟹ lỗi ("chưa đọc được key của domain").
//!
//! Key đã báo lỗi (sai, hết credit, không mở được) được đánh dấu tại chỗ theo `(tenant, nhà cung cấp, tag)`: lượt sau
//! không chọn lại key đó dù IdP chưa kịp ghi trạng thái, còn key owner nhập lại (tag mới) dùng được ngay.

use super::sealed::Sealed;
use crate::config::Vendor;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bản niêm phong được hỏi lại sau chừng này.
pub const FRESH: Duration = Duration::from_secs(60);
/// IdP lỗi: dùng bản cũ tối đa chừng này kể từ lần đọc được gần nhất.
pub const STALE_MAX: Duration = Duration::from_secs(10 * 60);
/// IdP lỗi: hỏi lại sau chừng này (không dồn dập khi IdP đang khởi động lại).
pub const RETRY: Duration = Duration::from_secs(10);
/// Số tenant giữ tối đa; quá thì bỏ mục cũ nhất.
const MAX_TENANTS: usize = 1_000;
/// Số dấu "key đã lỗi" giữ tối đa; quá thì bỏ hết (IdP vẫn giữ trạng thái đã báo).
const MAX_MARKS: usize = 10_000;
/// Trạng thái IdP ghi cho key không dùng được. Trạng thái khác (IdP mới hơn Scribe) không chặn key, chỉ log một lần.
const BROKEN: [&str; 3] = ["invalid", "quota", "unreadable"];
/// Số trạng thái lạ nhớ để log một lần; quá thì không log thêm.
const MAX_UNKNOWN_STATUSES: usize = 32;

/// Một key của tenant như IdP trả: nhà cung cấp, trạng thái IdP đang ghi (vắng = dùng được, xem [`BROKEN`]) và bản
/// niêm phong. Không `Debug`.
#[derive(Clone)]
pub struct StoredKey {
    pub vendor: Vendor,
    pub status: Option<String>,
    pub sealed: Sealed,
}

#[derive(Deserialize)]
struct Listing {
    keys: Vec<RawKey>,
}

#[derive(Deserialize)]
struct RawKey {
    vendor: String,
    #[serde(default)]
    status: Option<String>,
    sealed: Sealed,
}

/// Đọc thân JSON của API: nhà cung cấp lạ (IdP mới hơn Scribe) bị bỏ qua.
pub fn parse_listing(body: &[u8]) -> Result<Vec<StoredKey>, String> {
    let listing: Listing = serde_json::from_slice(body).map_err(|_| "IdP trả JSON không đúng dạng".to_string())?;
    Ok(listing
        .keys
        .into_iter()
        .filter_map(|k| Some(StoredKey { vendor: Vendor::parse(&k.vendor)?, status: k.status, sealed: k.sealed }))
        .collect())
}

struct Entry {
    keys: Arc<Vec<StoredKey>>,
    /// Lần đọc được gần nhất.
    fetched: Instant,
    /// Hỏi IdP lại từ lúc này.
    next_check: Instant,
}

/// Bản trong cache của một tenant, theo tuổi.
enum Cached {
    /// Chưa quá [`FRESH`] (hay chưa tới lúc hỏi lại sau lỗi): dùng luôn.
    Fresh(Arc<Vec<StoredKey>>),
    /// Tới lúc hỏi lại nhưng chưa quá [`STALE_MAX`]: dùng luôn, hỏi lại nền.
    Stale(Arc<Vec<StoredKey>>),
    /// Không có hay quá [`STALE_MAX`]: phải chờ IdP.
    Missing,
}

/// Dấu "không dùng" của một bản key; `reported` = đã gửi IdP (lần báo không tới thì xoá để lần lỗi sau báo lại).
struct Mark {
    status: &'static str,
    reported: bool,
}

/// Đồng hồ tiêm được cho test.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

pub struct KeyCache {
    entries: Mutex<HashMap<String, Entry>>,
    /// Tenant IdP vừa lỗi mà chưa có bản nào: lỗi và lúc được hỏi lại.
    failed: Mutex<HashMap<String, (String, Instant)>>,
    loading: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Tenant đang được hỏi lại nền (một lượt mỗi tenant).
    refreshing: Mutex<HashSet<String>>,
    /// `(tenant, nhà cung cấp, tag)` ⟹ dấu Scribe đã đặt.
    marks: Mutex<HashMap<(String, Vendor, String), Mark>>,
    /// Trạng thái lạ IdP đã trả (đã log).
    unknown_statuses: Mutex<HashSet<String>>,
    now: Clock,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Default for KeyCache {
    fn default() -> Self {
        KeyCache::with_clock(Arc::new(Instant::now))
    }
}

impl KeyCache {
    pub fn with_clock(now: Clock) -> KeyCache {
        KeyCache {
            entries: Mutex::default(),
            failed: Mutex::default(),
            loading: Mutex::default(),
            refreshing: Mutex::default(),
            marks: Mutex::default(),
            unknown_statuses: Mutex::default(),
            now,
        }
    }

    fn cached(&self, tenant_id: &str, now: Instant) -> Cached {
        match lock(&self.entries).get(tenant_id) {
            Some(e) if now < e.next_check => Cached::Fresh(e.keys.clone()),
            Some(e) if now.duration_since(e.fetched) < STALE_MAX => Cached::Stale(e.keys.clone()),
            _ => Cached::Missing,
        }
    }

    /// Key của `tenant_id`: bản còn mới; bản cũ (trả ngay, hỏi lại IdP nền); hay chưa có gì thì chờ hỏi IdP qua `fetch`.
    /// Mỗi tenant một lần hỏi cùng lúc.
    pub async fn get<F, Fut>(self: &Arc<Self>, tenant_id: &str, fetch: F) -> Result<Arc<Vec<StoredKey>>, String>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<StoredKey>, String>> + Send + 'static,
    {
        match self.cached(tenant_id, (self.now)()) {
            Cached::Fresh(keys) => Ok(keys),
            Cached::Stale(keys) => {
                self.refresh_in_background(tenant_id, fetch);
                Ok(keys)
            }
            Cached::Missing => self.load(tenant_id, fetch).await,
        }
    }

    /// Hỏi lại IdP nền cho `tenant_id`, trừ khi đang có lượt hỏi nền của tenant đó.
    fn refresh_in_background<F, Fut>(self: &Arc<Self>, tenant_id: &str, fetch: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Vec<StoredKey>, String>> + Send + 'static,
    {
        if !lock(&self.refreshing).insert(tenant_id.to_string()) {
            return;
        }
        let (cache, tenant_id) = (self.clone(), tenant_id.to_string());
        tokio::spawn(async move {
            if let Err(e) = cache.load(&tenant_id, fetch).await {
                tracing::warn!(tenant = %tenant_id, "hỏi lại key của domain lỗi: {e}");
            }
            lock(&cache.refreshing).remove(&tenant_id);
        });
    }

    /// Hỏi IdP (một lượt mỗi tenant cùng lúc) và ghi kết quả; IdP lỗi thì giữ bản cũ trong [`STALE_MAX`].
    async fn load<F, Fut>(&self, tenant_id: &str, fetch: F) -> Result<Arc<Vec<StoredKey>>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<StoredKey>, String>>,
    {
        let gate = lock(&self.loading).entry(tenant_id.to_string()).or_default().clone();
        let held = gate.lock().await;
        let now = (self.now)();
        // Lượt khác vừa hỏi xong trong lúc chờ.
        if let Cached::Fresh(keys) = self.cached(tenant_id, now) {
            return Ok(keys);
        }
        if let Some((e, until)) = lock(&self.failed).get(tenant_id) {
            if now < *until {
                return Err(e.clone());
            }
        }
        let result = fetch().await;
        let now = (self.now)();
        let out = match result {
            Ok(keys) => {
                let keys = Arc::new(keys);
                let mut entries = lock(&self.entries);
                if entries.len() >= MAX_TENANTS && !entries.contains_key(tenant_id) {
                    if let Some(oldest) = entries.iter().min_by_key(|(_, e)| e.fetched).map(|(k, _)| k.clone()) {
                        entries.remove(&oldest);
                    }
                }
                entries.insert(tenant_id.to_string(), Entry { keys: keys.clone(), fetched: now, next_check: now + FRESH });
                lock(&self.failed).remove(tenant_id);
                Ok(keys)
            }
            Err(e) => {
                let mut entries = lock(&self.entries);
                match entries.get_mut(tenant_id).filter(|entry| now.duration_since(entry.fetched) < STALE_MAX) {
                    Some(entry) => {
                        tracing::warn!(tenant = %tenant_id, "đọc key của domain lỗi, dùng bản cũ: {e}");
                        entry.next_check = now + RETRY;
                        Ok(entry.keys.clone())
                    }
                    None => {
                        entries.remove(tenant_id);
                        let mut failed = lock(&self.failed);
                        if failed.len() >= MAX_TENANTS {
                            failed.retain(|_, (_, until)| *until > now);
                        }
                        failed.insert(tenant_id.to_string(), (e.clone(), now + RETRY));
                        Err(e)
                    }
                }
            }
        };
        drop(held);
        lock(&self.loading).remove(tenant_id);
        out
    }

    /// Đánh dấu bản key `tag` của `vendor` ở `tenant_id` là `status` (lượt sau không dùng bản đó). `true` khi cần báo IdP:
    /// bản key này chưa báo, hay lần báo trước không tới IdP ([`KeyCache::forget_report`]).
    pub fn mark(&self, tenant_id: &str, vendor: Vendor, tag: &str, status: &'static str) -> bool {
        let mut marks = lock(&self.marks);
        if marks.len() >= MAX_MARKS {
            marks.clear();
        }
        let mark = marks.entry((tenant_id.to_string(), vendor, tag.to_string())).or_insert(Mark { status, reported: false });
        mark.status = status;
        !std::mem::replace(&mut mark.reported, true)
    }

    /// Lần báo bản key này không tới IdP: giữ dấu "không dùng", nhưng lần lỗi sau được báo lại.
    pub fn forget_report(&self, tenant_id: &str, vendor: Vendor, tag: &str) {
        if let Some(mark) = lock(&self.marks).get_mut(&(tenant_id.to_string(), vendor, tag.to_string())) {
            mark.reported = false;
        }
    }

    /// Trạng thái không dùng được của một key: IdP đã ghi một trạng thái trong [`BROKEN`], hoặc Scribe đã đánh dấu bản key
    /// này. `None` = dùng được.
    pub fn problem(&self, tenant_id: &str, key: &StoredKey) -> Option<String> {
        if let Some(status) = &key.status {
            if BROKEN.contains(&status.as_str()) {
                return Some(status.clone());
            }
            self.note_unknown_status(status);
        }
        lock(&self.marks).get(&(tenant_id.to_string(), key.vendor, key.sealed.tag.clone())).map(|m| m.status.to_string())
    }

    /// Trạng thái IdP lạ: không chặn key, log một lần mỗi trạng thái.
    fn note_unknown_status(&self, status: &str) {
        let mut seen = lock(&self.unknown_statuses);
        if seen.len() < MAX_UNKNOWN_STATUSES && seen.insert(status.to_string()) {
            tracing::warn!(status = %status, "IdP trả trạng thái key lạ — bỏ qua, key vẫn dùng");
        }
    }

    #[cfg(test)]
    pub fn reported(&self, tenant_id: &str, vendor: Vendor, tag: &str) -> bool {
        lock(&self.marks).get(&(tenant_id.to_string(), vendor, tag.to_string())).is_some_and(|m| m.reported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    fn key(vendor: Vendor, tag: &str) -> StoredKey {
        StoredKey {
            vendor,
            status: None,
            sealed: Sealed {
                iv: "iv".into(),
                ciphertext: "c".into(),
                tag: tag.into(),
                wrapped_key: "w".into(),
                recipient: "0000000000000000".into(),
            },
        }
    }

    /// Đồng hồ giả: bắt đầu từ `Instant::now()`, tiến theo số giây đặt vào `offset`.
    fn clock() -> (Clock, Arc<AtomicU64>) {
        let base = Instant::now();
        let offset = Arc::new(AtomicU64::new(0));
        let o = offset.clone();
        (Arc::new(move || base + Duration::from_secs(o.load(Ordering::SeqCst))), offset)
    }

    /// `fetch` cho [`KeyCache::get`] từ một future dựng sẵn (chưa chạy tới khi cache gọi và chờ nó).
    fn lazy<Fut>(fut: Fut) -> impl FnOnce() -> Fut {
        move || fut
    }

    /// Chờ việc nền (hỏi lại IdP) chạy tới khi `done`, tối đa ~1 giây.
    async fn settle(done: impl Fn() -> bool) {
        for _ in 0..200 {
            if done() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(done(), "việc nền chưa xong");
    }

    #[test]
    fn listings_skip_unknown_vendors_and_keep_statuses() {
        let body = br#"{"keys":[
            {"vendor":"openai","hint":"abcd","updatedAt":"x","sealed":{"kid":"recipient","iv":"a","ciphertext":"b","tag":"c","wrappedKey":"d","recipient":"e"}},
            {"vendor":"mistral","hint":"abcd","sealed":{"kid":"recipient","iv":"a","ciphertext":"b","tag":"c","wrappedKey":"d","recipient":"e"}},
            {"vendor":"gemini","status":"quota","sealed":{"kid":"recipient","iv":"a","ciphertext":"b","tag":"t2","wrappedKey":"d","recipient":"e"}}
        ]}"#;
        let keys = parse_listing(body).expect("đọc");
        assert_eq!(keys.iter().map(|k| k.vendor).collect::<Vec<_>>(), vec![Vendor::OpenAi, Vendor::Gemini]);
        assert_eq!(keys[1].status.as_deref(), Some("quota"));
        assert_eq!(keys[0].sealed.wrapped_key, "d");
        assert!(parse_listing(b"{}").is_err());
    }

    #[tokio::test]
    async fn fresh_keys_are_reused_then_asked_again() {
        let (now, offset) = clock();
        let cache = Arc::new(KeyCache::with_clock(now));
        let calls = Arc::new(AtomicUsize::new(0));
        let fetch = |c: Arc<AtomicUsize>| async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(vec![key(Vendor::OpenAi, "t1")])
        };
        cache.get("ten_a", lazy(fetch(calls.clone()))).await.expect("lần đầu");
        cache.get("ten_a", lazy(fetch(calls.clone()))).await.expect("từ cache");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        offset.store(61, Ordering::SeqCst);
        cache.get("ten_a", lazy(fetch(calls.clone()))).await.expect("bản cũ, hỏi lại nền");
        settle(|| calls.load(Ordering::SeqCst) == 2).await;
        // Tenant khác có bản riêng.
        cache.get("ten_b", lazy(fetch(calls.clone()))).await.expect("tenant khác");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn stale_keys_are_served_at_once_and_refreshed_once_in_the_background() {
        let (now, offset) = clock();
        let cache = Arc::new(KeyCache::with_clock(now));
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(tokio::sync::Notify::new());
        // IdP trả `tag` sau khi `gate` (nếu có) được mở.
        let fetch = |tag: &'static str, gate: Option<Arc<tokio::sync::Notify>>, c: Arc<AtomicUsize>| async move {
            c.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = gate {
                gate.notified().await;
            }
            Ok(vec![key(Vendor::OpenAi, tag)])
        };
        let tag_of = |keys: Arc<Vec<StoredKey>>| keys[0].sealed.tag.clone();
        assert_eq!(tag_of(cache.get("ten_a", lazy(fetch("t1", None, calls.clone()))).await.expect("lần đầu")), "t1");
        offset.store(61, Ordering::SeqCst);
        // Quá hạn: trả ngay bản cũ dù IdP chưa trả lời, và chỉ một lượt hỏi nền dù nhiều lượt cùng tới.
        for _ in 0..3 {
            let got = cache.get("ten_a", lazy(fetch("t2", Some(release.clone()), calls.clone()))).await.expect("bản cũ");
            assert_eq!(tag_of(got), "t1");
        }
        settle(|| calls.load(Ordering::SeqCst) == 2).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2, "một lượt hỏi nền mỗi tenant");
        // IdP trả lời: lượt sau thấy bản mới, không hỏi thêm.
        release.notify_one();
        settle(|| lock(&cache.refreshing).is_empty()).await;
        assert_eq!(tag_of(cache.get("ten_a", lazy(fetch("t3", None, calls.clone()))).await.expect("bản mới")), "t2");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Quá `STALE_MAX` không ai hỏi: chờ IdP, không trả bản quá cũ.
        offset.store(61 + 601, Ordering::SeqCst);
        assert_eq!(tag_of(cache.get("ten_a", lazy(fetch("t4", None, calls.clone()))).await.expect("chờ IdP")), "t4");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn an_idp_failure_uses_the_old_keys_for_ten_minutes_at_most() {
        let (now, offset) = clock();
        let cache = Arc::new(KeyCache::with_clock(now));
        let calls = Arc::new(AtomicUsize::new(0));
        let fetch = |up: bool, c: Arc<AtomicUsize>| async move {
            c.fetch_add(1, Ordering::SeqCst);
            if up {
                Ok(vec![key(Vendor::ElevenLabs, "t1")])
            } else {
                Err("không gọi được IdP (lỗi mạng)".to_string())
            }
        };
        let at = |s: u64| offset.store(s, Ordering::SeqCst);
        cache.get("ten_a", lazy(fetch(true, calls.clone()))).await.expect("đọc được");
        at(120);
        assert_eq!(cache.get("ten_a", lazy(fetch(false, calls.clone()))).await.expect("bản cũ").len(), 1);
        settle(|| calls.load(Ordering::SeqCst) == 2 && lock(&cache.refreshing).is_empty()).await;
        // Trong `RETRY` không hỏi lại.
        at(125);
        assert_eq!(cache.get("ten_a", lazy(fetch(false, calls.clone()))).await.expect("bản cũ").len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Quá 10 phút kể từ lần đọc được: lỗi.
        at(601);
        assert!(cache.get("ten_a", lazy(fetch(false, calls.clone()))).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        // Lỗi khi chưa có gì được nhớ `RETRY` giây: không hỏi lại.
        assert!(cache.get("ten_a", lazy(fetch(true, calls.clone()))).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        at(612);
        assert_eq!(cache.get("ten_a", lazy(fetch(true, calls.clone()))).await.expect("IdP lên lại").len(), 1);
    }

    #[test]
    fn marks_follow_the_exact_seal() {
        let cache = KeyCache::default();
        let k = key(Vendor::OpenAi, "t1");
        assert_eq!(cache.problem("ten_a", &k), None);
        assert!(cache.mark("ten_a", Vendor::OpenAi, "t1", "invalid"));
        assert!(!cache.mark("ten_a", Vendor::OpenAi, "t1", "invalid"), "báo một lần");
        assert_eq!(cache.problem("ten_a", &k).as_deref(), Some("invalid"));
        // Owner nhập lại key (tag mới), tenant khác, nhà cung cấp khác: không bị dấu.
        assert_eq!(cache.problem("ten_a", &key(Vendor::OpenAi, "t2")), None);
        assert_eq!(cache.problem("ten_b", &k), None);
        assert_eq!(cache.problem("ten_a", &key(Vendor::Gemini, "t1")), None);
        // IdP đã ghi trạng thái.
        let reported = StoredKey { status: Some("quota".into()), ..key(Vendor::Gemini, "t3") };
        assert_eq!(cache.problem("ten_a", &reported).as_deref(), Some("quota"));
    }

    #[test]
    fn a_report_that_never_reached_the_idp_is_sent_again_on_the_next_failure() {
        let cache = KeyCache::default();
        let k = key(Vendor::OpenAi, "t1");
        assert!(cache.mark("ten_a", Vendor::OpenAi, "t1", "quota"));
        assert!(cache.reported("ten_a", Vendor::OpenAi, "t1"));
        cache.forget_report("ten_a", Vendor::OpenAi, "t1");
        // Vẫn không dùng bản key đó, nhưng lần lỗi sau báo lại (một lần).
        assert_eq!(cache.problem("ten_a", &k).as_deref(), Some("quota"));
        assert!(cache.mark("ten_a", Vendor::OpenAi, "t1", "invalid"));
        assert!(!cache.mark("ten_a", Vendor::OpenAi, "t1", "invalid"));
        assert_eq!(cache.problem("ten_a", &k).as_deref(), Some("invalid"));
        // Bản key chưa từng đánh dấu: không có gì để quên.
        cache.forget_report("ten_a", Vendor::Gemini, "t9");
        assert_eq!(cache.problem("ten_a", &key(Vendor::Gemini, "t9")), None);
    }

    #[test]
    fn only_known_idp_statuses_block_a_key() {
        let cache = KeyCache::default();
        for status in BROKEN {
            let k = StoredKey { status: Some(status.into()), ..key(Vendor::OpenAi, "t1") };
            assert_eq!(cache.problem("ten_a", &k).as_deref(), Some(status));
        }
        // IdP mới hơn Scribe ghi trạng thái Scribe chưa biết: key vẫn dùng được (log một lần).
        for _ in 0..2 {
            let k = StoredKey { status: Some("checking".into()), ..key(Vendor::OpenAi, "t1") };
            assert_eq!(cache.problem("ten_a", &k), None);
        }
        assert_eq!(lock(&cache.unknown_statuses).len(), 1);
        // Trạng thái lạ không che dấu Scribe đã đặt.
        cache.mark("ten_a", Vendor::OpenAi, "t1", "invalid");
        let k = StoredKey { status: Some("checking".into()), ..key(Vendor::OpenAi, "t1") };
        assert_eq!(cache.problem("ten_a", &k).as_deref(), Some("invalid"));
    }
}
