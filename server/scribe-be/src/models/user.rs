//! User model. Định danh đăng nhập là `idpSub` (tài khoản ở auth-next), lưu cùng `tenantId: "primary"` (tenant duy
//! nhất) theo index unique `(tenantId, idpSub)`. User tạo thời đăng nhập Firebase còn `firebaseUid` và được gắn `idpSub`
//! ở lần đăng nhập IdP đầu tiên, theo email đã xác minh.

use crate::error::AppError;
use crate::tenant::PRIMARY_ID;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::error::{Error as MongoError, ErrorKind, WriteFailure};
use mongodb::options::{IndexOptions, ReturnDocument};
use mongodb::{Database, IndexModel};

const COLL: &str = "users";
const DUP_KEY: i32 = 11000;
/// Mongo: collection chưa có (database mới).
const NAMESPACE_NOT_FOUND: i32 = 26;
/// Index unique của danh tính đăng nhập.
const TENANT_SUB_INDEX: &str = "tenantId_idpSub_unique";
/// Index unique toàn cục của `idpSub` trước khi có trường `tenantId`: đã thay bằng [`TENANT_SUB_INDEX`], không tạo lại.
const GLOBAL_SUB_INDEX: &str = "idpSub_unique";

/// Index unique chỉ trên doc có trường đó (partial): user mới không có `firebaseUid`, user cũ chưa có
/// `idpSub`. Index `firebaseUid_1` cũ (unique, không partial) sẽ coi mọi user mới là trùng `null`, nên
/// được thay. `(tenantId, idpSub)` thay `idpSub_unique` (còn sót thì bỏ, sau khi index mới đã có, nên không lúc nào
/// thiếu ràng buộc). Gọi lúc khởi động, chặn khởi động nếu lỗi (đăng nhập dựa vào nó để không tạo user trùng).
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<Document>(COLL);
    let names = match coll.list_index_names().await {
        Ok(names) => names,
        Err(e) if command_code(&e) == Some(NAMESPACE_NOT_FOUND) => Vec::new(),
        Err(e) => return Err(format!("đọc index users: {e}")),
    };
    if names.iter().any(|n| n == "firebaseUid_1") {
        coll.drop_index("firebaseUid_1").await.map_err(|e| format!("bỏ index firebaseUid cũ: {e}"))?;
    }
    let unique = |keys: Document, name: &str, field: &str| {
        IndexModel::builder()
            .keys(keys)
            .options(
                IndexOptions::builder()
                    .name(name.to_string())
                    .unique(true)
                    .partial_filter_expression(doc! { field: { "$exists": true } })
                    .build(),
            )
            .build()
    };
    coll.create_index(unique(doc! { "firebaseUid": 1 }, "firebaseUid_unique", "firebaseUid"))
        .await
        .map_err(|e| format!("tạo index firebaseUid: {e}"))?;
    coll.create_index(unique(doc! { "tenantId": 1, "idpSub": 1 }, TENANT_SUB_INDEX, "idpSub"))
        .await
        .map_err(|e| format!("tạo index (tenantId, idpSub): {e}"))?;
    if names.iter().any(|n| n == GLOBAL_SUB_INDEX) {
        coll.drop_index(GLOBAL_SUB_INDEX).await.map_err(|e| format!("bỏ index idpSub toàn cục: {e}"))?;
    }
    Ok(())
}

fn command_code(e: &MongoError) -> Option<i32> {
    match *e.kind {
        ErrorKind::Command(ref ce) => Some(ce.code),
        _ => None,
    }
}

