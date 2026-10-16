//! `transcript_sentences` collection: index tìm kiếm, mỗi câu hiệu lực của một phiên một doc
//! `{userId, key, fileId, idx, at, text, norm, createdAt, gen, expiresAt}`.
//!
//! - Câu đã áp phần người dùng sửa; `at` là giây tính theo cả phiên (cùng mốc với màn hình đọc và biên
//!   bản); `idx` là thứ tự câu trong phiên (0, 1, …) để lấy câu trước/sau.
//! - `norm` = chữ đã chuẩn hoá (`search::norm::normalize`): viết thường, bỏ dấu, đ thành d. Text index
//!   trên `norm` với `default_language: "none"` (không stemming, không stopword), tiền tố `userId` nên mọi
//!   truy vấn `$text` phải lọc đúng một người.
//! - `createdAt` = lúc bắt đầu phiên: tìm kiếm lấy ứng viên từ phiên mới nhất trước khi chạm trần.
//! - `gen` = thế hệ của lần dựng: [`replace_session`] ghi thế hệ mới trước rồi mới xoá thế hệ cũ, nên
//!   không có lúc nào phiên mất hết câu; phiên còn hơn một thế hệ (bị ngắt giữa chừng) được đối soát
//!   định kỳ dựng lại (`search::index::reconcile`). Người đọc bỏ câu trùng `(key, idx)`.
//! - `expiresAt` = hạn của file chứa câu: TTL dọn câu của file hết hạn kể cả khi bước dựng lại sau lúc xoá
//!   file bị lỡ. Ở đây TTL native là đúng (khác `files`): index không có file đĩa đi kèm để mồ côi.
//!
//! Doc được thay theo cả phiên ([`replace_session`], gọi từ `search::index`); xoá transcript hoặc xoá file
//! thì xoá ngay câu liên quan ([`delete_session`], [`delete_file`]), câu mồ côi còn sót được quét định kỳ
//! ([`delete_orphans`]).

use crate::error::AppError;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::collections::{HashMap, HashSet};
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

/// Index theo phiên (thay cả phiên, đọc câu trước/sau), theo phiên mới nhất (ứng viên tìm theo tiền tố),
/// theo file (xoá file, quét mồ côi), text index trên `norm`, TTL theo hạn file.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Document>(COLL);
    coll.create_index(IndexModel::builder().keys(doc! { "userId": 1, "key": 1, "idx": 1 }).build())
        .await
        .map_err(|e| format!("index transcript_sentences.userId_key: {e}"))?;
    coll.create_index(IndexModel::builder().keys(doc! { "userId": 1, "createdAt": -1 }).build())
        .await
        .map_err(|e| format!("index transcript_sentences.userId_createdAt: {e}"))?;
    coll.create_index(IndexModel::builder().keys(doc! { "fileId": 1 }).build())
        .await
        .map_err(|e| format!("index transcript_sentences.fileId: {e}"))?;
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

/// Thay toàn bộ câu của phiên bằng `sentences` (theo thứ tự), `started` = lúc bắt đầu phiên. Trả số câu
/// đã ghi.
///
/// Ghi thế hệ mới trước, xoá thế hệ cũ sau: trong khoảng giữa người đọc thấy cả hai (bỏ trùng theo
/// `(key, idx)`), không bao giờ thấy phiên rỗng. Ghi lỗi giữa chừng thì gỡ phần thế hệ mới đã ghi và
/// giữ thế hệ cũ; tiến trình chết giữa chừng thì đối soát định kỳ dựng lại phiên còn hai thế hệ.
pub async fn replace_session(
    db: &Database,
    user_id: &str,
    key: &str,
    started: DateTime,
    sentences: &[NewSentence],
) -> Result<usize, AppError> {
    let coll = db.collection::<Document>(COLL);
    let gen = ObjectId::new();
    let written = if sentences.is_empty() {
        0
    } else {
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
                    "createdAt": started,
                    "gen": gen,
                    "expiresAt": s.expires_at,
                }
            })
            .collect();
        match coll.insert_many(docs).await {
            Ok(r) => r.inserted_ids.len(),
            Err(e) => {
                if let Err(undo) = coll.delete_many(doc! { "userId": user_id, "key": key, "gen": gen }).await {
                    tracing::warn!(key = %key, "gỡ câu ghi dở của index lỗi (đối soát sẽ dựng lại): {undo}");
                }
                return Err(AppError::Internal(format!("ghi câu vào index: {e}")));
            }
        }
    };
    coll.delete_many(doc! { "userId": user_id, "key": key, "gen": { "$ne": gen } })
        .await
        .map_err(|e| AppError::Internal(format!("xoá câu cũ của index: {e}")))?;
    Ok(written)
}

