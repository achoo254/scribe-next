//! Liên kết tài khoản Scribe với chat riêng của người dùng và bot Scribe trên chat.hoandat.com.
//!
//! - `users.chat = {chatId, linkedAt, name?}`: chat riêng nơi gửi biên bản; `name` = tên trên chat của
//!   người bấm xác nhận (liên kết cũ không có).
//! - `chat_link_tokens`: token một lần trong deep link `…/b/<bot>?start=<token>`. Chỉ lưu băm SHA-256
//!   (`_id`), hạn [`TOKEN_TTL_MINUTES`] phút; TTL index dọn doc hết hạn. `/start <token>` chưa liên kết
//!   mà chỉ gắn token với chat đầu tiên gửi nó ([`bind_token`], chờ xác nhận); nút "Xác nhận" từ đúng
//!   chat đó mới dùng token ([`take_token`] = xoá, nên một token chỉ liên kết được một lần). Chờ xác nhận
//!   hết hạn cùng token. `lang` = ngôn ngữ giao diện lúc tạo đường dẫn: bot trả lời các bước liên kết bằng ngôn ngữ
//!   đó (token cũ không có ⟹ tiếng Việt).
//! - `kv` doc `chat_link.offset`: offset long-poll đã xử lý, để khởi động lại không xử lý lại update.

use crate::error::AppError;
use crate::lang::Lang;
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

/// TTL trên `expiresAt` (Mongo dọn trễ tới ~1 phút; hạn vẫn kiểm trong [`bind_token`], [`take_token`]).
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

/// Liên kết hiện có của một user.
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub chat_id: i64,
    pub linked_at: Option<DateTime>,
    /// Tên trên chat lúc xác nhận; `None` với liên kết tạo trước khi lưu tên.
    pub name: Option<String>,
}

/// Liên kết chat của user (nếu có).
pub async fn linked(db: &Database, user_id: &str) -> Result<Option<Linked>, AppError> {
    let d = db
        .collection::<Document>(USERS)
        .find_one(doc! { "_id": user_oid(user_id)? })
        .projection(doc! { "chat": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("đọc liên kết chat: {e}")))?;
    Ok(d.as_ref().and_then(linked_of))
}

/// Chat id đã liên kết của user.
pub async fn linked_chat(db: &Database, user_id: &str) -> Result<Option<i64>, AppError> {
    Ok(linked(db, user_id).await?.map(|l| l.chat_id))
}

fn linked_of(user: &Document) -> Option<Linked> {
    let chat = user.get_document("chat").ok()?;
    let chat_id = chat.get_i64("chatId").ok().or_else(|| chat.get_i32("chatId").ok().map(i64::from))?;
    Some(Linked {
        chat_id,
        linked_at: chat.get_datetime("linkedAt").ok().copied(),
        name: chat.get_str("name").ok().filter(|n| !n.is_empty()).map(str::to_string),
    })
}

/// Gắn chat với user (thay liên kết cũ nếu có), kèm tên trên chat nếu biết. `None` khi không còn user;
/// `Some(chat cũ)` khi đã gắn.
pub async fn link(db: &Database, user_id: &str, chat_id: i64, name: Option<&str>) -> Result<Option<Option<i64>>, AppError> {
    let mut chat = doc! { "chatId": chat_id, "linkedAt": DateTime::now() };
    if let Some(name) = name {
        chat.insert("name", name);
    }
    let before = db
        .collection::<Document>(USERS)
        .find_one_and_update(doc! { "_id": user_oid(user_id)? }, doc! { "$set": { "chat": chat } })
        .projection(doc! { "chat": 1 })
        .return_document(mongodb::options::ReturnDocument::Before)
        .await
        .map_err(|e| AppError::Internal(format!("ghi liên kết chat: {e}")))?;
    Ok(before.map(|d| linked_of(&d).map(|l| l.chat_id)))
}

