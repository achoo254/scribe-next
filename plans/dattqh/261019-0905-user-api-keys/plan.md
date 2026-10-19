---
title: Key riêng — người dùng tự nhập key API
status: completed
created: 2026-10-19
tier: L
---

# Key riêng — người dùng tự nhập key API

Người dùng nhập key API của chính họ ("Key riêng") để chép lời, chép trực tiếp, viết biên bản, hỏi đáp và dịch bằng tài
khoản nhà cung cấp của họ, không tốn hạn mức của Scribe. Quyết định chốt qua Jev (19/10, hai lượt: phạm vi gốc rồi mở
rộng nhà cung cấp).

## Quyết định
1. **Nhà cung cấp và việc dùng được** (mở rộng 19/10):
   - Chép lời tệp (cả phiên iPhone): ElevenLabs (mặc định, có người nói, cả phiên một request), OpenAI `gpt-transcribe`
     (theo mảnh ≤ 45 giây, không người nói), Gemini (theo mảnh, JSON câu + giây; mốc là ước lượng, không người nói —
     giao diện nói rõ).
   - Chép trực tiếp iPhone: ElevenLabs Scribe v2 Realtime hoặc OpenAI Realtime. Gemini không dùng cho chép trực tiếp.
   - Biên bản, hỏi đáp, dịch: OpenAI, Gemini, Anthropic Claude; người dùng chọn một nhà cung cấp (radio) và model trong
     danh sách ngắn, mặc định model rẻ nhanh: `gpt-4.1-mini` (hoặc `gpt-4.1-nano`), `gemini-3.8-flash` (hoặc
     `gemini-3.5-flash-lite`), `claude-haiku-5-5` (hoặc `claude-sonnet-5-5`). Claude không chép lời.
   - Chọn key cho từng việc: nhà cung cấp người dùng chọn nếu key đó hợp lệ, không thì key hợp lệ đầu tiên theo thứ tự
     trên; không có key hợp lệ thì key server với hạn mức thường.
2. **Lưu trữ**: Mongo `user_keys` `{userId, provider, ciphertext, nonce, last4, createdAt, updatedAt, lastCheckedAt,
   status}` (`{userId, provider}` unique), AES-256-GCM (`aes-gcm`), bí mật 32 byte từ tệp `USER_KEYS_SECRET_PATH` (prod
   `/opt/scribe-be/.user-keys-secret`, 600; thiếu, sai cỡ hay quyền rộng hơn 600 ⟹ tính năng tắt, giao diện báo), nonce
   96 bit ngẫu nhiên mỗi lần, AAD = userId + nhà cung cấp. Lựa chọn ở `user_key_prefs`.
3. **API chỉ ghi**: không bao giờ trả key, chỉ nhà cung cấp, 4 ký tự cuối, trạng thái, giờ sửa/kiểm. Kiểm khi lưu bằng
   một lần gọi không tốn credit: ElevenLabs `POST /v1/single-use-token/realtime_scribe` (chạy với key chỉ có quyền
   Speech to Text — đã thử với key Starter của chủ dự án), OpenAI `GET /v1/models`, Gemini `GET /v1beta/models`,
   Anthropic `GET /v1/models`. Xoá bất cứ lúc nào. Lưu giới hạn 5 lần/phút mỗi tài khoản.
4. **Hạn mức**: việc chạy bằng key riêng không tính phút, lượt ngày, quỹ free, trần tổng, sổ giờ ElevenLabs, chờ tài
   khoản mới; dung lượng, cỡ tệp, rate limit vẫn áp. Vẫn ghi sổ chi tiết (`transcribe_runs`, `summary_runs`, `ask_runs`,
   `translate_runs` có `byok`) và sổ xem `b:<userId>:<tháng>`; `admin usage --recompute` bỏ qua các lượt này. Allowlist
   tính năng (`TRANSCRIBE_EMAILS`/`TRANSCRIBE_OPEN`, `REALTIME_EMAILS`) giữ nguyên.
5. **Key riêng lỗi lúc dùng** (401/403/402/hết credit): lượt dừng với "Key … của bạn không dùng được: … — sửa hoặc xoá
   key ở Key riêng", key bị đánh dấu `invalid`/`quota` (khớp đúng bản key đã dùng theo nonce), audit `user_key.failed`.
   Không chuyển sang key server, không đặt cờ ngắt hay dấu hết quota tháng của server. Lượt sau dùng key server (key
   không còn hợp lệ) — giao diện hiện trạng thái key và thẻ mức dùng trở lại phút của Scribe.
6. **Lượt xếp hàng** (cả phiên iPhone) chỉ ghi nhà cung cấp; worker đọc và giải mã key của chủ phiên lúc chạy.
7. **Ai**: mọi tài khoản đăng nhập (free và plus).
8. **Model chữ của server** (mở rộng 19/10): `LLM_PROVIDER` (`openai` mặc định | `gemini` | `anthropic`), `LLM_MODEL`
   (mặc định theo nhà cung cấp; `SUMMARY_MODEL` cũ vẫn nhận với OpenAI), key `OPENAI_KEY_PATH` / `GEMINI_KEY_PATH` /
   `ANTHROPIC_KEY_PATH`. Cùng lớp chuyển đổi với key riêng; chi phí quy ra trần theo bảng giá `summary/llm.rs` `MODELS`.