/// Tài khoản IdP `sub` → `_id` user (hex). Đã có `idpSub` thì dùng; chưa có thì gắn vào user cũ (thời Firebase) cùng
/// email — nguyên tử, chỉ doc chưa có `idpSub` — rồi mới tạo mới. Người gọi bảo đảm email đã xác minh. Tên và ảnh
/// lấy lại từ IdP mỗi lần đăng nhập (IdP không gửi thì xoá). Trả (id, đã gắn user cũ?).
pub async fn find_or_link(
    db: &Database,
    sub: &str,
    email: &str,
    name: Option<&str>,
    picture: Option<&str>,
) -> Result<(String, bool), AppError> {
    let coll = db.collection::<Document>(COLL);
    let text = |v: Option<&str>| v.map(|v| Bson::String(v.to_string())).unwrap_or(Bson::Null);
    let (name_bson, picture_bson) = (text(name), text(picture));
    let fail = |e: MongoError| AppError::Internal(format!("mongo user: {e}"));
    let identity = doc! { "tenantId": PRIMARY_ID, "idpSub": sub };
    let refresh = doc! { "$set": {
        "email": email, "name": name_bson.clone(), "picture": picture_bson.clone(), "lastLoginAt": DateTime::now(),
    } };
    if let Some(d) = coll
        .find_one_and_update(identity.clone(), refresh.clone())
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
                "tenantId": PRIMARY_ID,
                "firebaseUid": { "$exists": true },
                "idpSub": { "$exists": false },
                "email": { "$regex": pattern, "$options": "i" },
            },
            doc! { "$set": {
                "idpSub": sub, "email": email, "name": name_bson.clone(), "picture": picture_bson.clone(),
                "lastLoginAt": DateTime::now(),
            } },
        )
        .return_document(ReturnDocument::After)
        .await
        .map_err(fail)?;
    if linked.is_some() {
        return extract_id(linked).map(|id| (id, true));
    }
    let inserted = coll
        .insert_one(doc! {
            "tenantId": PRIMARY_ID,
            "idpSub": sub,
            "email": email,
            "name": name_bson,
            "picture": picture_bson,
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
            let d = coll.find_one(identity).await.map_err(fail)?;
            extract_id(d).map(|id| (id, false))
        }
        Err(e) => Err(fail(e)),
    }
}

/// `_id` của user có tài khoản IdP `sub`.
pub async fn find_id_by_sub(db: &Database, sub: &str) -> Result<Option<String>, AppError> {
    let d = db
        .collection::<Document>(COLL)
        .find_one(doc! { "tenantId": PRIMARY_ID, "idpSub": sub })
        .projection(doc! { "_id": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("mongo user: {e}")))?;
    Ok(d.and_then(|d| d.get_object_id("_id").ok().map(|o| o.to_hex())))
}

/// Email, tên, ảnh + gói chép lời của user (gói hết hạn do `plan::effective` xử lý).
pub struct Profile {
    pub email: String,
    /// Tên hiển thị từ IdP; user chưa đăng nhập lại từ khi Scribe lưu tên thì không có.
    pub name: Option<String>,
    /// URL ảnh đại diện (`https:`) từ IdP, như `name`.
    pub picture: Option<String>,
    pub plan: Option<String>,
    pub plan_until: Option<DateTime>,
    /// Lúc tạo tài khoản; user rất cũ không có trường này (coi như đã lâu).
    pub created_at: Option<DateTime>,
    /// Quản trị đã khoá tài khoản (`admin block`): không đăng nhập, không chép lời.
    pub blocked: bool,
    /// Người dùng đã xoá tài khoản, việc dọn đang chạy (`deletingAt`): coi như không còn.
    pub deleting: bool,
}

pub async fn find_profile(db: &Database, user_id: &str) -> Result<Option<Profile>, AppError> {
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized("user id không hợp lệ".into()))?;
    let doc = db
        .collection::<Document>(COLL)
        .find_one(doc! { "_id": oid })
        .projection(doc! { "email": 1, "name": 1, "picture": 1, "plan": 1, "planUntil": 1, "createdAt": 1, "blocked": 1, "deletingAt": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("mongo find user: {e}")))?;
    Ok(doc.map(|d| Profile {
        email: d.get_str("email").unwrap_or_default().to_string(),
        name: d.get_str("name").ok().map(str::to_string),
        picture: d.get_str("picture").ok().map(str::to_string),
        plan: d.get_str("plan").ok().map(str::to_string),
        plan_until: d.get_datetime("planUntil").ok().copied(),
        created_at: d.get_datetime("createdAt").ok().copied(),
        // Dấu khoá do CLI ghi là `{at, reason}`; `true` sửa tay cũng tính là khoá.
        blocked: d.get_document("blocked").is_ok() || d.get_bool("blocked").unwrap_or(false),
        deleting: d.get_datetime("deletingAt").is_ok(),
    }))
}

