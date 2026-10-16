---
phase: 1
title: "Backend: key server, sổ phút, quota, trần, CLI admin"
status: completed
priority: P1
effort: "M"
dependencies: []
---

# Phase 1: Backend — key server, sổ phút, quota, trần, CLI admin

## Goal

Chép lời bằng key OpenAI của chủ sản phẩm; mỗi giây chép được giữ chỗ trong sổ phút trước khi chi, không bao giờ vượt hạn mức gói của user hay trần chi phí toàn cục.

## Files

- Modify `server/scribe-be/build.rs`, `src/config.rs` — biến build `OPENAI_KEY_PATH` (bắt buộc khi có `TRANSCRIBE_EMAILS`), `TRANSCRIBE_FREE_MINUTES` (mặc định 120), `TRANSCRIBE_PLUS_MINUTES` (mặc định 3000), `TRANSCRIBE_FREE_POOL_USD` (quỹ chung của user `free`, mặc định 10), `TRANSCRIBE_MONTHLY_CAP_USD` (trần tổng chặn mọi người, mặc định 30); giá $0,003/phút là hằng. Hằng mặc định ở một chỗ.
- Modify `src/main.rs` — nhánh `admin` tách **ngay sau** `Config::from_env` + `db::connect`, trước session key, `ensure_indexes`, worker, sweeper (không được chạy worker khi gọi CLI trên prod; chạy bằng `sudo -u $DEPLOY_USER`). Đọc key từ tệp lúc khởi động (thiếu/rỗng/không bắt đầu `sk-` ⟹ chép lời tắt, log một dòng không có giá trị key).
- Modify `src/state.rs`, `src/transcribe/mod.rs` — bỏ `PendingKeys`, `ApiKey`, `drop_stale`, trường `apiKey` của `POST /api/transcripts` (serde bỏ qua trường thừa nên app cũ vẫn gửi được); worker dùng key server; `fail_unfinished` đổi thành trả `running` về `queued` (giữ nguyên `chargedS`).
- Create `src/models/usage.rs` — collection `transcribe_usage`: `{_id: "u:<userId>:<YYYY-MM>" | "free:<YYYY-MM>" | "all:<YYYY-MM>", seconds, updatedAt}`; `reserve(...)` theo thứ tự user → quỹ free (chỉ user `free`) → tổng, hỏng bước nào hoàn các bước trước; `refund(...)`.
- Create `src/models/plan.rs` — gói `free | plus`, `effective_plan(user, now)` (hết hạn ⟹ free), hạn mức giây theo gói.
- Modify `src/models/user.rs` — trường `plan`, `planUntil`; `find_by_email`, `set_plan`.
- Modify `src/models/transcript.rs` — trường `chargedS` (giây đã giữ cho lượt hiện tại), `month`; `enqueue` ghi cả hai.
- Create `src/admin.rs` — `scribe-be admin set-plan <email> <free|plus> [--until YYYY-MM-DD]` (khớp 0 hoặc > 1 user ⟹ lỗi), `scribe-be admin usage [<email>] [--month YYYY-MM] [--recompute] [--apply]`; in JSON; ghi `audit_events` (`admin.set_plan`, `admin.usage_recompute`).
- Modify `src/auth.rs` (`/api/me`) — thêm `transcription: { plan, planUntil, usedMinutes, limitMinutes, resetsAt, capReached }` cho user được chép.
- Modify `src/error.rs` — `AppError::QuotaExceeded { message, used, limit }` → 403 `{ error, code: "QUOTA_EXCEEDED", usedMinutes, limitMinutes }`; `code: "CAP_REACHED"` cho trần toàn cục.

## Thiết kế giữ chỗ (atomic)

Mongo prod là standalone (đo 2026-10-16: `hello().setName = null`), không có transaction: mọi bất biến dựa trên thao tác nguyên tử một doc.

