//! Retention sweep — tiến trình DUY NHẤT xoá file quá hạn (disk TRƯỚC → doc) + dọn orphan .part /
//! session cũ. Red-team #1: KHÔNG dùng TTL index native (tránh Mongo xoá doc out-of-band → mồ côi
//! file disk). Chạy 1 lần lúc start rồi lặp mỗi giờ.

use crate::config::RETENTION_DAYS;
use crate::error::AppError;
use crate::models::{file, sentence, transcript_doc, translation};
use crate::state::AppState;
use mongodb::bson::{doc, DateTime, Document};
use std::path::Path;
use std::time::{Duration, SystemTime};

const SWEEP_INTERVAL_SECS: u64 = 3600; // 1h
const ORPHAN_MAX_AGE_SECS: u64 = 86_400; // .part / session dở > 1 ngày → coi là mồ côi
const SESSION_COLLS: [&str; 2] = ["upload_sessions", "recording_sessions"];

pub fn start_sweeper(state: AppState) {
    tokio::spawn(async move {
        loop {
            // Panic-isolation từng vòng (giống jobs::start_worker) → sweep panic KHÔNG dừng loop vĩnh viễn.
            let st = state.clone();
            match tokio::spawn(async move { sweep_once(&st).await }).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("retention sweep lỗi: {e:?}"),
                Err(e) => tracing::error!("retention sweep panic (loop vẫn sống, thử lại chu kỳ sau): {e}"),
            }
            tokio::time::sleep(Duration::from_secs(SWEEP_INTERVAL_SECS)).await;
        }
    });
}

async fn sweep_once(state: &AppState) -> Result<(), AppError> {
    let data_dir = &state.config.data_dir;

    // 1. File quá hạn: xoá file DISK TRƯỚC → xoá doc SAU (thứ tự cố định, không đua).
    let expired = file::find_expired(&state.db).await?;
    let mut removed = 0u64;
    let mut bytes = 0u64;
    for f in &expired {
        // Xoá file DISK TRƯỚC. CHỈ xoá doc khi disk đã thực sự mất (Ok) hoặc vốn không tồn tại
        // (NotFound). Lỗi khác (permission/I/O) → GIỮ doc + log, retry chu kỳ sau — tránh mồ côi
        // disk (red-team #1: doc là "sổ đăng ký" duy nhất để trace file, mất doc = mất dấu file).
        match tokio::fs::remove_file(&f.path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(path = %f.path, "sweep: không xoá được file disk, giữ doc để retry: {e}");
                continue;
            }
        }
        // Đoạn cắt (output): dọn thư mục rỗng + zip của job kèm theo.
        if f.kind == "output" {
            if let Some(parent) = Path::new(&f.path).parent() {
                let _ = tokio::fs::remove_dir(parent).await; // {stem}/ — chỉ xoá khi đã rỗng
                if let Some(outdir) = parent.parent() {
                    let _ = tokio::fs::remove_dir(outdir).await; // {jobId}/ — chỉ khi rỗng
                }
            }
            if let Some(job_id) = &f.job_id {
                let zip = format!("{data_dir}/{}/outputs/{job_id}.zip", f.user_id);
                let _ = tokio::fs::remove_file(&zip).await;
            }
        }
        if let Err(e) = file::delete_doc(&state.db, f).await {
            tracing::warn!(path = %f.path, "sweep: xoá doc file lỗi (thử lại chu kỳ sau): {e:?}");
        }
        removed += 1;
        bytes += f.size.max(0) as u64;
    }

    // 2. Orphan .part (upload/recording bỏ dở) + session doc cũ + job doc quá hạn (chống phình coll +
    //    bỏ job "done" trỏ segment đã bị xoá). Job cắt xong nhanh nên quá RETENTION_DAYS = an toàn xoá.
    let orphan_parts = sweep_orphan_parts(data_dir).await;
    let stale_sessions = sweep_stale_sessions(&state.db).await;
    let stale_jobs = sweep_stale_jobs(&state.db).await;
    let orphan_docs = sweep_orphan_transcript_docs(&state.db).await + sweep_orphan_translations(&state.db).await;
    let orphan_sentences = sweep_orphan_sentences(&state.db).await;
    let reindexed = reconcile_search_index(&state.db).await;
    // Việc nền ghi thêm sau khi một tài khoản bị xoá (lượt chép xong muộn, job cắt): dọn lại.
    let deleted_accounts = crate::account::sweep_deleted(&state.db, data_dir).await;

    if removed > 0
        || orphan_parts > 0
        || stale_sessions > 0
        || stale_jobs > 0
        || orphan_docs > 0
        || orphan_sentences > 0
        || reindexed > 0
        || deleted_accounts > 0
    {
        tracing::info!(
            "retention: xoá {removed} file (~{} MB), {orphan_parts} .part mồ côi, {stale_sessions} session cũ, {stale_jobs} job cũ, {orphan_docs} transcript và bản dịch mồ côi, {orphan_sentences} câu mồ côi trong index tìm kiếm; dựng lại index {reindexed} phiên lệch; dọn thêm {deleted_accounts} tệp và doc của tài khoản đã xoá",
            bytes / 1_048_576
        );
    }
    Ok(())
}