/// Đánh dấu tài khoản đang bị xoá: từ đây mọi đường đăng nhập và phiên coi tài khoản như không còn.
pub async fn mark_deleting(db: &Database, user_id: &str) -> Result<(), AppError> {
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::BadRequest("user id không hợp lệ".into()))?;
    db.collection::<Document>(COLL)
        .update_one(doc! { "_id": oid, "deletingAt": { "$exists": false } }, doc! { "$set": { "deletingAt": DateTime::now() } })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("đánh dấu xoá tài khoản: {e}")))
}

/// Gói đang có hiệu lực của user (dung lượng, hạn tệp). Không có user thì là free.
pub async fn effective_plan(db: &Database, user_id: &str) -> Result<super::plan::Plan, AppError> {
    let profile = find_profile(db, user_id).await?;
    let (stored, until) = profile.map_or((None, None), |p| (p.plan, p.plan_until));
    Ok(super::plan::effective(stored.as_deref(), until, DateTime::now()))
}

/// `_id` (hex) của các user đang có gói plus hiệu lực lúc `now`.
pub async fn plus_user_ids(db: &Database, now: DateTime) -> Result<Vec<String>, AppError> {
    let read = |e: MongoError| AppError::Internal(format!("mongo users theo gói: {e}"));
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "plan": "plus" })
        .projection(doc! { "_id": 1, "plan": 1, "planUntil": 1 })
        .await
        .map_err(read)?;
    let mut out = Vec::new();
    while let Some(d) = futures_util::StreamExt::next(&mut cur).await {
        let d = d.map_err(read)?;
        let until = d.get_datetime("planUntil").ok().copied();
        let plan = super::plan::effective(d.get_str("plan").ok(), until, now);
        if let (super::plan::Plan::Plus, Ok(id)) = (plan, d.get_object_id("_id")) {
            out.push(id.to_hex());
        }
    }
    Ok(out)
}

/// Khoá (`Some(lý do)`) hoặc mở khoá (`None`) tài khoản. Trả `false` khi không có user.
pub async fn set_blocked(db: &Database, user_id: &str, reason: Option<&str>) -> Result<bool, AppError> {
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::BadRequest("user id không hợp lệ".into()))?;
    let update = match reason {
        Some(reason) => doc! { "$set": { "blocked": { "at": DateTime::now(), "reason": reason } } },
        None => doc! { "$unset": { "blocked": "" } },
    };
    db.collection::<Document>(COLL)
        .update_one(doc! { "_id": oid }, update)
        .await
        .map(|r| r.matched_count > 0)
        .map_err(|e| AppError::Internal(format!("mongo block user: {e}")))
}

