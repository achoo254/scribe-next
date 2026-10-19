//! `user_keys`: một doc mỗi `{userId, provider}` (index unique) — bản mã AES-GCM và nonce của key, 4 ký tự cuối để
//! người dùng nhận ra key, lúc tạo/sửa/kiểm và trạng thái (`valid` | `invalid` | `quota` | `unreadable`).
//! `user_key_prefs`: lựa chọn nhà cung cấp của người dùng cho từng việc (`_id` = userId).
//!
//! Danh sách trả cho API ([`list`]) không bao giờ đọc bản mã (projection); chỉ [`get`] (dùng lúc chạy việc) đọc nó.

use super::crypto::Sealed;
use crate::config::Vendor;
use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use futures_util::StreamExt;
use mongodb::bson::{doc, spec::BinarySubtype, Binary, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use serde_json::{json, Value};

const COLL: &str = "user_keys";
const PREFS_COLL: &str = "user_key_prefs";

/// Trạng thái của key đã lưu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Đã kiểm lúc lưu, chưa lỗi lúc dùng.
    Valid,
    /// Nhà cung cấp từ chối key lúc dùng (sai, thu hồi, thiếu quyền).
    Invalid,
    /// Tài khoản của key hết credit hoặc hạn mức.
    Quota,
    /// Không giải mã được (bí mật trên server đã đổi hoặc mất): phải nhập lại.
    Unreadable,
}

impl Status {
    pub fn code(self) -> &'static str {
        match self {
            Status::Valid => "valid",
            Status::Invalid => "invalid",
            Status::Quota => "quota",
            Status::Unreadable => "unreadable",
        }
    }

    fn parse(code: &str) -> Status {
        match code {
            "valid" => Status::Valid,
            "quota" => Status::Quota,
            "unreadable" => Status::Unreadable,
            _ => Status::Invalid,
        }
    }
}

/// Phần của key được phép cho người dùng xem.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInfo {
    pub vendor: Vendor,
    pub last4: String,
    pub status: Status,
    pub updated_at: DateTime,
    pub last_checked_at: Option<DateTime>,
}

impl KeyInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "provider": self.vendor.code(),
            "last4": self.last4,
            "status": self.status.code(),
            "updatedAt": self.updated_at.try_to_rfc3339_string().ok(),
            "lastCheckedAt": self.last_checked_at.and_then(|d| d.try_to_rfc3339_string().ok()),
        })
    }

    fn from_doc(d: &Document) -> Option<KeyInfo> {
        Some(KeyInfo {
            vendor: Vendor::parse(d.get_str("provider").ok()?)?,
            last4: d.get_str("last4").unwrap_or_default().to_string(),
            status: Status::parse(d.get_str("status").unwrap_or_default()),
            updated_at: *d.get_datetime("updatedAt").ok()?,
            last_checked_at: d.get_datetime("lastCheckedAt").ok().copied(),
        })
    }
}

/// Key đã lưu kèm bản mã (chỉ dùng lúc chạy việc).
pub struct StoredKey {
    pub info: KeyInfo,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

fn read_err(e: mongodb::error::Error) -> AppError {
    AppError::Internal(format!("đọc key riêng: {e}"))
}

pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<Document>(COLL)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "userId": 1, "provider": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await
        .map(|_| ())
        .map_err(|e| format!("index user_keys.userId_provider: {e}"))
}

fn binary(bytes: &[u8]) -> Binary {
    Binary { subtype: BinarySubtype::Generic, bytes: bytes.to_vec() }
}

