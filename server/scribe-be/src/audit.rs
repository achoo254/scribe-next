//! Audit trail: mỗi hành vi nghiệp vụ (đăng nhập/xuất, ghi, upload, cắt, tải, xoá) thành một doc
//! trong `audit_events` — tra lại được "ai làm gì, lúc nào, từ IP nào".
//!
//! Ghi nền (`tokio::spawn`): audit lỗi KHÔNG làm hỏng request của người dùng, chỉ log warn. IP lấy
//! từ task-local do `access_log` đặt cho request hiện tại, nên handler không cần nhận header.
//! TTL native trên `at` là đúng ở đây (khác `files`): audit không có file disk đi kèm để mồ côi.

use crate::state::AppState;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::time::Duration;

const COLL: &str = "audit_events";
/// Giữ audit 180 ngày (dài hơn retention file 14 ngày để còn tra được sau khi file đã tự xoá).
pub const AUDIT_RETENTION_DAYS: u64 = 180;

tokio::task_local! {
    /// IP client của request đang xử lý (đặt bởi `access_log::layer`).
    pub static CLIENT_IP: String;
}

/// Index tra theo user + thời gian, và TTL tự xoá sau `AUDIT_RETENTION_DAYS`.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Document>(COLL);
    coll.create_index(
        IndexModel::builder()
            .keys(doc! { "userId": 1, "at": -1 })
            .build(),
    )
    .await
    .map_err(|e| format!("index audit_events.userId: {e}"))?;
    coll.create_index(
        IndexModel::builder()
            .keys(doc! { "at": 1 })
            .options(
                IndexOptions::builder()
                    .expire_after(Duration::from_secs(AUDIT_RETENTION_DAYS * 86_400))
                    .build(),
            )
            .build(),
    )
    .await
    .map_err(|e| format!("index audit_events.at (TTL): {e}"))?;
    Ok(())
}

/// Ghi một sự kiện. `user_id` = None khi chưa xác định được user (vd đăng nhập bị từ chối).
/// `details` = các trường riêng của hành vi (fileId, name, size…), không chứa secret.
pub fn record(state: &AppState, user_id: Option<&str>, action: &str, details: Document) {
    let ip = CLIENT_IP
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "unknown".into());
    let mut event = doc! {
        "at": DateTime::now(),
        "action": action,
        "userId": user_id,
        "ip": ip,
    };
    event.extend(details);
    let db = state.db.clone();
    let action = action.to_string();
    tokio::spawn(async move {
        if let Err(e) = db.collection::<Document>(COLL).insert_one(event).await {
            tracing::warn!(action = %action, "ghi audit_events lỗi: {e}");
        }
    });
}
