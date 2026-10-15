//! `transcripts` collection: kết quả chép lời của một file bản ghi (`_id` = id của file) và cũng là
//! hàng đợi chép lời (status queued → running → done | failed).
//!
//! `runId` đổi mỗi lần xếp hàng lại: worker chỉ ghi kết quả khi `runId` còn khớp, nên bấm "Chép lời"
//! lần nữa trong lúc lượt cũ đang chạy không bị kết quả cũ đè lên.

use crate::error::AppError;
use crate::transcribe::chunking::Sentence;
use futures_util::StreamExt;
use mongodb::bson::{self, doc, oid::ObjectId, DateTime};
use mongodb::options::ReturnDocument;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "transcripts";

/// Lượt chép mất khoá vì backend khởi động lại (key chỉ nằm trong RAM).
pub const RESTARTED: &str = "Máy chủ đã khởi động lại trong lúc chép. Hãy bấm Chép lời lại.";

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub status: String, // queued | running | done | failed
    pub progress: f64,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sentences: Vec<Sentence>,
    pub model: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime,
}

impl TranscriptDoc {
    pub fn file_id(&self) -> String {
        self.id.to_hex()
    }
}

pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<TranscriptDoc>(COLL);
    coll.create_index(IndexModel::builder().keys(doc! { "status": 1, "updatedAt": 1 }).build())
        .await
        .map_err(|e| format!("index transcripts.status: {e}"))?;
    coll.create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build())
        .await
        .map_err(|e| format!("index transcripts.userId: {e}"))?;
    Ok(())
}

/// Xếp hàng (hoặc xếp lại) một file; xoá kết quả cũ.
pub async fn enqueue(db: &Database, file_id: ObjectId, user_id: &str, run_id: &str, model: &str) -> Result<(), AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .update_one(
            doc! { "_id": file_id, "userId": user_id },
            doc! {
                "$set": {
                    "userId": user_id,
                    "status": "queued",
                    "progress": 0.0,
                    "runId": run_id,
                    "text": "",
                    "sentences": [],
                    "model": model,
                    "updatedAt": DateTime::now(),
                },
                "$unset": { "error": "" },
            },
        )
        .upsert(true)
        .await
        .map_err(|e| AppError::Internal(format!("xếp hàng chép lời: {e}")))?;
    Ok(())
}

/// Lấy lượt chờ lâu nhất → running.
pub async fn claim_next(db: &Database) -> Result<Option<TranscriptDoc>, mongodb::error::Error> {
    db.collection::<TranscriptDoc>(COLL)
        .find_one_and_update(
            doc! { "status": "queued" },
            doc! { "$set": { "status": "running", "updatedAt": DateTime::now() } },
        )
        .sort(doc! { "updatedAt": 1 })
        .return_document(ReturnDocument::After)
        .await
}

/// Trả `false` khi lượt `run_id` không còn (đã xếp lại, hoặc file đã bị xoá): người gọi nên dừng.
async fn update_run(db: &Database, file_id: ObjectId, run_id: &str, set: bson::Document) -> Result<bool, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(doc! { "_id": file_id, "runId": run_id }, doc! { "$set": set })
        .await
        .map_err(|e| AppError::Internal(format!("cập nhật chép lời: {e}")))?;
    Ok(res.matched_count > 0)
}

pub async fn set_progress(db: &Database, file_id: ObjectId, run_id: &str, progress: f64) -> Result<bool, AppError> {
    update_run(db, file_id, run_id, doc! { "progress": progress, "updatedAt": DateTime::now() }).await
}

pub async fn finish_done(
    db: &Database,
    file_id: ObjectId,
    run_id: &str,
    text: &str,
    sentences: &[Sentence],
) -> Result<bool, AppError> {
    let sentences = bson::to_bson(sentences).map_err(|e| AppError::Internal(format!("bson câu: {e}")))?;
    let set = doc! {
        "status": "done",
        "progress": 100.0,
        "text": text,
        "sentences": sentences,
        "updatedAt": DateTime::now(),
    };
    update_run(db, file_id, run_id, set).await
}

pub async fn finish_failed(db: &Database, file_id: ObjectId, run_id: &str, error: &str) -> Result<bool, AppError> {
    update_run(db, file_id, run_id, doc! { "status": "failed", "error": error, "updatedAt": DateTime::now() }).await
}

