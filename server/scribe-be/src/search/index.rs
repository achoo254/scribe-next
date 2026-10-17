//! Dựng index tìm kiếm (`transcript_sentences`) của một phiên từ đúng đường ghép của màn hình đọc
//! (`docs::load_db`: câu máy đã áp câu sửa, giây theo cả phiên).
//!
//! Dựng lại khi lượt chép kết thúc, khi lưu câu sửa, khi một đoạn iPhone mới tới (mốc giây của các đoạn
//! sau dời theo), và khi xoá một file bản ghi (`file::delete_doc`: xoá tay, hết hạn, cắt bản ghi). Mỗi lần
//! dựng thay toàn bộ câu của phiên, nên không cần biết cái gì đã đổi. Xoá thì không chờ dựng lại: câu đã
//! xoá phải biến khỏi tìm kiếm và hỏi đáp ngay, nên [`clear`] (xoá transcript) và [`forget_file`] (xoá
//! file) xoá câu đồng bộ trước.
//!
//! [`schedule`] chạy việc dựng trong task riêng: handler bị huỷ (client ngắt kết nối) không được bỏ dở
//! giữa chừng. Mọi thao tác ghi của cùng một phiên chạy lần lượt (khoá theo phiên trong tiến trình — chỉ
//! một backend ghi index; lệnh `admin` không dựng), nên lần sau luôn thắng. Lượt dựng đang chờ chưa chạy
//! thì lần gọi sau không tạo thêm lượt (lượt đang chờ sẽ đọc dữ liệu mới nhất): retention xoá hàng loạt
//! đoạn của một phiên chỉ dựng lại phiên đó một lần.
//!
//! Dữ liệu có từ trước khi có index được đưa vào một lần lúc khởi động ([`start_backfill`], cờ `kv`
//! `search.backfill`). Đối soát định kỳ ([`reconcile`], gọi từ retention) dựng lại phiên lệch: có đoạn đã
//! chép xong mà không có câu nào, còn câu mà không còn đoạn nào chép xong, hoặc bị ngắt giữa lúc thay.

use crate::error::AppError;
use crate::models::{file, sentence};
use crate::transcribe::docs;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::Database;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

const KV: &str = "kv";
const BACKFILL_FLAG: &str = "search.backfill";

/// Khoá theo `<userId>|<key>`; mục không còn ai giữ được dọn khi bảng lớn.
static LOCKS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = LazyLock::new(Mutex::default);
/// Phiên đã có lượt dựng nền đang chờ (chưa bắt đầu đọc dữ liệu).
static PENDING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);

fn session_id(user_id: &str, key: &str) -> String {
    format!("{user_id}|{key}")
}

fn session_lock(user_id: &str, key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = LOCKS.lock().unwrap_or_else(|p| p.into_inner());
    if locks.len() > 1_000 {
        locks.retain(|_, l| Arc::strong_count(l) > 1);
    }
    Arc::clone(locks.entry(session_id(user_id, key)).or_default())
}

/// Dựng lại khi đã giữ khoá của phiên.
async fn rebuild(db: &Database, user_id: &str, key: &str) -> Result<usize, AppError> {
    let loaded = match docs::load_db(db, user_id, key).await {
        Ok(loaded) => loaded,
        Err(AppError::NotFound(_)) => return sentence::delete_session(db, user_id, key).await.map(|_| 0),
        Err(e) => return Err(e),
    };
    let started = loaded.started_at().unwrap_or_else(DateTime::now);
    sentence::replace_session(db, user_id, key, started, &loaded.index_sentences()).await
}

/// Thay toàn bộ câu của phiên trong index bằng câu hiện tại. Phiên không còn file nào thì xoá hết.
/// Trả số câu đã ghi.
pub async fn reindex(db: &Database, user_id: &str, key: &str) -> Result<usize, AppError> {
    let lock = session_lock(user_id, key);
    let _held = lock.lock().await;
    rebuild(db, user_id, key).await
}

/// Xoá ngay mọi câu của phiên (transcript vừa bị xoá). Chờ lượt dựng đang chạy của phiên xong trước, để
/// lượt đó không ghi lại câu đã xoá.
pub async fn clear(db: &Database, user_id: &str, key: &str) -> Result<u64, AppError> {
    let lock = session_lock(user_id, key);
    let _held = lock.lock().await;
    sentence::delete_session(db, user_id, key).await
}

