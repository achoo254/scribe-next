//! Dựng index tìm kiếm (`transcript_sentences`) của một phiên từ đúng đường ghép của màn hình đọc
//! (`docs::load_db`: câu máy đã áp câu sửa, giây theo cả phiên).
//!
//! Dựng lại khi lượt chép kết thúc, khi lưu câu sửa, khi xoá transcript, và khi xoá một file bản ghi
//! (`file::delete_doc`: xoá tay, hết hạn, cắt bản ghi). Mỗi lần dựng thay toàn bộ câu của phiên, nên
//! không cần biết cái gì đã đổi.
//!
//! [`schedule`] chạy việc dựng trong task riêng: handler bị huỷ (client ngắt kết nối) không được bỏ dở
//! giữa lúc xoá câu cũ và ghi câu mới. Hai lần dựng cùng phiên chạy lần lượt (khoá theo phiên trong tiến
//! trình — chỉ một backend ghi index; lệnh `admin` không dựng), nên lần sau luôn thắng.
//!
//! Dữ liệu có từ trước khi có index được đưa vào một lần lúc khởi động ([`start_backfill`]), xong thì
//! ghi cờ `kv` `search.backfill`; bị ngắt giữa chừng thì lần khởi động sau chạy lại từ đầu (dựng lại
//! là idempotent).

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

fn session_lock(user_id: &str, key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = LOCKS.lock().unwrap_or_else(|p| p.into_inner());
    if locks.len() > 1_000 {
        locks.retain(|_, l| Arc::strong_count(l) > 1);
    }
    Arc::clone(locks.entry(format!("{user_id}|{key}")).or_default())
}

/// Thay toàn bộ câu của phiên trong index bằng câu hiện tại. Phiên không còn file nào thì xoá hết.
/// Trả số câu đã ghi.
pub async fn reindex(db: &Database, user_id: &str, key: &str) -> Result<usize, AppError> {
    let lock = session_lock(user_id, key);
    let _held = lock.lock().await;
    let sentences = match docs::load_db(db, user_id, key).await {
        Ok(loaded) => loaded.index_sentences(),
        Err(AppError::NotFound(_)) => Vec::new(),
        Err(e) => return Err(e),
    };
    sentence::replace_session(db, user_id, key, &sentences).await
}

/// Dựng lại index của phiên trong task riêng; lỗi chỉ log.
pub fn schedule(db: &Database, user_id: &str, key: &str) {
    let (db, user_id, key) = (db.clone(), user_id.to_string(), key.to_string());
    tokio::spawn(async move {
        if let Err(e) = reindex(&db, &user_id, &key).await {
            tracing::warn!(key = %key, "dựng lại index tìm kiếm lỗi: {e:?}");
        }
    });
}

/// Như [`schedule`] cho phiên chứa file `file_id` (lượt chép kết thúc chỉ biết file).
pub fn schedule_file(db: &Database, user_id: &str, file_id: ObjectId) {
    let (db, user_id) = (db.clone(), user_id.to_string());
    tokio::spawn(async move {
        let found = file::list_owned_by_ids(&db, &user_id, &[file_id]).await;
        let key = match found {
            Ok(files) => files.into_iter().find(|f| f.kind == "recording").map(|f| file::session_key(&f.name, file_id)),
            Err(e) => {
                tracing::warn!(file = %file_id, "đọc file để dựng index lỗi: {e:?}");
                return;
            }
        };
        let Some(key) = key else { return };
        if let Err(e) = reindex(&db, &user_id, &key).await {
            tracing::warn!(key = %key, "dựng lại index tìm kiếm lỗi: {e:?}");
        }
    });
}

