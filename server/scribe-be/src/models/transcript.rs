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
//!
//! Lượt chạy bằng key riêng của người dùng ([`Billing::Own`]) chỉ ghi nhà cung cấp (`byok`), không có tháng/sổ nào:
//! không giữ chỗ, không quyết toán; lúc đóng chỉ ghi sổ chi tiết (`byok`) và sổ phút riêng của người dùng để họ xem.
//! Key không bao giờ nằm trong doc: worker đọc và giải mã key lúc chạy.
//!
//! Mỗi lượt ghi `tenantId: "primary"` và worker chỉ nhận lượt của tenant này ([`claim_next`]). Doc cũ có `keySource`
//! (thời còn key của chủ domain) vẫn đọc được: trường lạ bị bỏ qua.

use crate::config::Vendor;
use crate::error::AppError;
use crate::tenant::PRIMARY_ID;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Lượt chạy bằng key riêng của người dùng: mã nhà cung cấp (`elevenlabs` | `openai` | `gemini`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byok: Option<String>,
}

/// Ai trả tiền cho một lượt.
#[derive(Clone, Copy, Debug)]
pub enum Billing<'a> {
    /// Key server: phút đã giữ trên các sổ của `Charge`.
    Server(&'a Charge),
    /// Key riêng của người dùng, nhà cung cấp này.
    Own(Vendor),
}

impl TranscriptDoc {
    /// Nhà cung cấp của key riêng khi lượt chạy bằng key của người dùng.
    pub fn own_provider(&self) -> Option<Vendor> {
        self.byok.as_deref().and_then(Vendor::parse)
    }

    pub fn file_id(&self) -> String {
        self.id.to_hex()
    }

    pub fn charge(&self) -> Option<Charge> {
        if self.byok.is_some() {
            return None;
        }
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
    if let Some(provider) = &before.byok {
        // Key riêng: không sổ nào của Scribe đổi; ghi số giây cho người dùng xem và sổ chi tiết.
        let month = usage::month_key(chrono::Utc::now());
        if let Err(e) = usage::add_byok(db, &before.user_id, &month, before.sent_s).await {
            tracing::warn!(file = %before.id, "ghi giây chép bằng key riêng lỗi: {e:?}");
        }
        let run = doc! {
            "fileId": before.id,
            "runId": &before.run_id,
            "userId": &before.user_id,
            "month": &month,
            "byok": provider,
            "sentS": before.sent_s,
            "at": DateTime::now(),
        };
        if let Err(e) = db.collection::<bson::Document>(usage::RUNS_COLL).insert_one(run).await {
            tracing::error!(file = %before.id, "ghi sổ lượt chép bằng key riêng lỗi: {e}");
        }
        return;
    }
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
    // `claim_next`: lượt chờ sớm nhất của tenant duy nhất.
    coll.create_index(IndexModel::builder().keys(doc! { "status": 1, "tenantId": 1, "updatedAt": 1 }).build())
        .await
        .map_err(|e| format!("index transcripts.status_tenantId: {e}"))?;
    Ok(())
}

/// Trạng thái của file đã xếp nhưng chưa được nhận: worker chỉ lấy `queued`. Một lượt nhiều file (cả phiên) xếp ở
/// trạng thái này rồi mới bật cùng lúc ([`release_held`]), để worker không nhận lượt khi mới xếp được một phần.
pub const HELD: &str = "held";
/// Trạng thái còn trong hàng đợi (chưa kết thúc).
pub const ACTIVE: [&str; 3] = ["queued", "running", HELD];

/// Trạng thái trả cho client: [`HELD`] (chỉ tồn tại trong lúc xếp) là `queued`.
pub fn public_status(status: &str) -> &str {
    if status == HELD {
        "queued"
    } else {
        status
    }
}

/// Xếp hàng (hoặc xếp lại) một file với phần phút đã giữ; xoá kết quả cũ. Trả `false` khi file đang
/// `queued/running/held` (một lượt khác giữ nó): người gọi hoàn phần vừa giữ.
pub async fn enqueue(
    db: &Database,
    file_id: ObjectId,
    user_id: &str,
    run_id: &str,
    model: &str,
    billing: Billing<'_>,
    charged_s: i64,
) -> Result<bool, AppError> {
    enqueue_as(db, file_id, user_id, run_id, model, billing, charged_s, "queued").await
}

/// Như [`enqueue`] nhưng ở trạng thái [`HELD`]: chưa được nhận cho tới khi [`release_held`].
pub async fn enqueue_held(
    db: &Database,
    file_id: ObjectId,
    user_id: &str,
    run_id: &str,
    model: &str,
    billing: Billing<'_>,
    charged_s: i64,
) -> Result<bool, AppError> {
    enqueue_as(db, file_id, user_id, run_id, model, billing, charged_s, HELD).await
}

/// Bật mọi file [`HELD`] của lượt `run_id` sang `queued` trong một lệnh.
pub async fn release_held(db: &Database, user_id: &str, run_id: &str) -> Result<u64, AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .update_many(
            doc! { "userId": user_id, "runId": run_id, "status": HELD },
            doc! { "$set": { "status": "queued", "updatedAt": DateTime::now() } },
        )
        .await
        .map(|r| r.modified_count)
        .map_err(|e| AppError::Internal(format!("bật lượt chép lời: {e}")))
}