/// Các user có email này (không phân biệt hoa thường). Dùng cho CLI admin: phải khớp đúng một user.
pub async fn find_ids_by_email(db: &Database, email: &str) -> Result<Vec<String>, AppError> {
    let pattern = format!("^{}$", regex_escape(email));
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "tenantId": PRIMARY_ID, "email": { "$regex": pattern, "$options": "i" } })
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
            .insert_one(doc! { "tenantId": PRIMARY_ID, "firebaseUid": format!("fb-{tag}"), "email": email.to_lowercase(), "createdAt": DateTime::now() })
            .await
            .expect("user cũ")
            .inserted_id;

        // Lần đầu đăng nhập IdP: gắn vào user cũ (email khác hoa thường), giữ nguyên `_id`.
        let (id, linked) = find_or_link(&db, &format!("acc_{tag}"), &email, None, None).await.expect("gắn");
        assert!(linked);
        assert_eq!(Some(id.clone()), old.as_object_id().map(|o| o.to_hex()));
        // Lần sau: theo `idpSub`; tên và ảnh mới của IdP được lưu, IdP thôi gửi thì xoá.
        let picture = "https://lh3.googleusercontent.com/a/p1";
        assert_eq!(
            find_or_link(&db, &format!("acc_{tag}"), &email, Some("Minh Anh"), Some(picture)).await.expect("lại"),
            (id.clone(), false)
        );
        let profile = find_profile(&db, &id).await.expect("đọc").expect("có");
        assert_eq!((profile.name.as_deref(), profile.picture.as_deref()), (Some("Minh Anh"), Some(picture)));
        assert_eq!(find_or_link(&db, &format!("acc_{tag}"), &email, None, None).await.expect("lại"), (id.clone(), false));
        let profile = find_profile(&db, &id).await.expect("đọc").expect("có");
        assert_eq!((profile.name, profile.picture), (None, None));
        // Tài khoản IdP khác cùng email không chiếm được user đã gắn: tạo user mới.
        let (other, linked) = find_or_link(&db, &format!("acc2_{tag}"), &email, None, None).await.expect("khác");
        assert!(!linked);
        assert_ne!(other, id);
        // Nhiều user mới không có `firebaseUid` không đụng index unique.
        let (third, _) = find_or_link(&db, &format!("acc3_{tag}"), &format!("n3-{tag}@x.vn"), None, None).await.expect("mới");
        assert_eq!(find_id_by_sub(&db, &format!("acc3_{tag}")).await.expect("tìm"), Some(third.clone()));

        // User mới có `createdAt`; khoá rồi mở khoá.
        let profile = find_profile(&db, &third).await.expect("đọc").expect("có");
        assert!(profile.created_at.is_some() && !profile.blocked);
        assert!(set_blocked(&db, &third, Some("thử")).await.expect("khoá"));
        assert!(find_profile(&db, &third).await.expect("đọc").expect("có").blocked);
        assert!(set_blocked(&db, &third, None).await.expect("mở"));
        assert!(!find_profile(&db, &third).await.expect("đọc").expect("có").blocked);

        coll.delete_many(doc! { "email": { "$regex": tag.as_str() } }).await.expect("dọn");
    }

    /// `ensure_indexes` chỉ tạo index danh tính `tenantId_idpSub_unique`, không bao giờ tạo lại `idpSub_unique`; còn
    /// sót index cũ thì bỏ. Database riêng `<DB_NAME>_user_indexes` (bỏ index). `cargo test user_indexes -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn user_indexes_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &format!("{db_name}_user_indexes")).await.expect("kết nối Mongo");
        db.drop().await.expect("dọn");
        let names = |db: Database| async move {
            let mut names = db.collection::<Document>(COLL).list_index_names().await.expect("đọc index");
            names.sort();
            names
        };
        ensure_indexes(&db).await.expect("database mới");
        assert_eq!(names(db.clone()).await, vec!["_id_", "firebaseUid_unique", TENANT_SUB_INDEX]);
        // Index toàn cục còn sót từ thời trước: bị bỏ, không tạo lại.
        let global = IndexModel::builder()
            .keys(doc! { "idpSub": 1 })
            .options(IndexOptions::builder().name(GLOBAL_SUB_INDEX.to_string()).build())
            .build();
        db.collection::<Document>(COLL).create_index(global).await.expect("index cũ");
        ensure_indexes(&db).await.expect("lần hai");
        assert_eq!(names(db.clone()).await, vec!["_id_", "firebaseUid_unique", TENANT_SUB_INDEX]);
        db.drop().await.expect("dọn");
    }

    #[test]
    fn emails_are_matched_literally() {
        assert_eq!(regex_escape("a.b+c@x.vn"), "a\\.b\\+c@x\\.vn");
    }
}