/// Xoá job doc cũ hơn RETENTION_DAYS (output đã bị sweep xoá → job "done" trỏ segment chết; +chống phình).
async fn sweep_stale_jobs(db: &mongodb::Database) -> u64 {
    let cutoff =
        DateTime::from_millis(DateTime::now().timestamp_millis() - RETENTION_DAYS * 86_400_000);
    db.collection::<Document>("jobs")
        .delete_many(doc! { "createdAt": { "$lt": cutoff } })
        .await
        .map(|r| r.deleted_count)
        .unwrap_or(0)
}

/// Xoá tiêu đề/câu sửa của phiên không còn audio (bước dọn trong `file::delete_doc` có thể lỗi).
async fn sweep_orphan_transcript_docs(db: &mongodb::Database) -> u64 {
    transcript_doc::delete_orphans(db).await.unwrap_or_else(|e| {
        tracing::warn!("sweep transcript_docs mồ côi lỗi: {e:?}");
        0
    })
}

/// Xoá bản dịch của phiên không còn audio (bước dọn lúc xoá có thể lỗi).
async fn sweep_orphan_translations(db: &mongodb::Database) -> u64 {
    translation::delete_orphans(db).await.unwrap_or_else(|e| {
        tracing::warn!("sweep bản dịch mồ côi lỗi: {e:?}");
        0
    })
}

/// Xoá câu trong index tìm kiếm của file đã mất doc hoặc mất transcript (bước xoá đồng bộ lúc xoá có thể
/// lỗi). Chạy sau bước xoá file quá hạn ở trên.
async fn sweep_orphan_sentences(db: &mongodb::Database) -> u64 {
    sentence::delete_orphans(db).await.unwrap_or_else(|e| {
        tracing::warn!("sweep câu mồ côi của index tìm kiếm lỗi: {e:?}");
        0
    })
}

/// Dựng lại các phiên mà index tìm kiếm lệch với transcript (`search::index::reconcile`).
async fn reconcile_search_index(db: &mongodb::Database) -> usize {
    crate::search::index::reconcile(db).await.unwrap_or_else(|e| {
        tracing::warn!("đối soát index tìm kiếm lỗi: {e:?}");
        0
    })
}

/// Xoá `.part` trong `{data}/<user>/tmp/` cũ hơn 1 ngày (session bỏ dở). Trả số file đã xoá.
async fn sweep_orphan_parts(data_dir: &str) -> u64 {
    let mut n = 0u64;
    let Ok(mut users) = tokio::fs::read_dir(data_dir).await else {
        return 0;
    };
    while let Ok(Some(user)) = users.next_entry().await {
        let tmp = user.path().join("tmp");
        let Ok(mut rd) = tokio::fs::read_dir(&tmp).await else {
            continue;
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            if !entry.file_name().to_string_lossy().ends_with(".part") {
                continue;
            }
            let too_old = entry
                .metadata()
                .await
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .map(|age| age.as_secs() > ORPHAN_MAX_AGE_SECS)
                .unwrap_or(false);
            if too_old && tokio::fs::remove_file(entry.path()).await.is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Xoá session doc (upload + recording) cũ hơn 1 ngày. Trả tổng số doc đã xoá.
async fn sweep_stale_sessions(db: &mongodb::Database) -> u64 {
    let cutoff = DateTime::from_millis(DateTime::now().timestamp_millis() - ORPHAN_MAX_AGE_SECS as i64 * 1000);
    let mut n = 0u64;
    for coll in SESSION_COLLS {
        if let Ok(res) = db
            .collection::<Document>(coll)
            .delete_many(doc! { "createdAt": { "$lt": cutoff } })
            .await
        {
            n += res.deleted_count;
        }
    }
    n
}
