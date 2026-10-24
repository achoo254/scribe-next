//! Câu trả lời của IdP theo khoá (host): câu còn mới thì dùng lại, mỗi khoá chỉ một lần hỏi chạy cùng lúc, IdP lỗi thì
//! giữ câu cũ (thử lại sau [`RETRY`]) để tenant đã thấy vẫn chạy trong lúc IdP khởi động lại. Chỉ 404 thật mới được
//! nhớ là "không có tenant"; lỗi khi chưa biết gì thì trả lỗi. Cùng mốc với `TenantAnswerCache` của Localman
//! (`localman-next/packages/tenancy/src/tenant-answer-cache.ts`).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tenant tìm thấy được hỏi lại sau chừng này.
pub const FOUND_FRESH: Duration = Duration::from_secs(60);
/// Câu "không có" (404) được hỏi lại sau chừng này.
pub const MISSING_FRESH: Duration = Duration::from_secs(30);
/// IdP lỗi: dùng câu cũ chừng này rồi mới thử lại.
pub const RETRY: Duration = Duration::from_secs(10);
/// Số câu giữ tối đa mỗi loại: host ai cũng bịa ra được chỉ đẩy 404 khác ra, không bao giờ đẩy câu của tenant thật.
const MAX_ENTRIES: usize = 1_000;

#[derive(Clone)]
struct Entry<A> {
    answer: Option<A>,
    fresh_until: Instant,
}

/// Đồng hồ tiêm được cho test.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Vì sao không có câu trả lời.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// Hết ngân sách hỏi IdP (người gọi từ chối hỏi thêm): không hỏi, không nhớ gì.
    Busy,
    /// IdP lỗi mà chưa biết gì về khoá này; nhớ trong [`RETRY`] để không hỏi lại dồn dập.
    Failed(String),
}

pub struct AnswerCache<A> {
    found: Mutex<HashMap<String, Entry<A>>>,
    missing: Mutex<HashMap<String, Entry<A>>>,
    /// Khoá IdP vừa lỗi mà chưa có câu nào: lỗi và lúc được thử lại.
    failed: Mutex<HashMap<String, (String, Instant)>>,
    /// Khoá theo khoá cache: một lần hỏi IdP mỗi host.
    loading: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    now: Clock,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Bỏ khoá hỏi của một host khi lượt hỏi xong, kể cả khi request bị huỷ giữa chừng.
struct LoadingGuard<'a> {
    loading: &'a Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    key: &'a str,
}

impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        lock(self.loading).remove(self.key);
    }
}

/// Giữ map không quá [`MAX_ENTRIES`]: bỏ mục hết hạn trước, rồi mục cũ nhất.
fn bounded_insert<V>(map: &mut HashMap<String, V>, key: &str, value: V, expiry: impl Fn(&V) -> Instant, now: Instant) {
    if map.len() >= MAX_ENTRIES {
        map.retain(|_, v| expiry(v) > now);
        if map.len() >= MAX_ENTRIES {
            if let Some(oldest) = map.iter().min_by_key(|(_, v)| expiry(v)).map(|(k, _)| k.clone()) {
                map.remove(&oldest);
            }
        }
    }
    map.insert(key.to_string(), value);
}

