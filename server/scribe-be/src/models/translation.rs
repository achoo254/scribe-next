//! `transcript_translations` collection: bản dịch của một phiên (`key`, như `transcript_docs`) sang một
//! ngôn ngữ, một doc mỗi `{userId, key, lang}` (unique). Tách khỏi `transcript_docs` để danh sách và màn
//! hình đọc không phải đọc cả bản dịch.
//!
//! Câu dịch lưu theo `items.<fileId hex>.<chỉ số câu trong đoạn>` = `{h, t}`: `h` là băm chữ nguồn lúc dịch
//! (`crate::translate::source_hash`), `t` là bản dịch. Chữ nguồn hiện tại băm ra khác `h` thì câu đó là bản
//! dịch cũ. `$set` vào đường dẫn chưa có tạo doc con (khoá là chuỗi số), nên ghi từng lô không cần đọc trước.
//!
//! Tiến độ của lượt gần nhất ở `status` (`running | done | failed`), `done`/`total` (số câu của lượt đó),
//! `error`. Doc đi theo audio: xoá bản chép lời xoá doc, xoá một đoạn gỡ `items.<fileId>` ([`forget_file`]),
//! sweep xoá doc của phiên không còn file ([`delete_orphans`]).

use crate::error::AppError;
use crate::models::user::is_duplicate_key;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const COLL: &str = "transcript_translations";

/// Ngôn ngữ đích của bản dịch (cùng bộ ngôn ngữ với giao diện).
pub use crate::lang::Lang;

/// Một câu đã dịch: băm chữ nguồn lúc dịch và bản dịch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub h: String,
    pub t: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDoc {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub lang: String,
    #[serde(default)]
    pub model: String,
    /// `running | done | failed`.
    pub status: String,
    #[serde(default)]
    pub done: i64,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime,
    /// Theo id file (hex), rồi chỉ số câu trong đoạn (chuỗi).
    #[serde(default)]
    pub items: HashMap<String, HashMap<String, Item>>,
}

impl TranslationDoc {
    /// Câu đã dịch của câu `idx` trong đoạn `file_id`.
    pub fn item(&self, file_id: &str, idx: usize) -> Option<&Item> {
        self.items.get(file_id)?.get(&idx.to_string())
    }
}

/// Trạng thái lượt gần nhất của một ngôn ngữ, không kèm câu.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub lang: Lang,
    pub status: String,
    pub done: i64,
    pub total: i64,
    pub error: Option<String>,
}

/// Một câu vừa dịch xong, để ghi vào doc.
pub struct NewItem<'a> {
    pub file_id: &'a str,
    pub idx: usize,
    pub item: Item,
}

fn read_err(e: mongodb::error::Error) -> AppError {
    AppError::Internal(format!("đọc transcript_translations: {e}"))
}

/// Index unique `{userId, key, lang}`: hai lượt bắt đầu cùng lúc cũng chỉ tạo một doc.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let index = IndexModel::builder()
        .keys(doc! { "userId": 1, "key": 1, "lang": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();
    db.collection::<Document>(COLL)
        .create_index(index)
        .await
        .map_err(|e| format!("index transcript_translations.userId_key_lang: {e}"))?;
    Ok(())
}

pub async fn get(db: &Database, user_id: &str, key: &str, lang: Lang) -> Result<Option<TranslationDoc>, AppError> {
    db.collection::<TranslationDoc>(COLL)
        .find_one(doc! { "userId": user_id, "key": key, "lang": lang.code() })
        .await
        .map_err(read_err)
}

fn read_i64(d: &Document, field: &str) -> i64 {
    d.get_i64(field).ok().or_else(|| d.get_i32(field).ok().map(i64::from)).unwrap_or(0)
}

/// Trạng thái các bản dịch của phiên (theo thứ tự [`Lang::ALL`]), chỉ đọc phần tiến độ.
pub async fn progress(db: &Database, user_id: &str, key: &str) -> Result<Vec<Progress>, AppError> {
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "key": key })
        .projection(doc! { "_id": 0, "lang": 1, "status": 1, "done": 1, "total": 1, "error": 1 })
        .await
        .map_err(read_err)?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(read_err)?;
        let Some(lang) = d.get_str("lang").ok().and_then(Lang::parse) else { continue };
        out.push(Progress {
            lang,
            status: d.get_str("status").unwrap_or("done").to_string(),
            done: read_i64(&d, "done"),
            total: read_i64(&d, "total"),
            error: d.get_str("error").ok().map(str::to_string),
        });
    }
    out.sort_by_key(|p| Lang::ALL.iter().position(|l| *l == p.lang));
    Ok(out)
}