/// Gỡ mọi liên kết tới chat này (lệnh `/unlink` trong chat). Trả id các user đã gỡ.
pub async fn unlink_chat(db: &Database, chat_id: i64) -> Result<Vec<String>, AppError> {
    use futures_util::StreamExt;
    let coll = db.collection::<Document>(USERS);
    let filter = doc! { "chat.chatId": chat_id };
    let mut cur = coll
        .find(filter.clone())
        .projection(doc! { "_id": 1 })
        .await
        .map_err(|e| AppError::Internal(format!("tìm liên kết của chat: {e}")))?;
    let mut ids = Vec::new();
    while let Some(d) = cur.next().await {
        let d = d.map_err(|e| AppError::Internal(format!("tìm liên kết của chat: {e}")))?;
        if let Ok(id) = d.get_object_id("_id") {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    coll.update_many(doc! { "_id": { "$in": &ids }, "chat.chatId": chat_id }, doc! { "$unset": { "chat": "" } })
        .await
        .map_err(|e| AppError::Internal(format!("gỡ liên kết của chat: {e}")))?;
    Ok(ids.into_iter().map(ObjectId::to_hex).collect())
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

/// User của một token liên kết và ngôn ngữ để bot trả lời người đó.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenOwner {
    pub user_id: String,
    pub lang: Lang,
}

fn lang_of(token: &Document) -> Lang {
    token.get_str("lang").ok().and_then(Lang::parse).unwrap_or_default()
}

fn owner_of(token: &Document) -> Option<TokenOwner> {
    let user_id = token.get_str("userId").ok()?.to_string();
    Some(TokenOwner { user_id, lang: lang_of(token) })
}

/// Tạo token mới cho user (token cũ chưa dùng của user bị huỷ), nhớ ngôn ngữ giao diện `lang`. Trả token gốc — chỉ
/// đi vào deep link.
pub async fn create_token(db: &Database, user_id: &str, lang: Lang) -> Result<String, AppError> {
    let coll = db.collection::<Document>(TOKENS);
    coll.delete_many(doc! { "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("huỷ token liên kết cũ: {e}")))?;
    let token = crate::session::random_token();
    let expires = DateTime::from_millis(DateTime::now().timestamp_millis() + TOKEN_TTL_MINUTES * 60_000);
    let token_doc = doc! {
        "_id": hash_token(&token),
        "userId": user_id,
        "lang": lang.code(),
        "createdAt": DateTime::now(),
        "expiresAt": expires,
    };
    coll.insert_one(token_doc)
        .await
        .map_err(|e| AppError::Internal(format!("lưu token liên kết: {e}")))?;
    Ok(token)
}

/// Token còn hạn ở thời điểm `now` không.
pub fn token_fresh(expires_at: DateTime, now: DateTime) -> bool {
    now < expires_at
}

/// `/start <token>` từ `chat_id`: gắn token còn hạn với chat này (chat đầu tiên gửi nó) và trả user của
/// token. Token lạ, hết hạn hay đã gắn với chat khác → `None`. Chưa liên kết gì.
pub async fn bind_token(db: &Database, token: &str, chat_id: i64) -> Result<Option<TokenOwner>, AppError> {
    let filter = doc! {
        "_id": hash_token(token),
        "expiresAt": { "$gt": DateTime::now() },
        "$or": [{ "chatId": { "$exists": false } }, { "chatId": chat_id }],
    };
    let d = db
        .collection::<Document>(TOKENS)
        .find_one_and_update(filter, doc! { "$set": { "chatId": chat_id } })
        .await
        .map_err(|e| AppError::Internal(format!("gắn token liên kết: {e}")))?;
    Ok(d.as_ref().and_then(owner_of))
}

/// Nút "Xác nhận" từ `chat_id`: dùng token đã gắn với đúng chat này (xoá ngay, một lần) rồi trả user nếu
/// còn hạn. Token lạ, đã dùng, hết hạn hay gắn với chat khác → `None`.
pub async fn take_token(db: &Database, token: &str, chat_id: i64) -> Result<Option<TokenOwner>, AppError> {
    let d = db
        .collection::<Document>(TOKENS)
        .find_one_and_delete(doc! { "_id": hash_token(token), "chatId": chat_id })
        .await
        .map_err(|e| AppError::Internal(format!("dùng token liên kết: {e}")))?;
    Ok(d.and_then(|d| {
        let fresh = d.get_datetime("expiresAt").is_ok_and(|exp| token_fresh(*exp, DateTime::now()));
        fresh.then(|| owner_of(&d)).flatten()
    }))
}

/// Nút "Huỷ" từ `chat_id`: bỏ token đã gắn với chat này. Trả ngôn ngữ của token khi đã bỏ.
pub async fn drop_token(db: &Database, token: &str, chat_id: i64) -> Result<Option<Lang>, AppError> {
    db.collection::<Document>(TOKENS)
        .find_one_and_delete(doc! { "_id": hash_token(token), "chatId": chat_id })
        .await
        .map(|d| d.as_ref().map(lang_of))
        .map_err(|e| AppError::Internal(format!("huỷ token liên kết: {e}")))
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
    fn linked_chats_read_their_name_when_saved() {
        let at = DateTime::from_millis(0);
        let with_name = doc! { "chat": { "chatId": 12_i64, "linkedAt": at, "name": "An Nguyễn" } };
        assert_eq!(linked_of(&with_name), Some(Linked { chat_id: 12, linked_at: Some(at), name: Some("An Nguyễn".into()) }));
        let older = doc! { "chat": { "chatId": 12_i32 } };
        assert_eq!(linked_of(&older), Some(Linked { chat_id: 12, linked_at: None, name: None }));
        assert_eq!(linked_of(&doc! {}), None);
    }

    #[test]
    fn tokens_without_a_language_answer_in_vietnamese() {
        let owner = owner_of(&doc! { "userId": "u1", "lang": "zh-CN" });
        assert_eq!(owner, Some(TokenOwner { user_id: "u1".into(), lang: Lang::Zh }));
        assert_eq!(owner_of(&doc! { "userId": "u1" }).map(|o| o.lang), Some(Lang::Vi));
        assert_eq!(lang_of(&doc! { "lang": "xx" }), Lang::Vi);
        assert_eq!(owner_of(&doc! { "lang": "en" }), None);
    }

    #[test]
    fn tokens_expire_at_their_deadline() {
        let exp = DateTime::from_millis(1_000_000);
        assert!(token_fresh(exp, DateTime::from_millis(999_999)));
        assert!(!token_fresh(exp, DateTime::from_millis(1_000_000)));
        assert!(!token_fresh(exp, DateTime::from_millis(2_000_000)));
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// chat_link_tokens -- --ignored`. Token gắn với chat đầu tiên, dùng một lần, hết hạn bị từ chối, token
    /// mới huỷ token cũ.
    #[tokio::test]
    #[ignore]
    async fn chat_link_tokens_are_single_use_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());

        let old = create_token(&db, &user, Lang::Vi).await.expect("token cũ");
        let token = create_token(&db, &user, Lang::Zh).await.expect("token");
        let owner = Some(TokenOwner { user_id: user.clone(), lang: Lang::Zh });
        assert_eq!(bind_token(&db, &old, 1).await.expect("cũ"), None, "token mới phải huỷ token cũ");
        assert_eq!(take_token(&db, &token, 1).await.expect("chưa gắn"), None, "chưa /start thì chưa xác nhận được");
        assert_eq!(bind_token(&db, &token, 1).await.expect("gắn"), owner);
        assert_eq!(bind_token(&db, &token, 1).await.expect("gắn lại"), owner, "/start lại cùng chat");
        assert_eq!(bind_token(&db, &token, 2).await.expect("chat khác"), None);
        assert_eq!(take_token(&db, &token, 2).await.expect("chat khác"), None);
        assert_eq!(take_token(&db, &token, 1).await.expect("dùng"), owner);
        assert_eq!(take_token(&db, &token, 1).await.expect("dùng lại"), None);
        assert_eq!(bind_token(&db, "khong-co", 1).await.expect("lạ"), None);

        let cancelled = create_token(&db, &user, Lang::Vi).await.expect("token");
        bind_token(&db, &cancelled, 1).await.expect("gắn");
        assert_eq!(drop_token(&db, &cancelled, 2).await.expect("chat khác không huỷ được"), None);
        assert_eq!(drop_token(&db, &cancelled, 1).await.expect("huỷ"), Some(Lang::Vi));
        assert_eq!(take_token(&db, &cancelled, 1).await.expect("đã huỷ"), None);

        let expired = create_token(&db, &user, Lang::Vi).await.expect("token");
        bind_token(&db, &expired, 1).await.expect("gắn");
        db.collection::<Document>(TOKENS)
            .update_one(doc! { "_id": hash_token(&expired) }, doc! { "$set": { "expiresAt": DateTime::from_millis(0) } })
            .await
            .expect("cho hết hạn");
        assert_eq!(take_token(&db, &expired, 1).await.expect("hết hạn"), None);
        db.collection::<Document>(TOKENS).delete_many(doc! { "userId": &user }).await.expect("dọn");
    }
}
