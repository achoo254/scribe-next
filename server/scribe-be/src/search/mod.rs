//! Kho bản ghi: tìm câu trong mọi bản chép lời của một người (`GET /api/search`) và hỏi đáp có trích mốc
//! giây (`POST /api/ask`, xem `ask`).
//!
//! Tìm trên index `transcript_sentences` (`models::sentence`, dựng ở `index`): câu tìm được chuẩn hoá như
//! câu trong index (`norm`), nên gõ có dấu hay không dấu đều khớp. Lọc ứng viên bằng index (`$text` trên
//! các từ đã gõ xong; câu tìm một từ thì theo tiền tố, từ 2 ký tự), lấy từ phiên mới nhất trước, rồi
//! trong code đòi khớp cả cụm liền nhau, từ cuối theo tiền tố (đang gõ dở). Không có câu nào khớp cả cụm thì nới ra: câu chứa đủ các từ, thứ tự bất kỳ.
//!
//! Chỉ cần đăng nhập và chỉ thấy câu của chính mình (mọi truy vấn lọc `userId`). Audit không ghi câu tìm.

pub mod ask;
pub mod index;
pub mod norm;

use crate::auth::UserId;
use crate::error::AppError;
use crate::models::sentence::{self, Order, Row};
use crate::models::{file, transcript_doc};
use crate::state::AppState;
use crate::transcribe::docs::check_key;
use axum::extract::{Query, State};
use axum::{Extension, Json};
use mongodb::bson::DateTime;
use mongodb::Database;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Câu tìm dài nhất (ký tự).
const MAX_QUERY_CHARS: usize = 200;
const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;
/// Số câu ứng viên tối đa lấy từ index cho một lần tìm, phiên mới nhất trước (xếp hạng và lọc cụm làm
/// trong code): kho rất lớn với từ rất phổ biến thì phiên cũ nhất có thể không tới lượt.
const CANDIDATES: i64 = 2_000;
/// Câu tìm một từ cần ít nhất chừng này ký tự (đã chuẩn hoá) mới tìm theo tiền tố.
pub(crate) const MIN_PREFIX_CHARS: usize = 2;
/// Ô tìm gửi theo nhịp gõ (đã chờ một chút): trần mỗi phút của một người.
const PER_MINUTE: u32 = 120;

/// Tên hiển thị và lúc bắt đầu của một phiên.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SessionMeta {
    /// Tiêu đề người dùng đặt, hoặc tên file của bản ghi một file; phiên iPhone chưa đặt tên thì `None`
    /// (web tự đặt theo giờ ghi).
    pub title: Option<String>,
    pub created_at: Option<DateTime>,
}

impl SessionMeta {
    pub fn created_at_rfc3339(&self) -> Option<String> {
        self.created_at.and_then(|d| d.try_to_rfc3339_string().ok())
    }
}

/// Tên và lúc bắt đầu của các phiên `keys` của user: một truy vấn bản ghi của đúng các phiên đó, một truy
/// vấn tiêu đề trong `transcript_docs`, đều chỉ đọc trường cần.
pub(crate) async fn session_meta(db: &Database, user_id: &str, keys: &HashSet<String>) -> Result<HashMap<String, SessionMeta>, AppError> {
    let mut out: HashMap<String, SessionMeta> = HashMap::new();
    if keys.is_empty() {
        return Ok(out);
    }
    let wanted: Vec<String> = keys.iter().cloned().collect();
    for (key, name, created_at) in file::session_heads(db, user_id, &wanted).await? {
        let meta = out.entry(key.clone()).or_default();
        meta.created_at = Some(meta.created_at.map_or(created_at, |c| c.min(created_at)));
        if key.starts_with("f:") {
            meta.title = Some(name);
        }
    }
    for (key, title) in transcript_doc::titles(db, user_id, &wanted).await? {
        out.entry(key).or_default().title = Some(title);
    }
    Ok(out)
}

/// Số lần câu (token đã chuẩn hoá) chứa cụm `words` liền nhau, từ cuối khớp theo tiền tố.
fn phrase_hits(tokens: &[&str], words: &[&str]) -> usize {
    let Some((last, rest)) = words.split_last() else { return 0 };
    if tokens.len() < words.len() {
        return 0;
    }
    (0..=tokens.len() - words.len())
        .filter(|&i| tokens[i..i + rest.len()] == *rest && tokens[i + rest.len()].starts_with(last))
        .count()
}