/// Khi khởi động: key của các lượt dở đã mất cùng tiến trình cũ → báo người dùng bấm lại.
pub async fn fail_unfinished(db: &Database) -> Result<u64, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_many(
            doc! { "status": { "$in": ["queued", "running"] } },
            doc! { "$set": { "status": "failed", "error": RESTARTED, "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("đánh dấu chép lời dở: {e}")))?;
    Ok(res.modified_count)
}

/// Transcript của các file thuộc user (bỏ qua id không hợp lệ hoặc không phải của user).
pub async fn list_owned(db: &Database, user_id: &str, file_ids: &[ObjectId]) -> Result<Vec<TranscriptDoc>, AppError> {
    let mut cur = db
        .collection::<TranscriptDoc>(COLL)
        .find(doc! { "_id": { "$in": file_ids }, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("đọc chép lời: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc chép lời: {e}")))?);
    }
    Ok(out)
}

/// Trạng thái chép lời mọi file của user (chỉ `_id` + `status`, không kéo chữ) cho danh sách bản ghi.
pub async fn statuses(db: &Database, user_id: &str) -> Result<Vec<(String, String)>, AppError> {
    let mut cur = db
        .collection::<bson::Document>(COLL)
        .find(doc! { "userId": user_id })
        .projection(doc! { "status": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("đọc trạng thái chép lời: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("đọc trạng thái chép lời: {e}")))?;
        if let (Ok(id), Ok(status)) = (d.get_object_id("_id"), d.get_str("status")) {
            out.push((id.to_hex(), status.to_string()));
        }
    }
    Ok(out)
}

/// Xoá cùng file (gọi từ `file::delete_doc`, nơi duy nhất xoá doc file).
pub async fn delete(db: &Database, file_id: ObjectId) -> Result<(), AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .delete_one(doc! { "_id": file_id })
        .await
        .map_err(|e| AppError::Internal(format!("xoá chép lời: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vòng đời hàng đợi trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> cargo test
    /// transcript_queue -- --ignored`. Dùng user giả riêng và tự xoá doc đã tạo.
    #[tokio::test]
    #[ignore]
    async fn transcript_queue_lifecycle_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db = crate::db::connect(&uri).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let (a, b) = (ObjectId::new(), ObjectId::new());

        enqueue(&db, a, &user, "run-1", "m").await.expect("enqueue a");
        enqueue(&db, b, &user, "run-1", "m").await.expect("enqueue b");
        // Xếp lại a: lượt cũ (run-1) không còn được ghi kết quả.
        enqueue(&db, a, &user, "run-2", "m").await.expect("re-enqueue a");

        let mut claimed = Vec::new();
        while let Some(t) = claim_next(&db).await.expect("claim") {
            if t.user_id == user {
                claimed.push(t);
            } else {
                // Lượt thật của người khác: trả lại hàng đợi, không đụng.
                db.collection::<TranscriptDoc>(COLL)
                    .update_one(doc! { "_id": t.id }, doc! { "$set": { "status": "queued" } })
                    .await
                    .expect("trả lại");
                break;
            }
        }
        assert_eq!(claimed.len(), 2);
        assert!(claimed.iter().all(|t| t.status == "running"));

        let sentence = Sentence { start: 0.0, end: 1.0, text: "Xin chào.".into() };
        finish_done(&db, a, "run-1", "cũ", &[]).await.expect("stale finish");
        finish_done(&db, a, "run-2", "Xin chào.", std::slice::from_ref(&sentence)).await.expect("finish a");
        finish_failed(&db, b, "run-1", "OpenAI: API key không hợp lệ").await.expect("fail b");

        let docs = list_owned(&db, &user, &[a, b]).await.expect("list");
        let doc_a = docs.iter().find(|t| t.id == a).expect("a");
        assert_eq!((doc_a.status.as_str(), doc_a.text.as_str()), ("done", "Xin chào."));
        assert_eq!(doc_a.sentences, vec![sentence]);
        let doc_b = docs.iter().find(|t| t.id == b).expect("b");
        assert_eq!(doc_b.error.as_deref(), Some("OpenAI: API key không hợp lệ"));
        assert!(list_owned(&db, "người-khác", &[a, b]).await.expect("list other").is_empty());
        assert_eq!(statuses(&db, &user).await.expect("statuses").len(), 2);

        delete(&db, a).await.expect("xoá a");
        delete(&db, b).await.expect("xoá b");
        assert!(list_owned(&db, &user, &[a, b]).await.expect("list sau xoá").is_empty());
    }
}
