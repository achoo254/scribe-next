//! Key AI của tenant: chủ domain nhập ở `https://auth.<domain>/admin`, IdP niêm phong cho Scribe ([`sealed`]) và trả
//! bản niêm phong qua API nội bộ cho role `scribe` ([`cache`]). Tiền AI của tenant sổ là của chủ domain: lượt bằng key
//! tenant không trừ phút, quỹ hay trần USD (ghi như key riêng, `byok` kèm `keySource: "tenant"`); hạn mức dung lượng gói
//! free vẫn áp.
//!
//! - Chọn key một chỗ ([`choose`]): key riêng hợp lệ của người dùng → key tenant hợp lệ (thứ tự mặc định của
//!   `user_keys::Use::providers`) → key server, **chỉ ở tenant chính**. Tenant sổ không có key cho việc đó ⟹ 403
//!   `TENANT_KEY_MISSING`. Tenant chính không bao giờ dùng key tenant (hành vi giữ nguyên).
//! - Lượt xếp hàng chỉ ghi nhà cung cấp; worker mở key lúc chạy theo `tenantId` của lượt ([`open`]). Bản rõ sống trong
//!   `Zeroizing`, không vào log, audit, lỗi hay câu trả lời.
//! - Key tenant lỗi lúc dùng (sai, hết credit, không mở được): đánh dấu đúng bản niêm phong đó, báo IdP
//!   `POST …/scribe-keys/:vendor/status` kèm `recipient` và `tag` để `/admin` hiện cho chủ domain, audit
//!   `tenant_key.failed` ([`report`]). Không đụng cờ tạm ngưng hay báo động của key server, không chuyển sang key server.
//! - Bản niêm phong cho khoá Scribe không có (tệp khoá đã gỡ hay thiếu): lỗi phía vận hành, không phải của key owner ⟹
//!   không báo IdP, log lỗi và báo người vận hành qua bot một lần mỗi vân tay.
//!
//! Chưa có vòng khoá niêm phong (`TENANT_SEALING_KEYS_DIR`) hay API nội bộ ⟹ [`TenantKeys`] vắng và mọi tenant sổ "chưa
//! bật" (`tenant::Readiness::sealed_keys`).

pub mod cache;
pub mod sealed;

use crate::audit;
use crate::config::{Config, Vendor};
use crate::error::AppError;
use crate::oidc_transport::valid_tenant_id;
use crate::state::AppState;
use crate::tenant::internal_api::{send_error, InternalApi};
use crate::tenant::TenantCtx;
use crate::user_keys::{self, Problem, Use};
use axum::http::StatusCode;
use cache::{KeyCache, StoredKey};
use mongodb::bson::doc;
use sealed::Keyring;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
/// Báo trạng thái key lỗi mà IdP chưa nhận: thử lại chừng này lần, cách nhau [`REPORT_RETRY_EVERY`].
const REPORT_ATTEMPTS: u32 = 3;
const REPORT_RETRY_EVERY: Duration = Duration::from_secs(30);
/// Số vân tay thiếu khoá đã báo người vận hành giữ tối đa (quá thì chỉ log).
const MAX_ALERTED_RECIPIENTS: usize = 64;

/// Câu cho người dùng khi chưa đọc được key của domain (IdP lỗi quá lâu).
pub const UNAVAILABLE: &str = "Chưa đọc được key của domain lúc này, thử lại sau ít phút.";

/// Vòng khoá niêm phong + bản niêm phong theo tenant + API nội bộ để đọc và báo trạng thái.
pub struct TenantKeys {
    keyring: Keyring,
    cache: Arc<KeyCache>,
    api: Arc<InternalApi>,
    /// Vân tay đã báo người vận hành là thiếu khoá (một lần mỗi process).
    alerted: Mutex<HashSet<String>>,
    /// Khoảng cách giữa các lần báo lại trạng thái key mà IdP chưa nhận.
    report_retry_every: Duration,
}

