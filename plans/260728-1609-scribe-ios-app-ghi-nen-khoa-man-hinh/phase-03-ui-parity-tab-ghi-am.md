---
phase: 3
title: "UI parity tab Ghi âm"
status: pending
priority: P1
effort: "M"
dependencies: [2]
---

# Phase 3: UI parity tab Ghi âm

## Overview

Hiện tab **Ghi âm** trên iOS (Phase 1 ẩn nó đi) và nối vào engine native. Đúng ngôn ngữ thiết kế của app, nhưng phản ánh trung thực những gì iOS làm được và không làm được.

## Requirements

**Chức năng**
- Nút Ghi/Dừng, đồng hồ, level meter
- Chọn độ dài đoạn (1 / 5 / 10 phút, **mặc định 1** — D6)
- **Gom đoạn cùng phiên thành một dòng gấp/mở trong Lịch sử ghi** (V3) — họp 2 tiếng ở đoạn 1 phút là 120 file, không được đổ thành 120 dòng rời <!-- Updated: Validation Session 1 - V3 -->
- Trạng thái hàng đợi upload: đang gửi / còn N đoạn chờ mạng / lỗi + nút Thử lại
- Sau khi mở khoá máy: UI **ghi đè state từ Swift**, không tự đếm bằng timer JS (D2)
- Báo rõ khi phiên kết thúc do gián đoạn không phục hồi được, và khi đoạn đang mở bị mất do kill
- Cảnh báo còn đoạn chưa upload trước khi thoát app
- **Đăng xuất trên iOS gọi bridge native**, không phải `fetch` từ Vue (D5.2)

**Phi chức năng**
- Dùng component `Ui*` của `@hobui/ui`; không `!important`, không hardcode màu
- Tôn trọng safe area (Dynamic Island, home indicator)
- Mọi nhánh iOS sau `isNative` — web không hồi quy

## Architecture

```ts
const rec = isNative ? useRecorderNative() : useRecorder()
```

Hai composable dùng **hợp đồng đã chốt ở Phase 2**. Ba chỗ phải rẽ nhánh tường minh, không phải `v-if` trang trí:

1. **`toggle()` sau khi dừng** (`RecordView.vue:91-121`) — web: `stop()` trả `result.fileId` → `sp.createJob(...)`. iOS: `stop()` trả `null`, engine đã cắt sẵn theo phút, **không chạy job split**. Khối tiến độ job (`:210-217`) ẩn khi `isNative`.
2. **Làm mới lịch sử** — web: `watch(sp.status)`. iOS: sự kiện `segmentUploaded` từ plugin. `historyRef.refresh()` (`:100,119`) cần điểm kích hoạt riêng.
3. **`useSplitter`** (9 điểm dùng: 7, 12, 89, 99, 105, 116–119, 211–216) — trên iOS chỉ còn dùng cho tab Cắt file, không dùng trong luồng ghi.

**Đồng bộ khi foreground**: nghe `appStateChange` → `RecorderPlugin.getState()` → **ghi đè** state Vue. Đây là điểm dễ sai nhất của hybrid; Swift luôn thắng, không bao giờ merge.

**Không** tạo `UploadQueueStatus.vue` — dùng `UiAlert` / `UiProgressLinear` inline như pattern sẵn có (`RecordView.vue:207,211-216`). Với một người dùng, thêm component là indirection không đổi lấy gì.

<!-- Updated: Validation Session 1 - V3 gom nhóm theo phiên -->
**Gom nhóm Lịch sử ghi (V3).** Đoạn 1 phút giữ bán kính mất nhỏ khi OS kill, nhưng họp 2 tiếng đẻ ra 120 file. `RecordingHistory.vue` gom theo tiền tố tên `<sessionId>#<index>` (hợp đồng chốt ở Phase 2) thành một dòng: tên phiên, tổng thời lượng, tổng dung lượng, ngày hết hạn — bấm để mở danh sách đoạn con.

Gom theo **mẫu tên**, không theo cờ platform: file do web tạo không khớp mẫu nên hiển thị y như cũ. Nhờ vậy không cần nhánh `isNative` trong `RecordingHistory.vue` và **không có bề mặt hồi quy web**.

## Related Code Files