/// Bắt đầu một lượt: tạo doc nếu chưa có, đặt `running` với `total` câu. Câu đã dịch giữ nguyên.
pub async fn start_run(db: &Database, user_id: &str, key: &str, lang: Lang, model: &str, total: i64) -> Result<(), AppError> {
    let coll = db.collection::<Document>(COLL);
    let filter = doc! { "userId": user_id, "key": key, "lang": lang.code() };
    let update = doc! {
        "$set": { "status": "running", "done": 0_i64, "total": total, "model": model, "updatedAt": DateTime::now() },
        "$unset": { "error": "" },
        "$setOnInsert": { "createdAt": DateTime::now() },
    };
    match coll.update_one(filter.clone(), update.clone()).upsert(true).await {
        Ok(_) => Ok(()),
        // Thua cuộc đua tạo doc với unique index: lần hai khớp doc vừa tạo.
        Err(e) if is_duplicate_key(&e) => coll
            .update_one(filter, update)
            .upsert(true)
            .await
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("bắt đầu lượt dịch (lần hai): {e}"))),
        Err(e) => Err(AppError::Internal(format!("bắt đầu lượt dịch: {e}"))),
    }
}

/// Ghi các câu của một lô và cộng tiến độ. Không upsert: bản chép lời bị xoá giữa lượt thì không tạo lại doc.
pub async fn save_items(db: &Database, user_id: &str, key: &str, lang: Lang, items: &[NewItem<'_>]) -> Result<(), AppError> {
    let mut set = doc! { "updatedAt": DateTime::now() };
    for n in items {
        let value = mongodb::bson::to_bson(&n.item).map_err(|e| AppError::Internal(format!("mã hoá câu dịch: {e}")))?;
        set.insert(format!("items.{}.{}", n.file_id, n.idx), value);
    }
    db.collection::<Document>(COLL)
        .update_one(
            doc! { "userId": user_id, "key": key, "lang": lang.code() },
            doc! { "$set": set, "$inc": { "done": items.len() as i64 } },
        )
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("ghi câu dịch: {e}")))
}

/// Kết thúc lượt: `done`, hoặc `failed` kèm lý do cho người đọc.
pub async fn finish(db: &Database, user_id: &str, key: &str, lang: Lang, error: Option<&str>) -> Result<(), AppError> {
    let update = match error {
        None => doc! { "$set": { "status": "done", "updatedAt": DateTime::now() }, "$unset": { "error": "" } },
        Some(e) => doc! { "$set": { "status": "failed", "error": e, "updatedAt": DateTime::now() } },
    };
    db.collection::<Document>(COLL)
        .update_one(doc! { "userId": user_id, "key": key, "lang": lang.code() }, update)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("kết thúc lượt dịch: {e}")))
}

/// Một đoạn của phiên bị xoá: gỡ câu dịch của đoạn đó ở mọi ngôn ngữ. `file_id` là ObjectId nên an toàn trong
/// field path.
pub async fn forget_file(db: &Database, user_id: &str, key: &str, file_id: ObjectId) -> Result<(), AppError> {
    db.collection::<Document>(COLL)
        .update_many(
            doc! { "userId": user_id, "key": key },
            doc! { "$unset": { format!("items.{}", file_id.to_hex()): "" } },
        )
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("gỡ câu dịch của đoạn: {e}")))
}

/// Xoá mọi bản dịch của phiên.
pub async fn delete(db: &Database, user_id: &str, key: &str) -> Result<(), AppError> {
    db.collection::<Document>(COLL)
        .delete_many(doc! { "userId": user_id, "key": key })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("xoá bản dịch: {e}")))
}