impl TenantKeys {
    /// Thiếu `TENANT_SEALING_KEYS_DIR` hay API nội bộ ⟹ `Ok(None)` (tính năng tắt, log một dòng). Thư mục khoá hỏng ⟹
    /// `Err` (không chứa nội dung khoá).
    pub fn load(config: &Config, api: Option<Arc<InternalApi>>) -> Result<Option<TenantKeys>, String> {
        let Some(dir) = config.tenant_sealing_keys_dir.as_deref() else {
            tracing::info!("key AI của tenant tắt (chưa đặt TENANT_SEALING_KEYS_DIR) — chỉ phục vụ tenant chính");
            return Ok(None);
        };
        let Some(api) = api else {
            tracing::info!("key AI của tenant tắt (chưa có API nội bộ của IdP) — chỉ phục vụ tenant chính");
            return Ok(None);
        };
        let keyring = Keyring::load_dir(dir)?;
        tracing::info!(recipients = ?keyring.recipients(), "key AI của tenant bật");
        Ok(Some(TenantKeys::new(keyring, api)))
    }

    pub fn new(keyring: Keyring, api: Arc<InternalApi>) -> TenantKeys {
        TenantKeys {
            keyring,
            cache: Arc::new(KeyCache::default()),
            api,
            alerted: Mutex::default(),
            report_retry_every: REPORT_RETRY_EVERY,
        }
    }

    async fn keys(&self, tenant_id: &str) -> Result<Arc<Vec<StoredKey>>, String> {
        if !valid_tenant_id(tenant_id) {
            return Err("id tenant không hợp lệ".into());
        }
        let (api, id) = (self.api.clone(), tenant_id.to_string());
        self.cache.get(tenant_id, move || async move { fetch(&api, &id).await }).await
    }

    /// Nhà cung cấp có key dùng được (IdP chưa ghi lỗi, Scribe chưa báo lỗi bản niêm phong đó).
    async fn usable(&self, tenant_id: &str) -> Result<Vec<Vendor>, String> {
        let keys = self.keys(tenant_id).await?;
        Ok(keys.iter().filter(|k| self.cache.problem(tenant_id, k).is_none()).map(|k| k.vendor).collect())
    }
}

/// `GET /internal/tenants/:id/scribe-keys`. 404 (IdP không còn phục vụ tenant) ⟹ không key nào.
async fn fetch(api: &InternalApi, tenant_id: &str) -> Result<Vec<StoredKey>, String> {
    let res = api
        .get(&format!("/internal/tenants/{tenant_id}/scribe-keys"), FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| send_error(&e))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    if !status.is_success() {
        return Err(format!("IdP trả {status}"));
    }
    let body = res.bytes().await.map_err(|e| send_error(&e))?;
    cache::parse_listing(&body)
}

/// Key nào chạy một việc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Key riêng của người dùng.
    Own(Vendor),
    /// Key của chủ domain (tenant sổ).
    Tenant(Vendor),
    /// Key server, có hạn mức (chỉ tenant chính).
    Server,
}

impl Choice {
    /// Nhà cung cấp của key không phải của server.
    pub fn vendor(self) -> Option<Vendor> {
        match self {
            Choice::Own(v) | Choice::Tenant(v) => Some(v),
            Choice::Server => None,
        }
    }
}

/// Luật chọn key (không I/O): `own` là key riêng hợp lệ của người dùng cho việc `use_` (đã áp lựa chọn của họ),
/// `tenant_usable` là nhà cung cấp có key tenant dùng được. Key server chỉ khi `primary`.
pub fn decide(primary: bool, own: Option<Vendor>, tenant_usable: &[Vendor], use_: Use) -> Result<Choice, AppError> {
    if let Some(v) = own {
        return Ok(Choice::Own(v));
    }
    if primary {
        return Ok(Choice::Server);
    }
    user_keys::pick(use_, &user_keys::Prefs::default(), tenant_usable).map(Choice::Tenant).ok_or_else(crate::tenant::key_missing)
}

/// Key cho việc `use_` của `user_id` ở `tenant`. Lỗi: 403 `TENANT_KEY_MISSING` (tenant sổ không có key cho việc này),
/// 503 khi chưa đọc được key của domain.
pub async fn choose(state: &AppState, tenant: &TenantCtx, user_id: &str, use_: Use) -> Result<Choice, AppError> {
    let active = user_keys::active(state, user_id).await?;
    let own = match use_ {
        Use::Transcribe => active.transcribe,
        Use::Live => active.live,
        Use::Text => active.text.map(|(v, _)| v),
    };
    if own.is_some() || tenant.primary {
        return decide(tenant.primary, own, &[], use_);
    }
    let usable = match &state.tenant_keys {
        Some(keys) => keys.usable(&tenant.id).await.map_err(|e| {
            tracing::warn!(tenant = %tenant.id, "đọc key của domain lỗi: {e}");
            AppError::Unavailable(UNAVAILABLE.into())
        })?,
        None => Vec::new(),
    };
    decide(false, None, &usable, use_)
}

