//! Phiên đăng nhập lưu trong Mongo (thu hồi được), kèm trạng thái lượt đăng nhập web, thiết bị iPhone và
//! `jti` logout token đã nhận.
//!
//! - `auth_states`: `state` → `codeVerifier` + `nonce` của một lượt đăng nhập web, dùng một lần, TTL 10 phút.
//! - `sessions`: `_id` = băm token cookie; `client` web | mobile; web trượt tới 7 ngày, mobile 12 giờ và chỉ
//!   gia hạn qua làm mới ở IdP.
//! - `devices`: `_id` = băm token thiết bị của app; refresh token IdP mã hoá; khoá `refreshingAt` để chỉ
//!   một lần làm mới chạy (refresh token của IdP dùng một lần).
//! - `logout_jtis`: chống phát lại logout token, TTL 10 phút.

use crate::error::AppError;
use crate::session::{hash_token, random_token};
use futures_util::StreamExt;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::time::Duration;

const STATES: &str = "auth_states";
const SESSIONS: &str = "sessions";
const DEVICES: &str = "devices";
const JTIS: &str = "logout_jtis";

pub const WEB_TTL_S: i64 = 12 * 3600;
pub const WEB_MAX_AGE_S: i64 = 7 * 86_400;
pub const MOBILE_TTL_S: i64 = 12 * 3600;
/// Lần làm mới bị kẹt (tiến trình chết giữa chừng) quá mốc này thì cho lần sau làm tiếp.
const REFRESH_LOCK_S: i64 = 60;

fn after(seconds: i64) -> DateTime {
    DateTime::from_millis(DateTime::now().timestamp_millis() + seconds * 1000)
}

fn internal(what: &'static str) -> impl Fn(mongodb::error::Error) -> AppError {
    move |e| AppError::Internal(format!("{what}: {e}"))
}

pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let ttl = |field: &str, secs: u64| {
        IndexModel::builder()
            .keys(doc! { field: 1 })
            .options(IndexOptions::builder().expire_after(Duration::from_secs(secs)).build())
            .build()
    };
    let e = |what: &str| {
        let what = what.to_string();
        move |err: mongodb::error::Error| format!("index {what}: {err}")
    };
    db.collection::<Document>(STATES).create_index(ttl("createdAt", 600)).await.map_err(e(STATES))?;
    db.collection::<Document>(JTIS).create_index(ttl("at", 600)).await.map_err(e(JTIS))?;
    let sessions = db.collection::<Document>(SESSIONS);
    sessions.create_index(ttl("expiresAt", 0)).await.map_err(e(SESSIONS))?;
    sessions.create_index(IndexModel::builder().keys(doc! { "idpSid": 1 }).build()).await.map_err(e(SESSIONS))?;
    sessions.create_index(IndexModel::builder().keys(doc! { "deviceId": 1 }).build()).await.map_err(e(SESSIONS))?;
    sessions.create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build()).await.map_err(e(SESSIONS))?;
    db.collection::<Document>(DEVICES)
        .create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build())
        .await
        .map_err(e(DEVICES))?;
    Ok(())
}

// MARK: Lượt đăng nhập web

pub struct LoginState {
    pub code_verifier: String,
    pub nonce: String,
}

pub async fn save_state(db: &Database, state: &str, verifier: &str, nonce: &str) -> Result<(), AppError> {
    db.collection::<Document>(STATES)
        .insert_one(doc! { "_id": state, "codeVerifier": verifier, "nonce": nonce, "createdAt": DateTime::now() })
        .await
        .map(|_| ())
        .map_err(internal("lưu lượt đăng nhập"))
}

/// Lấy và xoá (dùng một lần). TTL index có thể chậm tới 60 giây nên tự kiểm hạn 10 phút.
pub async fn take_state(db: &Database, state: &str) -> Result<Option<LoginState>, AppError> {
    let d = db
        .collection::<Document>(STATES)
        .find_one_and_delete(doc! { "_id": state, "createdAt": { "$gt": after(-600) } })
        .await
        .map_err(internal("đọc lượt đăng nhập"))?;
    Ok(d.and_then(|d| {
        Some(LoginState {
            code_verifier: d.get_str("codeVerifier").ok()?.to_string(),
            nonce: d.get_str("nonce").ok()?.to_string(),
        })
    }))
}

// MARK: Phiên

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Client {
    Web,
    Mobile,
}