/// Xoá mọi câu của phiên (xoá transcript). Trả số câu đã xoá.
pub async fn delete_session(db: &Database, user_id: &str, key: &str) -> Result<u64, AppError> {
    db.collection::<Document>(COLL)
        .delete_many(doc! { "userId": user_id, "key": key })
        .await
        .map(|r| r.deleted_count)
        .map_err(|e| AppError::Internal(format!("xoá câu của phiên khỏi index: {e}")))
}

/// Xoá câu của một file (xoá đoạn hoặc bản ghi). Trả số câu đã xoá.
pub async fn delete_file(db: &Database, user_id: &str, file_id: ObjectId) -> Result<u64, AppError> {
    db.collection::<Document>(COLL)
        .delete_many(doc! { "userId": user_id, "fileId": file_id })
        .await
        .map(|r| r.deleted_count)
        .map_err(|e| AppError::Internal(format!("xoá câu của file khỏi index: {e}")))
}

/// Trong `ids`, các id có doc trong `coll` (một truy vấn, chỉ đọc `_id`).
async fn existing_ids(db: &Database, coll: &str, ids: &[ObjectId]) -> Result<HashSet<ObjectId>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc {coll} để quét câu mồ côi: {e}"));
    let mut cur = db
        .collection::<Document>(coll)
        .find(doc! { "_id": { "$in": ids } })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(read)?;
    let mut out = HashSet::new();
    while let Some(r) = cur.next().await {
        out.extend(r.map_err(read)?.get_object_id("_id").ok());
    }
    Ok(out)
}

/// Xoá câu của file đã mất doc file hoặc mất transcript (bước xoá ngay lúc xoá file hay xoá transcript bị
/// lỗi, hoặc tiến trình dừng giữa chừng). Doc transcript có `_id` = id file. Trả số câu đã xoá.
pub async fn delete_orphans(db: &Database) -> Result<u64, AppError> {
    let coll = db.collection::<Document>(COLL);
    let ids: Vec<ObjectId> = coll
        .distinct("fileId", doc! {})
        .await
        .map_err(|e| AppError::Internal(format!("quét file trong index: {e}")))?
        .into_iter()
        .filter_map(|b| b.as_object_id())
        .collect();
    let mut orphans: Vec<ObjectId> = Vec::new();
    for batch in ids.chunks(500) {
        let files = existing_ids(db, "files", batch).await?;
        let transcripts = existing_ids(db, "transcripts", batch).await?;
        orphans.extend(batch.iter().filter(|id| !files.contains(*id) || !transcripts.contains(*id)));
    }
    if orphans.is_empty() {
        return Ok(0);
    }
    coll.delete_many(doc! { "fileId": { "$in": orphans } })
        .await
        .map(|r| r.deleted_count)
        .map_err(|e| AppError::Internal(format!("xoá câu mồ côi của index: {e}")))
}

