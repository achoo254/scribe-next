//! Giới hạn lưu trữ, chung cho mọi người dùng (chủ dự án chốt 02/11/2026: bỏ gói free/plus, mọi người một mức bằng
//! mức free cũ): tổng 200 MB, mỗi tệp tối đa 200 MB, tệp mới tự xoá sau 7 ngày. Việc AI chạy bằng key riêng của người
//! dùng nên không có phút hay lượt AI nào.
//!
//! Trường `plan`/`planUntil` cũ trong `users` không còn được đọc. Tệp đã có giữ nguyên `expiresAt` ghi lúc tạo (tệp của
//! tài khoản plus cũ vẫn theo hạn 14 ngày của nó); tài khoản đang dùng quá 200 MB không bị xoá gì, chỉ không ghi hay tải
//! lên thêm được tới khi xoá bớt.

const MIB: u64 = 1024 * 1024;

/// Giới hạn lưu trữ: tổng, mỗi tệp, số ngày giữ tệp mới.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Storage {
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
    pub retention_days: i64,
}

/// Giới hạn của mọi người dùng.
pub const LIMITS: Storage = Storage { quota_bytes: 200 * MIB, max_file_bytes: 200 * MIB, retention_days: 7 };

impl Storage {
    /// Câu báo hết dung lượng (413) cho người dùng đọc; iPhone hiện nguyên câu này.
    pub fn quota_message(&self) -> String {
        format!("Đã hết dung lượng lưu trữ ({}). Xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.", size_label(self.quota_bytes))
    }

    /// Câu báo tệp vượt cỡ tối đa (413).
    pub fn file_too_large_message(&self) -> String {
        format!("Mỗi tệp tối đa {}.", size_label(self.max_file_bytes))
    }
}

/// "200 MB", "2 GB" (đơn vị 1024, khớp cách web hiện dung lượng).
pub fn size_label(bytes: u64) -> String {
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB && bytes.is_multiple_of(GIB) {
        format!("{} GB", bytes / GIB)
    } else {
        format!("{} MB", bytes.div_ceil(MIB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyone_gets_two_hundred_megabytes_for_seven_days() {
        assert_eq!((LIMITS.quota_bytes, LIMITS.max_file_bytes, LIMITS.retention_days), (200 << 20, 200 << 20, 7));
        assert!(LIMITS.quota_message().contains("200 MB"));
        assert_eq!(LIMITS.file_too_large_message(), "Mỗi tệp tối đa 200 MB.");
        assert_eq!(size_label(1), "1 MB");
        assert_eq!(size_label(2 << 30), "2 GB");
    }
}
