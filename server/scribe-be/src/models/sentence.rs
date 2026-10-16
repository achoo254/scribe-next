//! `transcript_sentences` collection: index tìm kiếm, mỗi câu hiệu lực của một phiên một doc
//! `{userId, key, fileId, idx, at, text, norm, expiresAt}`.
//!
//! - Câu đã áp phần người dùng sửa; `at` là giây tính theo cả phiên (cùng mốc với màn hình đọc và biên
//!   bản); `idx` là thứ tự câu trong phiên (0, 1, …) để lấy câu trước/sau.
//! - `norm` = chữ đã chuẩn hoá (`search::norm::normalize`): viết thường, bỏ dấu, đ thành d. Text index
//!   trên `norm` với `default_language: "none"` (không stemming, không stopword), tiền tố `userId` nên mọi
//!   truy vấn `$text` phải lọc đúng một người.
//! - `expiresAt` = hạn của file chứa câu: TTL dọn câu của file hết hạn kể cả khi bước dựng lại sau lúc xoá
//!   file bị lỡ. Ở đây TTL native là đúng (khác `files`): index không có file đĩa đi kèm để mồ côi.
//!
//! Doc chỉ được thay theo cả phiên ([`replace_session`], gọi từ `search::index::reindex`).

use crate::error::AppError;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::time::Duration;

const COLL: &str = "transcript_sentences";

/// Một câu sắp ghi vào index (đã có `idx` theo thứ tự trong phiên).
#[derive(Debug, Clone, PartialEq)]
pub struct NewSentence {
    pub file_id: ObjectId,
    pub at: f64,
    pub text: String,
    pub expires_at: DateTime,
}

/// Một câu đọc từ index.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub file_id: ObjectId,
    pub idx: i64,
    pub at: f64,
    pub text: String,
    pub norm: String,
}

fn read_i64(d: &Document, field: &str) -> i64 {
    d.get_i64(field).ok().or_else(|| d.get_i32(field).ok().map(i64::from)).unwrap_or(0)
}

fn row_of(d: &Document) -> Option<Row> {
    Some(Row {
        key: d.get_str("key").ok()?.to_string(),
        file_id: d.get_object_id("fileId").ok()?,
        idx: read_i64(d, "idx"),
        at: d.get_f64("at").unwrap_or(0.0),
        text: d.get_str("text").ok()?.to_string(),
        norm: d.get_str("norm").unwrap_or_default().to_string(),
    })
}

const ROW_FIELDS: [&str; 6] = ["key", "fileId", "idx", "at", "text", "norm"];

fn projection() -> Document {
    let mut p = Document::new();
    for f in ROW_FIELDS {
        p.insert(f, 1);
    }
    p.insert("_id", 0);
    p
}

async fn collect(cur: mongodb::error::Result<mongodb::Cursor<Document>>, what: &str) -> Result<Vec<Row>, AppError> {
    let mut cur = cur.map_err(|e| AppError::Internal(format!("{what}: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("{what}: {e}")))?;
        out.extend(row_of(&d));
    }
    Ok(out)
}

/// Index theo phiên (thay cả phiên, đọc câu trước/sau), text index trên `norm`, TTL theo hạn file.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Document>(COLL);
    coll.create_index(IndexModel::builder().keys(doc! { "userId": 1, "key": 1, "idx": 1 }).build())
        .await
        .map_err(|e| format!("index transcript_sentences.userId_key: {e}"))?;
    let text = IndexModel::builder()
        .keys(doc! { "userId": 1, "norm": "text" })
        .options(IndexOptions::builder().default_language("none".to_string()).name("userId_norm_text".to_string()).build())
        .build();
    coll.create_index(text).await.map_err(|e| format!("index transcript_sentences.norm (text): {e}"))?;
    let ttl = IndexModel::builder()
        .keys(doc! { "expiresAt": 1 })
        .options(IndexOptions::builder().expire_after(Duration::from_secs(0)).build())
        .build();
    coll.create_index(ttl).await.map_err(|e| format!("index transcript_sentences.expiresAt (TTL): {e}"))?;
    Ok(())
}

/// Thay toàn bộ câu của phiên bằng `sentences` (theo thứ tự). Trả số câu đã ghi.
pub async fn replace_session(db: &Database, user_id: &str, key: &str, sentences: &[NewSentence]) -> Result<usize, AppError> {
    let coll = db.collection::<Document>(COLL);
    coll.delete_many(doc! { "userId": user_id, "key": key })
        .await
        .map_err(|e| AppError::Internal(format!("xoá câu cũ của index: {e}")))?;
    if sentences.is_empty() {
        return Ok(0);
    }
    let docs: Vec<Document> = sentences
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            doc! {
                "userId": user_id,
                "key": key,
                "fileId": s.file_id,
                "idx": idx as i64,
                "at": s.at,
                "text": &s.text,
                "norm": crate::search::norm::normalize(&s.text),
                "expiresAt": s.expires_at,
            }
        })
        .collect();
    coll.insert_many(docs)
        .await
        .map(|r| r.inserted_ids.len())
        .map_err(|e| AppError::Internal(format!("ghi câu vào index: {e}")))
}