/// Các phiên `(userId, key)` đang có câu trong index, kèm việc phiên có đúng một thế hệ hay không
/// (`false`: bị ngắt giữa lúc thay, hoặc doc từ trước khi có `gen`). Quét cả collection (đối soát định kỳ).
pub async fn indexed_sessions(db: &Database) -> Result<HashMap<(String, String), bool>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("liệt kê phiên trong index: {e}"));
    let pipeline = vec![doc! { "$group": {
        "_id": { "u": "$userId", "k": "$key" },
        "gens": { "$addToSet": { "$ifNull": ["$gen", null] } },
    } }];
    let mut cur = db.collection::<Document>(COLL).aggregate(pipeline).allow_disk_use(true).await.map_err(read)?;
    let mut out = HashMap::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(read)?;
        let Ok(id) = d.get_document("_id") else { continue };
        let (Ok(user_id), Ok(key)) = (id.get_str("u"), id.get_str("k")) else { continue };
        let gens = d.get_array("gens").map(|g| g.as_slice()).unwrap_or_default();
        let single = gens.len() == 1 && gens[0].as_object_id().is_some();
        out.insert((user_id.to_string(), key.to_string()), single);
    }
    Ok(out)
}

fn scope(user_id: &str, key: Option<&str>) -> Document {
    let mut filter = doc! { "userId": user_id };
    if let Some(key) = key {
        filter.insert("key", key);
    }
    filter
}

/// Thứ tự lấy câu khớp trước khi chạm `limit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Điểm text cao trước (hỏi đáp: vài chục câu liên quan nhất).
    Relevance,
    /// Phiên mới nhất trước (tìm kiếm: xếp hạng làm trong code, trần ứng viên không được bỏ sót phiên mới).
    Newest,
}

/// Câu có ít nhất một trong `terms` (token đã chuẩn hoá), theo `order`, tối đa `limit`.
pub async fn text_matches(
    db: &Database,
    user_id: &str,
    key: Option<&str>,
    terms: &[String],
    order: Order,
    limit: i64,
) -> Result<Vec<Row>, AppError> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let mut filter = scope(user_id, key);
    filter.insert("$text", doc! { "$search": terms.join(" ") });
    let mut proj = projection();
    let sort = match order {
        Order::Relevance => {
            proj.insert("score", doc! { "$meta": "textScore" });
            doc! { "score": { "$meta": "textScore" } }
        }
        Order::Newest => doc! { "createdAt": -1 },
    };
    let cur = db.collection::<Document>(COLL).find(filter).projection(proj).sort(sort).limit(limit).await;
    collect(cur, "tìm câu theo từ").await
}

/// Câu có một token bắt đầu bằng `prefix` (một token đã chuẩn hoá: chỉ gồm chữ và số, không có ký tự đặc
/// biệt của regex), phiên mới nhất trước, tối đa `limit`. Không dùng được text index (text index chỉ khớp
/// nguyên token), nên chỉ dùng khi câu tìm có một từ; quét câu của một người theo index
/// `{userId, createdAt}` từ phiên mới nhất và dừng khi đủ `limit`.
pub async fn prefix_matches(db: &Database, user_id: &str, key: Option<&str>, prefix: &str, limit: i64) -> Result<Vec<Row>, AppError> {
    if prefix.is_empty() || !prefix.chars().all(char::is_alphanumeric) {
        return Ok(Vec::new());
    }
    let mut filter = scope(user_id, key);
    filter.insert("norm", doc! { "$regex": format!("(^| ){prefix}") });
    let cur = db
        .collection::<Document>(COLL)
        .find(filter)
        .projection(projection())
        .sort(doc! { "createdAt": -1 })
        .limit(limit)
        .await;
    collect(cur, "tìm câu theo tiền tố").await
}