## Bảng giá dùng (kiểm 19/10/2026, USD mỗi triệu token)
| Model | Vào | Ra | Nguồn |
|---|---|---|---|
| `gpt-4.1-mini` / `gpt-4.1-nano` | 0,40 / 0,10 | 1,60 / 0,40 | <https://developers.openai.com/api/docs/pricing> |
| `gemini-3.8-flash` | 0,75 | 3,75 | <https://ai.google.dev/gemini-api/docs/pricing> (gấp đôi từ 1/1/2027) |
| `gemini-3.5-flash-lite` | 0,30 | 2,50 | như trên |
| `claude-haiku-5-5` | 0,10 (≤100k token vào), 0,50 (trên) | 0,50 / 2,50 | <https://platform.claude.com/docs/en/about-claude/pricing> |
| `claude-sonnet-5-5` | 2 | 10 | như trên |

`gpt-transcribe` 0,0045 USD/phút (chỉ dùng với key riêng, không quy vào sổ).

## Mô hình đe doạ
- **Lưu gì**: key API của người dùng — lộ ra là người khác tiêu tiền của họ. Bản mã ở Mongo (cổng Mongo mở trong mạng
  nhóm), bí mật ở tệp 600 trên server: lộ riêng DB hay riêng bản sao lưu DB không lộ key. Lộ cả server (root) thì lộ key —
  chấp nhận, như key server.
- **AAD** gắn bản mã với userId + nhà cung cấp: chép bản mã sang tài khoản khác hay ô nhà cung cấp khác không giải được.
- **Không rò**: API không trả key; key không vào log (access log chỉ method/path), audit, thông báo lỗi (thông điệp của
  nhà cung cấp chỉ ghi log, không chứa key), URL (chỉ header). Bản rõ trong `Zeroizing`, giải mã ngay trước khi dùng.
- **Lạm dụng tính năng lưu** (dò key, gọi nhà cung cấp hộ): 5 lần/phút mỗi tài khoản + rate limit chung.
- **Mất bí mật**: key không đọc được (`unreadable`), người dùng nhập lại; sao lưu tệp bí mật riêng với Mongo.
- **Tiền của Scribe**: không bao giờ âm thầm chuyển việc của người dùng sang key server giữa chừng.

## Hợp đồng API
- `GET /api/me/keys` → `{available, keys: [{provider, last4, status, updatedAt, lastCheckedAt}], prefs: {transcribe,
  live, text, textModel}, active: {…}, models: {openai|gemini|anthropic: [{id, label}]}}`.
- `PUT /api/me/keys/{provider}` `{key}` → `{provider, last4, status, updatedAt, lastCheckedAt}`; 400 `KEY_FORMAT`,
  `KEY_REJECTED`, `KEY_NO_CREDIT`; 503 `KEY_CHECK_FAILED`, `USER_KEYS_DISABLED`; 429.
- `DELETE /api/me/keys/{provider}` → `{deleted}`.
- `PUT /api/me/key-prefs` `{transcribe?, live?, text?, textModel?}` → như GET; 400 khi nhà cung cấp không làm được việc
  đó hoặc model không có trong danh sách.
- `GET /api/me` thêm `ownKeys: {available, transcribe, live, text, textModel, minutes}`.
- Mọi tính năng: key riêng không mở được hoặc bị từ chối lúc request ⟹ 403 `USER_KEY_FAILED` với câu chỉ cách sửa.

## Kết quả (19/10/2026)
- Backend: `server/scribe-be/src/user_keys/` (crypto, store, check, api), `summary/llm.rs` (lớp chuyển đổi ba nhà cung
  cấp + bảng giá), `summary/meter.rs` (`reserve_own`), `transcribe/gemini.rs`, `transcribe/openai.rs`
  (`gpt-transcribe`), worker/whole/realtime/summary/ask/translate theo key riêng, `config.rs` (`Vendor`, `LLM_*`,
  `USER_KEYS_SECRET_PATH`).
- Web: hộp "Key riêng" (`components/user-keys-dialog.tsx`, `key-choice-section.tsx`), mục account menu, thẻ mức dùng
  "Dùng key của bạn".
- Docs: `docs/codebase-summary.md`, `docs/deploy-web-production.md` (§Key riêng: tạo tệp bí mật, sao lưu),
  `docs/project-roadmap.md`.

## Tiêu chí nghiệm thu
- [x] Lưu/kiểm/xoá key bốn nhà cung cấp; API không bao giờ trả key (unit test duyệt mọi trường phản hồi).
- [x] Mã hoá: roundtrip, sai AAD/bí mật/bản mã thì không giải được, 1.000 lần mã hoá 1.000 nonce khác nhau, tệp bí mật
      phải đúng 32 byte và quyền 600.
- [x] Việc bằng key riêng không tính sổ nào (unit test + Mongo dev: sổ phút không đổi, sổ xem `b:` cộng, sổ chi tiết có
      `byok`, lượt `byok` không vào trần của đối soát).
- [x] Lỗi key riêng không đặt cờ ngắt/dấu hết quota của server (unit test các hàm quyết định).
- [x] Lớp chuyển đổi: request/response từng nhà cung cấp với máy chủ giả; OpenAI thử thật; kiểm key thật OpenAI và
      ElevenLabs (key chỉ có Speech to Text hợp lệ, key sai bị từ chối); `gpt-transcribe` thử thật. Gemini, Anthropic
      **chưa thử với key thật** (chưa có key).
- [x] Giao diện điện thoại và máy tính, sáng/tối: e2e với stub, ảnh chụp đã xem; vitest cho hộp thoại và thẻ mức dùng.

## Việc của chủ dự án trước khi deploy
- Tạo tệp bí mật trên server và thêm `USER_KEYS_SECRET_PATH` vào `.env.production` (deploy doc §Key riêng); sao lưu tệp
  riêng với Mongo.
- Khi có key Gemini/Anthropic: thử một lượt thật mỗi nhà cung cấp (biên bản, và chép lời Gemini).
