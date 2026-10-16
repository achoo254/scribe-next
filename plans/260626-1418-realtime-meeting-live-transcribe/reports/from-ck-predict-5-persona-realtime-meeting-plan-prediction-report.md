# Prediction Report — Realtime Meeting + Live Transcribe tiếng Việt

**Ngày:** 2026-06-29 · **Skill:** ck:predict (5-persona debate) · **Input:** `plan.md` (260626-1418) + 6 phase + spike results
**Căn cứ verified:** `tauri.conf.json:24` (csp:null), `commands.rs:146-153` (transcribe_files paths không whitelist), `transcribe.rs:166-177` (reader 1 kênh), `transcribe.rs:183-189` (Mutex giữ khi write_all blocking), `main.py:125` (stdin đơn luồng), `main.py:80` (BatchedInferencePipeline), `main.py:28-29,73-82` (global _model reload), README (Win+macOS) vs `tauri.conf.json:29` (target nsis only), spike report (G3+G4 GO; G1/G2/G5/G6 pending).

## Verdict: **CAUTION** — đi đúng theo cổng gate đã thiết kế, KHÔNG cấp ngân sách Phase 2+ trước khi G1/G2/G6 có số thật.

Không có STOP trigger nào kích hoạt: (a) mọi finding Critical bảo mật đều CÓ mitigation khả thi đã gán vào phase — không có auth-bypass "không lối thoát"; (b) không có bất tương thích kiến trúc nền tảng; (c) ẩn số performance lớn nhất (G1/G2) đã được **gate tường minh**, không bị bỏ qua. Đây là plan hiếm khi de-risk kỹ tới mức này (Validation Log + Red Team 4 reviewer + spike). Rủi ro còn lại là **thực thi** + **ẩn số đo lường chưa chốt**, không phải lỗi thiết kế.

---

## Agreements (toàn bộ / ≥4 persona đồng thuận)

- **Cấu trúc gate là xương sống đúng.** Phase 0 (value gate, loopback) + Phase 1 (feasibility gate, G1/G2/G6) + cổng tái đánh giá trước khi cấp ngân sách Phase 2+ = quản trị rủi ro chuẩn. Tất cả 5 persona đều phản đối việc cam kết 24-37 ngày trước khi có số G1/G2/G6.
- **Các quyết định kiến trúc lõi hợp lý** cho mục tiêu đã nêu: dùng LiveKit (KHÔNG tự viết SFU), host-transcribes-all + active-speaker gating, server pure-relay, retention text-only 1 ngày.
- **Red-team đã gánh phần nặng.** 8 Critical đã có disposition + mitigation gán phase; không còn lỗ hổng auth không mitigation.
- **Ẩn số quyết định sống/chết = G1/G2 (RTF/latency streaming ASR trên laptop i5 4 nhân) + G6 (chân audio WebView2→Rust→sidecar).** Mọi thứ hạ nguồn phụ thuộc cái này; hiện CHỈ có tone tổng hợp, CHƯA có speech tiếng Việt thật. G3 (RAM ~2.1GB, no-OOM) + G4 (webrtcvad) đã ĐẠT bằng số thật → nỗi lo OOM dual-model bị bác bỏ.
- **Scope lớn & thật; giá trị (caption tiếng Việt per-speaker + privacy) khác biệt nhưng mong manh về adoption** so với Meet/Zoom.

## Conflicts & Resolutions

