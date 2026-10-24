//! Mở key AI mà chủ domain niêm phong cho Scribe ở IdP (`sealForRecipient` của auth-next, ADR 0012 §2): khoá AES-256-GCM
//! ngẫu nhiên mã hoá key (IV 12 byte, tag 16 byte, AAD `<tenantId>\nscribe-<vendor>`), khoá AES bọc bằng RSA-OAEP
//! SHA-256 (MGF1 SHA-256) với public key của Scribe. Mọi phần nhị phân là base64url không padding.
//!
//! Vòng khoá niêm phong ([`Keyring`]): thư mục `TENANT_SEALING_KEYS_DIR` (700), mỗi tệp `<recipient>.pem` là private
//! key RSA ≥ 3072 bit PKCS#8 (600). `recipient` = 16 ký tự hex đầu của SHA-256 trên SPKI DER của public key, cùng cách
//! IdP tính: bản niêm phong mang `recipient` của khoá đã dùng, nên xoay khoá chỉ là thêm tệp mới (khoá cũ vẫn mở bản cũ).
//!
//! Lỗi mở không phân biệt loại (khoá lạ, bọc hỏng, AES hỏng đều là `None`) và không bao giờ trả ra mạng: Scribe không
//! phải oracle giải mã (RUSTSEC-2023-0071 của crate `rsa`, ADR 0001 §6). Bản rõ nằm trong `Zeroizing`, private key tự
//! ghi đè khi huỷ (`RsaPrivateKey` là `ZeroizeOnDrop`); không kiểu nào ở đây có `Debug` in nội dung.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use zeroize::Zeroizing;

/// Cỡ tối thiểu của khoá niêm phong (cùng mức IdP đòi ở public key).
pub const MIN_BITS: usize = 3072;
const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const AES_KEY_LEN: usize = 32;

/// Bản niêm phong của một key như API nội bộ trả (`sealed` của `GET /internal/tenants/:id/scribe-keys`).
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Sealed {
    pub iv: String,
    pub ciphertext: String,
    /// Tag AES-GCM: nhận diện đúng lần niêm phong này (owner lưu lại key thì tag mới), gửi kèm khi báo lỗi key.
    pub tag: String,
    pub wrapped_key: String,
    /// Vân tay public key đã dùng để bọc.
    pub recipient: String,
}

/// Dữ liệu kèm (AAD) của key `vendor` thuộc tenant `tenant_id`, đúng như IdP dùng lúc niêm phong.
pub fn context(tenant_id: &str, vendor: &str) -> String {
    format!("{tenant_id}\nscribe-{vendor}")
}

/// 16 ký tự hex đầu của SHA-256 trên SPKI DER của `public`.
pub fn recipient_of(public: &RsaPublicKey) -> Result<String, String> {
    let der = public.to_public_key_der().map_err(|_| "không mã hoá được public key".to_string())?;
    let digest = Sha256::digest(der.as_bytes());
    Ok(digest.iter().take(8).map(|b| format!("{b:02x}")).collect())
}

/// Một khoá niêm phong. Không `Debug`/`Clone`.
pub struct SealingKey {
    recipient: String,
    key: RsaPrivateKey,
}

impl SealingKey {
    /// PEM PKCS#8 của private key RSA ≥ [`MIN_BITS`] bit. Lỗi không chứa nội dung PEM.
    pub fn from_pem(pem: &str) -> Result<SealingKey, String> {
        let key = RsaPrivateKey::from_pkcs8_pem(pem).map_err(|_| "không phải private key RSA dạng PKCS#8 PEM".to_string())?;
        let bits = key.size() * 8;
        if bits < MIN_BITS {
            return Err(format!("khoá RSA {bits} bit, cần tối thiểu {MIN_BITS}"));
        }
        SealingKey::from_key(key)
    }

    fn from_key(key: RsaPrivateKey) -> Result<SealingKey, String> {
        let recipient = recipient_of(&key.to_public_key())?;
        Ok(SealingKey { recipient, key })
    }

    pub fn recipient(&self) -> &str {
        &self.recipient
    }
}

/// Các khoá niêm phong đang giữ (nhiều khoá khi đang xoay), chọn theo `recipient`.
pub struct Keyring {
    keys: Vec<SealingKey>,
}