/// Câu có đủ các từ (thứ tự bất kỳ, từ cuối theo tiền tố): số token khớp; thiếu từ nào thì `None`.
fn word_hits(tokens: &[&str], words: &[&str]) -> Option<usize> {
    let (last, rest) = words.split_last()?;
    if !rest.iter().all(|w| tokens.contains(w)) || !tokens.iter().any(|t| t.starts_with(last)) {
        return None;
    }
    Some(tokens.iter().filter(|t| rest.contains(t) || t.starts_with(last)).count())
}

/// Một câu khớp, kèm điểm: số lần khớp cả cụm (hoặc số token khớp khi đã nới ra).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hit {
    pub row: Row,
    pub score: usize,
}

/// Giữ câu khớp cả cụm; không có câu nào thì nới ra câu chứa đủ các từ.
fn rank(candidates: Vec<Row>, words: &[&str]) -> Vec<Hit> {
    let mut seen = HashSet::new();
    let unique: Vec<Row> = candidates.into_iter().filter(|r| seen.insert((r.key.clone(), r.idx))).collect();
    let phrase: Vec<Hit> = unique
        .iter()
        .filter_map(|r| {
            let score = phrase_hits(&norm::tokens(&r.norm), words);
            (score > 0).then(|| Hit { row: r.clone(), score })
        })
        .collect();
    if !phrase.is_empty() {
        return phrase;
    }
    unique
        .into_iter()
        .filter_map(|r| word_hits(&norm::tokens(&r.norm), words).map(|score| Hit { row: r, score }))
        .collect()
}

/// Liên quan hơn trước; cùng điểm thì phiên mới hơn trước, trong phiên theo thời gian.
fn order(hits: &mut [Hit], meta: &HashMap<String, SessionMeta>) {
    let started = |key: &str| meta.get(key).and_then(|m| m.created_at).map_or(0, |d| d.timestamp_millis());
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| started(&b.row.key).cmp(&started(&a.row.key)))
            .then_with(|| a.row.key.cmp(&b.row.key))
            .then_with(|| a.row.at.partial_cmp(&b.row.at).unwrap_or(Ordering::Equal))
    });
}

/// Câu khớp `query` của user (cả kho hoặc một phiên), đã xếp hạng. Câu tìm rỗng sau khi chuẩn hoá thì rỗng.
pub(crate) async fn find(db: &Database, user_id: &str, key: Option<&str>, query: &str) -> Result<Vec<Hit>, AppError> {
    let normalized = norm::normalize(query);
    let words = norm::tokens(&normalized);
    let candidates = match words.as_slice() {
        [] => return Ok(Vec::new()),
        // Tiền tố một ký tự khớp gần như mọi câu: quét nặng mà kết quả vô nghĩa.
        [only] if only.chars().count() < MIN_PREFIX_CHARS => return Ok(Vec::new()),
        [only] => sentence::prefix_matches(db, user_id, key, only, CANDIDATES).await?,
        [done @ .., _typing] => {
            let terms: Vec<String> = done.iter().map(|w| w.to_string()).collect();
            sentence::text_matches(db, user_id, key, &terms, Order::Newest, CANDIDATES).await?
        }
    };
    Ok(rank(candidates, &words))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
    limit: Option<usize>,
    key: Option<String>,
}

