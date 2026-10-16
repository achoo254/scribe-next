---
phase: 6
title: "AI Biên bản / Tóm tắt sau họp (Claude)"
status: deferred
priority: P3
effort: "3-5d"
dependencies: [3, 5]
---

# Phase 6: AI Biên bản / Tóm tắt sau họp bằng Claude — POST-COMPLETION

> ⏸️ **DEFERRED — chỉ làm SAU khi Phase 0–5 đã triển khai hoàn thiện & ổn định.** Đây là tính năng tăng giá trị (value-add), KHÔNG nằm trong chuỗi build MVP. Không khởi động khi MVP chưa chạy ổn production.

## Overview

Sau cuộc họp, lấy **transcript đã ghép per-speaker** (text, đã có sẵn từ Phase 3 caption-as-record / Phase 5 retention) → gửi cho **Claude** sinh **biên bản có cấu trúc**: tóm tắt, quyết định, **action items kèm người phụ trách**, phân đoạn theo chủ đề, (tùy chọn) dịch. Đây là **hậu xử lý transcript** — KHÔNG phải ASR (xem ghi chú "Claude không transcribe audio").

## Requirements

- **Functional:** từ transcript text 1 cuộc họp → gọi Claude → trả biên bản (Markdown/`.docx`): tóm tắt ngắn, danh sách quyết định, action items (ai · việc · hạn), phân đoạn chủ đề, tùy chọn dịch sang ngôn ngữ khác.
- **Non-functional:**
  - **Opt-in + minh bạch tuyệt đối** — đây là điểm nhạy cảm nhất (xem Privacy).
  - Chạy **một lần sau họp** (batch, không realtime) → latency vài–vài chục giây chấp nhận được.

## Architecture

```
Transcript per-speaker (text, .txt/.srt từ Phase 3/5)
        │  (chỉ TEXT — không media)
        ▼
[Gọi Claude — Anthropic SDK]
   model: claude-opus-4-8 (chất lượng) | claude-haiku-4-5 (rẻ)
   adaptive thinking; structured outputs (output_config.format) cho action items
        ▼
Biên bản .md/.docx → lưu phía CLIENT (nhất quán "server không ôm dữ liệu nặng")
```

- **Nơi chạy (chốt khi làm phase này):**
  - **(a) Client/host gọi Claude trực tiếp** bằng **API key của chính khách hàng** → server hoàn toàn không dính, thân thiện privacy nhất. Ưu tiên.
  - (b) Service nhỏ phía server gọi Claude (dùng transcript text đang giữ 24h ở Phase 5) — tiện hơn nhưng server xử lý text qua Claude; chỉ làm nếu khách chấp nhận.
- **Model:** mặc định `claude-opus-4-8` (biên bản chất lượng cao); `claude-haiku-4-5` cho bản rẻ/nhanh. Dùng **structured outputs** để action items ra JSON chuẩn (ai/việc/hạn). Cân nhắc skill `docx` của Claude để xuất `.docx` đẹp.

## Related Code Files

- Create: module/endpoint "tạo biên bản" (vị trí tùy quyết định a/b) gọi Anthropic SDK.
- Create: UI "Tạo biên bản AI" trong app (Vue + design system) — nút sau khi họp kết thúc, chọn model + ngôn ngữ + có dịch không.
- Read for context: transcript output Phase 3 (caption-as-record), retention store Phase 5.
- Reference: skill `claude-api` (model id, adaptive thinking, structured outputs) — đọc trước khi code.

## Implementation Steps

1. Chốt nơi chạy (client trực tiếp vs service server) + cách khách cấp Claude API key/auth.
2. Thiết kế prompt biên bản tiếng Việt + schema structured output cho action items (ai · việc · hạn · trạng thái).
3. Gọi Claude (Anthropic SDK, `claude-opus-4-8`, adaptive thinking, streaming nếu output dài) trên transcript đã ghép.
4. Render biên bản (Markdown trong app + tùy chọn xuất `.docx`); cho sửa tay trước khi lưu.
5. UI opt-in rõ ràng + disclosure (xem Privacy) + chọn model/ngôn ngữ.
6. Test với transcript họp thật nhiều người; kiểm action items đúng người, không bịa.

## Success Criteria

- [ ] Từ transcript 1 cuộc họp tiếng Việt → ra biên bản có tóm tắt + quyết định + action items gắn đúng người.
- [ ] Tính năng **mặc định TẮT**, bật phải có disclosure rõ "transcript text được gửi tới Anthropic".
- [ ] Biên bản lưu phía client (hoặc theo quyết định a/b), server không ôm thêm dữ liệu nặng.
- [ ] Không khởi động phase này khi MVP (Phase 0–5) chưa chạy ổn định production.

## Risk Assessment

- **🔴 Privacy — mâu thuẫn USP.** USP sản phẩm là "dữ liệu không rời máy"; tính năng này **gửi transcript text lên cloud Anthropic**. BẮT BUỘC: opt-in mặc định tắt + disclosure rõ ràng + ưu tiên dùng API key của chính khách. Không bật ngầm.
- **Chi phí token** theo độ dài họp — hiển thị ước tính/cảnh báo; cho chọn Haiku để rẻ.
- **Claude bịa action item / sai người** — dùng structured output + cho người dùng sửa trước khi lưu; biên bản là "nháp AI", không phải sự thật tuyệt đối.
- **Phụ thuộc internet + auth** — tính năng online, khác hẳn core offline; phải tách rõ để không kéo theo phụ thuộc cho phần họp/transcribe.
