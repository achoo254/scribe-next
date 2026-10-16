//! `transcript_docs` collection: phần người dùng tạo ra cho một transcript (tiêu đề, câu đã sửa),
//! một doc cho mỗi phiên ghi. Kết quả máy vẫn ở `transcripts` (mỗi file một doc).
//!
//! `key` là khoá phiên (`file::session_key`): `s:<sessionId>` cho phiên iPhone nhiều đoạn,
//! `f:<fileId>` cho bản ghi một file. Doc tạo lười khi người dùng đổi tên hoặc sửa lần đầu.
//!
//! Doc còn giữ biên bản (`summary`, xem `crate::summary`) và dấu mốc (`bookmarks`). Dấu mốc được đánh
//! lúc đang ghi, khi phiên có thể chưa có file nào: doc tạo lười bằng upsert, sweep mồ côi chừa doc
//! còn mới ([`ORPHAN_GRACE_HOURS`]) nếu doc chưa có biên bản. Biên bản viết từ audio của phiên nên đi
//! theo audio: xoá một đoạn là bỏ biên bản ([`forget_segment`]), phiên hết file là xoá cả doc.
//!
//! Phần sửa của một đoạn gắn với `runId` của lượt chép đã sinh ra câu máy: chép lại thì `runId` đổi
//! và phần sửa cũ tự hết hiệu lực ([`effective_texts`]), không cần dọn đồng bộ.

use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const COLL: &str = "transcript_docs";

/// Câu đã sửa của một đoạn, kèm lượt chép mà các câu này thay thế.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentEdit {
    #[serde(rename = "runId")]
    pub run_id: String,
    pub texts: Vec<String>,
}

/// Một quyết định trong biên bản; `at` = giây tính từ đầu phiên.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub text: String,
    pub at: f64,
}

/// Một việc cần làm; người làm và hạn chỉ có khi bản ghi nói rõ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub text: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    pub at: f64,
}

/// Biên bản đã lưu. `sourceHash` = băm chữ đầu vào: chữ hiện tại khác thì biên bản đã cũ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSummary {
    pub model: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
    #[serde(rename = "sourceHash")]
    pub source_hash: String,
    pub summary: String,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// Dấu mốc người dùng bấm lúc ghi; `at` = giây tính từ đầu phiên.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub at: f64,
    #[serde(default)]
    pub label: Option<String>,
}

/// Số dấu mốc tối đa của một doc.
pub const MAX_BOOKMARKS: usize = 200;
/// Sweep mồ côi chừa doc tạo chưa tới chừng này giờ: dấu mốc của phiên đang ghi có trước file.
const ORPHAN_GRACE_HOURS: i64 = 24;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Theo id file (hex).
    #[serde(default)]
    pub edits: HashMap<String, SegmentEdit>,
    #[serde(default)]
    pub summary: Option<StoredSummary>,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
}

/// Câu sửa của đoạn còn dùng được: cùng lượt chép và cùng số câu với câu máy hiện tại.
pub fn effective_texts<'a>(edit: Option<&'a SegmentEdit>, run_id: &str, sentences: usize) -> Option<&'a [String]> {
    edit.filter(|e| e.run_id == run_id && e.texts.len() == sentences).map(|e| e.texts.as_slice())
}

/// Index unique `{userId, key}`: hai lần sửa đầu tiên chạy đua cũng chỉ tạo một doc.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let index = IndexModel::builder()
        .keys(doc! { "userId": 1, "key": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    db.collection::<Document>(COLL)
        .create_index(index)
        .await
        .map_err(|e| format!("index transcript_docs.userId_key: {e}"))?;
    Ok(())
}

pub async fn get_many(db: &Database, user_id: &str, keys: &[String]) -> Result<Vec<SessionDoc>, AppError> {
    let mut cur = db
        .collection::<SessionDoc>(COLL)
        .find(doc! { "userId": user_id, "key": { "$in": keys } })
        .await
        .map_err(|e| AppError::Internal(format!("đọc transcript_docs: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc transcript_docs: {e}")))?);
    }
    Ok(out)
}

/// Tiêu đề người dùng đặt của các phiên `keys` (phiên chưa đặt tên thì không có), chỉ đọc `key` và `title`.
pub async fn titles(db: &Database, user_id: &str, keys: &[String]) -> Result<HashMap<String, String>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc tiêu đề transcript_docs: {e}"));
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "key": { "$in": keys }, "title": { "$type": "string" } })
        .projection(doc! { "_id": 0, "key": 1, "title": 1 })
        .await
        .map_err(read)?;
    let mut out = HashMap::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(read)?;
        if let (Ok(key), Ok(title)) = (d.get_str("key"), d.get_str("title")) {
            out.insert(key.to_string(), title.to_string());
        }
    }
    Ok(out)
}

