//! Lắp router: public (health/auth) + protected (guard middleware + upload/jobs).

use crate::state::AppState;
use crate::{access_log, auth, chat_link, files, jobs, rate_limit, realtime, recordings, search, summary, transcribe, translate, upload};
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn build_router(state: AppState) -> Router {
    // Transcript theo phiên + liên kết chat + tìm kiếm/hỏi đáp: body chỉ là chữ (tiêu đề, câu sửa, dấu mốc,
    // câu hỏi, ngôn ngữ dịch) → trần riêng 1 MiB thay vì 8 MiB.
    let transcript_docs = Router::new()
        .route("/api/transcript-docs", get(transcribe::docs::list))
        .route(
            "/api/transcript-docs/{key}",
            get(transcribe::docs::get).patch(transcribe::docs::rename).delete(transcribe::docs::delete),
        )
        .route("/api/transcript-docs/{key}/status", get(transcribe::docs::status))
        .route("/api/transcript-docs/{key}/segments/{file_id}", put(transcribe::docs::save_segment))
        .route("/api/transcript-docs/{key}/summary", post(summary::create))
        .route("/api/transcript-docs/{key}/share", post(summary::share::share))
        .route("/api/transcript-docs/{key}/translation", get(translate::get).post(translate::create))
        .route("/api/transcript-docs/{key}/translation/status", get(translate::status))
        .route("/api/transcript-docs/{key}/bookmarks", post(transcribe::bookmarks::add))
        .route("/api/transcript-docs/{key}/bookmarks/{id}", delete(transcribe::bookmarks::remove))
        .route("/api/chat/link", get(chat_link::status).post(chat_link::create).delete(chat_link::remove))
        .route("/api/search", get(search::search))
        .route("/api/ask", post(search::ask::ask))
        .layer(DefaultBodyLimit::max(1024 * 1024));

    // Route cần đăng nhập → guard verify session; rate-limit chung (ngưỡng cao, không cản recording).
    let protected = Router::new()
        .route("/api/me", get(auth::me))
        .route("/api/me/usage", get(files::usage))
        .route("/api/upload/init", post(upload::init))
        .route("/api/upload/chunk", post(upload::chunk))
        .route("/api/upload/status", get(upload::status))
        .route("/api/upload/finalize", post(upload::finalize))
        // Mode A — ghi (streaming append open-ended) + phát lại (stream + Range).
        .route("/api/recordings/init", post(recordings::init))
        .route("/api/recordings/chunk", post(recordings::chunk))
        .route("/api/recordings/finalize", post(recordings::finalize))
        .route("/api/files", get(files::list).delete(files::delete_all))
        // Đặt TRƯỚC "/api/files/{id}" cho rõ (matchit ưu tiên segment tĩnh nên không nhầm với id).
        .route("/api/files/archive", get(files::archive))
        .route("/api/files/manage", get(files::manage_list))
        .route("/api/files/{id}", get(files::stream).delete(files::delete))
        .route("/api/jobs", post(jobs::create))
        .route("/api/jobs/{id}", get(jobs::get))
        .route("/api/jobs/{id}/download", get(jobs::download))
        .route("/api/transcripts", post(transcribe::create).get(transcribe::get))
        .route("/api/transcripts/export", get(transcribe::export))
        // Chép trực tiếp trên iPhone: token tạm thời của OpenAI Realtime + báo số giây đã stream.
        .route("/api/realtime/token", post(realtime::token))
        .route("/api/realtime/usage", post(realtime::report_usage))
        // Chunk upload 5MB/lần → nâng body limit (mặc định axum 2MB). Không buffer cả file (append disk).
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        // Gộp SAU `.layer` ở trên: nhóm này giữ trần 1 MiB của riêng nó.
        .merge(transcript_docs)
        .route_layer(from_fn_with_state(state.clone(), auth::require_auth))
        // api_limit thêm SAU → chạy TRƯỚC require_auth (reject rẻ trước khi verify session).
        .route_layer(from_fn_with_state(state.clone(), rate_limit::api_limit));

    // Auth: rate-limit chặt (chống brute/amplification). Health KHÔNG giới hạn (systemd/nginx liveness).
    let auth_routes = Router::new()
        .route("/api/auth/login", get(auth::login))
        .route("/api/auth/callback", get(auth::callback))
        .route("/api/auth/native/config", get(auth::native_config))
        .route("/api/auth/native/refresh", post(auth::native_refresh))
        .route("/api/auth/logout", post(auth::logout))
        .route_layer(from_fn_with_state(state.clone(), rate_limit::auth_limit));

    // Đổi code của app: route public, mỗi lần gọi IdP ⟹ giới hạn chặt riêng.
    let exchange_routes = Router::new()
        .route("/api/auth/native/exchange", post(auth::native_exchange))
        .route_layer(from_fn_with_state(state.clone(), rate_limit::exchange_limit));

    // Back-channel logout: IdP gọi (logout token có chữ ký), không giới hạn tần suất — bị 429 thì phiên
    // web sống tiếp sau khi người dùng đã đăng xuất ở IdP.
    let backchannel = Router::new().route("/api/auth/backchannel-logout", post(auth::backchannel_logout));

    let public = Router::new()
        .route("/api/health", get(health))
        .merge(auth_routes)
        .merge(exchange_routes)
        .merge(backchannel);

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        // Ngoài cùng: thấy cả request bị rate-limit/401 và đặt IP cho audit.
        .layer(from_fn(access_log::layer))
        .with_state(state)
}

/// Health check qua nginx — KHÔNG cần DB (deploy gate + systemd liveness).
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