/// Xoá ngay câu của file `file_id` thuộc phiên `key` (file vừa bị xoá); như [`clear`] về khoá.
pub async fn forget_file(db: &Database, user_id: &str, key: &str, file_id: ObjectId) -> Result<u64, AppError> {
    let lock = session_lock(user_id, key);
    let _held = lock.lock().await;
    sentence::delete_file(db, user_id, file_id).await
}

/// Dựng lại index của phiên trong task riêng; lỗi chỉ log. Phiên đã có lượt đang chờ thì không thêm lượt.
pub fn schedule(db: &Database, user_id: &str, key: &str) {
    let id = session_id(user_id, key);
    if !PENDING.lock().unwrap_or_else(|p| p.into_inner()).insert(id.clone()) {
        return;
    }
    let (db, user_id, key) = (db.clone(), user_id.to_string(), key.to_string());
    tokio::spawn(async move {
        let lock = session_lock(&user_id, &key);
        let _held = lock.lock().await;
        // Hết chờ trước khi đọc: thay đổi tới sau lúc này cần một lượt mới.
        PENDING.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
        if let Err(e) = rebuild(&db, &user_id, &key).await {
            tracing::warn!(key = %key, "dựng lại index tìm kiếm lỗi: {e:?}");
        }
    });
}

/// Như [`schedule`] cho phiên chứa file `file_id` (lượt chép kết thúc chỉ biết file).
pub fn schedule_file(db: &Database, user_id: &str, file_id: ObjectId) {
    let (db, user_id) = (db.clone(), user_id.to_string());
    tokio::spawn(async move {
        match file::list_owned_by_ids(&db, &user_id, &[file_id]).await {
            Ok(files) => {
                if let Some(key) = files.into_iter().find(file::FileDoc::in_history).and_then(|f| f.session_key()) {
                    schedule(&db, &user_id, &key);
                }
            }
            Err(e) => tracing::warn!(file = %file_id, "đọc file để dựng index lỗi: {e:?}"),
        }
    });
}

/// Các phiên `(userId, key)` có ít nhất một đoạn đã chép xong ra câu (đoạn im lặng chép xong mà không có
/// câu nào thì không cần index, và không được bị đối soát coi là thiếu mãi).
async fn transcribed_sessions(db: &Database) -> Result<BTreeSet<(String, String)>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc transcripts để dựng index: {e}"));
    let mut done: HashMap<String, HashSet<ObjectId>> = HashMap::new();
    let mut cur = db
        .collection::<Document>("transcripts")
        .find(doc! { "status": "done", "sentences.0": { "$exists": true } })
        .projection(doc! { "_id": 1, "userId": 1 })
        .await
        .map_err(read)?;
    while let Some(r) = cur.next().await {
        let d = r.map_err(read)?;
        if let (Ok(id), Ok(user_id)) = (d.get_object_id("_id"), d.get_str("userId")) {
            done.entry(user_id.to_string()).or_default().insert(id);
        }
    }
    let mut out = BTreeSet::new();
    for (user_id, ids) in done {
        let wanted: Vec<ObjectId> = ids.into_iter().collect();
        for f in file::list_owned_by_ids(db, &user_id, &wanted).await? {
            if let Some(key) = f.session_key().filter(|_| f.in_history()) {
                out.insert((user_id.clone(), key));
            }
        }
    }
    Ok(out)
}

/// Dựng lại lần lượt các phiên; trả (số câu đã ghi, số phiên lỗi). Lỗi từng phiên chỉ log.
async fn reindex_all(db: &Database, sessions: &BTreeSet<(String, String)>, what: &str) -> (usize, usize) {
    let (mut written, mut failed) = (0_usize, 0_usize);
    for (user_id, key) in sessions {
        match reindex(db, user_id, key).await {
            Ok(n) => written += n,
            Err(e) => {
                failed += 1;
                tracing::warn!(key = %key, "{what}: dựng index lỗi: {e:?}");
            }
        }
    }
    (written, failed)
}

async fn backfill_done(db: &Database) -> Result<bool, AppError> {
    db.collection::<Document>(KV)
        .find_one(doc! { "_id": BACKFILL_FLAG })
        .await
        .map(|flag| flag.is_some())
        .map_err(|e| AppError::Internal(format!("đọc cờ dựng index: {e}")))
}

