//! Gói của user (chưa bán: gói do quản trị cấp bằng `scribe-be admin set-plan`): hạn mức chép lời và
//! dung lượng lưu trữ.

use crate::config::{
    TranscribeLimits, FREE_MAX_FILE_BYTES, FREE_QUOTA_BYTES, FREE_RETENTION_DAYS, PLUS_MAX_FILE_BYTES,
    PLUS_QUOTA_BYTES, RETENTION_DAYS,
};
use mongodb::bson::DateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Free,
    Plus,
}

impl Plan {
    pub fn parse(name: &str) -> Option<Plan> {
        match name {
            "free" => Some(Plan::Free),
            "plus" => Some(Plan::Plus),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Plan::Free => "free",
            Plan::Plus => "plus",
        }
    }

    /// Hạn mức giây audio mỗi tháng của gói.
    pub fn limit_seconds(self, limits: &TranscribeLimits) -> i64 {
        match self {
            Plan::Free => limits.free_seconds,
            Plan::Plus => limits.plus_seconds,
        }
    }

    /// Dung lượng lưu trữ của gói: tổng, mỗi tệp, số ngày giữ tệp mới.
    pub fn storage(self) -> Storage {
        match self {
            Plan::Free => Storage {
                quota_bytes: FREE_QUOTA_BYTES,
                max_file_bytes: FREE_MAX_FILE_BYTES,
                retention_days: FREE_RETENTION_DAYS,
            },
            Plan::Plus => Storage {
                quota_bytes: PLUS_QUOTA_BYTES,
                max_file_bytes: PLUS_MAX_FILE_BYTES,
                retention_days: RETENTION_DAYS,
            },
        }
    }
}

/// Giới hạn lưu trữ của một gói.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Storage {
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
    pub retention_days: i64,
}

impl Storage {
    /// Câu báo hết dung lượng (413) cho người dùng đọc; iPhone hiện nguyên câu này.
    pub fn quota_message(&self) -> String {
        format!("Đã hết dung lượng lưu trữ ({}). Xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.", size_label(self.quota_bytes))
    }

    /// Câu báo tệp vượt cỡ tối đa của gói (413).
    pub fn file_too_large_message(&self) -> String {
        format!("Mỗi tệp tối đa {}.", size_label(self.max_file_bytes))
    }
}

/// "200 MB", "2 GB" (đơn vị 1024, khớp cách web hiện dung lượng).
pub fn size_label(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB && bytes.is_multiple_of(GIB) {
        format!("{} GB", bytes / GIB)
    } else {
        format!("{} MB", bytes.div_ceil(MIB))
    }
}

/// Gói đang có hiệu lực: gói lưu trên user, hết hạn (`until` đã qua) thì về free. Không cần job dọn.
pub fn effective(stored: Option<&str>, until: Option<DateTime>, now: DateTime) -> Plan {
    match stored.and_then(Plan::parse) {
        Some(Plan::Plus) if until.is_none_or(|u| u > now) => Plan::Plus,
        _ => Plan::Free,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_lapses_back_to_free() {
        let now = DateTime::from_millis(1_000_000);
        assert_eq!(effective(Some("plus"), None, now), Plan::Plus);
        assert_eq!(effective(Some("plus"), Some(DateTime::from_millis(2_000_000)), now), Plan::Plus);
        assert_eq!(effective(Some("plus"), Some(DateTime::from_millis(500_000)), now), Plan::Free);
        assert_eq!(effective(None, None, now), Plan::Free);
        assert_eq!(effective(Some("gold"), None, now), Plan::Free);
    }

    #[test]
    fn free_storage_is_two_hundred_megabytes_for_seven_days() {
        let free = Plan::Free.storage();
        assert_eq!((free.quota_bytes, free.max_file_bytes, free.retention_days), (200 << 20, 200 << 20, 7));
        let plus = Plan::Plus.storage();
        assert_eq!((plus.quota_bytes, plus.max_file_bytes, plus.retention_days), (5 << 30, 2 << 30, 14));
        assert!(free.quota_message().contains("200 MB"));
        assert!(plus.quota_message().contains("5 GB"));
        assert_eq!(plus.file_too_large_message(), "Mỗi tệp tối đa 2 GB.");
        assert_eq!(size_label(1), "1 MB");
    }
}
