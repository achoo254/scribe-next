//! `transcripts` collection: kết quả chép lời của một file bản ghi (`_id` = id của file) và cũng là
//! hàng đợi chép lời (status queued → running → done | failed).
//!
//! `runId` đổi mỗi lần xếp hàng lại: worker chỉ ghi kết quả khi `runId` còn khớp, nên bấm "Chép lời"
//! lần nữa trong lúc lượt cũ đang chạy không bị kết quả cũ đè lên.
//!
//! Tiền: mỗi lượt mang phần phút đã giữ chỗ (`chargedS`, xem `usage`) và số giây audio đã thật sự gửi
//! OpenAI (`sentS`, cộng TRƯỚC mỗi lần gửi). Lượt kết thúc (xong, lỗi) đóng theo hai bước, mỗi bước lọc
//! theo `runId` nên lượt cũ không chạm lượt mới: (1) `open: true → false`, `settled: false`; (2) nhận
//! quyền quyết toán `settled: false → true` rồi cộng chênh lệch `sentS − chargedS` vào sổ. Bước (2) chạy
//! lại được: Mongo standalone có thể báo lỗi dù đã ghi, lần thử lại vẫn quyết toán đúng một lần.

use crate::error::AppError;
use crate::models::usage::{self, Charge};
use crate::transcribe::chunking::Sentence;
use futures_util::StreamExt;
use mongodb::bson::{self, doc, oid::ObjectId, DateTime};
use mongodb::options::ReturnDocument;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "transcripts";

/// Lượt bị backend khởi động lại cắt ngang lần thứ hai: dừng hẳn thay vì gửi lại audio lần nữa.
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
    /// Giây đã giữ chỗ trong sổ phút cho lượt này.
    #[serde(rename = "chargedS", default)]
    pub charged_s: i64,
    /// Giây audio đã gửi OpenAI (kể cả gửi lại) — số thật để quyết toán.
    #[serde(rename = "sentS", default)]
    pub sent_s: i64,
    /// Tháng của sổ đã giữ chỗ.
    #[serde(default)]
    pub month: Option<String>,
    /// Lượt có tính vào quỹ chung của gói free không.
    #[serde(rename = "freePool", default)]
    pub free_pool: bool,
    /// Gói free: ngày của sổ ngày (`usage::Charge`).
    #[serde(default)]
    pub day: Option<String>,
    /// Lượt còn chạy, sổ còn mở.
    #[serde(default)]
    pub open: bool,
    /// Đã quyết toán vào sổ (chỉ có ý nghĩa khi `open = false`).
    #[serde(default)]
    pub settled: bool,
    /// Số lần lượt bị cắt ngang vì backend khởi động lại.
    #[serde(default)]
    pub attempts: i32,
}

impl TranscriptDoc {
    pub fn file_id(&self) -> String {
        self.id.to_hex()
    }

    pub fn charge(&self) -> Option<Charge> {
        Some(Charge {
            user_id: self.user_id.clone(),
            month: self.month.clone()?,
            free_pool: self.free_pool,
            day: self.day.clone(),
        })
    }
}

/// Quyết toán sổ của một lượt vừa đóng (doc là bản TRƯỚC khi đóng). Lỗi chỉ log: tiền đã chi, không
/// làm hỏng việc ghi kết quả; lệnh đối soát `admin usage --recompute` sửa được về sau.
async fn settle_closed(db: &Database, before: &TranscriptDoc) {
    let Some(charge) = before.charge() else { return };
    let delta = before.sent_s - before.charged_s;
    if let Err(e) = usage::settle(db, &charge, delta).await {
        tracing::error!(file = %before.id, delta, "quyết toán phút lỗi: {e:?}");
    }
    // Sổ chi tiết chỉ thêm (doc lượt bị ghi đè khi chép lại): nguồn của lệnh đối soát.
    let run = doc! {
        "fileId": before.id,
        "runId": &before.run_id,
        "userId": &before.user_id,
        "month": &charge.month,
        "freePool": charge.free_pool,
        "day": &charge.day,
        "chargedS": before.charged_s,
        "sentS": before.sent_s,
        "at": DateTime::now(),
    };
    if let Err(e) = db.collection::<bson::Document>(usage::RUNS_COLL).insert_one(run).await {
        tracing::error!(file = %before.id, "ghi sổ lượt chép lỗi: {e}");
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

/// Xếp hàng (hoặc xếp lại) một file với phần phút đã giữ; xoá kết quả cũ. Trả `false` khi file đang
/// `queued/running` (một lượt khác giữ nó): người gọi hoàn phần vừa giữ.
pub async fn enqueue(
    db: &Database,
    file_id: ObjectId,
    user_id: &str,
    run_id: &str,
    model: &str,
    charge: &Charge,
    charged_s: i64,
) -> Result<bool, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(
            doc! { "_id": file_id, "userId": user_id, "status": { "$nin": ["queued", "running"] } },
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
                    "chargedS": charged_s,
                    "sentS": 0_i64,
                    "month": &charge.month,
                    "freePool": charge.free_pool,
                    "day": &charge.day,
                    "open": true,
                    "settled": false,
                    "attempts": 0,
                },
                "$unset": { "error": "" },
            },
        )
        .upsert(true)
        .await;
    match res {
        Ok(_) => Ok(true),
        // Doc đang queued/running không khớp filter ⟹ upsert chèn trùng `_id`.
        Err(e) if crate::models::user::is_duplicate_key(&e) => Ok(false),
        Err(e) => Err(AppError::Internal(format!("xếp hàng chép lời: {e}"))),
    }
}

