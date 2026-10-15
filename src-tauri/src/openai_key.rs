// OpenAI API key cho nguồn chép cloud (Mode C) — lưu trong OS keychain (macOS Keychain / Windows
// Credential Manager) qua crate keyring, KHÔNG nằm trong settings.json. Frontend chỉ biết có/không;
// giá trị key chỉ được Rust đọc lúc gửi job cho sidecar (qua stdin pipe).
use keyring::Entry;

const SERVICE: &str = "vn.inet.onescribe";
const ACCOUNT: &str = "openai-api-key";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("Không mở được keychain: {e}"))
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Không đọc được API key từ keychain: {e}")),
    }
}

pub fn save(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API key trống".into());
    }
    entry()?
        .set_password(key)
        .map_err(|e| format!("Không lưu được API key vào keychain: {e}"))
}

pub fn delete() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Không xoá được API key: {e}")),
    }
}
