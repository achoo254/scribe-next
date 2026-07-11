//! Kết nối MongoDB (DB riêng `onescribe`). `with_uri_str` là lazy — không ping ngay nên khởi động
//! được cả khi chưa có mongod (kết nối thật xảy ra ở op đầu tiên).

use mongodb::{Client, Database};

pub async fn connect(uri: &str) -> Result<Database, String> {
    let client = Client::with_uri_str(uri)
        .await
        .map_err(|e| format!("kết nối MongoDB: {e}"))?;
    Ok(client.database("onescribe"))
}