#[allow(clippy::too_many_arguments)]
async fn enqueue_as(
    db: &Database,
    file_id: ObjectId,
    user_id: &str,
    run_id: &str,
    model: &str,
    billing: Billing<'_>,
    charged_s: i64,
    status: &str,
) -> Result<bool, AppError> {
    let mut set = doc! {
        "userId": user_id,
        "tenantId": PRIMARY_ID,
        "status": status,
        "progress": 0.0,
        "runId": run_id,
        "text": "",
        "sentences": [],
        "model": model,
        "updatedAt": DateTime::now(),
        "chargedS": charged_s,
        "sentS": 0_i64,
        "open": true,
        "settled": false,
        "attempts": 0,
    };
    let mut unset = doc! { "error": "" };
    match billing {
        Billing::Server(charge) => {
            set.insert("month", &charge.month);
            set.insert("freePool", charge.free_pool);
            set.insert("day", charge.day.as_deref());
            unset.insert("byok", "");
        }
        Billing::Own(provider) => {
            set.insert("byok", provider.code());
            set.insert("chargedS", 0_i64);
            for field in ["month", "freePool", "day"] {
                unset.insert(field, "");
            }
        }
    }
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(
            doc! { "_id": file_id, "userId": user_id, "status": { "$nin": ACTIVE.to_vec() } },
            doc! { "$set": set, "$unset": unset },
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

/// Lấy lượt chờ lâu nhất của tenant duy nhất → running; lượt mang `tenantId` khác không bao giờ được nhận. Lượt bị hoãn
/// ([`defer_run`]) có `updatedAt` ở tương lai: chưa tới lượt.
pub async fn claim_next(db: &Database) -> Result<Option<TranscriptDoc>, mongodb::error::Error> {
    claim_next_in(db, doc! {}).await
}

/// [`claim_next`] trong phạm vi `scope` (test chỉ nhận lượt của user giả của nó).
async fn claim_next_in(db: &Database, scope: bson::Document) -> Result<Option<TranscriptDoc>, mongodb::error::Error> {
    // Điều kiện của hàng đợi ghi sau cùng: `scope` không đè được trạng thái hay `tenantId`.
    let mut filter = scope;
    filter.extend(doc! { "status": "queued", "tenantId": PRIMARY_ID, "updatedAt": { "$lte": DateTime::now() } });
    db.collection::<TranscriptDoc>(COLL)
        .find_one_and_update(filter, doc! { "$set": { "status": "running", "updatedAt": DateTime::now() } })
        .sort(doc! { "updatedAt": 1 })
        .return_document(ReturnDocument::After)
        .await
}

/// Hoãn mọi file còn chờ hoặc vừa nhận của lượt `run_id` (phiên iPhone còn đoạn đang tải lên) tới `until`.
pub async fn defer_run(db: &Database, user_id: &str, run_id: &str, until: DateTime) -> Result<(), AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .update_many(
            doc! { "userId": user_id, "runId": run_id, "open": true, "status": { "$in": ["queued", "running"] } },
            doc! { "$set": { "status": "queued", "updatedAt": until } },
        )
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("hoãn lượt chép lời: {e}")))
}

