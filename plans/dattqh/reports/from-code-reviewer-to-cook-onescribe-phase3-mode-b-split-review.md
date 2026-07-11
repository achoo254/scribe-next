# Review Phase 3 — Mode B cắt file (OneScribe Web BE + FE)

Phạm vi: `server/onescribe-be/src/{upload.rs,jobs.rs,worker/*,models/{file,job}.rs}` + edits `{config.rs,error.rs,routes.rs,main.rs,Cargo.toml}`; `web/src/{views/SplitView.vue,composables/use-upload.ts,composables/use-splitter.ts}` + edits `{router.ts,HomeView.vue}`; `crates/core-split/src/*` (dùng bởi SplitWorker). Đối chiếu `phase-03-mode-b-c-t-file.md`.

## CRITICAL

### C1 — Quota/disk-exhaustion bypass qua session upload không finalize (không cần concurrency)
`upload.rs:34-67` (`init`) không giới hạn số session mở/user; `upload.rs:80-115` (`chunk`) chỉ so `usage + written(session hiện tại) > QUOTA_BYTES`, với `usage = file::user_usage_bytes()` (`file.rs:100-119`) **chỉ SUM `files` collection (đã finalize)**, KHÔNG cộng byte của các session `.part` khác (đang mở hoặc bị bỏ dở). Không có sweep/TTL nào dọn session/`.part` mồ côi (grep xác nhận không có `sweep`/`orphan`/session-count limit trong `src/`).

→ Một tài khoản (allowlist đang mở cho mọi Google account — `phase-05...md:29`) chỉ cần lặp tuần tự: `init` → `chunk` tới gần 2GB (`MAX_FILE_SIZE_BYTES`) → **không finalize** → lặp lại. Mỗi vòng lặp tốn thật ~2GB disk trên box dùng chung ("box .65"), và **không bao giờ được tính vào quota 5GB** vì `user_usage_bytes` không thấy session chưa finalize. Không cần nhiều request đồng thời — chạy tuần tự cũng đủ lấp đầy disk vô hạn. Đây chính là phần đã được plan tự flag rồi bỏ sót khi code: `phase-03...md:31` "Giới hạn số session mở đồng thời/user; sweep dọn `.part` mồ côi" — KHÔNG có trong code.

Fix đề xuất (cần trước khi bật public upload production):
1. Cộng dồn byte các session `.part` đang mở (query `upload_sessions` + `fs::metadata` mỗi cái, hoặc lưu `writtenBytes` cached trong doc session) vào usage check ở `chunk()`/`init()`.
2. Giới hạn cứng số session mở/user (vd 3-5) ở `init()` — đếm `upload_sessions{userId}` trước khi insert mới.
3. Tối thiểu 1 sweep định kỳ (không cần đợi đủ Phase 5 retention.rs) xoá session+`.part` quá X phút không hoạt động (`createdAt`/`updatedAt` cũ) — kể cả cron đơn giản `tokio::spawn` interval trong `main.rs`.

**Khuyến nghị: không bật public route `/api/upload/*` trên production tới khi tối thiểu #2 hoặc #3 có mặt.**

## HIGH

### H1 — ffmpeg không timeout → "poison pill" job kẹt queue vĩnh viễn, sống sót qua restart
`crates/core-split/src/split.rs:107-148` (`run_ffmpeg`): `child.wait().await` không có `tokio::time::timeout`. Plan tự yêu cầu điều này: `phase-03...md:41` "[Red-team #15] ... **timeout job** (kill ffmpeg treo)" — liệt kê như mitigation của Phase 3 (không đánh dấu deferred sang Phase 5 như allowlist), nhưng KHÔNG có trong code (grep `timeout` trong `crates/core-split/src` và `server/onescribe-be/src` đều rỗng).

Vì `concurrency=1` (queue loop `jobs.rs:119-132` gọi `run_job(...).await` trực tiếp, tuần tự), một file input ác ý/hỏng khiến ffmpeg treo (chờ input/demux vòng lặp) sẽ giữ job ở `running` **mãi mãi**, chặn **toàn bộ hàng đợi cho MỌI user**. Tệ hơn: `models/job.rs:127-138` (`reset_running_to_queued`, gọi lúc BE start `main.rs:60`) sẽ set lại job đó `queued` sau restart → worker claim lại → ffmpeg treo lại → kẹt lại. Không có cơ chế đánh dấu "đã thử N lần → fail hẳn", nên **restart BE không giải quyết được poison-pill này** — cần can thiệp thủ công vào Mongo để tìm & fail job.

