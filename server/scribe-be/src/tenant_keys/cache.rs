//! Bản niêm phong key AI của từng tenant, đọc từ `GET /internal/tenants/:id/scribe-keys` và giữ [`FRESH`]. Chỉ giữ bản
//! niêm phong, không bao giờ giữ bản rõ: mỗi lần dùng mở lại ([`super::open`]). Mỗi tenant chỉ một lần hỏi IdP chạy cùng
//! lúc. IdP lỗi ⟹ dùng bản đã có nếu đọc được trong [`STALE_MAX`] (hỏi lại sau [`RETRY`]), quá hạn đó hay chưa có bản
//! nào ⟹ lỗi ("chưa đọc được key của domain").
//!
//! Key đã báo lỗi (sai, hết credit, không mở được) được đánh dấu tại chỗ theo `(tenant, nhà cung cấp, tag)`: lượt sau
//! không chọn lại key đó dù IdP chưa kịp ghi trạng thái, còn key owner nhập lại (tag mới) dùng được ngay.

use super::sealed::Sealed;
use crate::config::Vendor;
use serde::Deserialize;
use std::collections::HashMap;
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

/// Một key của tenant như IdP trả: nhà cung cấp, trạng thái IdP đang ghi (`invalid | quota | unreadable`, vắng = dùng
/// được) và bản niêm phong. Không `Debug`.
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

/// Đồng hồ tiêm được cho test.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

pub struct KeyCache {
    entries: Mutex<HashMap<String, Entry>>,
    /// Tenant IdP vừa lỗi mà chưa có bản nào: lỗi và lúc được hỏi lại.
    failed: Mutex<HashMap<String, (String, Instant)>>,
    loading: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// `(tenant, nhà cung cấp, tag)` ⟹ trạng thái Scribe đã báo.
    marks: Mutex<HashMap<(String, Vendor, String), &'static str>>,
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
            marks: Mutex::default(),
            now,
        }
    }

    fn cached(&self, tenant_id: &str, now: Instant) -> Option<Arc<Vec<StoredKey>>> {
        lock(&self.entries).get(tenant_id).filter(|e| now < e.next_check).map(|e| e.keys.clone())
    }

    /// Key của `tenant_id`: bản còn mới, hoặc hỏi IdP qua `fetch` (một lần mỗi tenant cùng lúc).
    pub async fn get<F, Fut>(&self, tenant_id: &str, fetch: F) -> Result<Arc<Vec<StoredKey>>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<StoredKey>, String>>,
    {
        if let Some(keys) = self.cached(tenant_id, (self.now)()) {
            return Ok(keys);
        }
        let gate = lock(&self.loading).entry(tenant_id.to_string()).or_default().clone();
        let held = gate.lock().await;
        let now = (self.now)();
        // Lượt khác vừa hỏi xong trong lúc chờ.
        if let Some(keys) = self.cached(tenant_id, now) {
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

    /// Đánh dấu bản key `tag` của `vendor` ở `tenant_id` là `status`. `true` khi dấu mới (lần đầu báo bản key này).
    pub fn mark(&self, tenant_id: &str, vendor: Vendor, tag: &str, status: &'static str) -> bool {
        let mut marks = lock(&self.marks);
        if marks.len() >= MAX_MARKS {
            marks.clear();
        }
        marks.insert((tenant_id.to_string(), vendor, tag.to_string()), status).is_none()
    }

    /// Trạng thái không dùng được của một key: IdP đã ghi, hoặc Scribe đã báo bản key này. `None` = dùng được.
    pub fn problem(&self, tenant_id: &str, key: &StoredKey) -> Option<String> {
        if let Some(status) = &key.status {
            return Some(status.clone());
        }
        lock(&self.marks).get(&(tenant_id.to_string(), key.vendor, key.sealed.tag.clone())).map(|s| (*s).to_string())
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
        let cache = KeyCache::with_clock(now);
        let calls = Arc::new(AtomicUsize::new(0));
        let fetch = |c: Arc<AtomicUsize>| async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(vec![key(Vendor::OpenAi, "t1")])
        };
        cache.get("ten_a", || fetch(calls.clone())).await.expect("lần đầu");
        cache.get("ten_a", || fetch(calls.clone())).await.expect("từ cache");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        offset.store(61, Ordering::SeqCst);
        cache.get("ten_a", || fetch(calls.clone())).await.expect("hỏi lại");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Tenant khác có bản riêng.
        cache.get("ten_b", || fetch(calls.clone())).await.expect("tenant khác");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn an_idp_failure_uses_the_old_keys_for_ten_minutes_at_most() {
        let (now, offset) = clock();
        let cache = KeyCache::with_clock(now);
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
        cache.get("ten_a", || fetch(true, calls.clone())).await.expect("đọc được");
        at(120);
        assert_eq!(cache.get("ten_a", || fetch(false, calls.clone())).await.expect("bản cũ").len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Trong `RETRY` không hỏi lại.
        at(125);
        assert_eq!(cache.get("ten_a", || fetch(false, calls.clone())).await.expect("bản cũ").len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Quá 10 phút kể từ lần đọc được: lỗi.
        at(601);
        assert!(cache.get("ten_a", || fetch(false, calls.clone())).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        // Lỗi khi chưa có gì được nhớ `RETRY` giây: không hỏi lại.
        assert!(cache.get("ten_a", || fetch(true, calls.clone())).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        at(612);
        assert_eq!(cache.get("ten_a", || fetch(true, calls.clone())).await.expect("IdP lên lại").len(), 1);
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
}
