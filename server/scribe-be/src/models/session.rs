//! Phiên đăng nhập lưu trong Mongo (thu hồi được), kèm trạng thái lượt đăng nhập web, thiết bị iPhone và
//! `jti` logout token đã nhận.
//!
//! - `auth_states`: `state` → `codeVerifier` + `nonce` của một lượt đăng nhập web, dùng một lần, TTL 10 phút.
//! - `sessions`: `_id` = băm token cookie; `client` web | mobile; web trượt tới 7 ngày, mobile 12 giờ và chỉ
//!   gia hạn qua làm mới ở IdP.
//! - `devices`: `_id` = băm token thiết bị của app; refresh token IdP mã hoá; khoá `refreshingAt` để chỉ
//!   một lần làm mới chạy (refresh token của IdP dùng một lần).
//! - `logout_jtis`: chống phát lại logout token, TTL 10 phút; `_id` = `<tenantId>:<jti>` (jti chỉ duy nhất trong
//!   một issuer).
//!
//! Mọi doc mang `tenantId`; phiên, lượt đăng nhập và jti chỉ đọc được trong đúng tenant của request (phiên của tenant
//! này gửi tới host của tenant khác là "không có phiên").

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

pub async fn save_state(db: &Database, tenant_id: &str, state: &str, verifier: &str, nonce: &str) -> Result<(), AppError> {
    db.collection::<Document>(STATES)
        .insert_one(doc! { "_id": state, "tenantId": tenant_id, "codeVerifier": verifier, "nonce": nonce, "createdAt": DateTime::now() })
        .await
        .map(|_| ())
        .map_err(internal("lưu lượt đăng nhập"))
}