Fix: bọc `child.wait()` bằng `tokio::time::timeout(Duration::from_secs(...), ...)`, hết giờ thì `child.kill()` + trả lỗi rõ ("ffmpeg timeout"); cân nhắc thêm field `attempts`/`maxAttempts` trên `Job` để job fail sau N lần retry thay vì requeue vô hạn.

### H2 — Worker loop không có panic isolation (crash toàn bộ queue nếu 1 job panic)
`jobs.rs:119-132` (`start_worker`): `run_job(&state, &j).await` được `.await` trực tiếp trong loop của MỘT `tokio::spawn` duy nhất. Hiện tại chưa tìm thấy đường panic cụ thể nào trong `core-split`/`split_worker.rs` (code đi theo `Result`, không thấy `.unwrap()`/`panic!` nguy hiểm), nhưng thiết kế **không có lớp bảo vệ nào** (`catch_unwind`, hoặc spawn riêng mỗi job rồi bắt `JoinError`): nếu BẤT KỲ panic nào xảy ra (kể cả do dependency tương lai, edge case chưa lường), toàn bộ task loop chết lặng lẽ — **không log rõ ràng, không tự phục hồi** — mọi job sau đó kẹt `queued` vĩnh viễn tới khi ai đó phát hiện worker ngừng chạy và restart thủ công.

Fix: `tokio::spawn` riêng cho mỗi `run_job` rồi `.await` `JoinHandle` — panic khi đó biến thành `Err(JoinError)` bắt được, log + `finish_failed`, loop tiếp tục thay vì chết.

## MEDIUM

### M1 — FE `pollOnce` ngừng poll vĩnh viễn khi gặp lỗi mạng/5xx tạm thời
`web/src/composables/use-splitter.ts:47-69`: nếu `fetch` reject (mất mạng) hoặc `res.ok` false (BE 5xx thoáng qua), catch block set `error.value` nhưng KHÔNG gọi lại `pollTimer = setTimeout(...)` — polling dừng hẳn dù job vẫn có thể đang chạy server-side. User phải reload trang. Nên phân biệt lỗi tạm thời (network/5xx → retry có backoff) và lỗi cuối (4xx ownership/not-found → dừng hẳn).

### M2 — `chunk()` chấp nhận offset tuỳ ý (không kiểm liền mạch) — đã verify KHÔNG bypass quota nhưng nên siết
`upload.rs:80-115`: `q.offset` client tự khai, không so với độ dài `.part` hiện có → có thể tạo file sparse (vd offset=2GB-1, ghi 1 byte). Đã trace kỹ: **không phải lỗ hổng bypass quota** — `metadata().len()` trả về apparent size (bao gồm hole), tức quota-check LUÔN tính dư (an toàn hơn thực tế), không tính thiếu; và finalize's magic-byte check (đọc header offset 0) sẽ chặn file có header rỗng do bị "nhảy cóc" ngay từ chunk đầu. Rủi ro thực tế: lãng phí round-trip / file hỏng tự làm khó chính user đó, không ảnh hưởng user khác. Đề xuất (không khẩn): validate `q.offset == metadata(&part).len()` hiện tại trước khi seek+write để buộc append tuần tự, đơn giản hoá lý luận và loại bỏ hẳn edge case sparse.

### M3 — Thiếu index cho truy vấn nóng
Không thấy `create_index` cho `files.userId` (dùng bởi `user_usage_bytes` aggregate mỗi lần chunk — `file.rs:100`) hay `jobs.status`/`createdAt` (dùng bởi `claim_next` mỗi giây khi queue rỗng — `job.rs:90-99`). Ở quy mô hiện tại (nội bộ) chưa ảnh hưởng, nhưng nên thêm trước khi traffic tăng — cùng chỗ với `ensure_indexes` đã có cho `users`.

## LOW

- `upload.rs:101` — `f.flush().await.ok()` nuốt lỗi flush im lặng. Nên propagate lỗi (dù rủi ro thấp vì `tokio::fs::File` chạy qua blocking task).
- `upload.rs:24` (`InitReq.name`) không giới hạn độ dài — client có thể gửi field `name` tới ~8MB (chỉ bị chặn bởi `DefaultBodyLimit` chung của router, không có validate riêng), phình Mongo doc `upload_sessions`/`files`. Thêm cap hợp lý (vd 255 ký tự).
- `jobs.rs:83-88` (`download`) — 2 request tải zip đồng thời của CÙNG 1 job/user ghi đè cùng `zip_path` qua `spawn_blocking` không khoá — chỉ ảnh hưởng dữ liệu của chính user đó (race vô hại), không phải lỗ hổng cross-user.

## Đối chiếu red-team / Success Criteria đã honor đúng (xác nhận qua code, không chỉ đọc plan)

