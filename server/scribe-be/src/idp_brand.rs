//! `GET /api/idp`: tenant của Host đang mở, công khai (trang đăng nhập chưa có phiên, trang "chưa bật"). Tên hiển thị
//! là tên tài khoản đăng nhập của tenant (`TENANT_DISPLAY_NAME` của IdP, vd "Auth Next"): giao diện gọi tài khoản người
//! dùng theo tên này thay vì viết cứng. Tenant và tên lấy từ `tenant::directory` (bộ nhớ), không gọi IdP ở đây.

use crate::tenant::Tenant;
use axum::{Extension, Json};
use serde_json::{json, Value};

/// Tên mặc định của tenant ở hoandat-auth (`TENANT_DISPLAY_NAME` khi IdP không đặt).
pub const DEFAULT_DISPLAY_NAME: &str = "Auth Next";
const MAX_NAME_CHARS: usize = 64;

/// Tên dùng được để hiện: bỏ khoảng trắng hai đầu, không rỗng, không ký tự điều khiển, tối đa 64 ký tự.
pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let ok = !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS && !name.chars().any(char::is_control);
    ok.then(|| name.to_string())
}

/// `GET /api/idp` — `{ displayName, domain, issuer, primary, scribe: "enabled" | "disabled" }`. Không có gì bí mật.
pub async fn info(Extension(tenant): Extension<Tenant>) -> Json<Value> {
    Json(json!({
        "displayName": tenant.display_name,
        "domain": tenant.domain,
        "issuer": tenant.issuer,
        "primary": tenant.primary,
        "scribe": if tenant.enabled { "enabled" } else { "disabled" },
    }))
}

/// Giá trị `IDP_DISPLAY_NAME` của bản build sau khi làm sạch; không đặt hoặc không dùng được ⟹ "Auth Next".
pub fn build_display_name(raw: Option<&str>) -> String {
    raw.and_then(clean_name).unwrap_or_else(|| DEFAULT_DISPLAY_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::{test_tenant, PRIMARY_ID};

    #[test]
    fn names_are_validated() {
        assert_eq!(clean_name(" Auth Next "), Some("Auth Next".into()));
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name("a\nb"), None);
        assert_eq!(clean_name(&"x".repeat(65)), None);
        assert_eq!(clean_name(&"đ".repeat(64)), Some("đ".repeat(64)));
        assert_eq!(build_display_name(None), "Auth Next");
        assert_eq!(build_display_name(Some(" ")), "Auth Next");
        assert_eq!(build_display_name(Some("hoandat")), "hoandat");
    }

    #[tokio::test]
    async fn info_describes_the_tenant_of_the_host() {
        let Json(primary) = info(Extension(test_tenant(PRIMARY_ID, true))).await;
        assert_eq!(
            primary,
            json!({ "displayName": "primary", "domain": "hoandat.com", "issuer": "https://auth.hoandat.com",
                    "primary": true, "scribe": "enabled" })
        );
        let Json(off) = info(Extension(test_tenant("ten_a", false))).await;
        assert_eq!((off["primary"].clone(), off["scribe"].clone()), (json!(false), json!("disabled")));
        assert_eq!(off["domain"], "ten_a.example");
    }
}
