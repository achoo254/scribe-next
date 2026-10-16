//! User model. Định danh đăng nhập là `idpSub` (tài khoản ở hoandat-auth). User tạo thời đăng nhập
//! Firebase còn `firebaseUid` và được gắn `idpSub` ở lần đăng nhập IdP đầu tiên, theo email đã xác minh.

use crate::error::AppError;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::error::{Error as MongoError, ErrorKind, WriteFailure};
use mongodb::options::{IndexOptions, ReturnDocument};
use mongodb::{Database, IndexModel};

const COLL: &str = "users";
const DUP_KEY: i32 = 11000;

/// Index unique chỉ trên doc có trường đó (partial): user mới không có `firebaseUid`, user cũ chưa có
/// `idpSub`. Index `firebaseUid_1` cũ (unique, không partial) sẽ coi mọi user mới là trùng `null`, nên
/// được thay. Gọi lúc khởi động, chặn khởi động nếu lỗi (đăng nhập dựa vào nó để không tạo user trùng).
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Document>(COLL);
    let names = coll.list_index_names().await.map_err(|e| format!("đọc index users: {e}"))?;
    if names.iter().any(|n| n == "firebaseUid_1") {
        coll.drop_index("firebaseUid_1").await.map_err(|e| format!("bỏ index firebaseUid cũ: {e}"))?;
    }
    for field in ["firebaseUid", "idpSub"] {
        let index = IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(
                IndexOptions::builder()
                    .name(format!("{field}_unique"))
                    .unique(true)
                    .partial_filter_expression(doc! { field: { "$exists": true } })
                    .build(),
            )
            .build();
        coll.create_index(index).await.map_err(|e| format!("tạo index {field}: {e}"))?;
    }
    Ok(())
}

/// Tài khoản IdP → `_id` user (hex). Đã có `idpSub` thì dùng; chưa có thì gắn vào user cũ (thời Firebase)
/// cùng email — nguyên tử, chỉ doc chưa có `idpSub` — rồi mới tạo mới. Người gọi bảo đảm email đã xác minh.
/// Trả (id, đã gắn user cũ?).
pub async fn find_or_link(
    db: &Database,
    sub: &str,
    email: &str,
    name: Option<&str>,
) -> Result<(String, bool), AppError> {
    let coll = db.collection::<Document>(COLL);
    let name_bson = name.map(|n| Bson::String(n.to_string())).unwrap_or(Bson::Null);
    let fail = |e: MongoError| AppError::Internal(format!("mongo user: {e}"));
    let refresh = doc! { "$set": { "email": email, "name": name_bson.clone(), "lastLoginAt": DateTime::now() } };
    if let Some(d) = coll
        .find_one_and_update(doc! { "idpSub": sub }, refresh.clone())
        .return_document(ReturnDocument::After)
        .await
        .map_err(fail)?
    {
        return extract_id(Some(d)).map(|id| (id, false));
    }
    let pattern = format!("^{}$", regex_escape(email));
    let linked = coll
        .find_one_and_update(
            doc! {
                "firebaseUid": { "$exists": true },
                "idpSub": { "$exists": false },
                "email": { "$regex": pattern, "$options": "i" },
            },
            doc! { "$set": { "idpSub": sub, "email": email, "name": name_bson.clone(), "lastLoginAt": DateTime::now() } },
        )
        .return_document(ReturnDocument::After)
        .await
        .map_err(fail)?;
    if linked.is_some() {
        return extract_id(linked).map(|id| (id, true));
    }
    let inserted = coll
        .insert_one(doc! {
            "idpSub": sub,
            "email": email,
            "name": name_bson,
            "createdAt": DateTime::now(),
            "lastLoginAt": DateTime::now(),
        })
        .await;
    match inserted {
        Ok(res) => res
            .inserted_id
            .as_object_id()
            .map(|o| (o.to_hex(), false))
            .ok_or_else(|| AppError::Internal("user _id không phải ObjectId".into())),
        // Hai lần đăng nhập đầu tiên chạy đua: bên thua đọc lại doc vừa tạo.
        Err(e) if is_duplicate_key(&e) => {
            let d = coll.find_one(doc! { "idpSub": sub }).await.map_err(fail)?;
            extract_id(d).map(|id| (id, false))
        }
        Err(e) => Err(fail(e)),
    }
}

/// `_id` của user có tài khoản IdP `sub`.
pub async fn find_id_by_sub(db: &Database, sub: &str) -> Result<Option<String>, AppError> {
    let d = db
        .collection::<Document>(COLL)
        .find_one(doc! { "idpSub": sub })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("mongo user: {e}")))?;
    Ok(d.and_then(|d| d.get_object_id("_id").ok().map(|o| o.to_hex())))
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

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// users_link -- --ignored`. Đổi index users của DB dev sang partial (như khi khởi động).
    #[tokio::test]
    #[ignore]
    async fn users_link_by_verified_email_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let email = format!("Old.User+{tag}@Example.vn");
        let coll = db.collection::<Document>(COLL);
        let old = coll
            .insert_one(doc! { "firebaseUid": format!("fb-{tag}"), "email": email.to_lowercase(), "createdAt": DateTime::now() })
            .await
            .expect("user cũ")
            .inserted_id;

        // Lần đầu đăng nhập IdP: gắn vào user cũ (email khác hoa thường), giữ nguyên `_id`.
        let (id, linked) = find_or_link(&db, &format!("acc_{tag}"), &email, None).await.expect("gắn");
        assert!(linked);
        assert_eq!(Some(id.clone()), old.as_object_id().map(|o| o.to_hex()));
        // Lần sau: theo `idpSub`.
        assert_eq!(find_or_link(&db, &format!("acc_{tag}"), &email, None).await.expect("lại"), (id.clone(), false));
        // Tài khoản IdP khác cùng email không chiếm được user đã gắn: tạo user mới.
        let (other, linked) = find_or_link(&db, &format!("acc2_{tag}"), &email, None).await.expect("khác");
        assert!(!linked);
        assert_ne!(other, id);
        // Nhiều user mới không có `firebaseUid` không đụng index unique.
        let (third, _) = find_or_link(&db, &format!("acc3_{tag}"), &format!("n3-{tag}@x.vn"), None).await.expect("mới");
        assert_eq!(find_id_by_sub(&db, &format!("acc3_{tag}")).await.expect("tìm"), Some(third.clone()));

        coll.delete_many(doc! { "email": { "$regex": tag.as_str() } }).await.expect("dọn");
    }

    #[test]
    fn emails_are_matched_literally() {
        assert_eq!(regex_escape("a.b+c@x.vn"), "a\\.b\\+c@x\\.vn");
    }
}
