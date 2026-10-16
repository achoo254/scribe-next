---
phase: 5
title: "Từ điển tiếng Việt, rà chuỗi, a11y"
status: completed
priority: P2
effort: "M"
dependencies: [4]
---

# Phase 5: Từ điển, rà chuỗi, a11y

## Tasks
- Chuyển nốt chuỗi hiển thị còn viết cứng trong `web/src/` vào `i18n/messages/vi/*` (Phase 2–4 đã chuyển phần chạm tới). Thông báo lỗi từ backend giữ nguyên (đã là tiếng Việt).
- Kiểm tra tự động: test quét `src/**/*.tsx` tìm chuỗi có dấu tiếng Việt ngoài `i18n/messages` (như cách `check:tokens` quét literal), cho phép danh sách ngoại lệ có lý do.
- Thuật ngữ thống nhất: nhãn "Transcript" (tiếng Anh) — chọn từ Việt cho nhãn tab/tiêu đề (quyết qua Jev), giữ thống nhất toàn app.
- a11y: axe không lỗi serious/critical ở mọi trang; điều hướng bàn phím top bar/menu; vùng chạm ≥ 44 px trên iPhone (`pointer-coarse:`).

## Verification
- vitest (dictionaries + quét chuỗi), Playwright + axe, lint/typecheck/check:tokens; deploy web prod.

## Kết quả (16/10)

Đã làm xong trong `web/`, chưa commit, chưa deploy.

**Chuỗi**
- Mọi chuỗi tiếng Việt còn viết cứng trong `web/src` (trừ `pages/dev/`, comment và test) đã chuyển vào `i18n/messages/vi`. Hai namespace mới: `errors.ts` (chuỗi dự phòng khi backend không gửi `{ error }`: mạng, tệp, transcript, cắt file, upload) và `recorder.ts` (lỗi bắt đầu ghi, hướng dẫn cấp quyền micro theo trình duyệt, ghi bị ngắt, thông báo nền của app iPhone, tên mặc định "Bản ghi {date}"). Các khoá còn lại nằm trong `common` (`edit`, `save`), `login` (mã lỗi `?error=`, kiểm tra phiên), `record` (nguồn, cắt sau khi ghi, live monitor, nhóm lịch sử), `transcript` (trạng thái, hộp chép lời, transcript panel, từng đoạn, thân trang đọc) và `ui` (đang tải, theme toggle, thanh dung lượng, player).
- Component dùng `useT()`. Module không phải React (`lib/api.ts`, các `*-api.ts`, `split-job`, `chunked-upload`, recorder, `recording-groups`, `reader-model`, `login-error`) và callback sống lâu (`use-native-recorder`, `use-chunked-upload`) gọi `activeTranslator().t(...)` lúc chạy, không đọc lúc import. Hằng `INTERRUPT_*`, `CHUNKS_LOST`, `NO_SOURCE` đã bỏ, thay bằng khoá. Bảng nhãn (`theme-toggle`, `record-sources`, `live-monitor`, `login-error`) giữ khoá dạng `as const satisfies …MessageKey`, nên `t()` vẫn được kiểm kiểu.
- Lỗi do backend gửi vẫn hiện nguyên văn. Câu chữ không đổi, nên các test đang so chuỗi tiếng Việt vẫn đạt mà không phải sửa, trừ những test có nhãn được đổi tên dưới đây.

**Đổi tên (Jev 0,81)**: nav, h1 của danh sách và breadcrumb của trang đọc đổi từ "Transcript" thành "Lời đã chép". URL `/transcripts` và tên trong code giữ nguyên. Hộp "Chép lại" ở Lịch sử ghi đổi "tab Transcript" thành "trang Lời đã chép". Câu "tab Ghi âm" ở trạng thái rỗng đã được sửa thành "trang Ghi âm" từ Phase 4; trong `src/` (ngoài trang dev) không còn chữ "tab" nào chỉ trang. Danh từ "transcript" trong câu ("Mở transcript", "Xoá transcript", "Chưa có transcript nào") giữ nguyên.

**Kiểm tra tự động**: `src/i18n/hard-coded-strings.test.ts` parse từng tệp `src/**/*.{ts,tsx}` bằng TypeScript (bỏ `i18n/messages`, `*.test.*`, `test/`, `pages/dev/`). Test lấy string literal, các phần của template và JSX text có chữ tiếng Việt (nguyên âm có dấu và đ, chuẩn hoá NFC, không phân biệt hoa thường). Comment không phải node nên không bị tính. Danh sách ngoại lệ `ALLOWED` ghi tệp, chuỗi và lý do, có test bắt buộc lý do không rỗng; hiện danh sách rỗng. Test có thêm hai test cho bộ quét: literal, template, JSX text và comment; chữ hoa và dấu tách rời. Đã thử cài `src/lib/planted-probe.ts` chứa `"Chuỗi cài thử"`: test fail với `../lib/planted-probe.ts:1 Chuỗi cài thử`. Xoá tệp đó đi thì 5/5 test đạt.

**a11y**: phase này không thêm control mới. axe (serious/critical) sạch ở mọi trang trên cả hai project. Hàng nav gọn trên iPhone vẫn vừa màn 393 px với nhãn dài hơn; test `scrollWidth <= innerWidth` vẫn đạt.

**Kiểm tra** (chạy lần lượt, trước khi chạy máy còn trống 49% bộ nhớ và không có job nặng nào khác)
- lint: `Checked 202 files in 61ms. No fixes applied.` typecheck (src, scripts, e2e): exit 0, 0 lỗi. check:tokens: không có literal, hai khối dark giống nhau, 82 cặp tương phản đạt.
- vitest `--maxWorkers=2`: 48 tệp, 242 test đạt. Trong đó `hard-coded-strings.test` là tệp mới với 5 test, và `dictionaries.test` xác nhận mọi khoá mới đều được dùng.
- build: `✓ built in 2.01s`, không có cảnh báo. `check:dist`: không có stub trong 6 tệp.
- e2e (`pnpm run test:e2e`): 54 đạt, 4 bỏ qua có chủ đích.
- Ảnh mốc: cập nhật có chủ đích 10 ảnh, đều vì nhãn nav/tiêu đề đổi: `topbar-{light,dark}` ×4 (chromium-desktop và webkit-iphone); `transcripts-*`, `reader-*`, `record-row-menu-*` ×6 trên webkit-iphone. Ở các ảnh này, hàng nav gọn và h1/breadcrumb hiện trong khung chụp. Đã xem diff: chỉ khác chữ ở hàng nav và tiêu đề.

**Còn mở**: chưa deploy web prod. Danh từ "transcript" trong câu có cần Việt hoá cho đồng bộ với "Lời đã chép" hay không là việc chờ Jev quyết.

- Jev 16/10: nhãn trang "Lời đã chép" (0,81); danh từ trong câu Việt hoá thành "bản chép lời" (0,99), vd "Mở bản chép lời", "Xoá bản chép lời", "Chưa có bản chép lời nào"; URL `/transcripts` giữ nguyên.