/// Tệp hay thư mục có bit quyền nào trong `mask` (unix); nơi khác không kiểm được.
fn too_open(meta: &std::fs::Metadata, mask: u32) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & mask != 0
    }
    #[cfg(not(unix))]
    {
        let _ = (meta, mask);
        false
    }
}

impl Keyring {
    #[cfg(test)]
    pub fn new(keys: Vec<SealingKey>) -> Keyring {
        Keyring { keys }
    }

    /// Đọc thư mục khoá. Thư mục không đọc được hay quyền rộng hơn 700 ⟹ `Err`. Tệp `.pem` có quyền rộng hơn 600, không
    /// phải khoá RSA ≥ 3072 bit hay tên lệch vân tay ⟹ bỏ tệp đó và log lỗi (không có nội dung tệp). Không còn khoá nào
    /// ⟹ `Err`.
    pub fn load_dir(dir: &str) -> Result<Keyring, String> {
        let path = Path::new(dir);
        let meta = std::fs::metadata(path).map_err(|e| format!("không đọc được thư mục khoá niêm phong ({})", e.kind()))?;
        if !meta.is_dir() {
            return Err("TENANT_SEALING_KEYS_DIR không phải thư mục".into());
        }
        if too_open(&meta, 0o077) {
            return Err("thư mục khoá niêm phong phải có quyền 700 (chỉ chủ đọc được)".into());
        }
        let entries = std::fs::read_dir(path).map_err(|e| format!("không đọc được thư mục khoá niêm phong ({})", e.kind()))?;
        let mut files: Vec<std::path::PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "pem"))
            .collect();
        files.sort();
        let mut keys = Vec::new();
        for file in files {
            let name = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            match load_file(&file) {
                Ok(key) if key.recipient == name => {
                    tracing::info!(recipient = %key.recipient, "khoá niêm phong key tenant");
                    keys.push(key);
                }
                Ok(key) => tracing::error!(
                    file = %name,
                    recipient = %key.recipient,
                    "tên tệp khoá niêm phong phải là vân tay của khoá — bỏ tệp này"
                ),
                Err(e) => tracing::error!(file = %name, "bỏ tệp khoá niêm phong: {e}"),
            }
        }
        if keys.is_empty() {
            return Err("không có khoá niêm phong nào dùng được".into());
        }
        Ok(Keyring { keys })
    }

    pub fn recipients(&self) -> Vec<&str> {
        self.keys.iter().map(SealingKey::recipient).collect()
    }

    /// Mở `sealed` với dữ liệu kèm `context`. Không có khoá của `recipient`, bọc hay bản mã hỏng, sai `context` ⟹ `None`
    /// (không phân biệt loại lỗi).
    pub fn open(&self, sealed: &Sealed, context: &str) -> Option<Zeroizing<String>> {
        let key = self.keys.iter().find(|k| k.recipient == sealed.recipient)?;
        let decode = |s: &str| URL_SAFE_NO_PAD.decode(s).ok();
        let (iv, mut body, tag, wrapped) =
            (decode(&sealed.iv)?, decode(&sealed.ciphertext)?, decode(&sealed.tag)?, decode(&sealed.wrapped_key)?);
        if iv.len() != IV_LEN || tag.len() != TAG_LEN {
            return None;
        }
        // Giải mã có blinding (ngẫu nhiên hoá phép mũ RSA) để thời gian không lộ thông tin về private key.
        let aes_key = Zeroizing::new(key.key.decrypt_blinded(&mut rand::thread_rng(), Oaep::new::<Sha256>(), &wrapped).ok()?);
        if aes_key.len() != AES_KEY_LEN {
            return None;
        }
        body.extend_from_slice(&tag);
        let cipher = Aes256Gcm::new_from_slice(&aes_key).ok()?;
        let plain = Zeroizing::new(cipher.decrypt(Nonce::from_slice(&iv), Payload { msg: &body, aad: context.as_bytes() }).ok()?);
        let text = std::str::from_utf8(&plain).ok()?;
        Some(Zeroizing::new(text.to_string()))
    }
}

fn load_file(file: &Path) -> Result<SealingKey, String> {
    let meta = std::fs::metadata(file).map_err(|e| format!("không đọc được tệp ({})", e.kind()))?;
    if too_open(&meta, 0o077) {
        return Err("tệp khoá phải có quyền 600 (chỉ chủ tệp đọc được)".into());
    }
    let pem = Zeroizing::new(std::fs::read_to_string(file).map_err(|e| format!("không đọc được tệp ({})", e.kind()))?);
    SealingKey::from_pem(&pem)
}