- **Path traversal (#8):** `part_path`/`load_session` (`upload.rs:209-232`) validate `Uuid::parse_str` TRƯỚC khi dựng path, cả 2 hàm — đúng. `userId` dùng dựng path đến từ session cookie đã verify (`auth::UserId`), không phải input client — đúng.
- **Ownership/IDOR:** mọi endpoint upload/jobs/download đều `load_session`/`get_owned` filter theo `userId` (`upload.rs:86,128,153`; `jobs.rs:39-41,53,72,78`) — verify chéo double-check ở `list_outputs_for_job` (lọc cả `jobId` lẫn `userId`) — tốt, defense-in-depth.
- **Append disk không buffer RAM:** `chunk()` dùng `seek`+`write_all` trên `Bytes` body, không giữ `Vec` toàn file — đúng yêu cầu. `DefaultBodyLimit::max(8MB)` (`routes.rs:22`) khớp chunk 5MB FE.
- **Job queue atomic + crash recovery thứ tự đúng:** `claim_next` dùng `find_one_and_update` atomic (`job.rs:90-99`); `main.rs:60-65` gọi `reset_running_to_queued` **TRƯỚC** `start_worker` — đúng thứ tự tránh race khi vừa restart.
- **Magic-byte finalize:** `infer::get_from_path` check `Audio`/`Video` matcher, xoá `.part` nếu fail (`upload.rs:163-171`) — đúng. Extension file cuối lấy từ `infer` (không phải client), path cuối dùng `ObjectId` — không có cách nào client control filename/path → loại luôn zip-slip/path-traversal ở khâu output.
- **Data leak:** `AppError::Internal` redact chi tiết, chỉ log server-side qua `tracing::error!`, trả client "internal server error" (`error.rs:32-41`) — verify không có biến thể `BadRequest/NotFound/Forbidden` nào nội suy `{e}` lộ chi tiết (grep xác nhận).
- **Cookie:** `HttpOnly+Secure+SameSite=Lax` đúng (`auth.rs:88-96`).
- **FE:** không dùng `v-html`, không có XSS vector trong `SplitView.vue`; route guard `requiresAuth` nhất quán pattern cũ (`router.ts`) — chỉ là UX gate, enforcement thật ở BE cookie/middleware.

## Plan Success Criteria — trạng thái

- [x] Append disk, không buffer RAM — đạt.
- [x] Ownership `/api/jobs/:id`, download chặn cross-user — đạt.
- [x] Zip tải hoạt động; size cap + magic-byte chặn file rác — đạt.
- [x] Job queue concurrency=1 (đúng theo cấu trúc 1 loop task) — đạt.
- [ ] **"Giới hạn số session mở đồng thời/user; sweep dọn `.part` mồ côi"** (`phase-03...md:31`) — CHƯA làm → xem C1.
- [ ] **"timeout job (kill ffmpeg treo)"** (`phase-03...md:41`, red-team #15) — CHƯA làm → xem H1.
- [~] "Cắt phút/MB/parts đúng như desktop" — logic tái dùng nguyên `core-split` (bê từ desktop `splitter.rs` theo comment lib.rs), nhưng review này KHÔNG chạy integration test/so khớp output thực tế với desktop — cần `tester` verify riêng (ngoài scope static review).

## Unresolved questions

1. C1/H1 có được coi là "đủ điều kiện" để merge Phase 3 vào production ngay, hay phải chặn lại chờ ít nhất phần tối thiểu của Phase 5 (session-limit + sweep + ffmpeg timeout) trước khi bật route upload public? Đề xuất chặn.
2. Phase 5 (`quota.rs`, `retention.rs`) có kế hoạch cụ thể ngày nào chưa — để biết cửa sổ rủi ro C1 tồn tại bao lâu trên production.

**Status:** DONE_WITH_CONCERNS
**Summary:** Ownership/path-traversal/RAM-buffer/magic-byte/crash-recovery-order đều đúng theo red-team đã chốt. Phát hiện 1 lỗ hổng Critical (quota bypass qua session không finalize — không cần concurrency, tự sweep tuần tự cũng đủ lấp disk box dùng chung) và 2 High (ffmpeg không timeout → poison-pill kẹt queue toàn cục sống sót qua restart; worker loop không panic-isolated) đều là các mitigation plan đã tự liệt kê ở Phase 3 nhưng thiếu trong code — không phải suy diễn, đã grep xác nhận không tồn tại.
**Concerns/Blockers:** C1 và H1 nên được coi là blocking trước khi mở public upload trên production (box dùng chung, allowlist đang mở mọi tài khoản Google).
