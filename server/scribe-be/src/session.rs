//! Bí mật của phiên: khoá 32 byte sinh lúc chạy lần đầu (tệp `chmod 600`, KHÔNG nhúng vào binary — lộ
//! binary không lộ khoá), token ngẫu nhiên, băm token để lưu, mã hoá refresh token IdP của thiết bị.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Đọc khoá từ file, hoặc sinh mới (32 byte ngẫu nhiên) + ghi `chmod 600` nếu chưa có.
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
    Ok(())
}

/// Token ngẫu nhiên 256 bit, base64url (cookie phiên, token thiết bị, state).
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Băm SHA-256 (hex) để lưu token: bản sao lưu DB không dùng được làm phiên.
pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn cipher(key: &[u8]) -> Aes256Gcm {
    // Khoá riêng cho việc mã hoá, dẫn xuất từ khoá phiên.
    let derived = Sha256::new().chain_update(b"scribe/device-refresh/v1").chain_update(key).finalize();
    Aes256Gcm::new(&derived)
}

/// Mã hoá AES-256-GCM, kết quả base64url(nonce ‖ ciphertext).
pub fn seal(key: &[u8], plain: &str) -> String {
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let sealed = cipher(key)
        .encrypt(Nonce::from_slice(&nonce), plain.as_bytes())
        .expect("AES-GCM mã hoá chuỗi trong bộ nhớ không lỗi");
    URL_SAFE_NO_PAD.encode([nonce.as_slice(), sealed.as_slice()].concat())
}

pub fn open(key: &[u8], sealed: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(sealed).ok()?;
    if bytes.len() < 13 {
        return None;
    }
    let (nonce, data) = bytes.split_at(12);
    let plain = cipher(key).decrypt(Nonce::from_slice(nonce), data).ok()?;
    String::from_utf8(plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_values_open_only_with_the_same_key() {
        let key = [7u8; 32];
        let sealed = seal(&key, "refresh-token");
        assert_eq!(open(&key, &sealed).as_deref(), Some("refresh-token"));
        assert_eq!(open(&[8u8; 32], &sealed), None);
        assert_ne!(seal(&key, "refresh-token"), sealed, "nonce mới mỗi lần");
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let (a, b) = (random_token(), random_token());
        assert_ne!(a, b);
        assert_eq!(hash_token("x").len(), 64);
        assert_eq!(hash_token("x"), hash_token("x"));
    }
}