- Modify: `web/src/views/HomeView.vue` — bỏ ẩn tab Ghi âm trên iOS
- Modify: `web/src/views/RecordView.vue` — chọn composable, 3 nhánh trên, khối trạng thái upload inline, chọn độ dài đoạn
- Modify: `web/src/composables/use-recorder-native.ts` — resync khi foreground
- Modify: `web/src/composables/use-auth.ts` — `logout()` nhánh `isNative` gọi bridge native
- Modify: `web/src/components/RecordingHistory.vue` — gom nhóm theo mẫu tên `<sessionId>#<index>` (V3) + đánh dấu đoạn đang chờ upload

## Implementation Steps

1. Hiện lại tab Ghi âm, nối `useRecorderNative`.
2. Rẽ nhánh `toggle()` và ẩn khối job split khi `isNative`.
3. Nối `segmentUploaded` → `historyRef.refresh()`.
4. Trạng thái hàng đợi inline + nút Thử lại (nút này là **đường phục hồi duy nhất** cho đoạn ở nhóm "kết thúc" — không được bỏ).
5. Logout qua bridge native.
6. Resync khi foreground; test bằng cách khoá máy 10 phút rồi mở lại.
7. Cảnh báo trước khi thoát khi còn đoạn tồn; safe area.

## Success Criteria

**Parity so với web** (tự tay kiểm trên iPhone):
- [ ] Ghi mic → dừng → các đoạn vào Lịch sử ghi, phát lại được
- [ ] Ghi **30 phút / đoạn 1 phút** → Lịch sử ghi hiện **một dòng phiên**, mở ra thấy 30 đoạn con (V3)
- [ ] Bản ghi cũ tạo từ web vẫn hiện dạng dòng rời như trước — gom nhóm không đụng chúng
- [ ] Đồng hồ + level meter đúng khi app đang mở
- [ ] Cắt file (Mode B) đầy đủ
- [ ] Lịch sử ghi: phát, tua, xoá, hiện ngày hết hạn
- [ ] QuotaBar cập nhật sau khi ghi và sau khi xoá
- [ ] Không có toggle "Tiếng hệ thống" / "Ghi kèm màn hình"

**Hành vi riêng iOS:**
- [ ] Khoá máy 10 phút rồi mở → đồng hồ hiển thị **đúng** thời gian đã trôi (lấy từ Swift)
- [ ] Còn đoạn chưa upload → thoát app hiện cảnh báo
- [ ] Gián đoạn không phục hồi được → báo rõ, phần đã ghi được lưu
- [ ] Đoạn đang mở mất do kill → thông báo nêu rõ khoảng thời gian mất
- [ ] **Đăng xuất → hàng đợi dừng ngay; đoạn tồn KHÔNG upload vào tài khoản cũ** (kiểm bằng log server)
- [ ] Không phần tử nào bị Dynamic Island / home indicator che

**Kỹ thuật:**
- [ ] `vue-tsc --noEmit` + `vite build` EXIT 0
- [ ] Web không hồi quy: **`npm run dev` trỏ staging** → ghi thử bằng đường cũ trên Chrome desktop vẫn chạy. (Bản 1 ghi "mở `web/dist` trên Chrome" — bất khả thi: `file://` không có backend và `getUserMedia` cần secure context, `use-recorder.ts` có sẵn nhánh `SecurityError` cho đúng ca này. Gate cũ luôn fail nên sẽ bị bỏ qua, tức là không gate gì cả)

## Risk Assessment

**Hồi quy web (Trung bình).** `RecordView.vue` dùng chung. Ba nhánh ở mục Architecture là thay đổi cấu trúc thật, không phải `v-if` trang trí. Nếu file thành rối thì tách phần thân UI chung ra component con — **không** fork thành hai view.

**Resync sai sau foreground (Trung bình).** Vue và Swift lệch nhau thì người dùng thấy đồng hồ sai và mất niềm tin. Swift là chân lý, Vue luôn ghi đè.

**Rollback (đã sửa so với bản 1).** Revert Phase 3 làm mất nút "Thử lại" — đường phục hồi duy nhất cho đoạn đã bị đánh dấu thất bại. Nếu đã có bản ghi thật trên máy thì rollback sẽ bỏ rơi chúng. Trước khi revert phải drain hàng đợi.