pub async fn get(db: &Database, user_id: &str, key: &str) -> Result<Option<SessionDoc>, AppError> {
    Ok(get_many(db, user_id, &[key.to_string()]).await?.into_iter().next())
}

/// Upsert theo `{userId, key}`; thua cuộc đua với unique index thì chạy lại (lần hai khớp doc vừa tạo).
async fn upsert(db: &Database, user_id: &str, key: &str, update: Document) -> Result<(), AppError> {
    let coll = db.collection::<Document>(COLL);
    let filter = doc! { "userId": user_id, "key": key };
    let mut update = update;
    update.insert("$setOnInsert", doc! { "createdAt": DateTime::now() });
    let first = coll.update_one(filter.clone(), update.clone()).upsert(true).await;
    match first {
        Ok(_) => Ok(()),
        Err(e) if is_duplicate_key(&e) => coll
            .update_one(filter, update)
            .upsert(true)
            .await
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("ghi transcript_docs (lần hai): {e}"))),
        Err(e) => Err(AppError::Internal(format!("ghi transcript_docs: {e}"))),
    }
}

/// Đặt tiêu đề; `None` = về tên mặc định (không tạo doc chỉ để xoá tên).
pub async fn set_title(db: &Database, user_id: &str, key: &str, title: Option<&str>) -> Result<(), AppError> {
    match title {
        Some(title) => upsert(db, user_id, key, doc! { "$set": { "title": title, "updatedAt": DateTime::now() } }).await,
        None => db
            .collection::<Document>(COLL)
            .update_one(
                doc! { "userId": user_id, "key": key },
                doc! { "$unset": { "title": "" }, "$set": { "updatedAt": DateTime::now() } },
            )
            .await
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("xoá tiêu đề: {e}"))),
    }
}

/// Ghi câu sửa của một đoạn. `file_id` đã là ObjectId nên `to_hex()` an toàn trong field path.
pub async fn set_edits(db: &Database, user_id: &str, key: &str, file_id: ObjectId, edit: &SegmentEdit) -> Result<(), AppError> {
    let path = format!("edits.{}", file_id.to_hex());
    let update = doc! {
        "$set": {
            path: { "runId": &edit.run_id, "texts": &edit.texts },
            "updatedAt": DateTime::now(),
        },
    };
    upsert(db, user_id, key, update).await
}

/// Lưu (hoặc thay) biên bản của phiên.
pub async fn set_summary(db: &Database, user_id: &str, key: &str, summary: &StoredSummary) -> Result<(), AppError> {
    let value = mongodb::bson::to_bson(summary).map_err(|e| AppError::Internal(format!("mã hoá biên bản: {e}")))?;
    upsert(db, user_id, key, doc! { "$set": { "summary": value, "updatedAt": DateTime::now() } }).await
}

/// Kết quả thêm một dấu mốc.
#[derive(Debug, Clone, PartialEq)]
pub enum BookmarkAdd {
    Added,
    /// Mốc cùng id đã có (client gửi lại sau khi mất trả lời): không thêm lần hai.
    Exists(Bookmark),
    /// Doc đã đủ [`MAX_BOOKMARKS`].
    Full,
}

/// Thêm một dấu mốc, idempotent theo `bookmark.id`. Điều kiện "chưa có phần tử thứ 200" và "chưa có mốc
/// cùng id" nằm trong filter nên hai lần thêm đồng thời không vượt trần cũng không nhân đôi; filter không
/// khớp doc đã có (đầy, hoặc trùng id) thì upsert đụng unique index.
pub async fn add_bookmark(db: &Database, user_id: &str, key: &str, bookmark: &Bookmark) -> Result<BookmarkAdd, AppError> {
    let coll = db.collection::<Document>(COLL);
    let last_slot = format!("bookmarks.{}", MAX_BOOKMARKS - 1);
    let filter = doc! {
        "userId": user_id,
        "key": key,
        last_slot: { "$exists": false },
        "bookmarks.id": { "$ne": &bookmark.id },
    };
    let value = mongodb::bson::to_bson(bookmark).map_err(|e| AppError::Internal(format!("mã hoá dấu mốc: {e}")))?;
    let update = doc! {
        "$push": { "bookmarks": value },
        "$set": { "updatedAt": DateTime::now() },
        "$setOnInsert": { "createdAt": DateTime::now() },
    };
    let added = match coll.update_one(filter.clone(), update.clone()).upsert(true).await {
        Ok(_) => true,
        // Doc đã có mà không khớp filter (đầy, trùng id), hoặc thua cuộc đua tạo doc: thử lại không upsert.
        Err(e) if is_duplicate_key(&e) => coll
            .update_one(filter, update)
            .await
            .map(|r| r.matched_count > 0)
            .map_err(|e| AppError::Internal(format!("thêm dấu mốc (lần hai): {e}")))?,
        Err(e) => return Err(AppError::Internal(format!("thêm dấu mốc: {e}"))),
    };
    if added {
        return Ok(BookmarkAdd::Added);
    }
    let existing = get(db, user_id, key).await?.and_then(|d| d.bookmarks.into_iter().find(|b| b.id == bookmark.id));
    Ok(existing.map_or(BookmarkAdd::Full, BookmarkAdd::Exists))
}