/// Trả một file vừa nhận về hàng đợi, chờ tới `until`. Worker gọi khi lượt của file đang chạy ở vòng khác. Chỉ đổi khi
/// file còn `running` của lượt `run_id` đang mở: lượt kia đã chép xong file này thì giữ nguyên kết quả.
pub async fn requeue_claimed(db: &Database, file_id: ObjectId, run_id: &str, until: DateTime) -> Result<(), AppError> {
    db.collection::<TranscriptDoc>(COLL)
        .update_one(
            doc! { "_id": file_id, "runId": run_id, "open": true, "status": "running" },
            doc! { "$set": { "status": "queued", "updatedAt": until } },
        )
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("trả lượt chép lời về hàng đợi: {e}")))
}

/// Id mọi file của lượt `run_id` (mọi trạng thái).
pub async fn run_file_ids(db: &Database, user_id: &str, run_id: &str) -> Result<std::collections::HashSet<ObjectId>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc file của lượt: {e}"));
    let mut cur = db
        .collection::<bson::Document>(COLL)
        .find(doc! { "userId": user_id, "runId": run_id })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(read)?;
    let mut out = std::collections::HashSet::new();
    while let Some(d) = cur.next().await {
        if let Ok(id) = d.map_err(read)?.get_object_id("_id") {
            out.insert(id);
        }
    }
    Ok(out)
}