/// Lưu (hoặc thay) key đã kiểm: trạng thái `valid`, giờ kiểm = giờ lưu.
pub async fn save(db: &Database, user_id: &str, vendor: Vendor, sealed: &Sealed, last4: &str) -> Result<KeyInfo, AppError> {
    let now = DateTime::now();
    let filter = doc! { "userId": user_id, "provider": vendor.code() };
    let update = doc! {
        "$set": {
            "ciphertext": binary(&sealed.ciphertext),
            "nonce": binary(&sealed.nonce),
            "last4": last4,
            "status": Status::Valid.code(),
            "updatedAt": now,
            "lastCheckedAt": now,
        },
        "$setOnInsert": { "createdAt": now },
    };
    let coll = db.collection::<Document>(COLL);
    // Hai lần lưu cùng lúc: bên thua upsert gặp trùng khoá, lần thử lại cập nhật doc bên kia vừa tạo.
    for attempt in 0..2 {
        match coll.update_one(filter.clone(), update.clone()).upsert(true).await {
            Ok(_) => break,
            Err(e) if attempt == 0 && is_duplicate_key(&e) => continue,
            Err(e) => return Err(AppError::Internal(format!("lưu key riêng: {e}"))),
        }
    }
    Ok(KeyInfo { vendor, last4: last4.to_string(), status: Status::Valid, updated_at: now, last_checked_at: Some(now) })
}

/// Các key của người dùng, không kèm bản mã.
pub async fn list(db: &Database, user_id: &str) -> Result<Vec<KeyInfo>, AppError> {
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id })
        .projection(doc! { "provider": 1, "last4": 1, "status": 1, "updatedAt": 1, "lastCheckedAt": 1 })
        .await
        .map_err(read_err)?;
    let mut out = Vec::new();
    while let Some(d) = cur.next().await {
        out.extend(KeyInfo::from_doc(&d.map_err(read_err)?));
    }
    out.sort_by_key(|k| Vendor::ALL.iter().position(|v| *v == k.vendor));
    Ok(out)
}

/// Key của một nhà cung cấp, kèm bản mã.
pub async fn get(db: &Database, user_id: &str, vendor: Vendor) -> Result<Option<StoredKey>, AppError> {
    let d = db
        .collection::<Document>(COLL)
        .find_one(doc! { "userId": user_id, "provider": vendor.code() })
        .await
        .map_err(read_err)?;
    let Some(d) = d else { return Ok(None) };
    let (Some(info), Ok(ciphertext), Ok(nonce)) = (KeyInfo::from_doc(&d), d.get_binary_generic("ciphertext"), d.get_binary_generic("nonce"))
    else {
        return Ok(None);
    };
    Ok(Some(StoredKey { info, ciphertext: ciphertext.clone(), nonce: nonce.clone() }))
}

/// Xoá key; `true` khi có key để xoá.
pub async fn delete(db: &Database, user_id: &str, vendor: Vendor) -> Result<bool, AppError> {
    db.collection::<Document>(COLL)
        .delete_one(doc! { "userId": user_id, "provider": vendor.code() })
        .await
        .map(|r| r.deleted_count > 0)
        .map_err(|e| AppError::Internal(format!("xoá key riêng: {e}")))
}

/// Đặt trạng thái của đúng bản key đã dùng (khớp `nonce`): người dùng vừa lưu key mới thì lỗi của key cũ không đè lên.
pub async fn set_status(db: &Database, user_id: &str, vendor: Vendor, nonce: &[u8], status: Status) -> Result<bool, AppError> {
    db.collection::<Document>(COLL)
        .update_one(
            doc! { "userId": user_id, "provider": vendor.code(), "nonce": binary(nonce) },
            doc! { "$set": { "status": status.code(), "lastCheckedAt": DateTime::now() } },
        )
        .await
        .map(|r| r.modified_count > 0)
        .map_err(|e| AppError::Internal(format!("ghi trạng thái key riêng: {e}")))
}

/// Lựa chọn nhà cung cấp đã lưu (mã thô; `super::Prefs::from_doc` kiểm lại).
pub async fn get_prefs(db: &Database, user_id: &str) -> Result<Option<Document>, AppError> {
    db.collection::<Document>(PREFS_COLL).find_one(doc! { "_id": user_id }).await.map_err(read_err)
}

