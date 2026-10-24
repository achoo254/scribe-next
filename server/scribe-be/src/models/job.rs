//! `jobs` collection: job queue Mode B (split). Claim atomic concurrency=1, xoay vòng theo tenant như hàng đợi chép
//! lời (`transcript::claim_next`).

use crate::error::AppError;
use core_split::SplitCriterion;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::ReturnDocument;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "jobs";

/// Index `{status, createdAt}` — tăng tốc claim_next (queued cũ nhất) + poll theo trạng thái; `{status, tenantId,
/// createdAt}` cho lượt xoay vòng theo tenant.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Job>(COLL);
    coll.create_index(IndexModel::builder().keys(doc! { "status": 1, "createdAt": 1 }).build())
        .await
        .map_err(|e| format!("index jobs.status: {e}"))?;
    coll.create_index(IndexModel::builder().keys(doc! { "status": 1, "tenantId": 1, "createdAt": 1 }).build())
        .await
        .map_err(|e| format!("index jobs.status_tenantId: {e}"))?;
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
    /// Tenant của chủ job (lượt xoay vòng trong hàng đợi).
    #[serde(rename = "tenantId", default = "crate::tenant::primary_id")]
    pub tenant_id: String,
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
    tenant_id: &str,
    user_id: &str,
    input_file_id: &str,
    criterion: SplitCriterion,
    value: f64,
    recording_split: bool,
) -> Result<String, AppError> {
    let job = Job {
        id: None,
        user_id: user_id.into(),
        tenant_id: tenant_id.into(),
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

/// Claim job queued cũ nhất → running (atomic, concurrency=1). Có `last_tenant` (tenant của job vừa chạy) thì ưu tiên
/// job cũ nhất của tenant khác, không có mới lấy job cũ nhất bất kỳ. heartbeatAt để sweep stale.
pub async fn claim_next(db: &Database, last_tenant: Option<&str>) -> Result<Option<Job>, mongodb::error::Error> {
    claim_next_in(db, last_tenant, doc! {}).await
}

/// [`claim_next`] trong phạm vi `scope` (test chỉ nhận job của user giả của nó).
async fn claim_next_in(db: &Database, last_tenant: Option<&str>, scope: Document) -> Result<Option<Job>, mongodb::error::Error> {
    let coll = db.collection::<Job>(COLL);
    let mut queued = doc! { "status": "queued" };
    queued.extend(scope);
    let claim = |filter: Document| {
        coll.find_one_and_update(filter, doc! { "$set": { "status": "running", "heartbeatAt": DateTime::now() } })
            .sort(doc! { "createdAt": 1 })
            .return_document(ReturnDocument::After)
    };
    if let Some(last) = last_tenant {
        let mut other = queued.clone();
        other.insert("tenantId", doc! { "$ne": last });
        if let Some(job) = claim(other).await? {
            return Ok(Some(job));
        }
    }
    claim(queued).await
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Job cắt xoay vòng theo tenant như hàng đợi chép lời. `cargo test jobs_rotate -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn jobs_rotate_between_tenants_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let (ten_a, ten_b) = (format!("ten_a_{tag}"), format!("ten_b_{tag}"));
        let (user_a, user_b) = (format!("selftest-a-{tag}"), format!("selftest-b-{tag}"));
        for (tenant, user, n) in [(&ten_a, &user_a, 2), (&ten_b, &user_b, 2)] {
            for _ in 0..n {
                create_split(&db, tenant, user, "f", SplitCriterion::Minutes, 1.0, false).await.expect("tạo");
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }
        let scope = doc! { "userId": { "$in": [&user_a, &user_b] } };
        let (mut order, mut last) = (Vec::new(), None::<String>);
        while let Some(job) = claim_next_in(&db, last.as_deref(), scope.clone()).await.expect("nhận") {
            order.push(job.tenant_id.clone());
            last = Some(job.tenant_id);
        }
        assert_eq!(order, vec![ten_a.clone(), ten_b.clone(), ten_a.clone(), ten_b.clone()]);
        db.collection::<Job>(COLL).delete_many(scope).await.expect("dọn");
    }
}
