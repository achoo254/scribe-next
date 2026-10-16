//! Gói chép lời của user (chưa bán: gói do quản trị cấp bằng `scribe-be admin set-plan`).

use crate::config::TranscribeLimits;
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
}