1. `need` = Σ `ceil(durationS)` của các file trong yêu cầu (`durationS` thiếu hoặc ≤ 0 ⟹ probe lại; vẫn không có ⟹ 400).
2. Đảm bảo doc tháng tồn tại: `update_one(_id, {$setOnInsert:{seconds:0}}, upsert)`; lỗi trùng khoá khi đua ⟹ bỏ qua.
3. Giữ phía user: `update_one({_id:"u:…", seconds:{$lte: limit-need}}, {$inc:{seconds:need}})`; matched 0 ⟹ `QuotaExceeded`.
4. Giữ quỹ free (chỉ khi gói hiệu lực là `free`), rồi trần tổng, cùng cách; matched 0 ⟹ hoàn các bước trước, `CAP_REACHED`.
5. `enqueue` từng file với `chargedS` = phần của file, `month`, `sentS = 0`, `attempts = 0`, **chỉ khi doc chưa ở `queued/running`** (thêm `status ∉ {queued, running}` vào filter, upsert giữ nguyên; trùng khoá ⟹ đang chạy). Không xếp được ⟹ hoàn phần của file đó và trả 409 cho file đó. Doc cũ đang `done/failed` còn `chargedS > 0` thì hoàn trước khi ghi đè (bằng bước 6).
6. Hoàn phút (OpenAI tính theo audio đã gửi, kể cả lần thử lại và khi lượt hỏng giữa chừng): worker `$inc sentS` **trước** mỗi lần gửi một mảnh, kể cả lần thử lại. Kết thúc lượt không thành công dùng **một** lệnh `find_one_and_update({_id, runId, chargedS: {$gt: 0}}, {$set: {status: failed, chargedS: 0, …}})` với `ReturnDocument::Before`, rồi `$inc` âm `max(0, chargedS − sentS)` vào đúng `month` của doc trả về — gắn `runId` nên hoàn muộn của lượt cũ không chạm lượt mới, và chỉ một lệnh thắng. Xoá transcript/file khi chưa xong: `find_one_and_delete` rồi hoàn `max(0, chargedS − sentS − PARALLEL × CHUNK_MAX)` (bảo thủ: các mảnh đang bay vẫn bị tính). Lượt `done` không hoàn (phần im lặng coi như phí giữ chỗ).
7. Giây gửi đi không vượt phần đã giữ: worker probe thời lượng thật; `ceil(probe) > chargedS` ⟹ giữ thêm phần chênh (user + quỹ + tổng), không giữ được thì `failed` + hoàn.
8. Restart: `running → queued` và `attempts += 1`; `attempts ≥ 2` ⟹ `failed` + hoàn, chặn vòng crash chi tiền mãi.
9. Key server hỏng (401 / hết credit): cờ ngắt trong RAM dừng nhận lượt mới, lượt hiện tại `failed` + hoàn, thông báo người dùng "Chép lời tạm ngưng, đã báo quản trị", audit `transcript.key_invalid` một lần; khởi động lại để thử key mới.
10. Trần: mức 80% và 100% (quỹ free và tổng) ghi `audit_events` (`transcript.cap_80`, `transcript.cap_reached`) một lần mỗi tháng.
11. Đối soát: `scribe-be admin usage --recompute <YYYY-MM>` tính lại sổ từ `transcripts` (Σ `chargedS` của lượt còn giữ + `done`) và in chênh lệch; `--apply` mới ghi.

## Tasks

1. Config + đọc key + tắt chép lời an toàn khi thiếu key; test `config` mặc định.
2. `usage.rs` + `plan.rs` + test thuần (`month_key` theo Asia/Ho_Chi_Minh ở biên 00:00 ngày 1; `effective_plan` hết hạn).
3. Nối giữ chỗ vào `create`; hoàn ở worker (`record` failed, `CANCELLED`), `docs::delete`, `file::delete_doc`, xếp lại.
4. Worker không còn cần key RAM; khởi động đưa `running` → `queued`.
5. `admin` subcommand + audit; `/api/me`; lỗi có mã.
6. Docs comment đầu module `transcribe/mod.rs` cập nhật.

## Verification

- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Test Mongo thật (`#[ignore]`, DB dev): 20 task giữ chỗ song song cho cùng user chỉ nhận đúng số vừa hạn mức; quỹ free chặn user free nhưng không chặn plus; trần tổng chặn cả hai; hoàn không hai lần; `failed` → chép lại → hoàn muộn của lượt cũ không chạm lượt mới; hai POST đồng thời cùng file chỉ một lượt được xếp, lượt kia được hoàn; restart hai lần ⟹ `failed` và hoàn đúng; `admin usage` không đổi doc `transcripts`.
- Dev local: chép một phiên thật bằng key thử của chủ sản phẩm (đặt ở tệp trong `DATA_DIR` dev, 600), kiểm sổ phút khớp `Σ durationS`.

## Risks

- Cách OpenAI tính tiền khi lượt hỏng giữa chừng: dùng ước lượng bảo thủ (tính phần mảnh đã gửi); đối chiếu bảng usage OpenAI sau một tuần.
- Tháng theo giờ Việt Nam: dùng `chrono` + offset cố định +07:00 (không DST), không cần crate múi giờ.