impl<A: Clone> AnswerCache<A> {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }

    pub fn with_clock(now: Clock) -> Self {
        Self { found: Mutex::default(), missing: Mutex::default(), failed: Mutex::default(), loading: Mutex::default(), now }
    }

    fn entry(&self, key: &str) -> Option<Entry<A>> {
        let found = lock(&self.found).get(key).cloned();
        found.or_else(|| lock(&self.missing).get(key).cloned())
    }

    fn store(&self, key: &str, entry: Entry<A>) {
        lock(&self.found).remove(key);
        lock(&self.missing).remove(key);
        lock(&self.failed).remove(key);
        let now = (self.now)();
        let mut map = if entry.answer.is_some() { lock(&self.found) } else { lock(&self.missing) };
        bounded_insert(&mut map, key, entry, |e| e.fresh_until, now);
    }

    /// Lỗi IdP còn được nhớ của `key` (chưa tới lúc thử lại).
    fn recent_failure(&self, key: &str) -> Option<String> {
        let now = (self.now)();
        lock(&self.failed).get(key).filter(|(_, until)| now < *until).map(|(e, _)| e.clone())
    }

    /// Câu còn mới của `key`, không thì hỏi bằng `load` (`Ok(None)` = IdP trả 404) nếu `allow_load()` cho phép (ngân
    /// sách hỏi IdP; hết ngân sách thì dùng câu cũ nếu có, không thì [`LookupError::Busy`]).
    pub async fn get<F, Fut>(&self, key: &str, allow_load: impl FnOnce() -> bool, load: F) -> Result<Option<A>, LookupError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<A>, String>>,
    {
        let known = self.entry(key);
        if let Some(e) = known.as_ref().filter(|e| (self.now)() < e.fresh_until) {
            return Ok(e.answer.clone());
        }
        if let Some(e) = self.recent_failure(key) {
            return Err(LookupError::Failed(e));
        }
        if !allow_load() {
            return known.map(|e| e.answer).ok_or(LookupError::Busy);
        }
        let gate = lock(&self.loading).entry(key.to_string()).or_default().clone();
        let _held = gate.lock().await;
        let _cleanup = LoadingGuard { loading: &self.loading, key };
        // Lần hỏi khác vừa xong trong lúc chờ khoá: dùng luôn câu (hay lỗi) của nó.
        let known = self.entry(key);
        if let Some(e) = known.as_ref().filter(|e| (self.now)() < e.fresh_until) {
            return Ok(e.answer.clone());
        }
        if let Some(e) = self.recent_failure(key) {
            return Err(LookupError::Failed(e));
        }
        match load().await {
            Ok(answer) => {
                let ttl = if answer.is_some() { FOUND_FRESH } else { MISSING_FRESH };
                self.store(key, Entry { answer: answer.clone(), fresh_until: (self.now)() + ttl });
                Ok(answer)
            }
            Err(e) => match known {
                Some(known) => {
                    tracing::warn!(key, "tra tenant ở IdP lỗi: {e} — dùng câu trả lời cũ");
                    self.store(key, Entry { answer: known.answer.clone(), fresh_until: (self.now)() + RETRY });
                    Ok(known.answer)
                }
                None => {
                    let now = (self.now)();
                    bounded_insert(&mut lock(&self.failed), key, (e.clone(), now + RETRY), |(_, u)| *u, now);
                    Err(LookupError::Failed(e))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn clock() -> (Clock, Arc<AtomicU64>) {
        let start = Instant::now();
        let offset = Arc::new(AtomicU64::new(0));
        let o = offset.clone();
        (Arc::new(move || start + Duration::from_secs(o.load(Ordering::SeqCst))), offset)
    }

    #[tokio::test]
    async fn answers_are_reused_until_stale_and_kept_while_the_idp_fails() {
        let (now, offset) = clock();
        let cache: AnswerCache<String> = AnswerCache::with_clock(now);
        let calls = AtomicU64::new(0);
        let load = |answer: Result<Option<String>, String>| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move { answer }
        };
        assert_eq!(cache.get("a", || true, || load(Ok(Some("t1".into())))).await, Ok(Some("t1".into())));
        // Còn mới: không hỏi lại.
        assert_eq!(cache.get("a", || true, || load(Ok(Some("t2".into())))).await, Ok(Some("t1".into())));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Hết 60 giây: hỏi lại; IdP lỗi ⟹ câu cũ, giữ thêm 10 giây.
        offset.store(61, Ordering::SeqCst);
        assert_eq!(cache.get("a", || true, || load(Err("sập".into()))).await, Ok(Some("t1".into())));
        offset.store(65, Ordering::SeqCst);
        assert_eq!(cache.get("a", || true, || load(Ok(Some("t3".into())))).await, Ok(Some("t1".into())), "chưa tới lúc thử lại");
        offset.store(72, Ordering::SeqCst);
        assert_eq!(cache.get("a", || true, || load(Ok(Some("t3".into())))).await, Ok(Some("t3".into())));
        // Chưa biết gì mà IdP lỗi ⟹ lỗi, nhớ 10 giây: không hỏi lại dồn dập.
        assert_eq!(cache.get("b", || true, || load(Err("sập".into()))).await, Err(LookupError::Failed("sập".into())));
        let before = calls.load(Ordering::SeqCst);
        assert!(cache.get("b", || true, || load(Ok(Some("t".into())))).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), before, "trong 10 giây không hỏi lại");
        // Hết ngân sách: không hỏi IdP, không nhớ gì.
        assert_eq!(cache.get("z", || false, || load(Ok(Some("t".into())))).await, Err(LookupError::Busy));
        assert_eq!(calls.load(Ordering::SeqCst), before);
        // 404 được nhớ 30 giây.
        assert_eq!(cache.get("c", || true, || load(Ok(None))).await, Ok(None));
        assert_eq!(cache.get("c", || true, || load(Ok(Some("x".into())))).await, Ok(None));
        offset.store(103, Ordering::SeqCst);
        assert_eq!(cache.get("c", || true, || load(Ok(Some("x".into())))).await, Ok(Some("x".into())));
        // Câu cũ vẫn dùng được khi hết ngân sách; khoá hỏi không còn sót lại.
        offset.store(200, Ordering::SeqCst);
        assert_eq!(cache.get("c", || false, || load(Ok(None))).await, Ok(Some("x".into())));
        assert!(lock(&cache.loading).is_empty());
    }

    #[tokio::test]
    async fn one_question_per_key_runs_at_a_time() {
        let cache: Arc<AnswerCache<u32>> = Arc::new(AnswerCache::new());
        let calls = Arc::new(AtomicU64::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let (cache, calls) = (cache.clone(), calls.clone());
            tasks.push(tokio::spawn(async move {
                cache
                    .get("host", || true, || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(Some(7))
                    })
                    .await
            }));
        }
        for t in tasks {
            assert_eq!(t.await.expect("task"), Ok(Some(7)));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
