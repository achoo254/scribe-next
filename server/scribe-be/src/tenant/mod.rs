//! Scribe chỉ phục vụ một tenant: `hoandat.com` (`PUBLIC_URL`, IdP `OIDC_ISSUER`). Đa tenant theo domain đã bỏ (ADR
//! `docs/decisions/0001-tenants-by-domain.md`, superseded).
//!
//! Dữ liệu giữ trường `tenantId` và các index `tenantId_*`: danh tính (`users`, phiên, thiết bị, lượt đăng nhập, jti)
//! và hàng đợi (`transcripts`, `jobs`) ghi với [`PRIMARY_ID`] và đọc có lọc theo nó, nên doc của tenant khác không bao
//! giờ được phục vụ hay nhận chạy.

/// Id của tenant duy nhất (cùng id với `PRIMARY_TENANT_ID` của auth-next).
pub const PRIMARY_ID: &str = "primary";

/// [`PRIMARY_ID`] làm giá trị mặc định khi đọc doc thiếu `tenantId` (serde `default`).
pub fn primary_id() -> String {
    PRIMARY_ID.to_string()
}