fn scope(user_id: &str, key: Option<&str>) -> Document {
    let mut filter = doc! { "userId": user_id };
    if let Some(key) = key {
        filter.insert("key", key);
    }
    filter
}

/// Câu có ít nhất một trong `terms` (token đã chuẩn hoá), điểm text cao trước, tối đa `limit`.
pub async fn text_matches(db: &Database, user_id: &str, key: Option<&str>, terms: &[String], limit: i64) -> Result<Vec<Row>, AppError> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let mut filter = scope(user_id, key);
    filter.insert("$text", doc! { "$search": terms.join(" ") });
    let mut proj = projection();
    proj.insert("score", doc! { "$meta": "textScore" });
    let cur = db
        .collection::<Document>(COLL)
        .find(filter)
        .projection(proj)
        .sort(doc! { "score": { "$meta": "textScore" } })
        .limit(limit)
        .await;
    collect(cur, "tìm câu theo từ").await
}

/// Câu có một token bắt đầu bằng `prefix` (một token đã chuẩn hoá: chỉ gồm chữ và số, không có ký tự đặc
/// biệt của regex), tối đa `limit`. Không dùng được text index (text index chỉ khớp nguyên token), nên chỉ
/// dùng khi câu tìm có một từ; quét trong phạm vi câu của một người qua index `{userId, key}`.
pub async fn prefix_matches(db: &Database, user_id: &str, key: Option<&str>, prefix: &str, limit: i64) -> Result<Vec<Row>, AppError> {
    if prefix.is_empty() || !prefix.chars().all(char::is_alphanumeric) {
        return Ok(Vec::new());
    }
    let mut filter = scope(user_id, key);
    filter.insert("norm", doc! { "$regex": format!("(^| ){prefix}") });
    let cur = db.collection::<Document>(COLL).find(filter).projection(projection()).limit(limit).await;
    collect(cur, "tìm câu theo tiền tố").await
}

/// Mọi câu của một phiên theo thứ tự.
pub async fn session_rows(db: &Database, user_id: &str, key: &str) -> Result<Vec<Row>, AppError> {
    let cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "key": key })
        .projection(projection())
        .sort(doc! { "idx": 1 })
        .await;
    collect(cur, "đọc câu của phiên").await
}

/// Câu theo các khoảng `(key, idx đầu, idx cuối)` (gồm cả hai đầu), một truy vấn.
pub async fn ranges(db: &Database, user_id: &str, spans: &[(String, i64, i64)]) -> Result<Vec<Row>, AppError> {
    if spans.is_empty() {
        return Ok(Vec::new());
    }
    let or: Vec<Document> = spans
        .iter()
        .map(|(key, from, to)| doc! { "key": key, "idx": { "$gte": from, "$lte": to } })
        .collect();
    let cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "$or": or })
        .projection(projection())
        .await;
    collect(cur, "đọc câu lân cận").await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// sentence_index -- --ignored`. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn sentence_index_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());
        let key = format!("f:{}", ObjectId::new().to_hex());
        let file = ObjectId::new();
        let expires = DateTime::from_millis(DateTime::now().timestamp_millis() + 3_600_000);
        let s = |at: f64, text: &str| NewSentence { file_id: file, at, text: text.into(), expires_at: expires };
        let first = [s(0.0, "Xin chào mọi người."), s(3.5, "Hôm nay bàn về Hợp đồng thuê nhà."), s(9.0, "Đồng ý ký hợp đồng.")];
        assert_eq!(replace_session(&db, &user, &key, &first).await.expect("ghi"), 3);
        // Thay cả phiên: câu cũ biến mất.
        let second = [s(0.0, "Xin chào."), s(2.0, "Bàn về hợp đồng thuê nhà."), s(5.0, "Đồng ý ký hợp đồng."), s(8.0, "Hẹn thứ Sáu.")];
        assert_eq!(replace_session(&db, &user, &key, &second).await.expect("thay"), 4);
        let rows = session_rows(&db, &user, &key).await.expect("đọc");
        assert_eq!(rows.iter().map(|r| r.idx).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(rows[1].norm, "ban ve hop dong thue nha");

        let hits = text_matches(&db, &user, None, &["hop".into()], 10).await.expect("text");
        assert_eq!(hits.len(), 2);
        assert!(text_matches(&db, "người-khác", None, &["hop".into()], 10).await.expect("text").is_empty());
        let pre = prefix_matches(&db, &user, Some(&key), "thu", 10).await.expect("prefix");
        assert_eq!(pre.iter().map(|r| r.idx).collect::<Vec<_>>(), vec![1, 3]);
        let near = ranges(&db, &user, &[(key.clone(), 1, 2)]).await.expect("ranges");
        assert_eq!(near.len(), 2);

        assert_eq!(replace_session(&db, &user, &key, &[]).await.expect("xoá"), 0);
        assert!(session_rows(&db, &user, &key).await.expect("đọc").is_empty());
    }
}
