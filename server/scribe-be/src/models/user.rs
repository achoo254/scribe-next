//! User model + upsert theo `firebaseUid` (Firebase UID = nguồn định danh ổn định).

use crate::error::AppError;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::error::{Error as MongoError, ErrorKind, WriteFailure};
use mongodb::options::{IndexOptions, ReturnDocument};
use mongodb::{Database, IndexModel};

const COLL: &str = "users";
const DUP_KEY: i32 = 11000;

/// Tạo unique index `firebaseUid` (idempotent) — chặn tạo user trùng khi 2 request upsert đua nhau.
/// Gọi lúc BE start; non-blocking (log lỗi, không chặn khởi động nếu Mongo tạm chưa sẵn).
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let index = IndexModel::builder()
        .keys(doc! { "firebaseUid": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    db.collection::<Document>(COLL)
        .create_index(index)
        .await
        .map_err(|e| format!("tạo unique index firebaseUid: {e}"))?;
    Ok(())
}

/// Upsert user theo `firebaseUid`; trả `_id` (hex) làm subject cho session BE.
/// `createdAt` chỉ set khi insert; email/name cập nhật mỗi lần login. Nếu 2 request đua chèn cùng
/// `firebaseUid` → unique index báo duplicate-key ở request thua → retry (giờ match filter → update).
pub async fn upsert_and_get_id(
    db: &Database,
    firebase_uid: &str,
    email: &str,
    name: Option<&str>,
) -> Result<String, AppError> {
    match run_upsert(db, firebase_uid, email, name).await {
        Ok(doc) => extract_id(doc),
        Err(e) if is_duplicate_key(&e) => {
            let doc = run_upsert(db, firebase_uid, email, name)
                .await
                .map_err(|e| AppError::Internal(format!("mongo upsert user (retry): {e}")))?;
            extract_id(doc)
        }
        Err(e) => Err(AppError::Internal(format!("mongo upsert user: {e}"))),
    }
}

/// Email + gói chép lời của user (gói hết hạn do `plan::effective` xử lý).
pub struct Profile {
    pub email: String,
    pub plan: Option<String>,
    pub plan_until: Option<DateTime>,
}

pub async fn find_profile(db: &Database, user_id: &str) -> Result<Option<Profile>, AppError> {
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized("user id không hợp lệ".into()))?;
    let doc = db
        .collection::<Document>(COLL)
        .find_one(doc! { "_id": oid })
        .projection(doc! { "email": 1, "plan": 1, "planUntil": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("mongo find user: {e}")))?;
    Ok(doc.map(|d| Profile {
        email: d.get_str("email").unwrap_or_default().to_string(),
        plan: d.get_str("plan").ok().map(str::to_string),
        plan_until: d.get_datetime("planUntil").ok().copied(),
    }))
}

/// Các user có email này (không phân biệt hoa thường). Dùng cho CLI admin: phải khớp đúng một user.
pub async fn find_ids_by_email(db: &Database, email: &str) -> Result<Vec<String>, AppError> {
    let pattern = format!("^{}$", regex_escape(email));
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "email": { "$regex": pattern, "$options": "i" } })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("mongo find user: {e}")))?;
    let mut ids = Vec::new();
    while let Some(d) = futures_util::StreamExt::next(&mut cur).await {
        let d = d.map_err(|e| AppError::Internal(format!("mongo find user: {e}")))?;
        if let Ok(id) = d.get_object_id("_id") {
            ids.push(id.to_hex());
        }
    }
    Ok(ids)
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| if "\\.^$|?*+()[]{}/".contains(c) { vec!['\\', c] } else { vec![c] })
        .collect()
}

/// Đặt gói; `until` = None với gói plus nghĩa là không hết hạn; gói free xoá hạn.
pub async fn set_plan(db: &Database, user_id: &str, plan: &str, until: Option<DateTime>) -> Result<(), AppError> {
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::BadRequest("user id không hợp lệ".into()))?;
    let update = match until {
        Some(until) => doc! { "$set": { "plan": plan, "planUntil": until } },
        None => doc! { "$set": { "plan": plan }, "$unset": { "planUntil": "" } },
    };
    db.collection::<Document>(COLL)
        .update_one(doc! { "_id": oid }, update)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("mongo set plan: {e}")))
}

async fn run_upsert(
    db: &Database,
    firebase_uid: &str,
    email: &str,
    name: Option<&str>,
) -> Result<Option<Document>, MongoError> {
    let name_bson = name.map(|n| Bson::String(n.to_string())).unwrap_or(Bson::Null);
    let update = doc! {
        "$set": { "email": email, "name": name_bson },
        "$setOnInsert": { "firebaseUid": firebase_uid, "createdAt": DateTime::now() },
    };
    db.collection::<Document>(COLL)
        .find_one_and_update(doc! { "firebaseUid": firebase_uid }, update)
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
}

fn extract_id(doc: Option<Document>) -> Result<String, AppError> {
    let doc = doc.ok_or_else(|| AppError::Internal("upsert không trả về document".into()))?;
    let id = doc
        .get_object_id("_id")
        .map_err(|e| AppError::Internal(format!("đọc _id: {e}")))?;
    Ok(id.to_hex())
}

/// Lỗi Mongo có phải duplicate-key (E11000) không — dùng để retry upsert khi đua unique index.
pub(crate) fn is_duplicate_key(e: &MongoError) -> bool {
    match *e.kind {
        ErrorKind::Write(WriteFailure::WriteError(ref we)) => we.code == DUP_KEY,
        ErrorKind::Command(ref ce) => ce.code == DUP_KEY,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_matched_literally() {
        assert_eq!(regex_escape("a.b+c@x.vn"), "a\\.b\\+c@x\\.vn");
    }
}
