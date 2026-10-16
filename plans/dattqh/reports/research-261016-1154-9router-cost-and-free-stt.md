# 9router cho Scribe: có giảm chi phí OpenAI và thêm tài nguyên free không?

Thời điểm nghiên cứu: 16/10/2026 11:54 (Asia/Saigon). Skill định tuyến: Jev chọn `ak-research` (0,99).

## Kết luận

**Không nên dùng 9router cho Scribe.** Nó không làm chép lời rẻ hơn và không thêm nguồn chép lời miễn phí nào.
Mục tiêu "không thêm chi phí, người dùng được dùng nhiều hơn" đạt được tốt hơn bằng cách gọi thẳng
**Groq whisper-large-v3-turbo (free tier)** làm tầng đầu, hết quota hoặc lỗi thì quay về OpenAI như hiện nay.

## Vì sao 9router không hợp

- Chi phí của Scribe là **speech-to-text**: backend gọi `gpt-4o-mini-transcribe` tại
  `server/scribe-be/src/transcribe/openai.rs:12` (khoảng 0,003 USD/phút). Phần lớn tầng "free" của 9router là credit
  chat/coding (Kiro ~50 credit/tháng, OpenCode Free có danh sách thay đổi, Vertex 300 USD dùng thử 90 ngày), không
  dùng được để chép audio.
- Chép lời trong 9router là provider "Self-hosted STT": trỏ tới whisper.cpp/faster-whisper do mình tự chạy. Máy .65
  dùng chung, còn khoảng 3 GB RAM trống và không có GPU, nên không chạy whisper ở đó được.
- 9router viết cho **một lập trình viên dùng công cụ của chính họ**, với OAuth gói cá nhân. Dùng gói cá nhân đó để
  phục vụ người dùng cuối có rủi ro vi phạm điều khoản của provider.
- Nó thêm một dịch vụ nữa trên máy dùng chung. Lợi ích duy nhất là đổi provider qua cấu hình, mà backend Rust làm
  việc đó trực tiếp được với ít bộ phận hơn.

## Phương án đề xuất: Groq free làm tầng đầu

- API tương thích OpenAI `/v1/audio/transcriptions`, nên chỉ cần đổi URL, key và tên model trong `openai.rs`.
- Free tier: khoảng 28.800 giây audio/ngày (~8 giờ), 2.000 request/ngày, file tối đa 25 MB. Quota **tính theo tổ
  chức**, tức là mọi user dùng chung 8 giờ/ngày đó. Gói trả phí ~0,04 USD/giờ, rẻ hơn OpenAI (~0,18 USD/giờ).
- Điều kiện trước khi làm: **đo chất lượng tiếng Việt** của whisper-large-v3-turbo so với `gpt-4o-mini-transcribe`
  trên vài bản ghi thật. Nếu kém rõ rệt thì không đáng.
- Cần giữ cách đo phút hiện có (`transcribe_usage`), ghi thêm provider vào `transcribe_runs`, và chia file trên
  25 MB (hoặc chuyển thẳng file lớn sang OpenAI).
- Đường lùi: một cờ cấu hình tắt tầng Groq, quay về OpenAI.

| Phương án | Jev | Ghi chú |
|---|---|---|
| A. Giữ nguyên OpenAI | 0,00 | Không rủi ro, không thêm lợi ích |
| **B. Groq free + fallback OpenAI** | **0,98** | Đo tiếng Việt trước |
| C. BYOK (người dùng tự mang key) | 0,02 | Có thể làm sau B; phải lưu key người dùng an toàn |
| D. Dựng 9router làm proxy | 0,00 | Không có STT free, thêm rủi ro |

## Việc tiếp theo

1. Chạy thử 5–10 bản ghi tiếng Việt thật qua Groq và so với transcript OpenAI đang có (chạy ở máy dev, không đụng prod).
2. Nếu đạt: lên plan cho tầng provider (Groq trước, OpenAI sau) trong `transcribe/`, có cờ bật/tắt.

## Câu hỏi còn mở

- Mức chất lượng tiếng Việt nào là chấp nhận được (theo cảm nhận người đọc hay đo WER)?
- 8 giờ/ngày dùng chung có đủ với lượng chép lời hiện tại không? Cần đọc `transcribe_usage` trên prod để biết.

## Nguồn

- 9router README: https://github.com/decolua/9router
- 9router STT skill: https://www.skills.sh/decolua/9router/9router-stt
- Giá và giới hạn Groq: https://www.morphllm.com/groq-pricing, https://eesel.ai/blog/groq-pricing,
  https://fast.io/resources/groq-rate-limit.md
