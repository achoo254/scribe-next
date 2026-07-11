//! `files` collection: upload gốc + output đoạn cắt. Ownership theo `userId`.

use crate::error::AppError;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "files";

/// Index `userId` — tăng tốc ownership lookup + aggregate quota.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<FileDoc>(COLL)
        .create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build())
        .await
        .map_err(|e| format!("index files.userId: {e}"))?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub kind: String, // "upload" | "output"
    pub name: String,
    pub path: String,
    pub size: i64,
    #[serde(rename = "jobId", skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
}

impl FileDoc {
    pub fn new_upload(user_id: &str, name: &str, path: &str, size: i64, id: ObjectId) -> Self {
        FileDoc {
            id: Some(id),
            user_id: user_id.into(),
            kind: "upload".into(),
            name: name.into(),
            path: path.into(),
            size,
            job_id: None,
            created_at: DateTime::now(),
        }
    }
    pub fn new_output(user_id: &str, job_id: &str, name: &str, path: &str, size: i64) -> Self {
        FileDoc {
            id: None,
            user_id: user_id.into(),
            kind: "output".into(),
            name: name.into(),
            path: path.into(),
            size,
            job_id: Some(job_id.into()),
            created_at: DateTime::now(),
        }
    }
}

/// Insert file doc → trả `_id` hex.
pub async fn create(db: &Database, doc_: FileDoc) -> Result<String, AppError> {
    let res = db
        .collection::<FileDoc>(COLL)
        .insert_one(doc_)
        .await
        .map_err(|e| AppError::Internal(format!("insert file: {e}")))?;
    res.inserted_id
        .as_object_id()
        .map(|o| o.to_hex())
        .ok_or_else(|| AppError::Internal("file _id không phải ObjectId".into()))
}

/// Lấy file theo id + userId (ownership). None = không tồn tại hoặc không thuộc user.
pub async fn get_owned(
    db: &Database,
    file_id: &str,
    user_id: &str,
) -> Result<Option<FileDoc>, AppError> {
    let oid = ObjectId::parse_str(file_id)
        .map_err(|_| AppError::BadRequest("file id không hợp lệ".into()))?;
    db.collection::<FileDoc>(COLL)
        .find_one(doc! { "_id": oid, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("find file: {e}")))
}

/// Các đoạn output của 1 job (ownership).
pub async fn list_outputs_for_job(
    db: &Database,
    job_id: &str,
    user_id: &str,
) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "jobId": job_id, "userId": user_id, "kind": "output" })
        .await
        .map_err(|e| AppError::Internal(format!("find outputs: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc output: {e}")))?);
    }
    Ok(out)
}

/// Tổng byte đã dùng của user (quota — red-team #3).
pub async fn user_usage_bytes(db: &Database, user_id: &str) -> Result<u64, AppError> {
    let pipeline = vec![
        doc! { "$match": { "userId": user_id } },
        doc! { "$group": { "_id": null, "total": { "$sum": "$size" } } },
    ];
    let mut cur = db
        .collection::<Document>(COLL)
        .aggregate(pipeline)
        .await
        .map_err(|e| AppError::Internal(format!("aggregate usage: {e}")))?;
    if let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("đọc usage: {e}")))?;
        let total = d
            .get_i64("total")
            .or_else(|_| d.get_i32("total").map(i64::from))
            .unwrap_or(0);
        return Ok(total.max(0) as u64);
    }
    Ok(0)
}
