//! Mã hoá key riêng của người dùng: AES-256-GCM (crate `aes-gcm`), bí mật 32 byte đọc từ tệp `USER_KEYS_SECRET_PATH`
//! (ngoài repo, ngoài database, quyền 600). Mỗi lần mã hoá một nonce 96 bit ngẫu nhiên mới; dữ liệu kèm (AAD) là
//! `userId` và nhà cung cấp, nên bản mã chép sang tài khoản hay nhà cung cấp khác không giải được.
//!
//! Bí mật và bản rõ nằm trong `Zeroizing`: bộ nhớ được ghi đè khi giá trị bị huỷ. Không kiểu nào ở đây có `Debug`
//! in ra nội dung.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use std::path::Path;
use zeroize::Zeroizing;

pub const NONCE_LEN: usize = 12;
const SECRET_LEN: usize = 32;

/// Bí mật mã hoá key riêng. Không có `Debug`/`Display`/`Clone`.
pub struct Secret(Zeroizing<[u8; SECRET_LEN]>);

impl Secret {
    /// Đúng 32 byte, không hơn không kém (`head -c 32 /dev/urandom`).
    pub fn from_bytes(bytes: &[u8]) -> Option<Secret> {
        let array: [u8; SECRET_LEN] = bytes.try_into().ok()?;
        Some(Secret(Zeroizing::new(array)))
    }

    /// Đọc tệp bí mật. Lỗi (thiếu tệp, sai cỡ, quyền rộng hơn 600) trả câu cho log, không có nội dung tệp.
    pub fn load(path: &str) -> Result<Secret, String> {
        let p = Path::new(path);
        let meta = std::fs::metadata(p).map_err(|e| format!("không đọc được tệp bí mật key riêng ({})", e.kind()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("tệp bí mật key riêng phải có quyền 600 (chỉ chủ tệp đọc được)".into());
            }
        }
        #[cfg(not(unix))]
        let _ = meta;
        let bytes = Zeroizing::new(std::fs::read(p).map_err(|e| format!("không đọc được tệp bí mật key riêng ({})", e.kind()))?);
        Secret::from_bytes(&bytes).ok_or_else(|| format!("tệp bí mật key riêng phải dài đúng {SECRET_LEN} byte"))
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(self.0.as_slice().into())
    }
}

/// Bản mã và nonce của một key.
pub struct Sealed {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; NONCE_LEN],
}

/// Dữ liệu kèm: gắn bản mã với đúng tài khoản và nhà cung cấp.
fn aad(user_id: &str, vendor: &str) -> Vec<u8> {
    format!("scribe/user-key/v1\u{0}{user_id}\u{0}{vendor}").into_bytes()
}

/// Mã hoá `plain` cho `user_id` + `vendor` với nonce ngẫu nhiên mới.
pub fn seal(secret: &Secret, user_id: &str, vendor: &str, plain: &str) -> Result<Sealed, String> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let aad = aad(user_id, vendor);
    let ciphertext = secret
        .cipher()
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain.as_bytes(), aad: &aad })
        .map_err(|_| "không mã hoá được key".to_string())?;
    Ok(Sealed { ciphertext, nonce })
}

/// Giải mã; sai bí mật, sai tài khoản/nhà cung cấp, bản mã hỏng hay nonce sai cỡ thì `None`.
pub fn open(secret: &Secret, user_id: &str, vendor: &str, ciphertext: &[u8], nonce: &[u8]) -> Option<Zeroizing<String>> {
    if nonce.len() != NONCE_LEN {
        return None;
    }
    let aad = aad(user_id, vendor);
    let plain = Zeroizing::new(secret.cipher().decrypt(Nonce::from_slice(nonce), Payload { msg: ciphertext, aad: &aad }).ok()?);
    let text = std::str::from_utf8(&plain).ok()?;
    Some(Zeroizing::new(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(byte: u8) -> Secret {
        Secret::from_bytes(&[byte; 32]).expect("32 byte")
    }

    #[test]
    fn roundtrip_with_the_same_secret_user_and_vendor() {
        let s = secret(7);
        let sealed = seal(&s, "u1", "openai", "sk-test-1234").expect("mã hoá");
        let plain = open(&s, "u1", "openai", &sealed.ciphertext, &sealed.nonce).expect("giải mã");
        assert_eq!(plain.as_str(), "sk-test-1234");
        assert!(!sealed.ciphertext.windows(4).any(|w| w == b"1234"), "bản mã không chứa bản rõ");
    }

    #[test]
    fn wrong_aad_secret_or_tampering_fails() {
        let s = secret(7);
        let sealed = seal(&s, "u1", "openai", "sk-test-1234").expect("mã hoá");
        assert!(open(&s, "u2", "openai", &sealed.ciphertext, &sealed.nonce).is_none(), "tài khoản khác");
        assert!(open(&s, "u1", "gemini", &sealed.ciphertext, &sealed.nonce).is_none(), "nhà cung cấp khác");
        assert!(open(&secret(8), "u1", "openai", &sealed.ciphertext, &sealed.nonce).is_none(), "bí mật khác");
        let mut tampered = sealed.ciphertext.clone();
        tampered[0] ^= 1;
        assert!(open(&s, "u1", "openai", &tampered, &sealed.nonce).is_none(), "bản mã bị sửa");
        assert!(open(&s, "u1", "openai", &sealed.ciphertext, &sealed.nonce[..8]).is_none(), "nonce sai cỡ");
    }

    #[test]
    fn every_seal_uses_a_fresh_nonce() {
        let s = secret(1);
        let nonces: std::collections::HashSet<[u8; NONCE_LEN]> =
            (0..1_000).map(|_| seal(&s, "u", "elevenlabs", "sk_test_key").expect("mã hoá").nonce).collect();
        assert_eq!(nonces.len(), 1_000);
    }

    #[test]
    fn secret_file_must_be_32_bytes_and_owner_only() {
        assert!(Secret::from_bytes(&[0; 31]).is_none() && Secret::from_bytes(&[0; 33]).is_none());
        let dir = std::env::temp_dir().join(format!("scribe-uk-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("thư mục");
        let path = dir.join("secret");
        std::fs::write(&path, [9u8; 32]).expect("ghi");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
            assert!(Secret::load(path.to_str().expect("utf8")).is_err(), "644 bị từ chối");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        }
        assert!(Secret::load(path.to_str().expect("utf8")).is_ok());
        std::fs::write(&path, [9u8; 16]).expect("ghi");
        assert!(Secret::load(path.to_str().expect("utf8")).is_err(), "sai cỡ");
        assert!(Secret::load(dir.join("missing").to_str().expect("utf8")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
