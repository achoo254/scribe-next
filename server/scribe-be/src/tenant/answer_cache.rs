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

pub struct AnswerCache<A> {
    found: Mutex<HashMap<String, Entry<A>>>,
    missing: Mutex<HashMap<String, Entry<A>>>,
    /// Khoá theo khoá cache: một lần hỏi IdP mỗi host.
    loading: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    now: Clock,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl<A: Clone> AnswerCache<A> {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }

    pub fn with_clock(now: Clock) -> Self {
        Self { found: Mutex::default(), missing: Mutex::default(), loading: Mutex::default(), now }
    }

    fn entry(&self, key: &str) -> Option<Entry<A>> {
        let found = lock(&self.found).get(key).cloned();
        found.or_else(|| lock(&self.missing).get(key).cloned())
    }

    fn store(&self, key: &str, entry: Entry<A>) {
        lock(&self.found).remove(key);
        lock(&self.missing).remove(key);
        let mut map = if entry.answer.is_some() { lock(&self.found) } else { lock(&self.missing) };
        if map.len() >= MAX_ENTRIES {
            let now = (self.now)();
            map.retain(|_, e| e.fresh_until > now);
            if map.len() >= MAX_ENTRIES {
                if let Some(oldest) = map.iter().min_by_key(|(_, e)| e.fresh_until).map(|(k, _)| k.clone()) {
                    map.remove(&oldest);
                }
            }
        }
        map.insert(key.to_string(), entry);
    }

    /// Câu còn mới của `key`, không thì hỏi bằng `load` (`Ok(None)` = IdP trả 404).
    pub async fn get<F, Fut>(&self, key: &str, load: F) -> Result<Option<A>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<A>, String>>,
    {
        if let Some(e) = self.entry(key).filter(|e| (self.now)() < e.fresh_until) {
            return Ok(e.answer);
        }
        let gate = lock(&self.loading).entry(key.to_string()).or_default().clone();
        let _held = gate.lock().await;
        // Lần hỏi khác vừa xong trong lúc chờ khoá: dùng luôn.
        let known = self.entry(key);
        if let Some(e) = known.as_ref().filter(|e| (self.now)() < e.fresh_until) {
            return Ok(e.answer.clone());
        }
        let result = match load().await {
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
                None => Err(e),
            },
        };
        lock(&self.loading).remove(key);
        result
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
        assert_eq!(cache.get("a", || load(Ok(Some("t1".into())))).await, Ok(Some("t1".into())));
        // Còn mới: không hỏi lại.
        assert_eq!(cache.get("a", || load(Ok(Some("t2".into())))).await, Ok(Some("t1".into())));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Hết 60 giây: hỏi lại; IdP lỗi ⟹ câu cũ, giữ thêm 10 giây.
        offset.store(61, Ordering::SeqCst);
        assert_eq!(cache.get("a", || load(Err("sập".into()))).await, Ok(Some("t1".into())));
        offset.store(65, Ordering::SeqCst);
        assert_eq!(cache.get("a", || load(Ok(Some("t3".into())))).await, Ok(Some("t1".into())), "chưa tới lúc thử lại");
        offset.store(72, Ordering::SeqCst);
        assert_eq!(cache.get("a", || load(Ok(Some("t3".into())))).await, Ok(Some("t3".into())));
        // Chưa biết gì mà IdP lỗi ⟹ lỗi.
        assert!(cache.get("b", || load(Err("sập".into()))).await.is_err());
        // 404 được nhớ 30 giây.
        assert_eq!(cache.get("c", || load(Ok(None))).await, Ok(None));
        assert_eq!(cache.get("c", || load(Ok(Some("x".into())))).await, Ok(None));
        offset.store(103, Ordering::SeqCst);
        assert_eq!(cache.get("c", || load(Ok(Some("x".into())))).await, Ok(Some("x".into())));
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
                    .get("host", || async move {
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
