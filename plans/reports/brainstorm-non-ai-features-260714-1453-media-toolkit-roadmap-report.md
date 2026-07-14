# Brainstorm — Tính năng non-AI làm phong phú OneScribe

- Ngày: 2026-07-14
- Chế độ: brainstorm (không flag --html/--wiki)
- Trạng thái: ĐÃ CHỐT hướng + sequence + scope Wave 1
- Bối cảnh: OneScribe Web = ghi + xử lý media, **cố tình KHÔNG AI** (box .65 không GPU; transcript là P2 chờ hạ tầng)

## Vấn đề (problem-first)

User hỏi "thêm tính năng gì (non-AI) cho phong phú". Vấn đề thật bên dưới: **ghi/upload xong rồi dùng file làm gì?** Bản ghi hiện xuất `.webm` — khó dùng/chia sẻ trên đa số thiết bị & phần mềm. Đó là lỗ hổng lớn nhất, không phải thiếu tính năng lạ. Ưu tiên: biến file thành thứ **dùng được + chia sẻ được**, tái dùng ffmpeg/core-split (rẻ, đã có hạ tầng job/quota).

## Năng lực sẵn có (scout)

- Stack: Vue 3 + Vuetify/ViUI · Rust axum + MongoDB · ffmpeg qua crate `core-split` (`split_file`, `remux_copy_to_mkv`, `probe_file`, `size_cut_times`).
- Job queue + worker (`jobs.rs` + `worker/split_worker.rs`) + tải zip; output kind=`output`; quota 5GB; retention 14 ngày.
- Đang có: Ghi (mic/system/màn hình), Cắt file (phút/MB/số phần), lịch sử/phát lại, tải (từng/ngày/tất cả), quản lý tệp non-recording.
- Auth Google; deploy staging .65 sau Cloudflare (origin-bypass được chấp nhận → endpoint public cần phòng abuse thêm).

## Các hướng đã cân nhắc

| Hướng | Nội dung | Ưu | Nhược |
|---|---|---|---|
| A. Media toolkit | Convert/export, trim theo mốc, tách audio, ghép, nén | ROI cao, tái dùng ffmpeg/job, lấp lỗ hổng webm | — |
| B. Chia sẻ | Link token + hết hạn, không cần login | Khác biệt hoá, biến thành tool cộng tác | Bề mặt bảo mật/abuse cao |
| C. Tổ chức | Đổi tên/tag/ghi chú, tìm kiếm/lọc/sắp xếp | Rẻ, tăng giữ chân | Giá trị tăng dần theo số bản ghi |
| D. Trải nghiệm ghi | Pause/resume, chọn mic, auto-stop, chapters, waveform | Rẻ, parity desktop | Waveform nặng |

**Tránh (bẫy AI/GPU):** noise-removal ML, tách giọng, tóm tắt, nhận diện người nói. (`loudnorm`/`afftdn` của ffmpeg là non-AI, khả thi, nhưng đừng kỳ vọng như AI.)

## Quyết định chốt — Lộ trình cuốn chiếu 1→2→3→4

### Wave 1 — Media toolkit (LÀM TRƯỚC) · scope đã chốt: **Convert + Tách audio + Trim**
- **Convert/Export**: recording/upload → MP3 / M4A / MP4 / WAV. *[M]*
- **Tách audio từ video**: screen recording (video) → mp3 (biến thể convert `-vn`). *[S]*
- **Trim theo mốc in/out**: cắt 1 đoạn cụ thể (khác "Cắt file" chia đều). *[M]*
- *(ngoài scope lứa đầu, để lứa A sau)*: Ghép file, Nén bitrate.

### Wave 2 — Tổ chức & quản lý
- Đổi tên bản ghi *[S]* · Tag/ghi chú *[S-M]* · Tìm kiếm/lọc/sắp xếp lịch sử *[S]*.
- Làm trước Wave 4: link chia sẻ đẹp hơn khi file có tên/tag tử tế.