async fn backfill(db: &Database) -> Result<(), AppError> {
    if backfill_done(db).await? {
        return Ok(());
    }
    let sessions = transcribed_sessions(db).await?;
    let (written, failed) = reindex_all(db, &sessions, "dựng index lúc khởi động").await;
    // Ghi cờ cả khi vài phiên lỗi: đối soát định kỳ dựng lại đúng các phiên đó, không chạy lại cả lượt.
    db.collection::<Document>(KV)
        .update_one(
            doc! { "_id": BACKFILL_FLAG },
            doc! { "$set": {
                "doneAt": DateTime::now(),
                "sessions": sessions.len() as i64,
                "sentences": written as i64,
                "failed": failed as i64,
            } },
        )
        .upsert(true)
        .await
        .map_err(|e| AppError::Internal(format!("ghi cờ dựng index: {e}")))?;
    tracing::info!(sessions = sessions.len(), sentences = written, failed, "đã dựng index tìm kiếm cho dữ liệu cũ");
    Ok(())
}

/// Đưa dữ liệu cũ vào index một lần (nền, lỗi chỉ log). Index của collection đã tạo lúc khởi động.
pub fn start_backfill(db: Database) {
    tokio::spawn(async move {
        if let Err(e) = backfill(&db).await {
            tracing::warn!("dựng index tìm kiếm cho dữ liệu cũ lỗi: {e:?}");
        }
    });
}

/// Phiên cần dựng lại: có đoạn đã chép xong mà index không có câu nào hoặc còn hơn một thế hệ, cộng phiên
/// còn câu mà không còn đoạn nào chép xong.
fn stale_sessions(
    transcribed: &BTreeSet<(String, String)>,
    indexed: &HashMap<(String, String), bool>,
) -> BTreeSet<(String, String)> {
    let missing = transcribed.iter().filter(|s| indexed.get(*s) != Some(&true));
    let leftover = indexed.keys().filter(|s| !transcribed.contains(*s));
    missing.chain(leftover).cloned().collect()
}

