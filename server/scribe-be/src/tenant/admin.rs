//! Lệnh quản trị theo tenant (gọi từ `crate::admin`):
//!
//! ```text
//! scribe-be admin tenants
//! scribe-be admin tenant-disable <id>
//! scribe-be admin tenant-enable <id>
//! scribe-be admin purge-tenant <id> [--apply]
//! ```
//!
//! `tenant-disable` ghi cờ trong `kv`: trong vòng 60 giây (cache của danh bạ tenant) mọi route cần đăng nhập của tenant
//! trả 403 `TENANT_DISABLED`, web hiện trang "chưa bật"; dữ liệu giữ nguyên. `purge-tenant` xoá dữ liệu Scribe của
//! mọi user của tenant (như người dùng tự xoá tài khoản, `account::purge_user`) rồi phiên, thiết bị, lượt đăng nhập
//! còn sót; không có `--apply` thì chỉ đếm. Tenant chính không bao giờ bị tắt hay xoá bằng các lệnh này.

use super::{set_disabled, PRIMARY_ID};
use crate::config::Config;
use crate::models::{session as sessions, user};
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::Database;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Collection danh tính ngắn hạn còn sót sau khi xoá từng user.
const LEFTOVERS: [&str; 3] = ["devices", "auth_states", "logout_jtis"];

fn read(e: mongodb::error::Error) -> String {
    format!("đọc Mongo: {e}")
}

async fn audit(db: &Database, action: &str, details: Document) {
    let mut event = doc! { "at": DateTime::now(), "action": action, "userId": Bson::Null, "ip": "admin-cli" };
    event.extend(details);
    if let Err(e) = db.collection::<Document>("audit_events").insert_one(event).await {
        tracing::warn!("ghi audit lỗi: {e}");
    }
}

fn tenant_arg(args: &[String]) -> Result<&str, String> {
    let id = args.first().map(String::as_str).filter(|id| !id.starts_with("--") && !id.is_empty());
    id.ok_or_else(|| "thiếu id tenant".to_string())
}

/// Số user và tổng byte tệp của mỗi tenant, kèm cờ tắt.
pub async fn list(db: &Database) -> Result<Value, String> {
    let mut tenant_of: BTreeMap<String, String> = BTreeMap::new();
    let mut users: BTreeMap<String, u64> = BTreeMap::new();
    let mut cur = db.collection::<Document>("users").find(doc! {}).projection(doc! { "_id": 1, "tenantId": 1 }).await.map_err(read)?;
    while let Some(d) = cur.next().await {
        let d = d.map_err(read)?;
        let tenant = d.get_str("tenantId").unwrap_or("(chưa gắn)").to_string();
        if let Ok(id) = d.get_object_id("_id") {
            tenant_of.insert(id.to_hex(), tenant.clone());
        }
        *users.entry(tenant).or_default() += 1;
    }
    let mut bytes: BTreeMap<String, i64> = BTreeMap::new();
    let pipeline = [doc! { "$group": { "_id": "$userId", "bytes": { "$sum": "$size" } } }];
    let mut cur = db.collection::<Document>("files").aggregate(pipeline).await.map_err(read)?;
    while let Some(d) = cur.next().await {
        let d = d.map_err(read)?;
        let owner = d.get_str("_id").ok().and_then(|u| tenant_of.get(u)).cloned().unwrap_or_else(|| "(không rõ)".into());
        let size = d.get_i64("bytes").or_else(|_| d.get_i32("bytes").map(i64::from)).unwrap_or(0);
        *bytes.entry(owner).or_default() += size;
    }
    let mut disabled = Vec::new();
    let mut cur = db
        .collection::<Document>("kv")
        .find(doc! { "_id": { "$regex": "^tenant\\.disabled:" } })
        .await
        .map_err(read)?;
    while let Some(d) = cur.next().await {
        if let Some(id) = d.map_err(read)?.get_str("_id").ok().and_then(|k| k.strip_prefix("tenant.disabled:")) {
            disabled.push(id.to_string());
        }
    }
    let ids: std::collections::BTreeSet<&String> = users.keys().chain(bytes.keys()).collect();
    let tenants: Vec<Value> = ids
        .into_iter()
        .map(|id| {
            json!({
                "tenantId": id,
                "users": users.get(id).copied().unwrap_or(0),
                "fileBytes": bytes.get(id).copied().unwrap_or(0),
                "disabled": disabled.contains(id),
            })
        })
        .collect();
    Ok(json!({ "tenants": tenants, "disabled": disabled }))
}