/// Mọi câu của một phiên theo thứ tự (lúc đang thay thế hệ: mỗi `idx` một câu).
pub async fn session_rows(db: &Database, user_id: &str, key: &str) -> Result<Vec<Row>, AppError> {
    let cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "key": key })
        .projection(projection())
        .sort(doc! { "idx": 1 })
        .await;
    let mut rows = collect(cur, "đọc câu của phiên").await?;
    rows.dedup_by_key(|r| r.idx);
    Ok(rows)
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
        let started = DateTime::from_millis(1_000_000);
        let s = |at: f64, text: &str| NewSentence { file_id: file, at, text: text.into(), expires_at: expires };
        let first = [s(0.0, "Xin chào mọi người."), s(3.5, "Hôm nay bàn về Hợp đồng thuê nhà."), s(9.0, "Đồng ý ký hợp đồng.")];
        assert_eq!(replace_session(&db, &user, &key, started, &first).await.expect("ghi"), 3);
        // Thay cả phiên: câu cũ biến mất, còn đúng một thế hệ.
        let second = [s(0.0, "Xin chào."), s(2.0, "Bàn về hợp đồng thuê nhà."), s(5.0, "Đồng ý ký hợp đồng."), s(8.0, "Hẹn thứ Sáu.")];
        assert_eq!(replace_session(&db, &user, &key, started, &second).await.expect("thay"), 4);
        let rows = session_rows(&db, &user, &key).await.expect("đọc");
        assert_eq!(rows.iter().map(|r| r.idx).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        assert_eq!(rows[1].norm, "ban ve hop dong thue nha");
        let sessions = indexed_sessions(&db).await.expect("liệt kê");
        assert_eq!(sessions.get(&(user.clone(), key.clone())), Some(&true));

        let hits = text_matches(&db, &user, None, &["hop".into()], Order::Relevance, 10).await.expect("text");
        assert_eq!(hits.len(), 2);
        assert_eq!(text_matches(&db, &user, None, &["hop".into()], Order::Newest, 10).await.expect("text").len(), 2);
        assert!(text_matches(&db, "người-khác", None, &["hop".into()], Order::Relevance, 10).await.expect("text").is_empty());
        let pre = prefix_matches(&db, &user, Some(&key), "thu", 10).await.expect("prefix");
        assert_eq!(pre.iter().map(|r| r.idx).collect::<Vec<_>>(), vec![1, 3]);
        let near = ranges(&db, &user, &[(key.clone(), 1, 2)]).await.expect("ranges");
        assert_eq!(near.len(), 2);

        // Phiên mới hơn đứng trước khi lấy ứng viên.
        let newer = format!("f:{}", ObjectId::new().to_hex());
        replace_session(&db, &user, &newer, DateTime::from_millis(2_000_000), &[s(1.0, "Thuê xe.")]).await.expect("ghi");
        let pre = prefix_matches(&db, &user, None, "thu", 1).await.expect("prefix");
        assert_eq!(pre.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), vec![newer.as_str()]);
        assert_eq!(delete_session(&db, &user, &newer).await.expect("xoá phiên"), 1);

        // Doc từ trước khi có `gen` (hoặc thế hệ dở): đối soát nhận ra để dựng lại.
        db.collection::<Document>(COLL)
            .update_one(doc! { "userId": &user, "key": &key, "idx": 0 }, doc! { "$unset": { "gen": "" } })
            .await
            .expect("bỏ gen");
        assert_eq!(indexed_sessions(&db).await.expect("liệt kê").get(&(user.clone(), key.clone())), Some(&false));

        // File không còn doc file hay transcript: quét mồ côi xoá câu của nó.
        assert!(delete_orphans(&db).await.expect("quét") >= 4);
        assert!(session_rows(&db, &user, &key).await.expect("đọc").is_empty());

        replace_session(&db, &user, &key, started, &second).await.expect("ghi lại");
        assert_eq!(delete_file(&db, &user, file).await.expect("xoá file"), 4);
        assert_eq!(replace_session(&db, &user, &key, started, &[]).await.expect("xoá"), 0);
        assert!(session_rows(&db, &user, &key).await.expect("đọc").is_empty());
    }
}