/// Đối soát index với transcript (định kỳ): dựng lại các phiên lệch. Trả số phiên đã dựng lại. Chưa
/// backfill xong thì bỏ qua (backfill đang dựng cả kho).
pub async fn reconcile(db: &Database) -> Result<usize, AppError> {
    if !backfill_done(db).await? {
        return Ok(0);
    }
    let transcribed = transcribed_sessions(db).await?;
    let indexed = sentence::indexed_sessions(db).await?;
    let stale = stale_sessions(&transcribed, &indexed);
    let (_, failed) = reindex_all(db, &stale, "đối soát index").await;
    if failed > 0 {
        tracing::warn!(failed, "đối soát index: còn phiên dựng lỗi, thử lại chu kỳ sau");
    }
    Ok(stale.len() - failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_are_shared_per_session_and_pruned() {
        let a = session_lock("u", "s:aaaaaaaa-1111");
        let b = session_lock("u", "s:aaaaaaaa-1111");
        assert!(Arc::ptr_eq(&a, &b));
        assert!(!Arc::ptr_eq(&a, &session_lock("v", "s:aaaaaaaa-1111")));
    }

    #[test]
    fn stale_sessions_are_missing_split_or_left_over() {
        let s = |key: &str| ("u".to_string(), key.to_string());
        let transcribed = BTreeSet::from([s("f:ok"), s("f:missing"), s("f:split")]);
        let indexed = HashMap::from([(s("f:ok"), true), (s("f:split"), false), (s("f:gone"), true)]);
        let stale = stale_sessions(&transcribed, &indexed);
        assert_eq!(stale, BTreeSet::from([s("f:gone"), s("f:missing"), s("f:split")]));
    }

    /// Dựng lại theo vòng đời thật trên Mongo (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=…
    /// cargo test reindex_follows -- --ignored`. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn reindex_follows_transcripts_edits_and_deletes_on_real_mongo() {
        use crate::models::{transcript, transcript_doc};
        use crate::transcribe::chunking::Sentence;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        sentence::ensure_indexes(&db).await.expect("index");
        transcript_doc::ensure_indexes(&db).await.expect("index");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let key = format!("s:{session}");
        let (a, b) = (ObjectId::new(), ObjectId::new());
        for (id, index) in [(a, 0), (b, 1)] {
            let f = file::FileDoc::new_recording(file::Owner { user_id: &user, plan: crate::models::plan::Plan::Free }, &format!("{session}#{index}"), "/nonexistent", 1, 60.0, false, id);
            file::create(&db, f).await.expect("tạo file");
        }
        let charge = crate::models::usage::Charge { user_id: user.clone(), month: "test".into(), free_pool: false, day: None };
        let s = |start: f64, text: &str| Sentence { start, end: start + 1.0, text: text.into() };
        for (id, sentences) in [(a, vec![s(1.0, "Chào cả nhà."), s(5.0, "Bàn hợp đồng.")]), (b, vec![s(2.0, "Chốt giá.")])] {
            transcript::enqueue(&db, id, &user, "run-1", "m", &charge, 0).await.expect("enqueue");
            transcript::finish_done(&db, id, "run-1", "", &sentences).await.expect("xong");
        }
        assert_eq!(reindex(&db, &user, &key).await.expect("dựng"), 3);
        let rows = sentence::session_rows(&db, &user, &key).await.expect("đọc");
        assert_eq!(rows.iter().map(|r| (r.idx, r.at)).collect::<Vec<_>>(), vec![(0, 1.0), (1, 5.0), (2, 62.0)]);

        // Câu sửa có hiệu lực thay câu máy.
        let edit = transcript_doc::SegmentEdit { run_id: "run-1".into(), texts: vec!["Chốt giá 5 triệu.".into()] };
        transcript_doc::set_edits(&db, &user, &key, b, &edit).await.expect("sửa");
        reindex(&db, &user, &key).await.expect("dựng lại");
        let rows = sentence::session_rows(&db, &user, &key).await.expect("đọc");
        assert_eq!(rows[2].norm, "chot gia 5 trieu");

        // Tên và lúc bắt đầu phiên cho kết quả tìm: chỉ đọc bản ghi của đúng phiên, tiêu đề người dùng đặt.
        let wanted = HashSet::from([key.clone(), "f:000000000000000000000000".to_string()]);
        let meta = crate::search::session_meta(&db, &user, &wanted).await.expect("meta");
        assert_eq!(meta.len(), 1);
        assert!(meta[&key].title.is_none() && meta[&key].created_at.is_some());
        transcript_doc::set_title(&db, &user, &key, Some("Họp tuần")).await.expect("đặt tên");
        let meta = crate::search::session_meta(&db, &user, &wanted).await.expect("meta");
        assert_eq!(meta[&key].title.as_deref(), Some("Họp tuần"));

        // Backfill liệt kê phiên này (có đoạn đã chép xong).
        assert!(transcribed_sessions(&db).await.expect("liệt kê").contains(&(user.clone(), key.clone())));

        // Phiên mất câu (tiến trình chết giữa chừng): đối soát dựng lại. Đối soát chỉ chạy khi đã backfill.
        let flag_created = !backfill_done(&db).await.expect("cờ");
        if flag_created {
            db.collection::<Document>(KV).insert_one(doc! { "_id": BACKFILL_FLAG }).await.expect("cờ");
        }
        sentence::delete_session(&db, &user, &key).await.expect("xoá câu");
        assert!(reconcile(&db).await.expect("đối soát") >= 1);
        assert_eq!(sentence::session_rows(&db, &user, &key).await.expect("đọc").len(), 3);
        if flag_created {
            db.collection::<Document>(KV).delete_one(doc! { "_id": BACKFILL_FLAG }).await.expect("dọn cờ");
        }

        // Xoá đoạn a qua `delete_doc`: câu của a biến mất ngay; index của phiên được dựng lại (nền) để mốc
        // giây của b dời về đầu phiên.
        let fa = file::list_owned_by_ids(&db, &user, &[a]).await.expect("đọc").pop().expect("a");
        file::delete_doc(&db, &fa).await.expect("xoá a");
        let now = sentence::session_rows(&db, &user, &key).await.expect("đọc");
        assert!(now.iter().all(|r| r.file_id == b), "câu của đoạn đã xoá không còn");
        let mut left = Vec::new();
        for _ in 0..50 {
            left = sentence::session_rows(&db, &user, &key).await.expect("đọc").iter().map(|r| (r.idx, r.at)).collect();
            if left == [(0, 2.0)] {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(left, vec![(0, 2.0)]);

        // Xoá transcript: câu của phiên biến mất ngay.
        assert_eq!(clear(&db, &user, &key).await.expect("xoá"), 1);
        assert!(sentence::session_rows(&db, &user, &key).await.expect("đọc").is_empty());

        // Phiên hết file: index rỗng.
        reindex(&db, &user, &key).await.expect("dựng");
        db.collection::<Document>("files").delete_one(doc! { "_id": b }).await.expect("xoá b");
        transcript::delete(&db, b).await.expect("xoá transcript b");
        assert_eq!(reindex(&db, &user, &key).await.expect("dựng"), 0);
        assert!(sentence::session_rows(&db, &user, &key).await.expect("đọc").is_empty());
        db.collection::<Document>(crate::models::usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        transcript_doc::delete(&db, &user, &key).await.expect("dọn");
    }
}