/// `tenant-disable <id>` / `tenant-enable <id>`.
pub async fn set_enabled(db: &Database, args: &[String], enabled: bool) -> Result<Value, String> {
    let id = tenant_arg(args)?;
    if id == PRIMARY_ID {
        return Err("không tắt/bật tenant chính bằng lệnh này".into());
    }
    let changed = set_disabled(db, id, !enabled).await?;
    let action = if enabled { "admin.tenant_enable" } else { "admin.tenant_disable" };
    audit(db, action, doc! { "tenant": id, "changed": changed }).await;
    Ok(json!({ "tenantId": id, "disabled": !enabled, "changed": changed }))
}

/// `purge-tenant <id> [--apply]`.
pub async fn purge(config: &Config, db: &Database, args: &[String]) -> Result<Value, String> {
    let id = tenant_arg(args)?;
    if id == PRIMARY_ID {
        return Err("không xoá dữ liệu tenant chính bằng lệnh này".into());
    }
    let apply = args.iter().any(|a| a == "--apply");
    let ids = user::ids_of_tenant(db, id).await.map_err(|e| format!("{e:?}"))?;
    let mut counts = serde_json::Map::new();
    counts.insert("users".into(), json!(ids.len()));
    let mut file_count = 0_u64;
    let mut file_bytes = 0_i64;
    for user_id in &ids {
        let pipeline = [
            doc! { "$match": { "userId": user_id } },
            doc! { "$group": { "_id": Bson::Null, "n": { "$sum": 1 }, "bytes": { "$sum": "$size" } } },
        ];
        let mut cur = db.collection::<Document>("files").aggregate(pipeline).await.map_err(read)?;
        if let Some(d) = cur.next().await {
            let d = d.map_err(read)?;
            file_count += d.get_i32("n").map(|n| n as u64).unwrap_or(0);
            file_bytes += d.get_i64("bytes").or_else(|_| d.get_i32("bytes").map(i64::from)).unwrap_or(0);
        }
    }
    counts.insert("files".into(), json!(file_count));
    counts.insert("fileBytes".into(), json!(file_bytes));
    for coll in ["sessions", "devices", "auth_states", "logout_jtis"] {
        let n = db.collection::<Document>(coll).count_documents(doc! { "tenantId": id }).await.map_err(read)?;
        counts.insert(coll.into(), json!(n));
    }
    if !apply {
        return Ok(json!({ "tenantId": id, "applied": false, "found": counts }));
    }
    let (mut files, mut bytes, mut docs, mut failed) = (0_u64, 0_u64, 0_u64, 0_u64);
    for user_id in &ids {
        // Id user luôn là ObjectId; doc lạ thì bỏ qua thay vì đụng tới đĩa.
        if ObjectId::parse_str(user_id).is_err() {
            continue;
        }
        let purged = crate::account::purge_user(db, &config.data_dir, user_id).await.map_err(|e| format!("{e:?}"))?;
        files += purged.files;
        bytes += purged.bytes;
        docs += purged.docs;
        failed += purged.failed;
    }
    let sessions_removed = sessions::delete_by_tenant(db, id).await.map_err(|e| format!("{e:?}"))?.len() as u64;
    let mut leftovers = sessions_removed;
    for coll in LEFTOVERS {
        leftovers += db.collection::<Document>(coll).delete_many(doc! { "tenantId": id }).await.map_err(read)?.deleted_count;
    }
    let details = doc! {
        "tenant": id,
        "users": ids.len() as i64,
        "files": files as i64,
        "bytes": bytes as i64,
        "docs": (docs + leftovers) as i64,
        "failed": failed as i64,
    };
    audit(db, "admin.purge_tenant", details).await;
    Ok(json!({
        "tenantId": id,
        "applied": true,
        "found": counts,
        "removed": { "users": ids.len(), "files": files, "bytes": bytes, "docs": docs + leftovers, "failedFiles": failed },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::file::{FileDoc, Owner};
    use crate::models::plan::Plan;

    #[test]
    fn tenant_argument_is_required() {
        assert_eq!(tenant_arg(&["ten_a".into()]), Ok("ten_a"));
        assert!(tenant_arg(&[]).is_err());
        assert!(tenant_arg(&["--apply".into()]).is_err());
    }

    /// `purge-tenant` chỉ xoá dữ liệu của tenant được chỉ định; tenant khác (cùng `sub`) giữ nguyên.
    /// `cargo test purge_tenant -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn purge_tenant_leaves_other_tenants_alone_on_real_mongo() {
        let _sweep = crate::db::WHOLE_DB_SWEEP.lock().await;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        user::ensure_indexes(&db).await.expect("index");
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let (ten_a, ten_b) = (format!("ten_a_{tag}"), format!("ten_b_{tag}"));
        let sub = format!("selftest-purge-{tag}");
        let email = format!("p-{tag}@x.vn");
        let (a, _) = user::find_or_link(&db, &ten_a, &sub, &email, None).await.expect("a");
        let (b, _) = user::find_or_link(&db, &ten_b, &sub, &email, None).await.expect("b");
        let mut config = Config::for_tests();
        let data_dir = std::env::temp_dir().join(format!("scribe-purge-tenant-{tag}"));
        config.data_dir = data_dir.to_string_lossy().into_owned();
        for (tenant, user_id) in [(&ten_a, &a), (&ten_b, &b)] {
            let dir = format!("{}/{user_id}/recordings", config.data_dir);
            tokio::fs::create_dir_all(&dir).await.expect("thư mục");
            let path = format!("{dir}/x.webm");
            tokio::fs::write(&path, b"audio").await.expect("tệp");
            let f = FileDoc::new_recording(Owner { user_id, plan: Plan::Free }, "x.webm", &path, 5, 1.0, false, ObjectId::new());
            db.collection::<FileDoc>("files").insert_one(f).await.expect("file");
            sessions::create(&db, tenant, user_id, sessions::Client::Web, None, None).await.expect("phiên");
        }

        let dry = purge(&config, &db, std::slice::from_ref(&ten_a)).await.expect("đếm");
        assert_eq!((dry["applied"].clone(), dry["found"]["users"].clone(), dry["found"]["files"].clone()), (json!(false), json!(1), json!(1)));
        assert_eq!(db.collection::<Document>("files").count_documents(doc! { "userId": &a }).await.expect("đếm"), 1, "chưa --apply thì không xoá");

        let done = purge(&config, &db, &[ten_a.clone(), "--apply".into()]).await.expect("xoá");
        assert_eq!(done["removed"]["files"], json!(1));
        assert!(user::ids_of_tenant(&db, &ten_a).await.expect("users").is_empty());
        assert_eq!(user::ids_of_tenant(&db, &ten_b).await.expect("users"), vec![b.clone()], "tenant B còn nguyên");
        assert_eq!(db.collection::<Document>("files").count_documents(doc! { "userId": &b }).await.expect("đếm"), 1);
        assert_eq!(db.collection::<Document>("sessions").count_documents(doc! { "tenantId": &ten_b }).await.expect("đếm"), 1);
        assert_eq!(db.collection::<Document>("sessions").count_documents(doc! { "tenantId": &ten_a }).await.expect("đếm"), 0);
        assert!(purge(&config, &db, &[PRIMARY_ID.into(), "--apply".into()]).await.is_err(), "không xoá tenant chính");

        crate::account::purge_user(&db, &config.data_dir, &b).await.expect("dọn B");
        sessions::delete_by_tenant(&db, &ten_b).await.expect("dọn phiên B");
        let _ = tokio::fs::remove_dir_all(&data_dir).await;
    }
}