/// Các phiên `(userId, key)` có ít nhất một đoạn đã chép xong.
async fn transcribed_sessions(db: &Database) -> Result<BTreeSet<(String, String)>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc transcripts để dựng index: {e}"));
    let mut done: HashMap<String, HashSet<ObjectId>> = HashMap::new();
    let mut cur = db
        .collection::<Document>("transcripts")
        .find(doc! { "status": "done" })
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
        for f in file::list_recordings(db, &user_id).await? {
            if let Some(id) = f.id.filter(|id| ids.contains(id)) {
                out.insert((user_id.clone(), file::session_key(&f.name, id)));
            }
        }
    }
    Ok(out)
}

async fn backfill(db: &Database) -> Result<(), AppError> {
    let kv = db.collection::<Document>(KV);
    let flag = kv
        .find_one(doc! { "_id": BACKFILL_FLAG })
        .await
        .map_err(|e| AppError::Internal(format!("đọc cờ dựng index: {e}")))?;
    if flag.is_some() {
        return Ok(());
    }
    let sessions = transcribed_sessions(db).await?;
    let (mut written, mut failed) = (0_usize, 0_usize);
    for (user_id, key) in &sessions {
        match reindex(db, user_id, key).await {
            Ok(n) => written += n,
            Err(e) => {
                failed += 1;
                tracing::warn!(key = %key, "dựng index lúc khởi động lỗi: {e:?}");
            }
        }
    }
    if failed > 0 {
        // Không ghi cờ: lần khởi động sau thử lại cả lượt.
        return Err(AppError::Internal(format!("{failed}/{} phiên dựng index lỗi", sessions.len())));
    }
    kv.update_one(
        doc! { "_id": BACKFILL_FLAG },
        doc! { "$set": { "doneAt": DateTime::now(), "sessions": sessions.len() as i64, "sentences": written as i64 } },
    )
    .upsert(true)
    .await
    .map_err(|e| AppError::Internal(format!("ghi cờ dựng index: {e}")))?;
    tracing::info!(sessions = sessions.len(), sentences = written, "đã dựng index tìm kiếm cho dữ liệu cũ");
    Ok(())
}

/// Tạo index của `transcript_sentences` rồi đưa dữ liệu cũ vào index một lần (nền, lỗi chỉ log).
pub fn start_backfill(db: Database) {
    tokio::spawn(async move {
        if let Err(e) = sentence::ensure_indexes(&db).await {
            tracing::warn!("tạo index transcript_sentences thất bại: {e}");
            return;
        }
        if let Err(e) = backfill(&db).await {
            tracing::warn!("dựng index tìm kiếm cho dữ liệu cũ lỗi: {e:?}");
        }
    });
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
            let f = file::FileDoc::new_recording(&user, &format!("{session}#{index}"), "/nonexistent", 1, 60.0, false, id);
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

        // Backfill liệt kê phiên này (có đoạn đã chép xong).
        assert!(transcribed_sessions(&db).await.expect("liệt kê").contains(&(user.clone(), key.clone())));

        // Xoá đoạn a qua `delete_doc`: index của phiên được dựng lại (nền) chỉ còn câu của b.
        let fa = file::list_owned_by_ids(&db, &user, &[a]).await.expect("đọc").pop().expect("a");
        file::delete_doc(&db, &fa).await.expect("xoá a");
        let mut left = Vec::new();
        for _ in 0..50 {
            left = sentence::session_rows(&db, &user, &key).await.expect("đọc");
            if left.len() == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(left.iter().map(|r| (r.idx, r.at)).collect::<Vec<_>>(), vec![(0, 2.0)]);

        // Phiên hết file: index rỗng.
        db.collection::<Document>("files").delete_one(doc! { "_id": b }).await.expect("xoá b");
        transcript::delete(&db, b).await.expect("xoá transcript b");
        assert_eq!(reindex(&db, &user, &key).await.expect("dựng"), 0);
        assert!(sentence::session_rows(&db, &user, &key).await.expect("đọc").is_empty());
        db.collection::<Document>(crate::models::usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        transcript_doc::delete(&db, &user, &key).await.expect("dọn");
    }
}
