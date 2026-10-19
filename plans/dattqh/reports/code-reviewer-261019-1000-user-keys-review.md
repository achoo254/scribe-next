# Review bảo mật "Key riêng" (d2b00be, 7f6ab21, d82a6d8)

Đọc mã, không chạy test. Không Critical/High. Mã hoá AES-256-GCM, nonce ngẫu nhiên mỗi lần, AAD gắn userId+nhà cung cấp, không trả key, key chỉ đi trong header,
phân quyền theo user, không đi vòng tính phút, không tự chuyển sang key server khi key người dùng hỏng: đều đạt.

## Medium
- **M1** `summary/llm.rs:304`, `transcribe/gemini.rs:72`: server dùng Gemini thì phần "suy nghĩ" (tới 8.000 token) không tính vào khoản giữ chỗ → vượt trần tới ~10 phút mỗi lượt.
- **M2** `summary/llm.rs:124-130`: `LLM_MODEL` không có trong bảng giá bị tính theo giá model mặc định (vd Claude Opus tính như Haiku, rẻ hơn ~20 lần), không cảnh báo.

## Low
L1 báo hết quota trễ của token cũ đánh dấu nhầm key mới (`realtime.rs:533`) · L2 `/api/me` lỗi 500 khi đọc key riêng lỗi · L3 cảnh báo tạm ngưng vẫn ghi "key OpenAI" khi server dùng nhà cung cấp khác ·
L4 Gemini 429 hết quota của key người dùng không được nhận ra · L5 không giới hạn tần suất xin token chép trực tiếp bằng key riêng.

## Chưa kiểm
Thân request Anthropic (`thinking: between_tools`, `output_config.format`) và Gemini chưa gọi thật; lệnh tạo tệp khoá trong tài liệu cần sudo không mật khẩu.
