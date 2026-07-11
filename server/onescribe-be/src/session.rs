//! Session JWT (HS256) của BE — httpOnly cookie sau khi verify Firebase ID token.
//!
//! [Red-team #4] Signing key KHÔNG bake compile-time (nếu nhúng binary → `strings` moi ra → forge
//! session bất kỳ). BE tự sinh key ngẫu nhiên first-run, ghi file `chmod 600` ngoài web root.

use crate::error::AppError;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Claims session của BE: `sub` = user id (Mongo), `exp` epoch giây.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: String,
    pub exp: i64,
}

/// Đọc key từ file, hoặc sinh mới (32 byte ngẫu nhiên) + ghi `chmod 600` nếu chưa có.
pub fn load_or_create_key(path: &str) -> std::io::Result<Vec<u8>> {
    let p = Path::new(path);
    if let Ok(bytes) = std::fs::read(p) {
        if bytes.len() >= 32 {
            return Ok(bytes);
        }
    }
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(p, key)?;
    set_owner_only(p)?;
    Ok(key.to_vec())
}

#[cfg(unix)]
fn set_owner_only(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only(_p: &Path) -> std::io::Result<()> {
    Ok(()) // Windows dev: bỏ qua (server thật chạy Linux).
}

/// Cấp session token cho `user_id`, hết hạn sau `ttl_secs`.
pub fn issue(key: &[u8], user_id: &str, ttl_secs: i64) -> Result<String, AppError> {
    let claims = SessionClaims {
        sub: user_id.to_string(),
        exp: Utc::now().timestamp() + ttl_secs,
    };
    encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(key))
        .map_err(|e| AppError::Internal(format!("issue session: {e}")))
}

/// Verify session token → trả `user_id`. Sai/hết hạn → Unauthorized.
pub fn verify(key: &[u8], token: &str) -> Result<String, AppError> {
    let data = decode::<SessionClaims>(
        token,
        &DecodingKey::from_secret(key),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized("session không hợp lệ".into()))?;
    Ok(data.claims.sub)
}