/// Lấy và xoá (dùng một lần), chỉ lượt của tenant này. TTL index có thể chậm tới 60 giây nên tự kiểm hạn 10 phút.
pub async fn take_state(db: &Database, tenant_id: &str, state: &str) -> Result<Option<LoginState>, AppError> {
    let d = db
        .collection::<Document>(STATES)
        .find_one_and_delete(doc! { "_id": state, "tenantId": tenant_id, "createdAt": { "$gt": after(-600) } })
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
    tenant_id: &str,
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
            "tenantId": tenant_id,
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

/// Phiên còn hạn của token trong tenant này. Phiên web gần hết hạn được trượt thêm (tối đa 7 ngày từ lúc đăng nhập).
pub async fn find(db: &Database, tenant_id: &str, token: &str) -> Result<Option<Session>, AppError> {
    let id = hash_token(token);
    let coll = db.collection::<Document>(SESSIONS);
    let Some(d) = coll
        .find_one(doc! { "_id": &id, "tenantId": tenant_id, "expiresAt": { "$gt": DateTime::now() } })
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

/// Xoá một phiên của tenant này; trả id (băm) để xoá khỏi cache.
pub async fn delete(db: &Database, tenant_id: &str, token: &str) -> Result<String, AppError> {
    let id = hash_token(token);
    db.collection::<Document>(SESSIONS)
        .delete_one(doc! { "_id": &id, "tenantId": tenant_id })
        .await
        .map_err(internal("xoá phiên"))?;
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

/// Back-channel logout: xoá phiên web của tenant gắn phiên IdP `sid` (sid chỉ duy nhất trong một issuer).
pub async fn delete_web_by_idp_sid(db: &Database, tenant_id: &str, sid: &str) -> Result<Vec<String>, AppError> {
    delete_matching(db, doc! { "tenantId": tenant_id, "idpSid": sid, "client": "web" }).await
}

/// Mọi phiên của tenant (CLI `purge-tenant`, phần còn sót sau khi xoá từng user).
pub async fn delete_by_tenant(db: &Database, tenant_id: &str) -> Result<Vec<String>, AppError> {
    delete_matching(db, doc! { "tenantId": tenant_id }).await
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
    /// Mã của lần giữ khoá này: chỉ chính nó nhả được khoá.
    pub lock: String,
}

pub enum DeviceLock {
    Locked(Device),
    /// Một lần làm mới khác của máy đang chạy.
    Busy,
    /// Không có thiết bị (đã thu hồi, đã đăng xuất): app phải đăng nhập lại.
    Missing,
}

/// Lưu thiết bị mới, trả token thiết bị cho app (chỉ băm được lưu).
pub async fn create_device(db: &Database, tenant_id: &str, user_id: &str, refresh_sealed: &str) -> Result<(String, String), AppError> {
    let token = random_token();
    let id = hash_token(&token);
    db.collection::<Document>(DEVICES)
        .insert_one(doc! {
            "_id": &id,
            "tenantId": tenant_id,
            "userId": user_id,
            "refresh": refresh_sealed,
            "createdAt": DateTime::now(),
            "refreshedAt": DateTime::now(),
            "refreshingAt": null,
            "lock": null,
        })
        .await
        .map_err(internal("lưu thiết bị"))?;
    Ok((token, id))
}

/// Giữ quyền làm mới của thiết bị (một lần mỗi lúc: refresh token của IdP dùng một lần).
pub async fn lock_device(db: &Database, device_token: &str) -> Result<DeviceLock, AppError> {
    let id = hash_token(device_token);
    let lock = random_token();
    let coll = db.collection::<Document>(DEVICES);
    let d = coll
        .find_one_and_update(
            doc! { "_id": &id, "$or": [{ "refreshingAt": null }, { "refreshingAt": { "$lt": after(-REFRESH_LOCK_S) } }] },
            doc! { "$set": { "refreshingAt": DateTime::now(), "lock": &lock } },
        )
        .await
        .map_err(internal("khoá thiết bị"))?;
    let Some(d) = d else {
        let exists = coll.count_documents(doc! { "_id": &id }).await.map_err(internal("đọc thiết bị"))? > 0;
        return Ok(if exists { DeviceLock::Busy } else { DeviceLock::Missing });
    };
    let device = (|| {
        Some(Device {
            id: id.clone(),
            user_id: d.get_str("userId").ok()?.to_string(),
            refresh_sealed: d.get_str("refresh").ok()?.to_string(),
            lock: lock.clone(),
        })
    })();
    Ok(device.map_or(DeviceLock::Missing, DeviceLock::Locked))
}

/// Ghi refresh token mới (hoặc giữ cũ khi lỗi tạm) và nhả khoá — chỉ khi vẫn còn giữ khoá này. Mất khoá
/// (quá hạn, lần khác đã giữ) mà có token mới thì vẫn phải ghi: token cũ đã chết ở IdP.
pub async fn unlock_device(db: &Database, device: &Device, refresh_sealed: Option<&str>) -> Result<(), AppError> {
    let mut set = doc! { "refreshingAt": null, "lock": null };
    if let Some(r) = refresh_sealed {
        set.insert("refresh", r);
        set.insert("refreshedAt", DateTime::now());
    }
    let filter = if refresh_sealed.is_some() {
        doc! { "_id": &device.id }
    } else {
        doc! { "_id": &device.id, "lock": &device.lock }
    };
    db.collection::<Document>(DEVICES)
        .update_one(filter, doc! { "$set": set })
        .await
        .map(|_| ())
        .map_err(internal("nhả khoá thiết bị"))
}

/// Refresh token (đã mã hoá) của thiết bị — để thu hồi ở IdP khi đăng xuất.
pub async fn device_refresh(db: &Database, device_id: &str) -> Result<Option<String>, AppError> {
    let d = db
        .collection::<Document>(DEVICES)
        .find_one(doc! { "_id": device_id })
        .await
        .map_err(internal("đọc thiết bị"))?;
    Ok(d.and_then(|d| d.get_str("refresh").ok().map(str::to_string)))
}

/// `(id thiết bị, refresh token IdP đã mã hoá)` của mọi thiết bị của user, để thu hồi ở IdP trước khi xoá tài khoản.
pub async fn user_devices(db: &Database, user_id: &str) -> Result<Vec<(String, String)>, AppError> {
    let mut cur = db
        .collection::<Document>(DEVICES)
        .find(doc! { "userId": user_id })
        .projection(doc! { "_id": 1, "refresh": 1 })
        .await
        .map_err(internal("đọc thiết bị của user"))?;
    let mut out = Vec::new();
    while let Some(d) = cur.next().await {
        let d = d.map_err(internal("đọc thiết bị của user"))?;
        if let (Ok(id), Ok(refresh)) = (d.get_str("_id"), d.get_str("refresh")) {
            out.push((id.to_string(), refresh.to_string()));
        }
    }
    Ok(out)
}

/// Xoá thiết bị và mọi phiên của nó (máy bị thu hồi ở IdP, hoặc đăng xuất trên app).
pub async fn delete_device(db: &Database, device_id: &str) -> Result<Vec<String>, AppError> {
    db.collection::<Document>(DEVICES).delete_one(doc! { "_id": device_id }).await.map_err(internal("xoá thiết bị"))?;
    delete_matching(db, doc! { "deviceId": device_id }).await
}

// MARK: Logout token

fn jti_key(tenant_id: &str, jti: &str) -> String {
    format!("{tenant_id}:{jti}")
}

/// `jti` của logout token (của issuer tenant này) đã xử lý xong chưa (phát lại thì bỏ qua).
pub async fn seen_jti(db: &Database, tenant_id: &str, jti: &str) -> Result<bool, AppError> {
    db.collection::<Document>(JTIS)
        .count_documents(doc! { "_id": jti_key(tenant_id, jti) })
        .await
        .map(|n| n > 0)
        .map_err(internal("đọc jti"))
}

/// Ghi `jti` SAU khi đã xoá phiên: xoá lỗi thì IdP gửi lại và lần sau vẫn xử lý.
pub async fn record_jti(db: &Database, tenant_id: &str, jti: &str) -> Result<(), AppError> {
    let jti_doc = doc! { "_id": jti_key(tenant_id, jti), "tenantId": tenant_id, "at": DateTime::now() };
    match db.collection::<Document>(JTIS).insert_one(jti_doc).await {
        Ok(_) => Ok(()),
        Err(e) if crate::models::user::is_duplicate_key(&e) => Ok(()),
        Err(e) => Err(AppError::Internal(format!("ghi jti: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::PRIMARY_ID as P;

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
        save_state(&db, P, &state, "verifier", "nonce").await.expect("lưu");
        assert_eq!(take_state(&db, P, &state).await.expect("lấy").map(|s| s.nonce), Some("nonce".into()));
        assert!(take_state(&db, P, &state).await.expect("lấy lại").is_none());

        // Phiên web: tìm được, back-channel theo sid xoá đúng phiên web.
        let (web, _) = create(&db, P, &user, Client::Web, Some("sid-1"), None).await.expect("web");
        let (device_token, device_id) = create_device(&db, P, &user, "sealed").await.expect("thiết bị");
        let (mobile, _) = create(&db, P, &user, Client::Mobile, Some("sid-1"), Some(&device_id)).await.expect("mobile");
        assert_eq!(find(&db, P, &web).await.expect("tìm").map(|s| s.user_id), Some(user.clone()));
        assert_eq!(delete_web_by_idp_sid(&db, P, "sid-1").await.expect("bc").len(), 1);
        assert!(find(&db, P, &web).await.expect("tìm").is_none());
        assert!(find(&db, P, &mobile).await.expect("tìm").is_some(), "phiên mobile không theo phiên trình duyệt");

        // Hai lần làm mới đồng thời: chỉ một lần giữ được khoá.
        let (a, b) = tokio::join!(lock_device(&db, &device_token), lock_device(&db, &device_token));
        let (a, b) = (a.expect("a"), b.expect("b"));
        let locked = [&a, &b].into_iter().filter(|l| matches!(l, DeviceLock::Locked(_))).count();
        let busy = [&a, &b].into_iter().filter(|l| matches!(l, DeviceLock::Busy)).count();
        assert_eq!((locked, busy), (1, 1));
        let held = match (a, b) {
            (DeviceLock::Locked(d), _) | (_, DeviceLock::Locked(d)) => d,
            _ => unreachable!("một lần giữ được khoá"),
        };
        unlock_device(&db, &held, Some("sealed-2")).await.expect("nhả");
        match lock_device(&db, &device_token).await.expect("khoá") {
            DeviceLock::Locked(d) => assert_eq!(d.refresh_sealed, "sealed-2"),
            _ => panic!("phải giữ được khoá sau khi nhả"),
        }

        // Thu hồi thiết bị xoá phiên của nó.
        assert_eq!(delete_device(&db, &device_id).await.expect("xoá").len(), 1);
        assert!(find(&db, P, &mobile).await.expect("tìm").is_none());
        assert!(matches!(lock_device(&db, &device_token).await.expect("khoá"), DeviceLock::Missing));
        let jti = format!("jti-{user}");
        assert!(!seen_jti(&db, P, &jti).await.expect("đọc"));
        record_jti(&db, P, &jti).await.expect("ghi");
        record_jti(&db, P, &jti).await.expect("ghi lại không lỗi");
        assert!(seen_jti(&db, P, &jti).await.expect("đọc"));
        delete_by_user(&db, &user, None).await.expect("dọn");
    }

    /// Cô lập phiên: lượt đăng nhập, phiên, back-channel theo `sid` và jti của tenant A không đọc hay xoá được từ
    /// tenant B.
    #[tokio::test]
    #[ignore]
    async fn sessions_are_isolated_by_tenant_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let (a, b) = (format!("ten_a_{tag}"), format!("ten_b_{tag}"));
        let (user_a, user_b) = (format!("selftest-a-{tag}"), format!("selftest-b-{tag}"));

        let state = random_token();
        save_state(&db, &a, &state, "verifier", "nonce").await.expect("lưu");
        assert!(take_state(&db, &b, &state).await.expect("lấy").is_none(), "lượt của A hết hạn ở B");
        assert!(take_state(&db, &a, &state).await.expect("lấy").is_some(), "B lấy hụt không làm mất lượt của A");

        let (token_a, _) = create(&db, &a, &user_a, Client::Web, Some("sid-same"), None).await.expect("phiên A");
        let (token_b, _) = create(&db, &b, &user_b, Client::Web, Some("sid-same"), None).await.expect("phiên B");
        assert!(find(&db, &b, &token_a).await.expect("tìm").is_none(), "phiên A gửi tới host B ⟹ không có phiên");
        let _ = delete(&db, &b, &token_a).await.expect("xoá hụt");
        assert!(find(&db, &a, &token_a).await.expect("tìm").is_some(), "B không xoá được phiên của A");
        // Back-channel của issuer B với cùng `sid` chỉ xoá phiên B.
        assert_eq!(delete_web_by_idp_sid(&db, &b, "sid-same").await.expect("bc").len(), 1);
        assert!(find(&db, &a, &token_a).await.expect("tìm").is_some());
        assert!(find(&db, &b, &token_b).await.expect("tìm").is_none());
        // jti trùng giữa hai issuer không chặn nhau.
        let jti = format!("jti-{tag}");
        record_jti(&db, &a, &jti).await.expect("ghi");
        assert!(seen_jti(&db, &a, &jti).await.expect("đọc") && !seen_jti(&db, &b, &jti).await.expect("đọc"));
        assert_eq!(delete_by_tenant(&db, &a).await.expect("dọn").len(), 1);
        assert!(find(&db, &a, &token_a).await.expect("tìm").is_none());
    }
}