| Topic | Architect | Security | Performance | UX | Devil's Advocate | Resolution |
|-------|-----------|----------|-------------|-----|-----------------|------------|
| **Build platform vs companion+diarization** | Thiết kế mạch lạc, tách lớp tốt | Self-host = tự gánh attack surface (token/caption/TURN) | Per-track giảm tải ASR (chỉ active speaker) | Per-speaker là USP đọc dễ | **Diarization-trên-luồng-trộn** (loopback + pyannote) có thể cho nhãn người-nói mà KHÔNG cần SFU/web/TURN — plan chưa cân nhắc | Thêm 1 nhánh đo ở **Phase 0**: chạy thử diarization trên loopback mix; nếu "đủ tách người" → cân nhắc companion-mode rẻ hơn nhiều. Quyết tại cổng Phase 0 GO. |
| **Latency caption ≤4s** | OK cho luồng caption | — | Có thể KHÔNG đạt trên i5 4 nhân với 2 người chồng | 2-4s vẫn cần set kỳ vọng "live draft, không verbatim" | Nếu >4s → cả mô hình lung lay | Phase 1 đo thật; fallback đã có trong plan (giới hạn 1-2 active speaker, hạ tần suất). Đặt ngưỡng degradation rõ. |
| **Siết CSP (null→strict)** | Cần cho an toàn | BẮT BUỘC trước khi nhúng livekit-client | Siết sai có thể chặn wss LiveKit → vỡ kết nối | — | — | CSP cho phép tường minh `connect-src wss:// https://`, `media-src blob:`; **test livekit-client connect được SAU khi set CSP** = acceptance Phase 2. |
| **Cắt self-host / web client / monorepo** | Giữ (coherent) | — | — | — | Red-team đề xuất cắt cho gọn MVP | **User đã CHỐT giữ cả 3** (Decision #1, web client+monorepo). Theo guard-user-decisions: KHÔNG tự đảo. Surface, không flip. |
| **macOS** | WKWebView ≠ WebView2 (khác pipeline) | Cần NSCamera/Mic + entitlements | — | Khách macOS không join host được | Tăng scope nếu ôm macOS giờ | **Windows-first MVP** (spike đã chốt); xác nhận lại với user trước Phase 2 (open question). |

## Risk Summary

| Risk | Severity | Mitigation |
|------|----------|------------|
| G1/G2 (RTF<1, latency≤4s) CHƯA đo trên speech VI thật + máy min-spec | **Critical** | Chạy `stream_asr_spike.py` trên mẫu VI thật (1 người + 2-3 chồng), `--cpu-threads 4`, kiểm `latency_valid`. KHÔNG mở Phase 2 nếu chưa có số. |
| G6 chân audio WebView2→AudioWorklet→Rust→sidecar chưa chứng minh | **Critical** | Mini-spike riêng NGAY sau G1/G2 (cần app chạy): kiểm `MediaStreamTrackProcessor`/`AudioWorklet`, COOP/COEP cho SharedArrayBuffer trên `tauri://`, đo round-trip. Nếu fail, latency Python không cứu được. |
| Deadlock sidecar: `send()` giữ Mutex khi `write_all` blocking + PCM dồn nhanh hơn pipe 64KB (transcribe.rs:183-189) | **High** | Thread nền sở hữu ChildStdin + bounded channel drop-on-full + emit `backpressure`; tách session streaming khỏi vòng `for line in sys.stdin` đơn luồng (main.py:125) + khỏi BatchedInferencePipeline blocking. |
| 8 Critical bảo mật phải LANDED, không bị skip dưới áp lực thời gian (C6 role-from-client, C7 caption injection, C8 store no-auth, H1 CSP/XSS→transcribe_files) | **High** | Đưa thành acceptance gate cứng từng phase, không "tùy chọn". H1: `transcribe_files` (commands.rs:146) nhận `paths` tùy ý + csp:null = vuln HIỆN TẠI, meeting khuếch đại. |
| Monorepo refactor phá đường dẫn cứng (frontendDist `tauri.conf.json:10`, find_script `transcribe.rs:108-117`, build_pyinstaller.ps1, capabilities) | **High** | Checklist H6 trên nhánh riêng; verify CẢ `tauri dev` LẪN `tauri build` release (sidecar binary đúng chỗ) trước khi sang bước 2. Sai frontendDist → màn trắng không báo lỗi. |
| Adoption: phải cài host desktop + kéo người sang web client mới, bỏ Meet/Zoom | **Medium-High** | Phase 0 value gate trả lời "transcript có đủ dùng không"; cân nhắc companion-mode như fallback định vị. |
| UiRow/UiCol thiếu ở ui local (16 comp) | **Medium** | Chốt desktop import `@hobui/ui` npm đồng bộ web (giải luôn H5), pin version + `.npmrc` private registry. |
| Egress bandwidth ở 8 người = hóa đơn thật | **Medium** | Đo egress/giờ Phase 5; simulcast + giới hạn độ phân giải mặc định. |

## Recommendations

1. **Đóng G1/G2/G6 trước mọi thứ khác** — đây là điều kiện duy nhất để mở cổng Phase 2. Chuẩn bị 2 mẫu speech VI thật (1 người liên tục + 2-3 người chồng, có thuật ngữ kỹ thuật), chạy spike `--cpu-threads 4`, rồi mini-spike WebView2. Không có số = không cấp ngân sách. (Lý do: toàn bộ mô hình host-transcribes-all đứng/sập ở đây.)
2. **Thêm 1 nhánh đo diarization-trên-luồng-trộn vào Phase 0** — chạy thử speaker-diarization trên loopback mix tiếng Việt. Nếu đủ tách người, weigh lại companion-mode (rẻ hơn nhiều, không cần SFU/web/TURN) tại cổng Phase 0 GO. (Lý do: đây là alternative đơn giản duy nhất plan chưa cân; rẻ để loại trừ ngay trong value gate.)
3. **Biến 8 Critical bảo mật + H1 CSP thành acceptance gate cứng, không tùy chọn** — đặc biệt fix `transcribe_files` whitelist path + CSP khác null là vuln hiện tại, làm sớm ở Phase 2. Test livekit-client connect SAU khi siết CSP. (Lý do: dưới áp lực tiến độ, security fix là thứ dễ bị cắt nhất.)
4. **Validate mô hình concurrency sidecar (thread + bounded channel) ngay trong spike Phase 1**, không để tới Phase 3 — vì nó là rework của component đang chạy production (Mode C). (Lý do: deadlock toàn-app là failure mode tệ nhất, rẻ để chứng minh sớm.)
5. **Giữ nguyên 3 quyết định user đã chốt** (self-host, web client, monorepo) — KHÔNG để audit/YAGNI tự đảo. Nếu muốn xét lại, surface kèm dữ liệu mới, để user quyết. (Lý do: guard-user-decisions, tránh silent regression.)

---

## Câu hỏi mở (cần user quyết)

1. **macOS:** Xác nhận Meeting MVP = Windows-first? (WKWebView cần pipeline riêng + entitlements; `tauri.conf.json` hiện chỉ target `nsis`.) — Khuyến nghị: Windows-first.
2. **Diarization alternative:** Có muốn chèn nhánh đo diarization-trên-loopback vào Phase 0 để so với companion-mode không, trước khi cam kết build platform? — Khuyến nghị: có (rẻ, đo trong value gate).
3. **Mẫu speech tiếng Việt cho G1/G2:** tự thu giọng thật (phản ánh cross-talk) hay tạo TTS (tiện nhưng không thật)? — Khuyến nghị: thu thật ≥1 mẫu 2-3 người chồng.
