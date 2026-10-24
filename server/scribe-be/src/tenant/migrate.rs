//! Gắn `tenantId` cho dữ liệu có trước khi có tenant, chạy lúc khởi động trước khi nhận request và trước index unique
//! `(tenantId, idpSub)` (như `migrateToTenants` của hoandat-auth, ADR 0006 của nó).
//!
//! - `users`, `sessions`, `devices`, `transcripts`, `jobs`: doc chưa có `tenantId` ⟹ `tenantId: "primary"` (mọi dữ
//!   liệu cũ là của tenant chính). Chỉ thêm trường, không đổi gì khác.
//! - `auth_states`, `logout_jtis`: doc chưa có `tenantId` bị xoá (TTL 10 phút, khoá `_id` của jti đổi dạng): lượt đăng
//!   nhập đang dở lúc deploy phải bắt đầu lại.
//!
//! Lặp lại được: chạy lần hai không đổi gì. Đếm lại sau khi ghi; còn doc thiếu `tenantId` (ghi lỗi giữa chừng) ⟹ lỗi,
//! `main` dừng khởi động thay vì phục vụ với danh tính chưa gắn tenant.

use super::PRIMARY_ID;
use mongodb::bson::{doc, Document};
use mongodb::Database;

/// Collection gắn `tenantId: "primary"` cho doc cũ.
pub const TAGGED: [&str; 5] = ["users", "sessions", "devices", "transcripts", "jobs"];
/// Collection ngắn hạn: doc cũ bị xoá.
pub const DROPPED: [&str; 2] = ["auth_states", "logout_jtis"];

/// Số doc đã đổi ở mỗi collection.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub tagged: Vec<(&'static str, u64)>,
    pub dropped: Vec<(&'static str, u64)>,
}

impl Report {
    pub fn changed(&self) -> u64 {
        self.tagged.iter().chain(&self.dropped).map(|(_, n)| n).sum()
    }
}

fn missing() -> Document {
    doc! { "tenantId": { "$exists": false } }
}

/// Gắn và dọn; lỗi Mongo hay còn sót doc thiếu `tenantId` ⟹ `Err` (mô tả cho log).
pub async fn run(db: &Database) -> Result<Report, String> {
    let mut report = Report::default();
    for coll in TAGGED {
        let res = db
            .collection::<Document>(coll)
            .update_many(missing(), doc! { "$set": { "tenantId": PRIMARY_ID } })
            .await
            .map_err(|e| format!("gắn tenantId cho {coll}: {e}"))?;
        report.tagged.push((coll, res.modified_count));
    }
    for coll in DROPPED {
        let res = db
            .collection::<Document>(coll)
            .delete_many(missing())
            .await
            .map_err(|e| format!("dọn {coll} chưa có tenantId: {e}"))?;
        report.dropped.push((coll, res.deleted_count));
    }
    verify(db).await?;
    Ok(report)
}

/// Không còn doc nào thiếu `tenantId` ở các collection của migration.
pub async fn verify(db: &Database) -> Result<(), String> {
    let mut left = Vec::new();
    for coll in TAGGED.iter().chain(DROPPED.iter()) {
        let n = db
            .collection::<Document>(coll)
            .count_documents(missing())
            .await
            .map_err(|e| format!("đếm lại {coll}: {e}"))?;
        if n > 0 {
            left.push(format!("{coll}: {n}"));
        }
    }
    if !left.is_empty() {
        return Err(format!("còn doc chưa có tenantId sau migration ({})", left.join(", ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo trống riêng cho test (migration đụng mọi doc của các collection kể trên):
    /// `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test tenant_migration -- --ignored`. Dùng database
    /// riêng `<DB_NAME>_migrate` để không gắn tenant cho dữ liệu của test khác đang chạy.
    #[tokio::test]
    #[ignore]
    async fn tenant_migration_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &format!("{db_name}_migrate")).await.expect("kết nối Mongo");
        for coll in TAGGED.iter().chain(DROPPED.iter()) {
            db.collection::<Document>(coll).delete_many(doc! {}).await.expect("dọn");
        }
        db.collection::<Document>("users")
            .insert_many([doc! { "idpSub": "acc_old", "email": "a@x.vn" }, doc! { "tenantId": "ten_a", "idpSub": "acc_old" }])
            .await
            .expect("users");
        db.collection::<Document>("transcripts").insert_one(doc! { "userId": "u1", "status": "done" }).await.expect("transcripts");
        db.collection::<Document>("jobs").insert_one(doc! { "userId": "u1", "status": "queued" }).await.expect("jobs");
        db.collection::<Document>("sessions").insert_one(doc! { "_id": "s1", "userId": "u1" }).await.expect("sessions");
        db.collection::<Document>("devices").insert_one(doc! { "_id": "d1", "userId": "u1" }).await.expect("devices");
        db.collection::<Document>("auth_states").insert_one(doc! { "_id": "st1" }).await.expect("auth_states");
        db.collection::<Document>("logout_jtis")
            .insert_many([doc! { "_id": "j1" }, doc! { "_id": "primary:j2", "tenantId": "primary" }])
            .await
            .expect("jtis");

        let first = run(&db).await.expect("lần đầu");
        let get = |list: &[(&str, u64)], coll: &str| list.iter().find(|(c, _)| *c == coll).map(|(_, n)| *n);
        assert_eq!(get(&first.tagged, "users"), Some(1), "chỉ user chưa có tenantId");
        assert_eq!((get(&first.tagged, "transcripts"), get(&first.tagged, "jobs")), (Some(1), Some(1)));
        assert_eq!((get(&first.tagged, "sessions"), get(&first.tagged, "devices")), (Some(1), Some(1)));
        assert_eq!((get(&first.dropped, "auth_states"), get(&first.dropped, "logout_jtis")), (Some(1), Some(1)));
        let users = db.collection::<Document>("users");
        assert_eq!(users.count_documents(doc! { "tenantId": PRIMARY_ID, "idpSub": "acc_old" }).await.expect("đếm"), 1);
        assert_eq!(users.count_documents(doc! { "tenantId": "ten_a" }).await.expect("đếm"), 1, "tenant khác giữ nguyên");
        assert_eq!(db.collection::<Document>("logout_jtis").count_documents(doc! {}).await.expect("đếm"), 1);

        let second = run(&db).await.expect("lần hai");
        assert_eq!(second.changed(), 0, "chạy lại không đổi gì");
        // Doc thiếu tenantId xuất hiện (ghi lỗi giữa chừng): kiểm báo lỗi, khởi động dừng.
        db.collection::<Document>("jobs").insert_one(doc! { "userId": "u2" }).await.expect("job sót");
        assert!(verify(&db).await.expect_err("còn sót").contains("jobs: 1"));
        db.drop().await.expect("xoá database test");
    }
}
