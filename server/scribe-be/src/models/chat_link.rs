//! Liên kết tài khoản Scribe với chat riêng của người dùng và bot Scribe trên chat.hoandat.com.
//!
//! - `users.chat = {chatId, linkedAt}`: chat riêng nơi gửi biên bản.
//! - `chat_link_tokens`: token một lần trong deep link `…/b/<bot>?start=<token>`. Chỉ lưu băm SHA-256
//!   (`_id`), hạn [`TOKEN_TTL_MINUTES`] phút; TTL index dọn doc hết hạn. Dùng = xoá (`find_one_and_delete`),
//!   nên một token chỉ liên kết được một lần.
//! - `kv` doc `chat_link.offset`: offset long-poll đã xử lý, để khởi động lại không xử lý lại update.

use crate::error::AppError;
use crate::session::hash_token;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::time::Duration;

const USERS: &str = "users";
const TOKENS: &str = "chat_link_tokens";
const KV: &str = "kv";
const OFFSET_KEY: &str = "chat_link.offset";
pub const TOKEN_TTL_MINUTES: i64 = 10;

/// TTL trên `expiresAt` (Mongo dọn trễ tới ~1 phút; hạn vẫn kiểm trong [`consume_token`]).
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let index = IndexModel::builder()
        .keys(doc! { "expiresAt": 1 })
        .options(IndexOptions::builder().expire_after(Duration::from_secs(0)).build())
        .build();
    db.collection::<Document>(TOKENS)
        .create_index(index)
        .await
        .map(|_| ())
        .map_err(|e| format!("index chat_link_tokens.expiresAt (TTL): {e}"))
}

fn user_oid(user_id: &str) -> Result<ObjectId, AppError> {
    ObjectId::parse_str(user_id).map_err(|_| AppError::Unauthorized("user id không hợp lệ".into()))
}

/// Chat id đã liên kết của user.
pub async fn linked_chat(db: &Database, user_id: &str) -> Result<Option<i64>, AppError> {
    let d = db
        .collection::<Document>(USERS)
        .find_one(doc! { "_id": user_oid(user_id)? })
        .projection(doc! { "chat": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("đọc liên kết chat: {e}")))?;
    Ok(d.and_then(|d| d.get_document("chat").ok().and_then(chat_id_of)))
}

fn chat_id_of(chat: &Document) -> Option<i64> {
    chat.get_i64("chatId").ok().or_else(|| chat.get_i32("chatId").ok().map(i64::from))
}

/// Gắn chat với user (thay liên kết cũ nếu có). `false` khi không còn user.
pub async fn link(db: &Database, user_id: &str, chat_id: i64) -> Result<bool, AppError> {
    db.collection::<Document>(USERS)
        .update_one(
            doc! { "_id": user_oid(user_id)? },
            doc! { "$set": { "chat": { "chatId": chat_id, "linkedAt": DateTime::now() } } },
        )
        .await
        .map(|r| r.matched_count > 0)
        .map_err(|e| AppError::Internal(format!("ghi liên kết chat: {e}")))
}

/// Gỡ liên kết. `only_chat` = chỉ gỡ khi vẫn là chat này (bot bị chặn ở chat cũ không gỡ liên kết mới
/// vừa tạo chen vào). Trả `true` khi đã gỡ.
pub async fn unlink(db: &Database, user_id: &str, only_chat: Option<i64>) -> Result<bool, AppError> {
    let mut filter = doc! { "_id": user_oid(user_id)?, "chat": { "$exists": true } };
    if let Some(chat_id) = only_chat {
        filter = doc! { "_id": user_oid(user_id)?, "chat.chatId": chat_id };
    }
    db.collection::<Document>(USERS)
        .update_one(filter, doc! { "$unset": { "chat": "" } })
        .await
        .map(|r| r.modified_count > 0)
        .map_err(|e| AppError::Internal(format!("gỡ liên kết chat: {e}")))
}