/// Phần `tenantKeys` của `GET /api/me` ở tenant sổ: nhà cung cấp key tenant cho chép lời và cho model chữ (`null` = chủ
/// domain chưa có key cho việc đó). Đọc lỗi ⟹ `null` cả phần (không làm hỏng `/api/me`).
pub async fn me_summary(state: &AppState, tenant: &TenantCtx) -> Value {
    let Some(keys) = &state.tenant_keys else { return json!({ "transcribe": null, "text": null }) };
    match keys.usable(&tenant.id).await {
        Ok(usable) => {
            let none = user_keys::Prefs::default();
            json!({
                "transcribe": user_keys::pick(Use::Transcribe, &none, &usable).map(Vendor::code),
                "text": user_keys::pick(Use::Text, &none, &usable).map(Vendor::code),
            })
        }
        Err(e) => {
            tracing::warn!(tenant = %tenant.id, "đọc key của domain cho /api/me lỗi: {e}");
            Value::Null
        }
    }
}

/// Key tenant đã mở, sống ngắn: bản rõ bị ghi đè khi huỷ. Không có `Debug`.
pub struct TenantKey {
    pub vendor: Vendor,
    tenant_id: String,
    recipient: String,
    tag: String,
    value: Zeroizing<String>,
}

impl TenantKey {
    pub fn value(&self) -> &str {
        self.value.as_str()
    }
}

/// Câu người dùng thấy khi key của domain không dùng được.
pub fn failure_message(vendor: Vendor) -> String {
    format!("Key {} của domain không dùng được — chủ domain sửa ở trang quản trị.", vendor.label())
}

/// Lỗi trả cho request khi key tenant không dùng được.
pub fn tenant_key_error(message: String) -> AppError {
    AppError::Coded { status: StatusCode::FORBIDDEN, code: "TENANT_KEY_FAILED", message }
}

/// Mở key `vendor` của `tenant_id` lúc chạy việc. `Err` là câu cho người dùng: chưa đọc được key, chủ domain đã gỡ key,
/// key đã báo lỗi, Scribe không có khoá niêm phong của bản đó (báo người vận hành), hay có khoá mà bản niêm phong không
/// mở được (báo `unreadable` về IdP).
pub async fn open(state: &AppState, tenant_id: &str, vendor: Vendor) -> Result<TenantKey, String> {
    let Some(keys) = &state.tenant_keys else { return Err(failure_message(vendor)) };
    let listing = keys.keys(tenant_id).await.map_err(|e| {
        tracing::warn!(tenant = %tenant_id, "đọc key của domain lỗi: {e}");
        UNAVAILABLE.to_string()
    })?;
    let Some(stored) = listing.iter().find(|k| k.vendor == vendor) else {
        return Err(failure_message(vendor));
    };
    if keys.cache.problem(tenant_id, stored).is_some() {
        return Err(failure_message(vendor));
    }
    if !keys.keyring.holds(&stored.sealed.recipient) {
        keys.missing_sealing_key(state, tenant_id, vendor, &stored.sealed.recipient);
        return Err(failure_message(vendor));
    }
    let context = sealed::context(tenant_id, vendor.code());
    match keys.keyring.open(&stored.sealed, &context) {
        Some(value) => Ok(TenantKey {
            vendor,
            tenant_id: tenant_id.to_string(),
            recipient: stored.sealed.recipient.clone(),
            tag: stored.sealed.tag.clone(),
            value,
        }),
        None => {
            tracing::error!(tenant = %tenant_id, provider = vendor.code(), "không mở được key của domain (bản niêm phong hỏng hay sai tenant)");
            report_status(state, keys, tenant_id, vendor, &stored.sealed.recipient, &stored.sealed.tag, "unreadable").await;
            Err(failure_message(vendor))
        }
    }
}

