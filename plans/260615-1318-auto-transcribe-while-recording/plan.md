---
title: "Tự động transcript khi ghi âm (Mode A)"
description: "Tùy chọn opt-in: vừa ghi âm vừa transcribe từng đoạn, hiển thị transcript inline"
status: completed
priority: P2
created: "2026-06-15"
tags: [tauri, vue, audio, whisper, transcribe, mode-a]
---

# Tự động transcript khi ghi âm

> **Đóng plan (2026-10-19):** đã giao trên app desktop; app desktop bỏ ngày 2026-10-16. Dạng sau của tính năng là chép
> trực tiếp trên iPhone và chép lời trên server.

## Overview

Thêm tùy chọn **opt-in** trong tab Ghi âm (Mode A): bật "Tự động transcript khi ghi" →
mỗi đoạn ghi vừa finalize sẽ được transcribe ngay (real-time), hiển thị transcript inline
dưới từng đoạn, đồng thời lưu `.txt` + `.srt` cạnh đoạn.

**Quyết định người dùng (2026-06-15)**:
1. Thời điểm = **vừa ghi vừa transcribe** (real-time per-segment).
2. Đầu ra = **lưu .txt + .srt cạnh đoạn + hiển thị inline**.

## Key insight — tái sử dụng hạ tầng sẵn có

- Mode A đã emit `segment_created` (đoạn mới xuất hiện) + `segments_final` (khi dừng).
  Với ffmpeg segment muxer: khi đoạn N+1 xuất hiện thì đoạn N đã finalize → an toàn transcribe.
- Mode C đã có sidecar Python giữ model "ấm", xử lý tuần tự, lưu `.txt`/`.srt` cạnh file gốc,
  event `transcribe_msg` (download/model_ready/progress/done/error).
- → Feature **thuần frontend**: điều phối ở RecordView, **không sửa Rust**.

## Architecture

```
RecordView ── watch(segments.length) ──► flushFinalized()
   │  (đoạn cuối còn ghi dở → bỏ qua; idle → enqueue hết)
   ├── useRecordTranscribe.enqueue(path) ──► invoke transcribe_files (1 file)
   │        │ (sidecar Mode C giữ ấm, xếp hàng tuần tự)
   │        ◄─ transcribe_msg {progress|done|error, file} → map theo path
   └── SegmentTranscriptList: hiển thị từng đoạn + status/progress/text/copy/locate
```

## Related code files

- Create: `src/composables/use-record-transcribe.ts` — model status/download + map transcript theo path + enqueue.
- Create: `src/components/SegmentTranscriptList.vue` — danh sách đoạn + transcript inline (presentational).
- Create: `src/design-system/ui/vi-switch/UiSwitch.vue` — wrapper toggle (+ export index.ts).
- Modify: `src/views/RecordView.vue` — toggle + model picker/status + điều phối enqueue + hiển thị.

## Implementation steps

1. UiSwitch wrapper (Ui*-only) + export barrel.
2. Composable: reuse `model_status`/`model_path`/`ensure_model`/`transcribe_files`; listen `transcribe_msg`
   → map `{status,pct,text,txtPath,srtPath,error}` theo path; `enqueue` dedup + gate enabled/model.
3. SegmentTranscriptList: render từng đoạn (số thứ tự, tên, status, progress, textarea readonly, copy, locate).
4. RecordView: toggle + size select + model status/download; `flushFinalized()` chạy khi
   segments tăng / idle / model_ready / enabled — enqueue đoạn đã finalize; `rt.reset()` khi start mới.

## Todo

- [x] UiSwitch wrapper + export
- [x] use-record-transcribe composable
- [x] SegmentTranscriptList component
- [x] Tích hợp RecordView (toggle + model + điều phối + hiển thị)
- [x] Typecheck `vue-tsc --noEmit` → 0 lỗi
- [x] code-reviewer rà soát → finalize-timing/dedup/reactivity/backfill ĐÚNG; áp dụng fix:
      H1 (cho thử lại đoạn lỗi transient), H2 (handler chỉ tra entry có sẵn, tránh rò chéo phiên),
      L2 (xóa error khi đổi size), M2 (statusLabel default)
- [ ] Test thủ công: ghi nhiều đoạn, transcript hiện dần; tải model lần đầu; dừng → đoạn cuối transcribe
- [ ] MCP design system `validate_brand_usage` trên RecordView.vue + SegmentTranscriptList.vue

## Success criteria

- Bật toggle + model sẵn sàng → mỗi đoạn ghi xong tự transcribe, transcript hiện inline.
- `.txt` + `.srt` lưu cạnh mỗi đoạn.
- Tắt toggle → ghi âm hoạt động bình thường như cũ (không regress).
- Dừng ghi → đoạn cuối (đang ghi dở lúc dừng) cũng được transcribe.

## Notes / limitations (v1)

- ~~Tab dùng `<component :is>` (không keep-alive) → chuyển tab reset state.~~ **ĐÃ FIX (2026-06-15)**:
  `App.vue` bọc `<keep-alive>` → giữ state mỗi tab; điều phối auto-transcript chạy nền cả khi đổi tab
  (watcher + listener không bị unmount). Kèm: lọc `transcribe_msg` theo quyền sở hữu file (Mode C `owned`,
  Mode A `find`) + chặn `download`/`model_ready` chéo tab + `onActivated→refreshModel` re-sync.
- Toggle + size **không persist** (mặc định off + medium mỗi phiên) — đồng nhất với Mode C hiện tại.
- Trùng nhẹ logic model status/download với `use-transcribe.ts` (layout 2 view khác nhau nên chưa gộp).
- **Edge hiếm còn lại** (chấp nhận v1): tải model ĐỒNG THỜI ở cả 2 tab → `model_ready`/lỗi-chung không kèm
  size có thể hiển thị nhầm tab; tự lành qua `onActivated→refreshModel`. Fix triệt để cần backend gắn
  field `size`/source vào event (follow-up). Mode C "Hủy" kill sidecar dùng chung → dừng cả job nền Mode A.

## Unresolved questions

- Có cần persist lựa chọn toggle/size vào settings không? (hiện theo Mode C: không persist)
- Có cần nút "Hủy transcript" riêng trong tab Ghi âm khi hàng đợi còn chạy sau khi dừng?