/// Xoá một dấu mốc; `false` khi không có.
pub async fn remove_bookmark(db: &Database, user_id: &str, key: &str, id: &str) -> Result<bool, AppError> {
    db.collection::<Document>(COLL)
        .update_one(
            doc! { "userId": user_id, "key": key, "bookmarks.id": id },
            doc! { "$pull": { "bookmarks": { "id": id } }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await
        .map(|r| r.modified_count > 0)
        .map_err(|e| AppError::Internal(format!("xoá dấu mốc: {e}")))
}

/// Chia dấu mốc của một bản ghi cho các phần cắt ra từ nó (`durations` = thời lượng từng phần, theo thứ
/// tự): mốc thuộc phần chứa nó (`đầu ≤ at < cuối`), `at` tính lại từ đầu phần đó; mốc sau phần cuối
/// (thời lượng làm tròn) về phần cuối. Không có phần nào thì trả rỗng.
pub fn split_bookmarks(bookmarks: &[Bookmark], durations: &[f64]) -> Vec<Vec<Bookmark>> {
    let mut out = vec![Vec::new(); durations.len()];
    let Some(last) = durations.len().checked_sub(1) else { return out };
    for bookmark in bookmarks {
        let mut start = 0.0;
        let mut piece = last;
        for (i, d) in durations.iter().enumerate() {
            if bookmark.at < start + d.max(0.0) {
                piece = i;
                break;
            }
            if i < last {
                start += d.max(0.0);
            }
        }
        let at = (((bookmark.at - start).max(0.0)) * 10.0).round() / 10.0;
        if let Some(list) = out.get_mut(piece) {
            list.push(Bookmark { at, ..bookmark.clone() });
        }
    }
    out
}

/// Bản ghi `from_key` vừa được cắt thành các phần `pieces` (`(khoá, thời lượng)` theo thứ tự): chép dấu
/// mốc của nó sang doc của từng phần trước khi doc gốc bị xoá theo file gốc.
pub async fn move_bookmarks_to_pieces(
    db: &Database,
    user_id: &str,
    from_key: &str,
    pieces: &[(String, f64)],
) -> Result<usize, AppError> {
    let Some(source) = get(db, user_id, from_key).await? else { return Ok(0) };
    if source.bookmarks.is_empty() {
        return Ok(0);
    }
    let durations: Vec<f64> = pieces.iter().map(|(_, d)| *d).collect();
    let mut moved = 0;
    for ((key, _), marks) in pieces.iter().zip(split_bookmarks(&source.bookmarks, &durations)) {
        if marks.is_empty() {
            continue;
        }
        let values = mongodb::bson::to_bson(&marks).map_err(|e| AppError::Internal(format!("mã hoá dấu mốc: {e}")))?;
        let update = doc! { "$push": { "bookmarks": { "$each": values } }, "$set": { "updatedAt": DateTime::now() } };
        upsert(db, user_id, key, update).await?;
        moved += marks.len();
    }
    Ok(moved)
}

/// Một đoạn của phiên đã bị xoá: bỏ câu sửa của đoạn và biên bản (viết cả từ đoạn đó); phiên không còn
/// file nào thì xoá cả doc (tiêu đề, dấu mốc đi theo bản ghi).
pub async fn forget_segment(db: &Database, user_id: &str, key: &str, file_id: ObjectId) -> Result<(), AppError> {
    let path = format!("edits.{}", file_id.to_hex());
    db.collection::<Document>(COLL)
        .update_one(
            doc! { "userId": user_id, "key": key },
            doc! { "$unset": { path: "", "summary": "" }, "$set": { "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("bỏ câu sửa và biên bản: {e}")))?;
    if super::file::list_session(db, user_id, key).await?.is_empty() {
        delete(db, user_id, key).await?;
    }
    Ok(())
}

/// Gỡ câu sửa của đoạn chỉ khi nó còn thuộc lượt `run_id` (không đụng phần sửa của lượt mới hơn).
pub async fn unset_edits_of_run(
    db: &Database,
    user_id: &str,
    key: &str,
    file_id: ObjectId,
    run_id: &str,
) -> Result<(), AppError> {
    let path = format!("edits.{}", file_id.to_hex());
    let run_path = format!("{path}.runId");
    db.collection::<Document>(COLL)
        .update_one(doc! { "userId": user_id, "key": key, run_path: run_id }, doc! { "$unset": { path: "" } })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("bỏ câu sửa: {e}")))
}

pub async fn delete(db: &Database, user_id: &str, key: &str) -> Result<(), AppError> {
    db.collection::<Document>(COLL)
        .delete_one(doc! { "userId": user_id, "key": key })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("xoá transcript_docs: {e}")))
}

/// Xoá doc không còn file bản ghi nào của phiên (audio đã bị xoá tay, hết hạn, hay bị cắt). Doc chưa có
/// biên bản và tạo chưa tới [`ORPHAN_GRACE_HOURS`] giờ được chừa: phiên đang ghi có dấu mốc trước khi có
/// file. Doc có biên bản thì phiên đã từng có audio, hết file là mồ côi ngay.
/// Đây là cơ chế đảm bảo; bước dọn trong `file::delete_doc` chỉ là tối ưu. Trả số doc đã xoá.
pub async fn delete_orphans(db: &Database) -> Result<u64, AppError> {
    let cutoff = DateTime::from_millis(DateTime::now().timestamp_millis() - ORPHAN_GRACE_HOURS * 3_600_000);
    let old_enough = doc! { "$or": [
        { "createdAt": { "$lt": cutoff } },
        { "createdAt": { "$exists": false } },
        { "summary": { "$exists": true } },
    ] };
    let mut cur = db
        .collection::<Document>(COLL)
        .find(old_enough)
        .projection(doc! { "userId": 1, "key": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("quét transcript_docs: {e}")))?;
    let mut orphans = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("quét transcript_docs: {e}")))?;
        let (Ok(id), Ok(user_id), Ok(key)) = (d.get_object_id("_id"), d.get_str("userId"), d.get_str("key")) else {
            continue;
        };
        if super::file::list_session(db, user_id, key).await?.is_empty() {
            orphans.push(id);
        }
    }
    if orphans.is_empty() {
        return Ok(0);
    }
    db.collection::<Document>(COLL)
        .delete_many(doc! { "_id": { "$in": orphans } })
        .await
        .map(|r| r.deleted_count)
        .map_err(|e| AppError::Internal(format!("xoá transcript_docs mồ côi: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_follow_the_piece_they_fall_in() {
        let mark = |id: &str, at: f64| Bookmark { id: id.into(), at, label: Some(id.into()) };
        let marks = [mark("a", 0.0), mark("b", 59.9), mark("c", 60.0), mark("d", 125.36), mark("e", 200.0)];
        let pieces = split_bookmarks(&marks, &[60.0, 60.0, 60.0]);
        assert_eq!(pieces[0], vec![mark("a", 0.0), mark("b", 59.9)]);
        assert_eq!(pieces[1], vec![mark("c", 0.0)]);
        // Sau phần cuối (thời lượng làm tròn): về phần cuối, tính từ đầu phần đó.
        assert_eq!(pieces[2], vec![mark("d", 5.4), mark("e", 80.0)]);
        assert!(split_bookmarks(&marks, &[]).is_empty());
        assert_eq!(split_bookmarks(&marks, &[30.0])[0].len(), marks.len());
    }

    #[test]
    fn edits_apply_only_to_the_same_run_and_sentence_count() {
        let edit = SegmentEdit { run_id: "run-1".into(), texts: vec!["a".into(), "b".into()] };
        assert_eq!(effective_texts(Some(&edit), "run-1", 2), Some(&edit.texts[..]));
        assert_eq!(effective_texts(Some(&edit), "run-2", 2), None);
        assert_eq!(effective_texts(Some(&edit), "run-1", 3), None);
        assert_eq!(effective_texts(None, "run-1", 2), None);
    }

    /// Trần dấu mốc trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> TRANSCRIPT_TEST_DB_NAME=<db>
    /// cargo test bookmarks_cap -- --ignored`. Doc chưa có lúc thêm mốc đầu; thêm đồng thời không vượt trần.
    #[tokio::test]
    #[ignore]
    async fn bookmarks_cap_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let key = format!("f:{}", ObjectId::new().to_hex());
        let mark = |i: usize| Bookmark { id: format!("m{i}"), at: i as f64, label: None };
        let tasks = (0..MAX_BOOKMARKS + 20).map(|i| {
            let (db, user, key) = (db.clone(), user.clone(), key.clone());
            async move { add_bookmark(&db, &user, &key, &mark(i)).await.expect("thêm") }
        });
        let added = futures_util::future::join_all(tasks).await.into_iter().filter(|r| *r == BookmarkAdd::Added).count();
        assert_eq!(added, MAX_BOOKMARKS);
        assert_eq!(get(&db, &user, &key).await.expect("get").expect("doc").bookmarks.len(), MAX_BOOKMARKS);
        let first = get(&db, &user, &key).await.expect("get").expect("doc").bookmarks[0].id.clone();
        // Gửi lại mốc đã có (trả lời bị mất): trả mốc cũ, không thêm lần hai, kể cả khi doc đã đầy.
        let second = get(&db, &user, &key).await.expect("get").expect("doc").bookmarks[1].clone();
        assert_eq!(add_bookmark(&db, &user, &key, &second).await.expect("gửi lại"), BookmarkAdd::Exists(second.clone()));
        assert!(remove_bookmark(&db, &user, &key, &first).await.expect("xoá"));
        assert!(!remove_bookmark(&db, &user, &key, &first).await.expect("xoá lại"));
        assert_eq!(add_bookmark(&db, &user, &key, &mark(999)).await.expect("thêm sau khi xoá"), BookmarkAdd::Added);
        assert_eq!(add_bookmark(&db, &user, &key, &mark(999)).await.expect("gửi lại"), BookmarkAdd::Exists(mark(999)));
        assert_eq!(add_bookmark(&db, &user, &key, &mark(1000)).await.expect("đầy"), BookmarkAdd::Full);
        delete(&db, &user, &key).await.expect("dọn");
    }

    /// Cắt bản ghi trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> TRANSCRIPT_TEST_DB_NAME=<db>
    /// cargo test bookmarks_move -- --ignored`. Mốc của bản ghi gốc sang doc của từng phần, theo vị trí.
    #[tokio::test]
    #[ignore]
    async fn bookmarks_move_to_split_pieces_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let key = |id: ObjectId| format!("f:{}", id.to_hex());
        let (source, first, second) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
        assert_eq!(move_bookmarks_to_pieces(&db, &user, &key(source), &[(key(first), 60.0)]).await.expect("chưa có doc"), 0);
        for (id, at) in [("m1", 10.0), ("m2", 75.0), ("m3", 80.5)] {
            let mark = Bookmark { id: id.into(), at, label: None };
            assert_eq!(add_bookmark(&db, &user, &key(source), &mark).await.expect("thêm"), BookmarkAdd::Added);
        }
        let pieces = [(key(first), 60.0), (key(second), 60.0)];
        assert_eq!(move_bookmarks_to_pieces(&db, &user, &key(source), &pieces).await.expect("chép"), 3);
        let at_of = |d: SessionDoc| d.bookmarks.into_iter().map(|b| (b.id, b.at)).collect::<Vec<_>>();
        let a = get(&db, &user, &key(first)).await.expect("get").expect("doc phần 1");
        assert_eq!(at_of(a), vec![("m1".to_string(), 10.0)]);
        let b = get(&db, &user, &key(second)).await.expect("get").expect("doc phần 2");
        assert_eq!(at_of(b), vec![("m2".to_string(), 15.0), ("m3".to_string(), 20.5)]);
        for id in [source, first, second] {
            delete(&db, &user, &key(id)).await.expect("dọn");
        }
    }

    /// Vòng đời trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> TRANSCRIPT_TEST_DB_NAME=<db>
    /// cargo test transcript_docs_lifecycle -- --ignored`. User giả riêng, tự dọn doc đã tạo.
    #[tokio::test]
    #[ignore]
    async fn transcript_docs_lifecycle_on_real_mongo() {
        use crate::models::{file, transcript};
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let key = format!("s:{session}");
        let (a, b) = (ObjectId::new(), ObjectId::new());
        for (id, index) in [(a, 0), (b, 1)] {
            let f = file::FileDoc::new_recording(&user, &format!("{session}#{index}"), "/nonexistent", 1, 60.0, false, id);
            file::create(&db, f).await.expect("tạo file");
        }
        let files = file::list_session(&db, &user, &key).await.expect("list_session");
        assert_eq!(files.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0, 1]);

        // Hai lần ghi đầu tiên chạy đua: unique index + thử lại → đúng một doc.
        let e1 = SegmentEdit { run_id: "run-1".into(), texts: vec!["Một.".into()] };
        let e2 = SegmentEdit { run_id: "run-1".into(), texts: vec!["Hai.".into()] };
        let (r1, r2) = tokio::join!(set_edits(&db, &user, &key, a, &e1), set_edits(&db, &user, &key, b, &e2));
        r1.expect("edit a");
        r2.expect("edit b");
        let count = db
            .collection::<Document>(COLL)
            .count_documents(doc! { "userId": &user, "key": &key })
            .await
            .expect("đếm");
        assert_eq!(count, 1);

        set_title(&db, &user, &key, Some("Họp tuần")).await.expect("đặt tên");
        let d = get(&db, &user, &key).await.expect("get").expect("doc");
        assert_eq!(d.title.as_deref(), Some("Họp tuần"));
        assert_eq!(d.edits.get(&a.to_hex()), Some(&e1));
        set_title(&db, &user, &key, None).await.expect("xoá tên");
        assert_eq!(get(&db, &user, &key).await.expect("get").expect("doc").title, None);

        // Chép lại đoạn a: runId đổi → câu sửa của lượt cũ hết hiệu lực.
        let charge = crate::models::usage::Charge { user_id: user.clone(), month: "test".into(), free_pool: false, day: None };
        transcript::enqueue(&db, a, &user, "run-2", "m", &charge, 0).await.expect("enqueue");
        let run = transcript::statuses_for_files(&db, &user, &[a]).await.expect("status");
        assert_eq!(effective_texts(d.edits.get(&a.to_hex()), &run[0].run_id, 1), None);

        // Xoá đoạn a: câu sửa của a và biên bản dọn theo; còn b nên doc phiên chưa mồ côi.
        let stored = StoredSummary {
            model: "m".into(),
            created_at: DateTime::now(),
            source_hash: "h".into(),
            summary: "Tóm tắt".into(),
            decisions: vec![],
            actions: vec![],
        };
        set_summary(&db, &user, &key, &stored).await.expect("biên bản");
        let fa = files.iter().find(|(_, f)| f.id == Some(a)).map(|(_, f)| f).expect("fa");
        file::delete_doc(&db, fa).await.expect("xoá a");
        assert_eq!(get(&db, &user, &key).await.expect("get").expect("doc").summary, None);
        db.collection::<Document>(crate::models::usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        let d = get(&db, &user, &key).await.expect("get").expect("doc");
        assert!(!d.edits.contains_key(&a.to_hex()) && d.edits.contains_key(&b.to_hex()));
        assert!(transcript::statuses_for_files(&db, &user, &[a]).await.expect("status").is_empty());
        delete_orphans(&db).await.expect("sweep");
        assert!(get(&db, &user, &key).await.expect("get").is_some());

        // Xoá đoạn cuối mà không qua `delete_doc` (giả lập bước dọn bị lỗi): doc còn mới thì sweep chừa
        // (phiên đang ghi có dấu mốc trước file), doc đủ cũ thì bị dọn.
        db.collection::<Document>("files").delete_one(doc! { "_id": b }).await.expect("xoá b");
        delete_orphans(&db).await.expect("sweep");
        assert!(get(&db, &user, &key).await.expect("get").is_some());
        let two_days_ago = DateTime::from_millis(DateTime::now().timestamp_millis() - 48 * 3_600_000);
        db.collection::<Document>(COLL)
            .update_one(doc! { "userId": &user, "key": &key }, doc! { "$set": { "createdAt": two_days_ago } })
            .await
            .expect("lùi createdAt");
        assert!(delete_orphans(&db).await.expect("sweep") >= 1);
        assert!(get(&db, &user, &key).await.expect("get").is_none());

        // Doc còn mới nhưng có biên bản mà không còn file: sweep xoá ngay.
        set_summary(&db, &user, &key, &stored).await.expect("biên bản");
        assert!(delete_orphans(&db).await.expect("sweep") >= 1);
        assert!(get(&db, &user, &key).await.expect("get").is_none());
    }
}
