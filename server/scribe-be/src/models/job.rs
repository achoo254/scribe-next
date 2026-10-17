//! `jobs` collection: job queue Mode B (split). Claim atomic concurrency=1.

use crate::error::AppError;
use core_split::SplitCriterion;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use mongodb::options::ReturnDocument;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "jobs";

/// Index `{status, createdAt}` — tăng tốc claim_next (queued cũ nhất) + poll theo trạng thái.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<Job>(COLL)
        .create_index(IndexModel::builder().keys(doc! { "status": 1, "createdAt": 1 }).build())
        .await
        .map_err(|e| format!("index jobs.status: {e}"))?;
    Ok(())
}

/// Id file đầu vào của các job cắt đang chờ hoặc đang chạy của user ("Xoá tất cả" chừa các file này).
pub async fn busy_inputs(db: &Database, user_id: &str) -> Result<Vec<String>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("tra job đang chạy: {e}"));
    let mut cur = db
        .collection::<Job>(COLL)
        .find(doc! { "userId": user_id, "status": { "$in": ["queued", "running"] } })
        .await
        .map_err(read)?;
    let mut out = Vec::new();
    while let Some(j) = futures_util::StreamExt::next(&mut cur).await {
        out.push(j.map_err(read)?.input_file_id);
    }
    Ok(out)
}

/// Tham chiếu 1 đoạn kết quả (trả cho FE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegRef {
    pub name: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "type")]
    pub job_type: String,
    #[serde(rename = "inputFileId")]
    pub input_file_id: String,
    pub criterion: SplitCriterion,
    pub value: f64,
    /// true = cắt bản ghi (Mode A): đoạn lưu thành `kind:recording` (hiện Lịch sử ghi, phát/tua
    /// từng đoạn) + xoá bản ghi gốc. false = Mode B (Cắt file): đoạn `kind:output` → tải zip.
    /// `default` cho job cũ (trước tính năng) → false (giữ hành vi cũ).
    #[serde(rename = "recordingSplit", default)]
    pub recording_split: bool,
    pub status: String, // queued | running | done | failed
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<SegRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
}

impl Job {
    pub fn hex_id(&self) -> String {
        self.id.map(|o| o.to_hex()).unwrap_or_default()
    }
}

/// Tạo job split (status queued) → trả jobId hex.
pub async fn create_split(
    db: &Database,
    user_id: &str,
    input_file_id: &str,
    criterion: SplitCriterion,
    value: f64,
    recording_split: bool,
) -> Result<String, AppError> {
    let job = Job {
        id: None,
        user_id: user_id.into(),
        job_type: "split".into(),
        input_file_id: input_file_id.into(),
        criterion,
        value,
        recording_split,
        status: "queued".into(),
        progress: 0.0,
        segments: None,
        error: None,
        created_at: DateTime::now(),
    };
    let res = db
        .collection::<Job>(COLL)
        .insert_one(job)
        .await
        .map_err(|e| AppError::Internal(format!("insert job: {e}")))?;
    res.inserted_id
        .as_object_id()
        .map(|o| o.to_hex())
        .ok_or_else(|| AppError::Internal("job _id không phải ObjectId".into()))
}

pub async fn get_owned(db: &Database, job_id: &str, user_id: &str) -> Result<Option<Job>, AppError> {
    let oid = ObjectId::parse_str(job_id)
        .map_err(|_| AppError::BadRequest("job id không hợp lệ".into()))?;
    db.collection::<Job>(COLL)
        .find_one(doc! { "_id": oid, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("find job: {e}")))
}

/// Claim job queued cũ nhất → running (atomic, concurrency=1). heartbeatAt để sweep stale (Phase 5).
pub async fn claim_next(db: &Database) -> Result<Option<Job>, mongodb::error::Error> {
    db.collection::<Job>(COLL)
        .find_one_and_update(
            doc! { "status": "queued" },
            doc! { "$set": { "status": "running", "heartbeatAt": DateTime::now() } },
        )
        .sort(doc! { "createdAt": 1 })
        .return_document(ReturnDocument::After)
        .await
}

pub async fn finish_done(db: &Database, job_id: &str, segments: Vec<SegRef>) -> Result<(), AppError> {
    let oid = ObjectId::parse_str(job_id).map_err(|_| AppError::Internal("job id".into()))?;
    let segs = mongodb::bson::to_bson(&segments)
        .map_err(|e| AppError::Internal(format!("bson segments: {e}")))?;
    db.collection::<Job>(COLL)
        .update_one(
            doc! { "_id": oid },
            doc! { "$set": { "status": "done", "progress": 100.0, "segments": segs } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("finish job: {e}")))?;
    Ok(())
}

pub async fn finish_failed(db: &Database, job_id: &str, error: &str) -> Result<(), AppError> {
    let oid = ObjectId::parse_str(job_id).map_err(|_| AppError::Internal("job id".into()))?;
    db.collection::<Job>(COLL)
        .update_one(
            doc! { "_id": oid },
            doc! { "$set": { "status": "failed", "error": error } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("fail job: {e}")))?;
    Ok(())
}

/// Reset job kẹt `running` → `queued` khi BE start (hồi phục sau crash/restart — red-team).
pub async fn reset_running_to_queued(db: &Database) -> Result<u64, AppError> {
    let res = db
        .collection::<Job>(COLL)
        .update_many(
            doc! { "status": "running" },
            doc! { "$set": { "status": "queued", "progress": 0.0 } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("reset running jobs: {e}")))?;
    Ok(res.modified_count)
}
