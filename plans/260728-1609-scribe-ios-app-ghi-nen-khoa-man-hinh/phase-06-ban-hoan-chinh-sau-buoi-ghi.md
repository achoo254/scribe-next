---
phase: 6
title: "Chép lời bản ghi bằng OpenAI (key của server)"
status: completed
priority: P1
effort: "L"
dependencies: [2]
---

# Phase 6: Chép lời bản ghi bằng OpenAI

## Overview

Người dùng bấm "Chép lời" trên một bản ghi (hoặc cả một phiên ghi iPhone) trong Lịch sử ghi. Backend chép bằng `gpt-4o-mini-transcribe`, cắt mảnh ≤45 giây, rồi hiện chữ ngay dưới bản ghi; bấm câu để tua tới đoạn đó, tải txt/srt.

Logic chuyển từ bản Python đã kiểm chứng `sidecar/transcriber/openai_transcriber.py` (khớp TurboScribe 83,9–84,4%): cắt tại khoảng lặng, nén Opus, gọi song song 4 mảnh, mảnh lặp thì gọi lại với temperature cao hơn. Khác bản desktop: tìm khoảng lặng bằng ffmpeg `silencedetect` thay silero (không cài Python lên server); mốc theo câu, không theo từ.

Phase này không còn phụ thuộc Phase 5: làm trước được, không cần iPhone để kiểm phần lớn.

> **Cập nhật 2026-10-16:** chép lời dùng **key OpenAI của server**, tính vào phút của từng tài khoản (commit `a90a7e8`), mở cho mọi người từ 16/10 14:19 với hạn mức gói free (`docs/project-roadmap.md`). T1, T3, T4 dưới đây không còn hiệu lực; `SecretsPlugin.swift` chỉ còn xoá key người dùng tự nhập ở bản cũ khi đăng xuất. Đã chép lời trên iPhone thật 16/10.

## Quyết định (2026-10-15)

| # | Quyết định | Nguồn |
|---|---|---|
| T1 | **Key OpenAI do người dùng tự nhập** (như desktop). Thay R3 của Phase 5 (key chung trên backend) | Chủ sở hữu |
| T2 | Chỉ email `quocdat254@gmail.com` được chép lời: biến build `TRANSCRIBE_EMAILS`; rỗng = tắt tính năng | Chủ sở hữu |
| T3 | Key lưu trong **Keychain của iPhone**; web giữ trong `sessionStorage` của tab. Gửi kèm mỗi lần bấm "Chép lời" qua HTTPS; backend chỉ giữ trong RAM tới khi chép xong, không ghi DB, không ghi log. Backend khởi động lại giữa chừng thì job báo lỗi, người dùng bấm lại | Jev: 0,69 so với 0,31 cho lưu mã hoá trên server |
| T4 | Không có trần phút ở backend: tiền là của chính người dùng (trần ở Phase 5 cũ là để bảo vệ key chung) | Hệ quả của T1 |
| T5 | Chép **theo từng file bản ghi**; phiên iPhone (các đoạn `<sessionId>#<index>`) là nhiều job, giao diện ghép lại theo thứ tự với mốc cộng dồn | Đoạn 1 phút vốn đã vừa 1–2 mảnh; chép dần được khi đoạn còn đang tải lên |
| T6 | Kết quả ở collection riêng `transcripts` (`_id` = id của file), hàng đợi riêng (không dùng chung `jobs` của Cắt file) | Không làm phình `/api/files`; chép lời không chặn job cắt |

## API

- `POST /api/transcripts` `{fileIds: string[] (≤200), apiKey}` → xếp hàng các bản ghi của chính người dùng. 403 nếu email ngoài T2.
- `GET /api/transcripts?ids=a,b` (≤200 id, web tự chia lô) → trạng thái và nội dung từng file: `{status: queued|running|done|failed, progress, error?, sentences: [{start, end, text}]}`.
- `GET /api/transcripts/export?ids=a,b|session=<sessionId>&format=txt|srt&name=…` → một tệp tải về, ghép theo thứ tự (`session` = mọi đoạn của phiên iPhone, không giới hạn số đoạn), mốc cộng dồn theo `durationS`.
- `GET /api/me` thêm `canTranscribe`. `GET /api/files` thêm `transcript: {status}` cho mỗi bản ghi.
- Xoá file (tay hoặc retention) thì xoá luôn transcript.

## Related code files

- Create: `server/scribe-be/src/transcribe/` (`mod.rs` API + hàng đợi, `chunking.rs` cắt mảnh + phát hiện lặp, `openai.rs` gọi API, `export.rs` txt/srt)
- Modify: `server/scribe-be/src/{routes,state,config,main,auth,files,retention}.rs`, `models/`, `worker/mod.rs` (bỏ stub `TranscribeWorker`), `jobs.rs`, `Cargo.toml` (`reqwest` + `multipart`)
- Create: `web/src/features/transcript/*` (API, key, ghép phiên), `web/src/pages/record/transcript-panel.tsx`
- Modify: `web/src/pages/record/{recording-row,session-row}.tsx`, `features/files/files-api.ts`, `auth/auth-context.tsx`
- Create: `web/ios/App/App/SecretsPlugin.swift` (lưu/đọc/xoá key trong Keychain)

## Kết quả (2026-10-15)

- Chạy thật đoạn 10–15' qua `transcribe_path`: 8 mảnh, khớp TurboScribe 84,8–85,0%, lặp 0,4% (người nói lắp "hơn hơn hơn hơn"), 4,5–16 giây cho 5 phút. Câu dài hơn 120 ký tự (model hay bỏ dấu chấm cả mảnh) được tách tại dấu phẩy rồi tại khoảng trắng.
- Vòng đời hàng đợi chạy trên Mongo dev thật (test `transcript_queue_lifecycle_on_real_mongo`, tự dọn).
- Review: [báo cáo](../dattqh/reports/code-reviewer-261015-1906-openai-transcription-review.md). Đã sửa H1–H2 (key vào RAM trước khi xếp hàng; key của lượt mất được bỏ sau 60 giây khi hàng đợi rỗng), M1–M7, L1–L5. Giữ nguyên có chủ đích: L7 (key iPhone chỉ xoá khi đăng xuất), L8 (cắt bản ghi thì mất bản chép lời của bản gốc).
- Còn chờ: deploy prod kèm `TRANSCRIBE_EMAILS`, QA giao diện trên iPhone bằng tài khoản được phép.

## Success Criteria

- [ ] Chép 5 phút đoạn 10–15' buổi họp tham chiếu: khớp TurboScribe ≥ 83%, lặp 0%.
- [ ] Key sai hoặc hết credit: thông báo đúng, hàng đợi vẫn chạy file sau; key không xuất hiện trong log, audit hay Mongo.
- [ ] Tài khoản ngoài T2: không thấy nút, gọi API trực tiếp trả 403.
- [ ] Trên iPhone: nhập key một lần, mở lại app vẫn còn; chép một phiên nhiều đoạn, đọc ghép đúng thứ tự, bấm câu tua đúng đoạn.
- [ ] RAM `$DEPLOY_SERVICE` khi chép bản ghi 80 phút không tăng quá 150 MB so với lúc rảnh.
- [ ] `cargo clippy` sạch, `cargo test` pass (cắt mảnh, phát hiện lặp, srt); web lint/typecheck/test pass; `xcodebuild` 0 cảnh báo.
