//! Kết nối MongoDB (database theo `DB_NAME` lúc build). `with_uri_str` là lazy — không ping ngay nên khởi động
//! được cả khi chưa có mongod (kết nối thật xảy ra ở op đầu tiên).

use mongodb::{Client, Database};

pub async fn connect(uri: &str, db_name: &str) -> Result<Database, String> {
    let client = Client::with_uri_str(uri)
        .await
        .map_err(|e| format!("kết nối MongoDB: {e}"))?;
    Ok(client.database(db_name))
}

/// Test trên Mongo dev chạy đối soát cả database (`search::index::reconcile`) hoặc hoàn mọi phần giữ giờ ElevenLabs
/// (`usage::refund_provider_holds`) giữ khoá này; test có dữ liệu giả mà các bước đó sẽ "sửa" (câu của transcript chưa
/// xong, phần giữ giờ đang mở) cũng giữ, để hai bên không chạy chồng nhau.
#[cfg(test)]
pub static WHOLE_DB_SWEEP: std::sync::LazyLock<tokio::sync::Mutex<()>> = std::sync::LazyLock::new(Default::default);