/// Lượt `run_id` của file có trong hàng đợi không (kiểm lại sau một lần ghi báo lỗi).
pub async fn has_run(db: &Database, file_id: ObjectId, run_id: &str) -> Result<bool, AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .count_documents(doc! { "_id": file_id, "runId": run_id })
        .await
        .map(|n| n > 0)
        .map_err(|e| AppError::Internal(format!("đọc lượt chép lời: {e}")))
}

/// Cộng giây sắp gửi vào lượt (gọi TRƯỚC khi gửi). `false` khi lượt không còn (thay, xoá): dừng.
pub async fn add_sent(db: &Database, file_id: ObjectId, run_id: &str, seconds: i64) -> Result<bool, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(doc! { "_id": file_id, "runId": run_id, "open": true }, doc! { "$inc": { "sentS": seconds } })
        .await
        .map_err(|e| AppError::Internal(format!("ghi giây đã gửi: {e}")))?;
    Ok(res.matched_count > 0)
}

/// Ghi thêm phần đã giữ chỗ (thời lượng đo lại lớn hơn số đã giữ lúc xếp hàng).
pub async fn add_charge(db: &Database, file_id: ObjectId, run_id: &str, seconds: i64) -> Result<bool, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(doc! { "_id": file_id, "runId": run_id, "open": true }, doc! { "$inc": { "chargedS": seconds } })
        .await
        .map_err(|e| AppError::Internal(format!("ghi phần giữ thêm: {e}")))?;
    Ok(res.matched_count > 0)
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

/// Đóng lượt `run_id` (đặt `set`, `open: false`) rồi quyết toán. `false` khi lượt đã đóng từ trước
/// hoặc không còn (xếp lại, file bị xoá). Gọi lại sau lỗi vẫn an toàn: bước quyết toán chạy đúng một lần.
async fn close_run(db: &Database, file_id: ObjectId, run_id: &str, mut set: bson::Document) -> Result<bool, AppError> {
    set.insert("open", false);
    set.insert("settled", false);
    let closed = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(doc! { "_id": file_id, "runId": run_id, "open": true }, doc! { "$set": set })
        .await
        .map_err(|e| AppError::Internal(format!("đóng lượt chép lời: {e}")))?
        .modified_count
        > 0;
    settle_run(db, file_id, run_id).await?;
    Ok(closed)
}

/// Nhận quyền quyết toán lượt đã đóng (`settled: false → true`) và cộng chênh lệch vào sổ.
async fn settle_run(db: &Database, file_id: ObjectId, run_id: &str) -> Result<(), AppError> {
    let claimed = db
        .collection::<TranscriptDoc>(COLL)
        .find_one_and_update(
            doc! { "_id": file_id, "runId": run_id, "open": false, "settled": false },
            doc! { "$set": { "settled": true } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("quyết toán lượt chép lời: {e}")))?;
    if let Some(doc) = claimed {
        settle_closed(db, &doc).await;
    }
    Ok(())
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
    close_run(db, file_id, run_id, set).await
}

pub async fn finish_failed(db: &Database, file_id: ObjectId, run_id: &str, error: &str) -> Result<bool, AppError> {
    close_run(db, file_id, run_id, doc! { "status": "failed", "error": error, "updatedAt": DateTime::now() }).await
}

