//! API nội bộ của hoandat-auth (`IDP_INTERNAL_API_URL`, chỉ nghe trên loopback, không qua nginx), tài khoản Basic
//! `scribe` với bí mật trong tệp `IDP_INTERNAL_API_SECRET_PATH` (600). Một client dùng chung cho mọi việc Scribe hỏi
//! IdP qua đường này: tra tenant theo Host, discovery và JWKS của tenant sổ, key AI niêm phong của tenant.
//!
//! Client không theo redirect: header Basic chỉ đi tới đúng địa chỉ đã cấu hình. Lỗi trả mô tả không chứa bí mật.

use std::time::Duration;

/// Tài khoản Basic của Scribe ở API nội bộ của IdP.
const ROLE: &str = "scribe";

/// API nội bộ đã cấu hình đủ (địa chỉ + bí mật đọc được).
pub struct InternalApi {
    url: String,
    secret: String,
    http: reqwest::Client,
}

impl InternalApi {
    /// Đủ cấu hình thì dựng client; thiếu một trong hai biến ⟹ `Ok(None)` (tính năng phụ thuộc tắt). Tệp bí mật hỏng
    /// hay không dựng được client ⟹ `Err` với mô tả không chứa bí mật.
    pub fn from_config(url: Option<&str>, secret_path: Option<&str>) -> Result<Option<InternalApi>, String> {
        let (Some(url), Some(secret_path)) = (url, secret_path) else {
            if url.is_some() || secret_path.is_some() {
                return Err("cần đặt cả IDP_INTERNAL_API_URL và IDP_INTERNAL_API_SECRET_PATH".into());
            }
            return Ok(None);
        };
        let secret = read_secret(secret_path)?;
        InternalApi::new(url, &secret).map(Some)
    }

    pub fn new(url: &str, secret: &str) -> Result<InternalApi, String> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("không dựng được HTTP client: {e}"))?;
        Ok(InternalApi { url: url.trim().trim_end_matches('/').to_string(), secret: secret.to_string(), http })
    }

    /// `GET {url}{path}` có Basic `scribe` và hạn chờ `timeout`.
    pub fn get(&self, path: &str, timeout: Duration) -> reqwest::RequestBuilder {
        self.http.get(format!("{}{path}", self.url)).basic_auth(ROLE, Some(&self.secret)).timeout(timeout)
    }

    /// `POST {url}{path}` có Basic `scribe` và hạn chờ `timeout`.
    pub fn post(&self, path: &str, timeout: Duration) -> reqwest::RequestBuilder {
        self.http.post(format!("{}{path}", self.url)).basic_auth(ROLE, Some(&self.secret)).timeout(timeout)
    }
}

/// Mô tả lỗi gửi request không chứa URL hay header (bí mật nằm ở header).
pub fn send_error(e: &reqwest::Error) -> String {
    format!("không gọi được IdP ({})", if e.is_timeout() { "quá giờ" } else { "lỗi mạng" })
}

/// Bí mật Basic trong tệp (600): một dòng, không khoảng trắng; IdP đòi tối thiểu 32 ký tự.
pub fn read_secret(path: &str) -> Result<String, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("không đọc được tệp bí mật ({})", e.kind()))?;
    let secret = raw.trim();
    if secret.len() < 32 || secret.chars().any(char::is_whitespace) {
        return Err("tệp bí mật không đúng dạng (một dòng, tối thiểu 32 ký tự)".into());
    }
    Ok(secret.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "s3cret-s3cret-s3cret-s3cret-s3cret";

    #[test]
    fn secrets_and_config_are_validated() {
        let dir = std::env::temp_dir().join(format!("scribe-idp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tạo thư mục");
        let path = dir.join("secret");
        std::fs::write(&path, format!("{SECRET}\n")).expect("ghi");
        let path_str = path.to_str().expect("path");
        assert_eq!(read_secret(path_str), Ok(SECRET.to_string()));
        assert!(InternalApi::from_config(Some("http://127.0.0.1:1/"), Some(path_str)).expect("đủ").is_some());
        assert!(InternalApi::from_config(None, None).expect("tắt").is_none());
        assert!(InternalApi::from_config(Some("http://127.0.0.1:1"), None).is_err(), "thiếu một biến");
        std::fs::write(&path, "short").expect("ghi");
        assert!(read_secret(path_str).is_err());
        assert!(read_secret(dir.join("missing").to_str().expect("path")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