/// Niêm phong như IdP (chỉ cho test): AES-256-GCM khoá ngẫu nhiên, bọc RSA-OAEP SHA-256.
#[cfg(test)]
pub(crate) fn seal_for_tests(public: &RsaPublicKey, context: &str, plain: &str) -> Sealed {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut aes_key = [0u8; AES_KEY_LEN];
    let mut iv = [0u8; IV_LEN];
    rng.fill_bytes(&mut aes_key);
    rng.fill_bytes(&mut iv);
    let cipher = Aes256Gcm::new_from_slice(&aes_key).expect("khoá 32 byte");
    let mut body = cipher.encrypt(Nonce::from_slice(&iv), Payload { msg: plain.as_bytes(), aad: context.as_bytes() }).expect("mã hoá");
    let tag = body.split_off(body.len() - TAG_LEN);
    let wrapped = public.encrypt(&mut rng, Oaep::new::<Sha256>(), &aes_key).expect("bọc khoá");
    Sealed {
        iv: URL_SAFE_NO_PAD.encode(iv),
        ciphertext: URL_SAFE_NO_PAD.encode(body),
        tag: URL_SAFE_NO_PAD.encode(tag),
        wrapped_key: URL_SAFE_NO_PAD.encode(wrapped),
        recipient: recipient_of(public).expect("vân tay"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;

    /// Vector thử chéo ngôn ngữ của auth-next (`src/__tests__/fixtures/scribe-sealed-vector.json`, khoá chỉ cho test).
    pub(crate) fn vector() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/scribe-sealed-vector.json")).expect("vector JSON")
    }

    pub(crate) fn vector_key() -> SealingKey {
        SealingKey::from_pem(vector()["privateKeyPkcs8Pem"].as_str().expect("pem")).expect("khoá vector")
    }

    fn vector_sealed() -> Sealed {
        serde_json::from_value(vector()["sealed"].clone()).expect("sealed")
    }

    #[test]
    fn the_cross_language_vector_opens_to_its_plaintext() {
        let v = vector();
        let key = vector_key();
        assert_eq!(key.recipient(), v["recipient"].as_str().expect("recipient"));
        let context = context(v["tenantId"].as_str().expect("tenantId"), v["vendor"].as_str().expect("vendor"));
        assert_eq!(context, v["context"].as_str().expect("context"));
        let ring = Keyring::new(vec![key]);
        let plain = ring.open(&vector_sealed(), &context).expect("mở được");
        assert_eq!(plain.as_str(), v["plaintext"].as_str().expect("plaintext"));
        // Public key của vector cho đúng vân tay đó (cùng cách tính với IdP).
        let public = <RsaPublicKey as rsa::pkcs8::DecodePublicKey>::from_public_key_pem(v["publicKeySpkiPem"].as_str().expect("pub"))
            .expect("public key");
        assert_eq!(recipient_of(&public).expect("vân tay"), v["recipient"].as_str().expect("recipient"));
    }

    #[test]
    fn a_wrong_context_recipient_or_one_flipped_byte_never_opens() {
        let v = vector();
        let ring = Keyring::new(vec![vector_key()]);
        let good = vector_sealed();
        let context = v["context"].as_str().expect("context");
        assert!(ring.open(&good, "ten_vectorTenant0001\nscribe-gemini").is_none(), "nhà cung cấp khác");
        assert!(ring.open(&good, "ten_other\nscribe-openai").is_none(), "tenant khác");
        assert!(ring.open(&Sealed { recipient: "0000000000000000".into(), ..good.clone() }, context).is_none(), "khoá lạ");
        let flip = |field: &str| {
            let mut bytes = URL_SAFE_NO_PAD.decode(field).expect("base64url");
            bytes[0] ^= 1;
            URL_SAFE_NO_PAD.encode(bytes)
        };
        for tampered in [
            Sealed { ciphertext: flip(&good.ciphertext), ..good.clone() },
            Sealed { tag: flip(&good.tag), ..good.clone() },
            Sealed { iv: flip(&good.iv), ..good.clone() },
            Sealed { wrapped_key: flip(&good.wrapped_key), ..good.clone() },
        ] {
            assert!(ring.open(&tampered, context).is_none(), "một byte bị sửa");
        }
        assert!(ring.open(&Sealed { iv: "AAAA".into(), ..good.clone() }, context).is_none(), "IV sai cỡ");
        assert!(ring.open(&Sealed { tag: "!".into(), ..good }, context).is_none(), "không phải base64url");
    }

    #[test]
    fn keys_rotate_by_recipient() {
        // Khoá thứ hai nhỏ cho test nhanh (qua `from_key`, không qua kiểm cỡ của tệp).
        let small = RsaPrivateKey::new(&mut rand::thread_rng(), 1024).expect("sinh khoá");
        let other = SealingKey::from_key(small).expect("khoá");
        let other_public = other.key.to_public_key();
        let ring = Keyring::new(vec![other, vector_key()]);
        assert_eq!(ring.recipients().len(), 2);
        let sealed = seal_for_tests(&other_public, &context("ten_a", "elevenlabs"), "sk_rotated_key");
        assert_eq!(ring.open(&sealed, &context("ten_a", "elevenlabs")).expect("khoá mới").as_str(), "sk_rotated_key");
        // Bản cũ vẫn mở bằng khoá cũ.
        let v = vector();
        assert!(ring.open(&vector_sealed(), v["context"].as_str().expect("context")).is_some());
        // Gỡ khoá đã bọc ⟹ không mở được.
        let without = Keyring::new(vec![vector_key()]);
        assert!(without.open(&sealed, &context("ten_a", "elevenlabs")).is_none());
    }

    #[test]
    fn small_or_non_rsa_keys_are_refused() {
        let small = RsaPrivateKey::new(&mut rand::thread_rng(), 1024).expect("sinh khoá");
        let pem = rsa::pkcs8::EncodePrivateKey::to_pkcs8_pem(&small, rsa::pkcs8::LineEnding::LF).expect("pem");
        assert!(SealingKey::from_pem(&pem).err().expect("1024 bit bị từ chối").contains("3072"));
        assert!(SealingKey::from_pem("not a key").is_err());
        let public = vector()["publicKeySpkiPem"].as_str().expect("pub").to_string();
        assert!(SealingKey::from_pem(&public).is_err(), "public key không phải private key");
    }

    #[test]
    fn the_key_directory_keeps_only_files_named_by_their_fingerprint() {
        let dir = std::env::temp_dir().join(format!("scribe-seal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("thư mục");
        let pem = vector()["privateKeyPkcs8Pem"].as_str().expect("pem").to_string();
        let recipient = vector()["recipient"].as_str().expect("recipient").to_string();
        #[cfg(unix)]
        let chmod = |p: &Path, mode: u32| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).expect("chmod");
        };
        #[cfg(unix)]
        chmod(&dir, 0o700);
        let dir_str = dir.to_str().expect("utf8").to_string();
        assert!(Keyring::load_dir(&dir_str).is_err(), "thư mục rỗng");
        // Tên lệch vân tay: bỏ.
        let wrong = dir.join("0000000000000000.pem");
        std::fs::write(&wrong, &pem).expect("ghi");
        #[cfg(unix)]
        chmod(&wrong, 0o600);
        assert!(Keyring::load_dir(&dir_str).is_err(), "chỉ có tệp sai tên");
        std::fs::remove_file(&wrong).expect("xoá");
        let right = dir.join(format!("{recipient}.pem"));
        std::fs::write(&right, &pem).expect("ghi");
        std::fs::write(dir.join("README.txt"), "không phải khoá").expect("ghi");
        #[cfg(unix)]
        {
            chmod(&right, 0o644);
            assert!(Keyring::load_dir(&dir_str).is_err(), "tệp 644 bị bỏ");
            chmod(&right, 0o600);
            chmod(&dir, 0o755);
            assert!(Keyring::load_dir(&dir_str).is_err(), "thư mục 755 bị từ chối");
            chmod(&dir, 0o700);
        }
        let ring = Keyring::load_dir(&dir_str).expect("đọc được");
        assert_eq!(ring.recipients(), vec![recipient.as_str()]);
        assert!(Keyring::load_dir(dir.join("missing").to_str().expect("utf8")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