/// `GET /api/search?q=&limit=&key=` — câu khớp trong các bản chép lời của mình, tối đa 50, kèm câu ngay
/// trước và ngay sau. `total` = số câu khớp (trong phạm vi ứng viên đã lấy).
pub async fn search(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Value>, AppError> {
    let query = q.q.as_deref().map(str::trim).unwrap_or_default();
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(AppError::BadRequest(format!("câu tìm tối đa {MAX_QUERY_CHARS} ký tự")));
    }
    let key = q.key.as_deref().filter(|k| !k.is_empty());
    if let Some(key) = key {
        check_key(key)?;
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    if !state.rate.allow(&format!("search:{}", user.0), PER_MINUTE) {
        return Err(AppError::TooManyRequests("tìm quá nhanh, thử lại sau ít giây".into()));
    }
    let mut hits = find(&state.db, &user.0, key, query).await?;
    let total = hits.len();
    let keys: HashSet<String> = hits.iter().map(|h| h.row.key.clone()).collect();
    let meta = session_meta(&state.db, &user.0, &keys).await?;
    order(&mut hits, &meta);
    hits.truncate(limit);

    let spans: Vec<(String, i64, i64)> = hits.iter().map(|h| (h.row.key.clone(), h.row.idx - 1, h.row.idx + 1)).collect();
    let near: HashMap<(String, i64), String> = sentence::ranges(&state.db, &user.0, &spans)
        .await?
        .into_iter()
        .map(|r| ((r.key, r.idx), r.text))
        .collect();
    let results: Vec<Value> = hits
        .iter()
        .map(|h| {
            let m = meta.get(&h.row.key).cloned().unwrap_or_default();
            let at = |idx: i64| near.get(&(h.row.key.clone(), idx)).cloned();
            json!({
                "key": h.row.key,
                "title": m.title,
                "createdAt": m.created_at_rfc3339(),
                "fileId": h.row.file_id.to_hex(),
                "at": h.row.at,
                "text": h.row.text,
                "before": at(h.row.idx - 1),
                "after": at(h.row.idx + 1),
            })
        })
        .collect();
    Ok(Json(json!({ "results": results, "total": total })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    fn row(key: &str, idx: i64, at: f64, text: &str) -> Row {
        Row { key: key.into(), file_id: ObjectId::new(), idx, at, text: text.into(), norm: norm::normalize(text) }
    }

    #[test]
    fn phrase_requires_adjacent_words_and_prefix_on_the_last() {
        let t = norm::tokens("ky hop dong thue nha hop dong moi");
        assert_eq!(phrase_hits(&t, &["hop", "dong"]), 2);
        assert_eq!(phrase_hits(&t, &["hop", "do"]), 2, "từ cuối đang gõ dở");
        assert_eq!(phrase_hits(&t, &["dong", "hop"]), 0);
        assert_eq!(phrase_hits(&t, &["thue"]), 1);
        assert_eq!(phrase_hits(&t, &["th"]), 1);
        assert_eq!(phrase_hits(&t, &[]), 0);
        assert_eq!(phrase_hits(&["a"], &["a", "b"]), 0);
        assert_eq!(word_hits(&t, &["nha", "ky"]), Some(2));
        assert_eq!(word_hits(&t, &["nha", "gia"]), None);
        assert_eq!(word_hits(&t, &["ho", "nh"]), None, "từ đã gõ xong phải khớp nguyên token");
    }

    #[test]
    fn rank_prefers_phrases_then_relaxes_to_all_words() {
        let rows = vec![
            row("f:a", 0, 1.0, "Hợp đồng thuê nhà."),
            row("f:a", 1, 4.0, "Thuê nhà theo hợp đồng."),
            row("f:a", 1, 4.0, "Thuê nhà theo hợp đồng."),
            row("f:b", 0, 2.0, "Chỉ có nhà."),
        ];
        let hits = rank(rows.clone(), &["hop", "dong"]);
        assert_eq!(hits.len(), 2, "trùng (key, idx) bị bỏ");
        let relaxed = rank(rows, &["nha", "hop"]);
        assert_eq!(relaxed.iter().map(|h| h.row.idx).collect::<Vec<_>>(), vec![0, 1]);
        assert!(rank(vec![row("f:a", 0, 0.0, "Không liên quan.")], &["hop", "dong"]).is_empty());
    }

    #[test]
    fn order_is_relevance_then_newer_session_then_time() {
        let meta = HashMap::from([
            ("f:old".to_string(), SessionMeta { title: None, created_at: Some(DateTime::from_millis(1_000)) }),
            ("f:new".to_string(), SessionMeta { title: None, created_at: Some(DateTime::from_millis(9_000)) }),
        ]);
        let hit = |key: &str, at: f64, score: usize| Hit { row: row(key, at as i64, at, "x"), score };
        let mut hits = vec![hit("f:old", 5.0, 1), hit("f:new", 9.0, 1), hit("f:new", 3.0, 1), hit("f:old", 1.0, 2)];
        order(&mut hits, &meta);
        let got: Vec<(&str, f64)> = hits.iter().map(|h| (h.row.key.as_str(), h.row.at)).collect();
        assert_eq!(got, vec![("f:old", 1.0), ("f:new", 3.0), ("f:new", 9.0), ("f:old", 5.0)]);
    }
}