/// Tạo token mới cho user (token cũ chưa dùng của user bị huỷ). Trả token gốc — chỉ đi vào deep link.
pub async fn create_token(db: &Database, user_id: &str) -> Result<String, AppError> {
    let coll = db.collection::<Document>(TOKENS);
    coll.delete_many(doc! { "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("huỷ token liên kết cũ: {e}")))?;
    let token = crate::session::random_token();
    let expires = DateTime::from_millis(DateTime::now().timestamp_millis() + TOKEN_TTL_MINUTES * 60_000);
    coll.insert_one(doc! { "_id": hash_token(&token), "userId": user_id, "createdAt": DateTime::now(), "expiresAt": expires })
        .await
        .map_err(|e| AppError::Internal(format!("lưu token liên kết: {e}")))?;
    Ok(token)
}

/// Token còn hạn ở thời điểm `now` không.
pub fn token_fresh(expires_at: DateTime, now: DateTime) -> bool {
    now < expires_at
}

/// Dùng token: xoá ngay (một lần) rồi trả user nếu còn hạn. Token lạ, đã dùng hay hết hạn → `None`.
pub async fn consume_token(db: &Database, token: &str) -> Result<Option<String>, AppError> {
    let d = db
        .collection::<Document>(TOKENS)
        .find_one_and_delete(doc! { "_id": hash_token(token) })
        .await
        .map_err(|e| AppError::Internal(format!("dùng token liên kết: {e}")))?;
    Ok(d.and_then(|d| {
        let fresh = d.get_datetime("expiresAt").is_ok_and(|exp| token_fresh(*exp, DateTime::now()));
        fresh.then(|| d.get_str("userId").ok().map(str::to_string)).flatten()
    }))
}

/// Offset long-poll đã lưu (0 khi chưa có).
pub async fn load_offset(db: &Database) -> Result<i64, AppError> {
    let d = db
        .collection::<Document>(KV)
        .find_one(doc! { "_id": OFFSET_KEY })
        .await
        .map_err(|e| AppError::Internal(format!("đọc offset bot: {e}")))?;
    Ok(d.and_then(|d| d.get_i64("value").ok()).unwrap_or(0))
}

/// Lưu offset; chỉ tăng (`$max`), một bản backend cũ chạy song song không kéo lùi được.
pub async fn save_offset(db: &Database, offset: i64) -> Result<(), AppError> {
    db.collection::<Document>(KV)
        .update_one(
            doc! { "_id": OFFSET_KEY },
            doc! { "$max": { "value": offset }, "$set": { "updatedAt": DateTime::now() } },
        )
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("lưu offset bot: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_expire_at_their_deadline() {
        let exp = DateTime::from_millis(1_000_000);
        assert!(token_fresh(exp, DateTime::from_millis(999_999)));
        assert!(!token_fresh(exp, DateTime::from_millis(1_000_000)));
        assert!(!token_fresh(exp, DateTime::from_millis(2_000_000)));
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// chat_link_tokens -- --ignored`. Token dùng một lần, token hết hạn bị từ chối, token mới huỷ token cũ.
    #[tokio::test]
    #[ignore]
    async fn chat_link_tokens_are_single_use_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());

        let old = create_token(&db, &user).await.expect("token cũ");
        let token = create_token(&db, &user).await.expect("token");
        assert_eq!(consume_token(&db, &old).await.expect("cũ"), None, "token mới phải huỷ token cũ");
        assert_eq!(consume_token(&db, &token).await.expect("dùng"), Some(user.clone()));
        assert_eq!(consume_token(&db, &token).await.expect("dùng lại"), None);
        assert_eq!(consume_token(&db, "khong-co").await.expect("lạ"), None);

        let expired = create_token(&db, &user).await.expect("token");
        db.collection::<Document>(TOKENS)
            .update_one(doc! { "_id": hash_token(&expired) }, doc! { "$set": { "expiresAt": DateTime::from_millis(0) } })
            .await
            .expect("cho hết hạn");
        assert_eq!(consume_token(&db, &expired).await.expect("hết hạn"), None);
        db.collection::<Document>(TOKENS).delete_many(doc! { "userId": &user }).await.expect("dọn");
    }
}