/// Xoá bản dịch của phiên không còn file bản ghi nào (bước dọn lúc xoá có thể lỗi). Trả số doc đã xoá.
pub async fn delete_orphans(db: &Database) -> Result<u64, AppError> {
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! {})
        .projection(doc! { "userId": 1, "key": 1 })
        .await
        .map_err(read_err)?;
    let mut checked: HashMap<(String, String), bool> = HashMap::new();
    let mut orphans = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(read_err)?;
        let (Ok(id), Ok(user_id), Ok(key)) = (d.get_object_id("_id"), d.get_str("userId"), d.get_str("key")) else {
            continue;
        };
        let session = (user_id.to_string(), key.to_string());
        let empty = match checked.get(&session) {
            Some(empty) => *empty,
            None => {
                let empty = super::file::list_session(db, user_id, key).await?.is_empty();
                checked.insert(session, empty);
                empty
            }
        };
        if empty {
            orphans.push(id);
        }
    }
    if orphans.is_empty() {
        return Ok(0);
    }
    db.collection::<Document>(COLL)
        .delete_many(doc! { "_id": { "$in": orphans } })
        .await
        .map(|r| r.deleted_count)
        .map_err(|e| AppError::Internal(format!("xoá bản dịch mồ côi: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vòng đời trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri> TRANSCRIPT_TEST_DB_NAME=<db>
    /// cargo test translations_lifecycle -- --ignored`. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn translations_lifecycle_on_real_mongo() {
        use crate::models::file;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let key = format!("s:{session}");
        let (a, b) = (ObjectId::new(), ObjectId::new());
        for (id, index) in [(a, 0), (b, 1)] {
            let owner = file::Owner { user_id: &user, plan: crate::models::plan::Plan::Free };
            let f = file::FileDoc::new_recording(owner, &format!("{session}#{index}"), "/nonexistent", 1, 60.0, false, id);
            file::create(&db, f).await.expect("tạo file");
        }
        let (a_hex, b_hex) = (a.to_hex(), b.to_hex());
        let item = |t: &str| Item { h: format!("h-{t}"), t: t.into() };

        // Hai lượt bắt đầu cùng lúc: đúng một doc.
        let (r1, r2) = tokio::join!(
            start_run(&db, &user, &key, Lang::En, "m", 3),
            start_run(&db, &user, &key, Lang::En, "m", 3)
        );
        r1.expect("bắt đầu 1");
        r2.expect("bắt đầu 2");
        let count = db
            .collection::<Document>(COLL)
            .count_documents(doc! { "userId": &user, "key": &key })
            .await
            .expect("đếm");
        assert_eq!(count, 1);

        // Ghi theo lô vào đường dẫn chưa có: doc con theo chỉ số câu, tiến độ cộng dồn.
        let first = [NewItem { file_id: &a_hex, idx: 0, item: item("Hello.") }, NewItem { file_id: &a_hex, idx: 12, item: item("Twelve.") }];
        save_items(&db, &user, &key, Lang::En, &first).await.expect("lô 1");
        save_items(&db, &user, &key, Lang::En, &[NewItem { file_id: &b_hex, idx: 0, item: item("Bee.") }]).await.expect("lô 2");
        let d = get(&db, &user, &key, Lang::En).await.expect("get").expect("doc");
        assert_eq!((d.status.as_str(), d.done, d.total), ("running", 3, 3));
        assert_eq!(d.item(&a_hex, 12), Some(&item("Twelve.")));
        assert_eq!(d.item(&a_hex, 1), None);
        finish(&db, &user, &key, Lang::En, None).await.expect("xong");

        // Lượt sau giữ câu cũ, đặt lại tiến độ; lỗi giữa chừng ghi lý do.
        start_run(&db, &user, &key, Lang::En, "m", 1).await.expect("lượt 2");
        finish(&db, &user, &key, Lang::En, Some("lỗi thử")).await.expect("lỗi");
        let p = progress(&db, &user, &key).await.expect("progress");
        assert_eq!(p, vec![Progress { lang: Lang::En, status: "failed".into(), done: 0, total: 1, error: Some("lỗi thử".into()) }]);
        assert_eq!(get(&db, &user, &key, Lang::En).await.expect("get").expect("doc").item(&b_hex, 0), Some(&item("Bee.")));

        // Ngôn ngữ thứ hai, rồi xoá một đoạn: câu dịch của đoạn đó mất ở mọi ngôn ngữ.
        start_run(&db, &user, &key, Lang::Vi, "m", 1).await.expect("vi");
        save_items(&db, &user, &key, Lang::Vi, &[NewItem { file_id: &a_hex, idx: 0, item: item("Xin chào.") }]).await.expect("vi lô");
        forget_file(&db, &user, &key, a).await.expect("gỡ đoạn a");
        let en = get(&db, &user, &key, Lang::En).await.expect("get").expect("en");
        assert!(!en.items.contains_key(&a_hex) && en.item(&b_hex, 0).is_some());
        assert!(get(&db, &user, &key, Lang::Vi).await.expect("get").expect("vi").items.is_empty());
        assert_eq!(progress(&db, &user, &key).await.expect("progress").iter().map(|p| p.lang).collect::<Vec<_>>(), vec![Lang::En, Lang::Vi]);

        // Còn file thì sweep chừa; hết file thì xoá.
        delete_orphans(&db).await.expect("sweep");
        assert!(get(&db, &user, &key, Lang::En).await.expect("get").is_some());
        db.collection::<Document>("files").delete_many(doc! { "_id": { "$in": [a, b] } }).await.expect("xoá file");
        assert!(delete_orphans(&db).await.expect("sweep") >= 2);
        assert!(progress(&db, &user, &key).await.expect("progress").is_empty());

        // Lô ghi sau khi bản dịch đã bị xoá không tạo lại doc.
        save_items(&db, &user, &key, Lang::En, &[NewItem { file_id: &b_hex, idx: 1, item: item("Late.") }]).await.expect("lô muộn");
        assert!(get(&db, &user, &key, Lang::En).await.expect("get").is_none());
        delete(&db, &user, &key).await.expect("dọn");
    }
}
