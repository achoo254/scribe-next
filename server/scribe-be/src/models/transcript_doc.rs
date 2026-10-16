//! `transcript_docs` collection: phần người dùng tạo ra cho một transcript (tiêu đề, câu đã sửa),
//! một doc cho mỗi phiên ghi. Kết quả máy vẫn ở `transcripts` (mỗi file một doc).
//!
//! `key` là khoá phiên (`file::session_key`): `s:<sessionId>` cho phiên iPhone nhiều đoạn,
//! `f:<fileId>` cho bản ghi một file. Doc tạo lười khi người dùng đổi tên hoặc sửa lần đầu.
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

pub async fn unset_edits(db: &Database, user_id: &str, key: &str, file_id: ObjectId) -> Result<(), AppError> {
    let path = format!("edits.{}", file_id.to_hex());
    db.collection::<Document>(COLL)
        .update_one(doc! { "userId": user_id, "key": key }, doc! { "$unset": { path: "" } })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("bỏ câu sửa: {e}")))
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

/// Xoá doc không còn file bản ghi nào của phiên (audio đã bị xoá tay, hết hạn, hay bị cắt).
/// Đây là cơ chế đảm bảo; bước dọn trong `file::delete_doc` chỉ là tối ưu. Trả số doc đã xoá.
pub async fn delete_orphans(db: &Database) -> Result<u64, AppError> {
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! {})
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
    fn edits_apply_only_to_the_same_run_and_sentence_count() {
        let edit = SegmentEdit { run_id: "run-1".into(), texts: vec!["a".into(), "b".into()] };
        assert_eq!(effective_texts(Some(&edit), "run-1", 2), Some(&edit.texts[..]));
        assert_eq!(effective_texts(Some(&edit), "run-2", 2), None);
        assert_eq!(effective_texts(Some(&edit), "run-1", 3), None);
        assert_eq!(effective_texts(None, "run-1", 2), None);
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
        let charge = crate::models::usage::Charge { user_id: user.clone(), month: "test".into(), free_pool: false };
        transcript::enqueue(&db, a, &user, "run-2", "m", &charge, 0).await.expect("enqueue");
        let run = transcript::statuses_for_files(&db, &user, &[a]).await.expect("status");
        assert_eq!(effective_texts(d.edits.get(&a.to_hex()), &run[0].run_id, 1), None);

        // Xoá đoạn a: câu sửa của a dọn theo; còn b nên doc phiên chưa mồ côi.
        let fa = files.iter().find(|(_, f)| f.id == Some(a)).map(|(_, f)| f).expect("fa");
        file::delete_doc(&db, fa).await.expect("xoá a");
        db.collection::<Document>(crate::models::usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        let d = get(&db, &user, &key).await.expect("get").expect("doc");
        assert!(!d.edits.contains_key(&a.to_hex()) && d.edits.contains_key(&b.to_hex()));
        assert!(transcript::statuses_for_files(&db, &user, &[a]).await.expect("status").is_empty());
        delete_orphans(&db).await.expect("sweep");
        assert!(get(&db, &user, &key).await.expect("get").is_some());

        // Xoá đoạn cuối mà không qua `delete_doc` (giả lập bước dọn bị lỗi): sweep dọn doc phiên.
        db.collection::<Document>("files").delete_one(doc! { "_id": b }).await.expect("xoá b");
        assert!(delete_orphans(&db).await.expect("sweep") >= 1);
        assert!(get(&db, &user, &key).await.expect("get").is_none());
    }
}