### Wave 3 — Trải nghiệm ghi
- Pause/Resume *[M]* (cẩn thận chuỗi upload chunk/offset) · Chọn mic *[S]* · Auto-stop hẹn giờ *[S]* · Chapters/markers *[M]* · Waveform khi phát *[M]* (có thể hoãn).

### Wave 4 — Chia sẻ (LÀM CUỐI)
- Link chia sẻ token + hết hạn + revoke (± mật khẩu) *[L]*. Route public validate token, thêm rate-limit/phòng abuse (do origin-bypass .65).

## Wave 1 — chi tiết để bàn giao /ck:plan

**Expected output:** từ 1 bản ghi/upload, tạo file mới (MP3/M4A/MP4/WAV, hoặc audio tách ra, hoặc đoạn trim) → tải trực tiếp; file mới đếm quota + hiện ở "Tệp trên hệ thống".

**Acceptance criteria:**
- Recording webm → export MP3 tải được, phát đúng, đúng thời lượng.
- Screen recording (video) → tách audio ra MP3.
- Trim in=00:10 out=00:40 → file dài ~30s, đúng nội dung đoạn.
- Job hiển thị tiến độ (queued/running/done/failed) như split; lỗi ffmpeg → failed có thông báo.
- Ownership + quota + size cap + retention áp như split.

**Scope boundary (OUT):** ghép file, nén, mọi thứ Wave 2/3/4.

**Ràng buộc:** tái dùng `core-split` + job queue + worker; MongoDB; KHÔNG AI/GPU; giữ public contract split hiện có (`/api/jobs*` không phá vỡ).

**Touchpoints (dự kiến):**
- `crates/core-split/src/` — thêm `convert_file` / `extract_audio` / `trim_file` (+ test ffmpeg thật, khớp pattern `tests/split_file_ffmpeg.rs`).
- `server/onescribe-be/src/jobs.rs` + `models/job.rs` + `worker/split_worker.rs` — tổng quát job có `op` (split | convert | trim | extract) + params; output single-file → tải qua `/api/files/:id?download=1` đã có.
- `server/onescribe-be/src/files.rs` — (nếu cần) không đổi; output đã liệt kê ở `manage_list`.
- FE: `web/src/composables/use-splitter.ts` (tổng quát hoá thành media-job) hoặc composable mới; UI trong `SplitView.vue` / thêm hành động ở `RecordingHistory.vue` ("Xuất MP3/MP4", "Trim").

**Rủi ro / quyết định cho /ck:plan:**
- Schema job: thêm field `op` (mặc định "split" cho backward-compat) vs endpoint mới. Khuyến nghị: field `op` + params (DRY, tái dùng worker/poll).
- Trim video: `-c copy` (nhanh, cắt theo keyframe → lệch mốc) vs re-encode (chính xác, chậm). Khuyến nghị: audio re-encode (rẻ); video mặc định re-encode chính xác, cân nhắc copy cho file lớn.
- Convert MP4 từ webm: re-encode video (tốn CPU trên .65 không GPU) — cảnh báo thời lượng job dài với video lớn; có thể giới hạn/cảnh báo.

## Success metrics
- Ghi xong → xuất được MP3/MP4 dùng ngoài app (giá trị cốt lõi mới).
- Tỉ lệ job convert/trim done không lỗi; thời lượng job chấp nhận được với file audio (video cảnh báo).

## Next steps
1. `/ck:plan` cho Wave 1 (Convert + Tách audio + Trim) — dùng report này làm input.
2. Sau Wave 1: brainstorm/plan riêng cho Wave 2, rồi 3, rồi 4.

## Unresolved questions
- Convert video sang MP4 trên .65 (CPU-only) có thể chậm với file lớn — cần chốt giới hạn kích thước/thời lượng hoặc chỉ cho audio-only ở Wave 1? (đề xuất: Wave 1 tập trung audio convert + tách audio + trim; MP4 re-encode để cân nhắc khi có số liệu CPU thực tế.)