/// Khi khởi động: lượt đang `running` bị cắt ngang thì xếp lại (key server luôn có sẵn). Lượt đã bị cắt
/// ngang một lần rồi thì dừng hẳn và quyết toán, tránh vòng khởi động lại cứ chi tiền mãi.
/// Trả (số lượt xếp lại, số lượt dừng).
pub async fn requeue_interrupted(db: &Database) -> Result<(u64, u64), AppError> {
    let coll = db.collection::<TranscriptDoc>(COLL);
    // Lượt dở do bản cũ (trước khi có sổ phút) để lại: không có sổ để chạy tiếp ⟹ báo lỗi như trước.
    let legacy = coll
        .update_many(
            doc! { "status": { "$in": ["queued", "running"] }, "open": { "$ne": true } },
            doc! { "$set": { "status": "failed", "error": RESTARTED, "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("dọn lượt dở của bản cũ: {e}")))?
        .modified_count;
    let mut stuck = coll
        .find(doc! { "status": "running", "attempts": { "$gte": 1 } })
        .await
        .map_err(|e| AppError::Internal(format!("đọc lượt dở: {e}")))?;
    let mut stopped = 0;
    while let Some(t) = stuck.next().await {
        let t = t.map_err(|e| AppError::Internal(format!("đọc lượt dở: {e}")))?;
        if finish_failed(db, t.id, &t.run_id, RESTARTED).await? {
            stopped += 1;
        }
    }
    let res = coll
        .update_many(
            doc! { "status": "running" },
            doc! { "$set": { "status": "queued", "updatedAt": DateTime::now() }, "$inc": { "attempts": 1 } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("xếp lại lượt dở: {e}")))?;
    // Lượt đã đóng mà quyết toán bị gián đoạn (tiến trình chết giữa hai bước).
    let mut unsettled = coll
        .find(doc! { "open": false, "settled": false, "month": { "$exists": true } })
        .await
        .map_err(|e| AppError::Internal(format!("đọc lượt chưa quyết toán: {e}")))?;
    while let Some(t) = unsettled.next().await {
        let t = t.map_err(|e| AppError::Internal(format!("đọc lượt chưa quyết toán: {e}")))?;
        settle_run(db, t.id, &t.run_id).await?;
    }
    Ok((res.modified_count, stopped + legacy))
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

/// Trạng thái một lượt chép, không kèm câu (poll khi đang chép, xếp chép lời).
#[derive(Debug, Clone, PartialEq)]
pub struct RunStatus {
    pub file_id: ObjectId,
    pub status: String,
    pub progress: f64,
    pub run_id: String,
    pub error: Option<String>,
}

fn run_status(d: &bson::Document) -> Option<RunStatus> {
    Some(RunStatus {
        file_id: d.get_object_id("_id").ok()?,
        status: d.get_str("status").ok()?.to_string(),
        progress: d.get_f64("progress").unwrap_or(0.0),
        run_id: d.get_str("runId").unwrap_or_default().to_string(),
        error: d.get_str("error").ok().map(str::to_string),
    })
}

/// Trạng thái các file của user (projection: không kéo `text`/`sentences`).
pub async fn statuses_for_files(db: &Database, user_id: &str, file_ids: &[ObjectId]) -> Result<Vec<RunStatus>, AppError> {
    let mut cur = db
        .collection::<bson::Document>(COLL)
        .find(doc! { "_id": { "$in": file_ids }, "userId": user_id })
        .projection(doc! { "status": 1, "progress": 1, "runId": 1, "error": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("đọc trạng thái chép lời: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("đọc trạng thái chép lời: {e}")))?;
        out.extend(run_status(&d));
    }
    Ok(out)
}

/// Dòng tóm tắt cho danh sách transcript: trạng thái + vài câu đầu + tổng số câu.
#[derive(Debug)]
pub struct Preview {
    pub run: RunStatus,
    pub first_texts: Vec<String>,
    pub sentence_count: usize,
}

/// Số câu đầu kéo về cho dòng xem trước trong danh sách.
pub const PREVIEW_SENTENCES: i32 = 3;

/// Tóm tắt mọi lượt chép của user. Aggregate `$slice` để không kéo toàn bộ câu của phiên dài.
pub async fn previews(db: &Database, user_id: &str) -> Result<Vec<Preview>, AppError> {
    let pipeline = vec![
        doc! { "$match": { "userId": user_id } },
        doc! { "$project": {
            "status": 1, "progress": 1, "runId": 1, "error": 1,
            "first": { "$slice": [{ "$ifNull": ["$sentences.text", []] }, PREVIEW_SENTENCES] },
            "count": { "$size": { "$ifNull": ["$sentences", []] } },
        } },
    ];
    let mut cur = db
        .collection::<bson::Document>(COLL)
        .aggregate(pipeline)
        .await
        .map_err(|e| AppError::Internal(format!("đọc tóm tắt chép lời: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("đọc tóm tắt chép lời: {e}")))?;
        let Some(run) = run_status(&d) else { continue };
        let first_texts = d
            .get_array("first")
            .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let sentence_count = d.get_i32("count").map_or(0, |n| n.max(0) as usize);
        out.push(Preview { run, first_texts, sentence_count });
    }
    Ok(out)
}

/// Xoá một doc và quyết toán sổ nếu lượt còn mở. Giây đã gửi được cộng TRƯỚC mỗi lần gửi nên mảnh đang
/// bay lúc xoá đã nằm trong `sentS`; worker thấy lượt mất ở lần cộng kế tiếp và dừng.
async fn delete_and_settle(db: &Database, filter: bson::Document) -> Result<bool, AppError> {
    let before = db
        .collection::<TranscriptDoc>(COLL)
        .find_one_and_delete(filter)
        .await
        .map_err(|e| AppError::Internal(format!("xoá chép lời: {e}")))?;
    match before {
        Some(before) => {
            // Còn chạy, hoặc đã đóng mà chưa kịp quyết toán.
            if before.open || !before.settled {
                settle_closed(db, &before).await;
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Xoá transcript của các file thuộc user (xoá transcript, giữ audio).
pub async fn delete_owned(db: &Database, user_id: &str, file_ids: &[ObjectId]) -> Result<u64, AppError> {
    let mut removed = 0;
    for &id in file_ids {
        if delete_and_settle(db, doc! { "_id": id, "userId": user_id }).await? {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Xoá cùng file (gọi từ `file::delete_doc`, nơi duy nhất xoá doc file).
pub async fn delete(db: &Database, file_id: ObjectId) -> Result<(), AppError> {
    delete_and_settle(db, doc! { "_id": file_id }).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TranscribeLimits;

    /// Giữ chỗ gói free trên sổ giả: ngày dùng chính chuỗi `month` để dọn chung một regex.
    async fn take(db: &Database, user: &str, month: &str, need: i64, limits: &TranscribeLimits) -> Charge {
        let charge = Charge {
            user_id: user.to_string(),
            month: month.to_string(),
            free_pool: true,
            day: Some(month.to_string()),
        };
        usage::reserve(db, &charge, limits.free_seconds, need, limits).await.expect("giữ").expect("đủ");
        charge
    }

    /// Vòng đời hàng đợi + sổ phút trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri>
    /// TRANSCRIPT_TEST_DB_NAME=<db> cargo test transcript_queue -- --ignored`. User và tháng giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn transcript_queue_lifecycle_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let month = format!("test-{}", uuid::Uuid::new_v4().simple());
        let limits = TranscribeLimits {
            free_seconds: 10_000,
            plus_seconds: 10_000,
            free_pool_seconds: 10_000,
            monthly_cap_seconds: 10_000,
            free_daily_seconds: 10_000,
            free_delay_hours: 0,
        };
        let used = |db: Database, user: String, month: String| async move {
            usage::month_usage(&db, &user, &month).await.expect("đọc sổ").user_seconds
        };
        let (a, b, c) = (ObjectId::new(), ObjectId::new(), ObjectId::new());

        // Giữ 100 giây cho a và b, xếp hàng.
        let charge = take(&db, &user, &month, 100, &limits).await;
        assert!(enqueue(&db, a, &user, "run-1", "m", &charge, 60).await.expect("a"));
        assert!(enqueue(&db, b, &user, "run-1", "m", &charge, 40).await.expect("b"));
        // Xếp lại a khi a đang queued: bị từ chối (người gọi hoàn phần của nó).
        assert!(!enqueue(&db, a, &user, "run-x", "m", &charge, 60).await.expect("a lần hai"));

        // a chạy, gửi 50 giây rồi xong ⟹ quyết toán −10. b lỗi sau khi gửi 15 giây ⟹ hoàn 25.
        assert!(add_sent(&db, a, "run-1", 50).await.expect("sent a"));
        let sentence = Sentence { start: 0.0, end: 1.0, text: "Xin chào.".into() };
        assert!(finish_done(&db, a, "run-1", "Xin chào.", std::slice::from_ref(&sentence)).await.expect("xong a"));
        assert!(add_sent(&db, b, "run-1", 15).await.expect("sent b"));
        assert!(finish_failed(&db, b, "run-1", "lỗi").await.expect("lỗi b"));
        // Đóng lần hai (ghi lại kết quả) không quyết toán lần nữa.
        assert!(!finish_failed(&db, b, "run-1", "lỗi").await.expect("lỗi b lần hai"));
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 65);

        // Chép lại b (run-2) rồi lượt cũ run-1 mới báo lỗi muộn: không chạm run-2.
        let charge2 = take(&db, &user, &month, 40, &limits).await;
        assert!(enqueue(&db, b, &user, "run-2", "m", &charge2, 40).await.expect("b lại"));
        assert!(!finish_failed(&db, b, "run-1", "muộn").await.expect("muộn"));
        assert!(!add_sent(&db, b, "run-1", 99).await.expect("gửi muộn"));
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 105);

        // Xoá b khi run-2 đã gửi 10 giây ⟹ thu đúng 10 (hoàn 30).
        assert!(add_sent(&db, b, "run-2", 10).await.expect("sent b2"));
        delete(&db, b).await.expect("xoá b");
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 75);

        // Khởi động lại khi c đang chạy: lần một xếp lại, lần hai dừng và quyết toán.
        let charge3 = take(&db, &user, &month, 30, &limits).await;
        assert!(enqueue(&db, c, &user, "run-1", "m", &charge3, 30).await.expect("c"));
        let coll = db.collection::<TranscriptDoc>(COLL);
        coll.update_one(doc! { "_id": c }, doc! { "$set": { "status": "running" } }).await.expect("chạy");
        assert!(add_sent(&db, c, "run-1", 5).await.expect("sent c"));
        requeue_interrupted(&db).await.expect("lần một");
        let doc_c = coll.find_one(doc! { "_id": c }).await.expect("đọc").expect("c");
        assert_eq!((doc_c.status.as_str(), doc_c.attempts), ("queued", 1));
        coll.update_one(doc! { "_id": c }, doc! { "$set": { "status": "running" } }).await.expect("chạy lại");
        let (requeued, stopped) = requeue_interrupted(&db).await.expect("lần hai");
        assert!(stopped >= 1, "requeued={requeued} stopped={stopped}");
        let doc_c = coll.find_one(doc! { "_id": c }).await.expect("đọc").expect("c");
        assert_eq!(doc_c.status, "failed");
        // c đã gửi 5 giây trên 30 giây giữ chỗ ⟹ sổ = 75 + 5.
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 80);

        // Đóng được ghi nhưng báo lỗi (giả lập: open=false, settled=false), rồi thử lại: quyết toán đúng một lần.
        let d = ObjectId::new();
        let charge4 = take(&db, &user, &month, 20, &limits).await;
        assert!(enqueue(&db, d, &user, "run-1", "m", &charge4, 20).await.expect("d"));
        assert!(add_sent(&db, d, "run-1", 8).await.expect("sent d"));
        coll.update_one(doc! { "_id": d }, doc! { "$set": { "open": false, "settled": false, "status": "failed" } })
            .await
            .expect("đóng nửa chừng");
        assert!(!finish_failed(&db, d, "run-1", "lỗi").await.expect("thử lại"));
        assert!(!finish_failed(&db, d, "run-1", "lỗi").await.expect("thử lại lần hai"));
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 88);

        // Doc dở của bản cũ (không có sổ): khởi động thì báo lỗi, không kẹt.
        let legacy = ObjectId::new();
        db.collection::<bson::Document>(COLL)
            .insert_one(doc! { "_id": legacy, "userId": &user, "status": "running", "progress": 10.0,
                "runId": "old", "model": "m", "updatedAt": DateTime::now() })
            .await
            .expect("doc cũ");
        requeue_interrupted(&db).await.expect("khởi động");
        let doc_l = coll.find_one(doc! { "_id": legacy }).await.expect("đọc").expect("legacy");
        assert_eq!(doc_l.status, "failed");

        // Sổ chi tiết có đủ lượt đã quyết toán (a, b run-1, b run-2, c, d).
        let runs = db
            .collection::<bson::Document>(usage::RUNS_COLL)
            .count_documents(doc! { "userId": &user, "month": &month })
            .await
            .expect("đếm");
        assert_eq!(runs, 5);

        delete(&db, a).await.expect("dọn a");
        delete(&db, c).await.expect("dọn c");
        delete(&db, d).await.expect("dọn d");
        delete(&db, legacy).await.expect("dọn legacy");
        db.collection::<bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
    }
}