pub async fn save_prefs(db: &Database, user_id: &str, prefs: Document) -> Result<(), AppError> {
    let mut set = prefs;
    set.insert("updatedAt", DateTime::now());
    db.collection::<Document>(PREFS_COLL)
        .update_one(doc! { "_id": user_id }, doc! { "$set": set })
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("lưu lựa chọn key riêng: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_json_has_no_key_material() {
        let info = KeyInfo {
            vendor: Vendor::OpenAi,
            last4: "1234".into(),
            status: Status::Quota,
            updated_at: DateTime::from_millis(0),
            last_checked_at: None,
        };
        let v = info.to_json();
        assert_eq!(v["provider"], "openai");
        assert_eq!(v["status"], "quota");
        let keys: Vec<&String> = v.as_object().expect("object").keys().collect();
        assert_eq!(keys.len(), 5);
        for banned in ["key", "ciphertext", "nonce", "secret", "value"] {
            assert!(v.get(banned).is_none(), "{banned}");
        }
        assert_eq!(Status::parse("lạ"), Status::Invalid);
    }

    /// Kho key trên Mongo thật (DB dev): lưu → danh sách không có bản mã → đọc ra giải được → đánh dấu chỉ đúng bản
    /// key đã dùng → lưu lại thay bản cũ → xoá; lựa chọn lưu được. `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=…
    /// cargo test user_key_store -- --ignored`. User giả riêng, key giả, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn user_key_store_on_real_mongo() {
        use super::super::crypto;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let secret = crypto::Secret::from_bytes(&[3u8; 32]).expect("bí mật");
        let first = crypto::seal(&secret, &user, "openai", "sk-test-first-key-0001").expect("mã hoá");
        save(&db, &user, Vendor::OpenAi, &first, "0001").await.expect("lưu");
        let listed = list(&db, &user).await.expect("danh sách");
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].last4.as_str(), listed[0].status), ("0001", Status::Valid));
        let raw = db.collection::<Document>(COLL).find_one(doc! { "userId": &user }).await.expect("đọc").expect("có");
        let stored_bytes = raw.get_binary_generic("ciphertext").expect("bản mã");
        assert!(!String::from_utf8_lossy(stored_bytes).contains("sk-test"), "Mongo không có bản rõ");
        let stored = get(&db, &user, Vendor::OpenAi).await.expect("đọc").expect("có");
        let plain = crypto::open(&secret, &user, "openai", &stored.ciphertext, &stored.nonce).expect("giải mã");
        assert_eq!(plain.as_str(), "sk-test-first-key-0001");

        // Lưu key mới, rồi lượt cũ mới báo lỗi bằng nonce cũ: key mới không bị đánh dấu.
        let second = crypto::seal(&secret, &user, "openai", "sk-test-second-key-0002").expect("mã hoá");
        save(&db, &user, Vendor::OpenAi, &second, "0002").await.expect("lưu lại");
        assert!(!set_status(&db, &user, Vendor::OpenAi, &first.nonce, Status::Invalid).await.expect("đánh dấu cũ"));
        assert_eq!(list(&db, &user).await.expect("danh sách")[0].status, Status::Valid);
        assert!(set_status(&db, &user, Vendor::OpenAi, &second.nonce, Status::Quota).await.expect("đánh dấu"));
        let listed = list(&db, &user).await.expect("danh sách");
        assert_eq!((listed.len(), listed[0].last4.as_str(), listed[0].status), (1, "0002", Status::Quota));

        save_prefs(&db, &user, doc! { "text": "openai", "textModel": "gpt-4.1-nano" }).await.expect("lựa chọn");
        assert_eq!(get_prefs(&db, &user).await.expect("đọc").expect("có").get_str("textModel"), Ok("gpt-4.1-nano"));

        assert!(delete(&db, &user, Vendor::OpenAi).await.expect("xoá"));
        assert!(!delete(&db, &user, Vendor::OpenAi).await.expect("xoá lần hai"));
        assert!(list(&db, &user).await.expect("danh sách").is_empty());
        db.collection::<Document>(PREFS_COLL).delete_one(doc! { "_id": &user }).await.expect("dọn");
    }
}