impl Client {
    fn name(self) -> &'static str {
        match self {
            Client::Web => "web",
            Client::Mobile => "mobile",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub user_id: String,
}

/// Tạo phiên, trả token cookie (chỉ băm của nó được lưu).
pub async fn create(
    db: &Database,
    user_id: &str,
    client: Client,
    idp_sid: Option<&str>,
    device_id: Option<&str>,
) -> Result<(String, DateTime), AppError> {
    let token = random_token();
    let ttl = if client == Client::Web { WEB_TTL_S } else { MOBILE_TTL_S };
    let expires_at = after(ttl);
    db.collection::<Document>(SESSIONS)
        .insert_one(doc! {
            "_id": hash_token(&token),
            "userId": user_id,
            "client": client.name(),
            "idpSid": idp_sid,
            "deviceId": device_id,
            "createdAt": DateTime::now(),
            "expiresAt": expires_at,
        })
        .await
        .map_err(internal("tạo phiên"))?;
    Ok((token, expires_at))
}

/// Phiên còn hạn của token. Phiên web gần hết hạn được trượt thêm (tối đa 7 ngày từ lúc đăng nhập).
pub async fn find(db: &Database, token: &str) -> Result<Option<Session>, AppError> {
    let id = hash_token(token);
    let coll = db.collection::<Document>(SESSIONS);
    let Some(d) = coll
        .find_one(doc! { "_id": &id, "expiresAt": { "$gt": DateTime::now() } })
        .await
        .map_err(internal("đọc phiên"))?
    else {
        return Ok(None);
    };
    let expires_at = *d.get_datetime("expiresAt").map_err(|_| AppError::Internal("phiên thiếu hạn".into()))?;
    let created = d.get_datetime("createdAt").map(|c| c.timestamp_millis()).unwrap_or(0);
    let left_ms = expires_at.timestamp_millis() - DateTime::now().timestamp_millis();
    if d.get_str("client") == Ok("web") && left_ms < WEB_TTL_S * 1000 / 2 {
        let cap = created + WEB_MAX_AGE_S * 1000;
        let next = DateTime::from_millis((DateTime::now().timestamp_millis() + WEB_TTL_S * 1000).min(cap));
        if next > expires_at {
            coll.update_one(doc! { "_id": &id }, doc! { "$set": { "expiresAt": next } })
                .await
                .map_err(internal("gia hạn phiên"))?;
        }
    }
    let user_id = d.get_str("userId").map_err(|_| AppError::Internal("phiên thiếu user".into()))?.to_string();
    Ok(Some(Session { id, user_id }))
}

/// Xoá một phiên; trả id (băm) để xoá khỏi cache.
pub async fn delete(db: &Database, token: &str) -> Result<String, AppError> {
    let id = hash_token(token);
    db.collection::<Document>(SESSIONS).delete_one(doc! { "_id": &id }).await.map_err(internal("xoá phiên"))?;
    Ok(id)
}

async fn delete_matching(db: &Database, filter: Document) -> Result<Vec<String>, AppError> {
    let coll = db.collection::<Document>(SESSIONS);
    let mut cur = coll.find(filter.clone()).projection(doc! { "_id": 1 }).await.map_err(internal("tìm phiên"))?;
    let mut ids = Vec::new();
    while let Some(d) = cur.next().await {
        if let Ok(id) = d.map_err(internal("tìm phiên"))?.get_str("_id") {
            ids.push(id.to_string());
        }
    }
    coll.delete_many(doc! { "_id": { "$in": &ids } }).await.map_err(internal("xoá phiên"))?;
    Ok(ids)
}

/// Back-channel logout: xoá phiên web gắn phiên IdP `sid`.
pub async fn delete_web_by_idp_sid(db: &Database, sid: &str) -> Result<Vec<String>, AppError> {
    delete_matching(db, doc! { "idpSid": sid, "client": "web" }).await
}

/// Mọi phiên của user (bị chặn, hoặc logout token chỉ có `sub`).
pub async fn delete_by_user(db: &Database, user_id: &str, client: Option<Client>) -> Result<Vec<String>, AppError> {
    let mut filter = doc! { "userId": user_id };
    if let Some(c) = client {
        filter.insert("client", c.name());
    }
    delete_matching(db, filter).await
}

// MARK: Thiết bị iPhone

pub struct Device {
    pub id: String,
    pub user_id: String,
    pub refresh_sealed: String,
}

/// Lưu thiết bị mới, trả token thiết bị cho app (chỉ băm được lưu).
pub async fn create_device(db: &Database, user_id: &str, refresh_sealed: &str) -> Result<(String, String), AppError> {
    let token = random_token();
    let id = hash_token(&token);
    db.collection::<Document>(DEVICES)
        .insert_one(doc! {
            "_id": &id,
            "userId": user_id,
            "refresh": refresh_sealed,
            "createdAt": DateTime::now(),
            "refreshedAt": DateTime::now(),
            "refreshingAt": null,
        })
        .await
        .map_err(internal("lưu thiết bị"))?;
    Ok((token, id))
}

/// Giữ quyền làm mới của thiết bị. `None`: không có thiết bị, hoặc một lần làm mới khác đang chạy.
pub async fn lock_device(db: &Database, device_token: &str) -> Result<Option<Device>, AppError> {
    let id = hash_token(device_token);
    let d = db
        .collection::<Document>(DEVICES)
        .find_one_and_update(
            doc! { "_id": &id, "$or": [{ "refreshingAt": null }, { "refreshingAt": { "$lt": after(-REFRESH_LOCK_S) } }] },
            doc! { "$set": { "refreshingAt": DateTime::now() } },
        )
        .await
        .map_err(internal("khoá thiết bị"))?;
    Ok(d.and_then(|d| {
        Some(Device {
            id: id.clone(),
            user_id: d.get_str("userId").ok()?.to_string(),
            refresh_sealed: d.get_str("refresh").ok()?.to_string(),
        })
    }))
}

/// Ghi refresh token mới (hoặc giữ cũ khi lỗi tạm) và nhả khoá.
pub async fn unlock_device(db: &Database, device_id: &str, refresh_sealed: Option<&str>) -> Result<(), AppError> {
    let mut set = doc! { "refreshingAt": null };
    if let Some(r) = refresh_sealed {
        set.insert("refresh", r);
        set.insert("refreshedAt", DateTime::now());
    }
    db.collection::<Document>(DEVICES)
        .update_one(doc! { "_id": device_id }, doc! { "$set": set })
        .await
        .map(|_| ())
        .map_err(internal("nhả khoá thiết bị"))
}

/// Xoá thiết bị và mọi phiên của nó (máy bị thu hồi ở IdP, hoặc đăng xuất trên app).
pub async fn delete_device(db: &Database, device_id: &str) -> Result<Vec<String>, AppError> {
    db.collection::<Document>(DEVICES).delete_one(doc! { "_id": device_id }).await.map_err(internal("xoá thiết bị"))?;
    delete_matching(db, doc! { "deviceId": device_id }).await
}

// MARK: Logout token

/// `true` lần đầu thấy `jti` (lần sau là phát lại).
pub async fn first_seen_jti(db: &Database, jti: &str) -> bool {
    db.collection::<Document>(JTIS).insert_one(doc! { "_id": jti, "at": DateTime::now() }).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// sessions_on_real -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn sessions_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());

        // Lượt đăng nhập dùng một lần.
        let state = random_token();
        save_state(&db, &state, "verifier", "nonce").await.expect("lưu");
        assert_eq!(take_state(&db, &state).await.expect("lấy").map(|s| s.nonce), Some("nonce".into()));
        assert!(take_state(&db, &state).await.expect("lấy lại").is_none());

        // Phiên web: tìm được, back-channel theo sid xoá đúng phiên web.
        let (web, _) = create(&db, &user, Client::Web, Some("sid-1"), None).await.expect("web");
        let (device_token, device_id) = create_device(&db, &user, "sealed").await.expect("thiết bị");
        let (mobile, _) = create(&db, &user, Client::Mobile, Some("sid-1"), Some(&device_id)).await.expect("mobile");
        assert_eq!(find(&db, &web).await.expect("tìm").map(|s| s.user_id), Some(user.clone()));
        assert_eq!(delete_web_by_idp_sid(&db, "sid-1").await.expect("bc").len(), 1);
        assert!(find(&db, &web).await.expect("tìm").is_none());
        assert!(find(&db, &mobile).await.expect("tìm").is_some(), "phiên mobile không theo phiên trình duyệt");

        // Hai lần làm mới đồng thời: chỉ một lần giữ được khoá.
        let (a, b) = tokio::join!(lock_device(&db, &device_token), lock_device(&db, &device_token));
        assert_eq!([a.expect("a").is_some(), b.expect("b").is_some()].iter().filter(|x| **x).count(), 1);
        unlock_device(&db, &device_id, Some("sealed-2")).await.expect("nhả");
        assert_eq!(lock_device(&db, &device_token).await.expect("khoá").map(|d| d.refresh_sealed), Some("sealed-2".into()));

        // Thu hồi thiết bị xoá phiên của nó.
        assert_eq!(delete_device(&db, &device_id).await.expect("xoá").len(), 1);
        assert!(find(&db, &mobile).await.expect("tìm").is_none());
        assert!(first_seen_jti(&db, &format!("jti-{user}")).await);
        assert!(!first_seen_jti(&db, &format!("jti-{user}")).await);
        delete_by_user(&db, &user, None).await.expect("dọn");
    }
}