/// Key tenant lỗi lúc dùng: đánh dấu bản niêm phong đó, báo IdP, ghi audit (không có key), trả câu cho người dùng.
pub async fn report(state: &AppState, key: &TenantKey, problem: Problem) -> String {
    let status = match problem {
        Problem::Invalid => "invalid",
        Problem::NoCredit => "quota",
    };
    if let Some(keys) = &state.tenant_keys {
        report_status(state, keys, &key.tenant_id, key.vendor, &key.recipient, &key.tag, status).await;
    }
    failure_message(key.vendor)
}

impl TenantKeys {
    /// Bản niêm phong mang vân tay `recipient` mà vòng khoá không có (khoá đã gỡ khi lộ, tệp thiếu sau deploy). Không báo
    /// `unreadable` về IdP: lỗi phía vận hành, key của owner có thể vẫn tốt, và `/admin` sẽ không tự hiện gì; người vận
    /// hành khôi phục tệp khoá hoặc báo chủ domain nhập lại key. Báo động chỉ có vân tay (công khai), không có gì của khoá.
    fn missing_sealing_key(&self, state: &AppState, tenant_id: &str, vendor: Vendor, recipient: &str) {
        let shown = if recipient.len() == 16 && recipient.bytes().all(|b| b.is_ascii_hexdigit()) { recipient } else { "?" };
        tracing::error!(
            tenant = %tenant_id,
            provider = vendor.code(),
            recipient = %shown,
            "không có khoá niêm phong cho bản key của domain — không báo IdP, cần người vận hành"
        );
        let first = {
            let mut alerted = self.alerted.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            alerted.len() < MAX_ALERTED_RECIPIENTS && alerted.insert(shown.to_string())
        };
        if first {
            crate::notify::send(
                state,
                format!(
                    "Key AI của domain niêm phong cho khoá {shown} mà Scribe không có (tenant {tenant_id}, {}). Khôi phục \
                     tệp khoá vào TENANT_SEALING_KEYS_DIR, hoặc báo chủ domain nhập lại key — /admin không tự hiện lỗi này.",
                    vendor.label()
                ),
            );
        }
    }
}

async fn report_status(
    state: &AppState,
    keys: &Arc<TenantKeys>,
    tenant_id: &str,
    vendor: Vendor,
    recipient: &str,
    tag: &str,
    status: &'static str,
) {
    // Mỗi bản niêm phong chỉ báo một lần, trừ khi lần báo trước không tới IdP.
    if !keys.cache.mark(tenant_id, vendor, tag, status) {
        return;
    }
    tracing::warn!(tenant = %tenant_id, provider = vendor.code(), status, "key của domain không dùng được");
    audit::record(state, None, "tenant_key.failed", doc! { "tenantId": tenant_id, "provider": vendor.code(), "status": status });
    let path = format!("/internal/tenants/{tenant_id}/scribe-keys/{}/status", vendor.code());
    let body = json!({ "status": status, "recipient": recipient, "tag": tag });
    if post_status(&keys.api, &path, &body).await {
        return;
    }
    // IdP chưa nhận (đang khởi động lại?): thử lại nền, lượt của người dùng không phải chờ.
    let (keys, tenant_id, tag) = (keys.clone(), tenant_id.to_string(), tag.to_string());
    tokio::spawn(async move {
        for _ in 1..REPORT_ATTEMPTS {
            tokio::time::sleep(keys.report_retry_every).await;
            if post_status(&keys.api, &path, &body).await {
                return;
            }
        }
        // Vẫn không dùng bản key này, nhưng lần lỗi sau được báo lại để `/admin` có lúc biết.
        keys.cache.forget_report(&tenant_id, vendor, &tag);
        tracing::error!(
            tenant = %tenant_id,
            provider = vendor.code(),
            status,
            "IdP không nhận trạng thái key của domain sau {REPORT_ATTEMPTS} lần — lần lỗi sau sẽ báo lại"
        );
    });
}

/// Một lần báo trạng thái key; `true` khi IdP nhận (2xx).
async fn post_status(api: &InternalApi, path: &str, body: &Value) -> bool {
    match api.post(path, FETCH_TIMEOUT).json(body).send().await {
        Ok(res) if res.status().is_success() => true,
        Ok(res) => {
            tracing::warn!(path = %path, "báo trạng thái key của domain: IdP trả {}", res.status());
            false
        }
        Err(e) => {
            tracing::warn!(path = %path, "báo trạng thái key của domain: {}", send_error(&e));
            false
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