/// Nhận các file còn chờ của cùng lượt `run_id` (chép cả phiên trong một request) → running, trả mọi file đang chạy
/// của lượt trừ `except` (file worker vừa lấy).
pub async fn claim_run_members(
    db: &Database,
    user_id: &str,
    run_id: &str,
    except: ObjectId,
) -> Result<Vec<TranscriptDoc>, AppError> {
    let coll = db.collection::<TranscriptDoc>(COLL);
    let read = |e: mongodb::error::Error| AppError::Internal(format!("nhận lượt chép cả phiên: {e}"));
    coll.update_many(
        doc! { "userId": user_id, "runId": run_id, "status": "queued", "open": true },
        doc! { "$set": { "status": "running", "updatedAt": DateTime::now() } },
    )
    .await
    .map_err(read)?;
    let mut cur = coll
        .find(doc! { "userId": user_id, "runId": run_id, "status": "running", "open": true, "_id": { "$ne": except } })
        .await
        .map_err(read)?;
    let mut out = Vec::new();
    while let Some(t) = cur.next().await {
        out.push(t.map_err(read)?);
    }
    Ok(out)
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

/// Chỉ ghi lượt còn mở: tiến độ ghi muộn không đè lên lượt đã xong hay đã lỗi. Trả `false` khi lượt `run_id` không
/// còn mở (đã xếp lại, đã đóng, hoặc file đã bị xoá): người gọi nên dừng.
pub async fn set_progress(db: &Database, file_id: ObjectId, run_id: &str, progress: f64) -> Result<bool, AppError> {
    let res = db
        .collection::<TranscriptDoc>(COLL)
        .update_one(
            doc! { "_id": file_id, "runId": run_id, "open": true },
            doc! { "$set": { "progress": progress, "updatedAt": DateTime::now() } },
        )
        .await
        .map_err(|e| AppError::Internal(format!("cập nhật chép lời: {e}")))?;
    Ok(res.matched_count > 0)
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

/// Panic giữa lượt: mọi file còn mở của lượt `run_id` đóng với lỗi `error` (không chỉ file worker đã nhận).
pub async fn fail_open_run(db: &Database, user_id: &str, run_id: &str, error: &str) -> Result<u64, AppError> {
    let mut cur = db
        .collection::<TranscriptDoc>(COLL)
        .find(doc! { "userId": user_id, "runId": run_id, "open": true })
        .await
        .map_err(|e| AppError::Internal(format!("đọc lượt dở: {e}")))?;
    let mut ids = Vec::new();
    while let Some(t) = cur.next().await {
        ids.push(t.map_err(|e| AppError::Internal(format!("đọc lượt dở: {e}")))?.id);
    }
    let mut closed = 0;
    for id in ids {
        if finish_failed(db, id, run_id, error).await? {
            closed += 1;
        }
    }
    Ok(closed)
}

/// Khi khởi động: lượt đang `running` bị cắt ngang thì xếp lại (key server luôn có sẵn). Lượt đã bị cắt
/// ngang một lần rồi thì dừng hẳn và quyết toán, tránh vòng khởi động lại cứ chi tiền mãi.
/// Trả (số lượt xếp lại, số lượt dừng).
pub async fn requeue_interrupted(db: &Database) -> Result<(u64, u64), AppError> {
    let coll = db.collection::<TranscriptDoc>(COLL);
    // Lượt xếp dở (tiến trình chết trước khi bật): bật luôn, phút đã giữ.
    coll.update_many(doc! { "status": HELD }, doc! { "$set": { "status": "queued", "updatedAt": DateTime::now() } })
        .await
        .map_err(|e| AppError::Internal(format!("bật lượt xếp dở: {e}")))?;
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
        .find(doc! { "open": false, "settled": false, "$or": [{ "month": { "$exists": true } }, { "byok": { "$exists": true } }] })
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
            out.push((id.to_hex(), public_status(status).to_string()));
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
        status: public_status(d.get_str("status").ok()?).to_string(),
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

/// Xoá mọi lượt chép của user (xoá tài khoản), quyết toán lượt còn mở như khi xoá file. Trả số doc đã xoá.
pub async fn delete_user(db: &Database, user_id: &str) -> Result<u64, AppError> {
    let ids: Vec<ObjectId> = db
        .collection::<bson::Document>(COLL)
        .distinct("_id", doc! { "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("tìm lượt chép của user: {e}")))?
        .into_iter()
        .filter_map(|b| b.as_object_id())
        .collect();
    delete_owned(db, user_id, &ids).await
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

    #[test]
    fn an_own_key_run_has_no_ledger_to_charge() {
        let own: TranscriptDoc = bson::from_document(doc! {
            "_id": ObjectId::new(), "userId": "u", "status": "queued", "progress": 0.0, "runId": "r", "model": "scribe_v2",
            "updatedAt": DateTime::now(), "byok": "elevenlabs", "chargedS": 0_i64, "open": true,
        })
        .expect("doc");
        assert_eq!(own.own_provider(), Some(Vendor::ElevenLabs));
        assert!(own.charge().is_none(), "không giữ chỗ, không quyết toán sổ nào");
        let mut server = own.clone();
        server.byok = None;
        server.month = Some("2026-10".into());
        assert!(server.charge().is_some() && server.own_provider().is_none());
    }

    /// Doc ghi thời còn key của chủ domain (`keySource: "tenant"`) vẫn đọc được.
    #[test]
    fn an_old_key_source_still_deserializes() {
        let old: TranscriptDoc = bson::from_document(doc! {
            "_id": ObjectId::new(), "userId": "u", "tenantId": "primary", "status": "done", "progress": 100.0, "runId": "r",
            "model": "gpt-transcribe", "updatedAt": DateTime::now(), "byok": "openai", "keySource": "tenant", "chargedS": 0_i64,
        })
        .expect("doc có keySource cũ");
        assert_eq!(old.own_provider(), Some(Vendor::OpenAi));
    }

    /// Chỉ lượt của tenant duy nhất được nhận: lượt mang `tenantId` khác (hay thiếu `tenantId`) nằm yên ở `queued`, kể
    /// cả khi chờ lâu hơn. `cargo test transcripts_of_other_tenants -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn transcripts_of_other_tenants_are_never_claimed_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let coll = db.collection::<bson::Document>(COLL);
        let (other, untagged, mine) = (ObjectId::new(), ObjectId::new(), ObjectId::new());
        let queued = |id: ObjectId, tenant: Option<&str>| {
            let mut d = doc! { "_id": id, "userId": &user, "status": "queued", "progress": 0.0, "runId": "r-old",
                               "model": "m", "byok": "openai", "chargedS": 0_i64, "open": true, "updatedAt": DateTime::now() };
            if let Some(t) = tenant {
                d.insert("tenantId", t);
            }
            d
        };
        coll.insert_many([queued(other, Some("ten_other")), queued(untagged, None)]).await.expect("lượt lạ");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        assert!(enqueue(&db, mine, &user, "r-mine", "m", Billing::Own(Vendor::OpenAi), 0).await.expect("xếp"));
        let stored = coll.find_one(doc! { "_id": mine }).await.expect("đọc").expect("có");
        assert_eq!(stored.get_str("tenantId"), Ok(PRIMARY_ID), "lượt mới ghi tenantId của tenant duy nhất");
        let scope = doc! { "userId": &user };
        let claimed = claim_next_in(&db, scope.clone()).await.expect("nhận").expect("có lượt tenant chính");
        assert_eq!(claimed.id, mine);
        // Chỉ xét doc lạ: test khác chạy song song có thể xếp lại mọi lượt `running` của collection.
        let foreign = doc! { "_id": { "$in": [other, untagged] } };
        assert!(claim_next_in(&db, foreign.clone()).await.expect("nhận").is_none(), "lượt tenant khác không bao giờ được nhận");
        let mut still_queued = foreign;
        still_queued.insert("status", "queued");
        assert_eq!(coll.count_documents(still_queued).await.expect("đếm"), 2);
        coll.delete_many(scope).await.expect("dọn");
    }

    /// Worker trả file của lượt đang chạy ở vòng khác về hàng đợi: file chờ tới hạn mới được nhận lại; file lượt kia đã
    /// chép xong thì giữ nguyên. `cargo test busy_run_file -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn busy_run_file_is_requeued_but_finished_file_is_kept_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let coll = db.collection::<bson::Document>(COLL);
        let (busy, finished) = (ObjectId::new(), ObjectId::new());
        assert!(enqueue(&db, busy, &user, "r-busy", "m", Billing::Own(Vendor::OpenAi), 0).await.expect("xếp"));
        let scope = doc! { "userId": &user };
        let claimed = claim_next_in(&db, scope.clone()).await.expect("nhận").expect("có lượt");
        assert_eq!(claimed.id, busy);
        let until = DateTime::from_millis(DateTime::now().timestamp_millis() + 60_000);
        requeue_claimed(&db, busy, "r-busy", until).await.expect("trả về hàng đợi");
        let stored = coll.find_one(doc! { "_id": busy }).await.expect("đọc").expect("có");
        assert_eq!(stored.get_str("status"), Ok("queued"));
        assert!(claim_next_in(&db, scope.clone()).await.expect("nhận").is_none(), "chưa tới hạn thì chưa nhận lại");

        coll.insert_one(doc! { "_id": finished, "userId": &user, "status": "done", "progress": 1.0, "runId": "r-done",
                               "model": "m", "chargedS": 0_i64, "open": false, "tenantId": PRIMARY_ID,
                               "updatedAt": DateTime::now() })
            .await
            .expect("lượt xong");
        requeue_claimed(&db, finished, "r-done", until).await.expect("không lỗi");
        let kept = coll.find_one(doc! { "_id": finished }).await.expect("đọc").expect("có");
        assert_eq!(kept.get_str("status"), Ok("done"), "lượt đã đóng không bị xếp lại");
        coll.delete_many(scope).await.expect("dọn");
    }

    /// Lượt bằng key riêng trên Mongo thật (DB dev): không đụng sổ phút, quỹ free, trần tổng; giây đã gửi vào sổ riêng
    /// `b:` của người dùng và sổ chi tiết có `byok`. `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// own_key_run -- --ignored`. User và tháng giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn own_key_run_is_not_metered_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let month = usage::month_key(chrono::Utc::now());
        let before = usage::month_usage(&db, &user, &month).await.expect("sổ");
        let id = ObjectId::new();
        assert!(enqueue(&db, id, &user, "run-own", "scribe_v2", Billing::Own(Vendor::ElevenLabs), 0).await.expect("xếp"));
        let queued = db.collection::<TranscriptDoc>(COLL).find_one(doc! { "_id": id }).await.expect("đọc").expect("có");
        assert_eq!((queued.byok.as_deref(), queued.month.as_deref(), queued.charged_s), (Some("elevenlabs"), None, 0));
        assert!(add_sent(&db, id, "run-own", 90).await.expect("gửi"));
        assert!(finish_failed(&db, id, "run-own", "lỗi").await.expect("đóng"));
        let after = usage::month_usage(&db, &user, &month).await.expect("sổ");
        assert_eq!(after.user_seconds, before.user_seconds, "sổ phút của user không đổi");
        assert_eq!(usage::byok_seconds(&db, &user, &month).await.expect("sổ riêng"), 90);
        let run = db.collection::<bson::Document>(usage::RUNS_COLL).find_one(doc! { "userId": &user }).await.expect("đọc").expect("có");
        assert_eq!(run.get_str("byok"), Ok("elevenlabs"));
        assert!(run.get("chargedS").is_none() && run.get("freePool").is_none());
        // Xếp lại cùng file bằng key server: dấu `byok` bị gỡ.
        let charge = Charge { user_id: user.clone(), month: format!("test-{}", uuid::Uuid::new_v4().simple()), free_pool: false, day: None };
        assert!(enqueue(&db, id, &user, "run-server", "scribe_v2", Billing::Server(&charge), 0).await.expect("xếp lại"));
        let again = db.collection::<TranscriptDoc>(COLL).find_one(doc! { "_id": id }).await.expect("đọc").expect("có");
        assert!(again.byok.is_none() && again.charge().is_some());

        delete(&db, id).await.expect("dọn");
        db.collection::<bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$in": [format!("b:{user}:{month}"), format!("u:{user}:{}", charge.month)] } })
            .await
            .expect("dọn");
        db.collection::<bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{}$", charge.month) } })
            .await
            .expect("dọn");
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
        assert!(enqueue(&db, a, &user, "run-1", "m", Billing::Server(&charge), 60).await.expect("a"));
        assert!(enqueue(&db, b, &user, "run-1", "m", Billing::Server(&charge), 40).await.expect("b"));
        // Xếp lại a khi a đang queued: bị từ chối (người gọi hoàn phần của nó).
        assert!(!enqueue(&db, a, &user, "run-x", "m", Billing::Server(&charge), 60).await.expect("a lần hai"));

        // a chạy, gửi 50 giây rồi xong ⟹ quyết toán −10. b lỗi sau khi gửi 15 giây ⟹ hoàn 25.
        assert!(add_sent(&db, a, "run-1", 50).await.expect("sent a"));
        let sentence = Sentence { start: 0.0, end: 1.0, text: "Xin chào.".into(), speaker: None };
        assert!(finish_done(&db, a, "run-1", "Xin chào.", std::slice::from_ref(&sentence)).await.expect("xong a"));
        assert!(add_sent(&db, b, "run-1", 15).await.expect("sent b"));
        assert!(finish_failed(&db, b, "run-1", "lỗi").await.expect("lỗi b"));
        // Đóng lần hai (ghi lại kết quả) không quyết toán lần nữa.
        assert!(!finish_failed(&db, b, "run-1", "lỗi").await.expect("lỗi b lần hai"));
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 65);

        // Chép lại b (run-2) rồi lượt cũ run-1 mới báo lỗi muộn: không chạm run-2.
        let charge2 = take(&db, &user, &month, 40, &limits).await;
        assert!(enqueue(&db, b, &user, "run-2", "m", Billing::Server(&charge2), 40).await.expect("b lại"));
        assert!(!finish_failed(&db, b, "run-1", "muộn").await.expect("muộn"));
        assert!(!add_sent(&db, b, "run-1", 99).await.expect("gửi muộn"));
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 105);

        // Xoá b khi run-2 đã gửi 10 giây ⟹ thu đúng 10 (hoàn 30).
        assert!(add_sent(&db, b, "run-2", 10).await.expect("sent b2"));
        delete(&db, b).await.expect("xoá b");
        assert_eq!(used(db.clone(), user.clone(), month.clone()).await, 75);

        // Khởi động lại khi c đang chạy: lần một xếp lại, lần hai dừng và quyết toán.
        let charge3 = take(&db, &user, &month, 30, &limits).await;
        assert!(enqueue(&db, c, &user, "run-1", "m", Billing::Server(&charge3), 30).await.expect("c"));
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
        assert!(enqueue(&db, d, &user, "run-1", "m", Billing::Server(&charge4), 20).await.expect("d"));
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

    /// Chép cả phiên trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> TRANSCRIPT_TEST_DB_NAME=<db> cargo test
    /// transcript_whole_session -- --ignored`. Các đoạn chung `runId` được nhận cùng nhau; lượt hoãn chưa tới giờ thì
    /// không được lấy. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn transcript_whole_session_run_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let run = format!("run-{}", uuid::Uuid::new_v4());
        let charge = Charge { user_id: user.clone(), month: format!("test-{}", uuid::Uuid::new_v4().simple()), free_pool: false, day: None };
        let ids = [ObjectId::new(), ObjectId::new(), ObjectId::new()];
        for id in ids {
            assert!(enqueue(&db, id, &user, &run, "scribe_v2", Billing::Server(&charge), 60).await.expect("xếp"));
        }
        let coll = db.collection::<TranscriptDoc>(COLL);
        let mine = doc! { "userId": &user };

        // Hoãn cả lượt tới tương lai: worker không lấy được file nào của lượt.
        let later = DateTime::from_millis(DateTime::now().timestamp_millis() + 60_000);
        defer_run(&db, &user, &run, later).await.expect("hoãn");
        let deferred = coll.count_documents(doc! { "userId": &user, "status": "queued", "updatedAt": later }).await.expect("đếm");
        assert_eq!(deferred, 3);
        // Bộ lọc của `claim_next` (không gọi thẳng: nó lấy lượt chờ của mọi user trong DB dev).
        let ready = doc! { "userId": &user, "status": "queued", "updatedAt": { "$lte": DateTime::now() } };
        assert_eq!(coll.count_documents(ready).await.expect("đếm"), 0, "lượt hoãn không được lấy sớm");

        // Hết hoãn: nhận file đầu, rồi các file còn lại của lượt thành running cùng lúc.
        coll.update_many(mine.clone(), doc! { "$set": { "updatedAt": DateTime::now() } }).await.expect("hết hoãn");
        coll.update_one(doc! { "_id": ids[0] }, doc! { "$set": { "status": "running" } }).await.expect("nhận đầu");
        let others = claim_run_members(&db, &user, &run, ids[0]).await.expect("nhận cả lượt");
        let mut got: Vec<ObjectId> = others.iter().map(|t| t.id).collect();
        got.sort();
        let mut want = vec![ids[1], ids[2]];
        want.sort();
        assert_eq!(got, want);
        assert_eq!(coll.count_documents(doc! { "userId": &user, "status": "running" }).await.expect("đếm"), 3);
        // Lượt khác (runId khác) không bị gom vào.
        assert!(claim_run_members(&db, &user, "run-khac", ids[0]).await.expect("lượt khác").is_empty());

        for id in ids {
            delete(&db, id).await.expect("dọn");
        }
        db.collection::<bson::Document>(usage::RUNS_COLL).delete_many(mine).await.expect("dọn sổ");
        db.collection::<bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{}$", charge.month) } })
            .await
            .expect("dọn");
    }
}
