# Review Phase 3: router URL + provider giữ trạng thái (thay đổi chưa commit)

Thời điểm: 16/10/2026 15:40 (Asia/Saigon). Người review: code-reviewer. Không sửa file nào.

## Phạm vi
- 27 tệp sửa (+529/−599) và 11 tệp mới (khoảng 1.100 dòng) trong `web/`, cùng `web/ios/App/App/MainViewController.swift`.
- Trọng tâm: `routes.tsx`, `layout/signed-in-state.tsx`, `pages/record/record-state.tsx`, `pages/split/split-state.tsx`, `layout/leave-guard.tsx`, `layout/session-activity.ts`, `layout/transcript-nav.ts`, `pages/transcript/transcript-{page,reader}.tsx`, `features/recorder/use-{recorder,native-recorder}.ts`, `deploy/nginx-scribe.conf`.
- Đã chạy lại: `vitest --maxWorkers=1` cho `routes.test.tsx`, `record-state.test.tsx`, `transcript-reader.test.tsx`: 3 tệp, 15 test đạt. Lint, typecheck, build và Playwright thì không chạy lại; số liệu của tác giả được ghi nhận nguyên như trong "Kết quả".

## Đánh giá chung
Không thấy lỗi Critical hay High nào do thay đổi này gây ra. Những điểm chính đều kiểm được trên code:
- Cả hai recorder chỉ có một instance cho mỗi phiên đăng nhập. `useRecorder` và `useNativeRecorder` chỉ được gọi trong `record-state.tsx:86` và `record-state.tsx:157`.
- Route cha `/` có element tĩnh, không có `key`. Đổi route con chỉ thay `<Outlet/>`, nên provider không remount.
- `refresh()` của auth (`auth-context.tsx:101-104`) chỉ thay object `user`, nên provider re-render chứ không remount. Effect sync upload phụ thuộc `[email]` (`use-native-recorder.ts:100`), nên không chạy lại.
- Ở chế độ StrictMode dev, effect chạy hai lần nhưng vẫn an toàn: `NativeRecorder.attach` có bộ đếm `generation` (`native-recorder.ts:120-151`), và `cancelled` chặn `syncUploads` lần đầu vì `userKeyOf` là bất đồng bộ.
- Đăng xuất trên web dừng và lưu bản ghi trước (`session-activity.ts:61-67`). Trên iOS, `logout` gọi `NativeRecorderPlugin.signOut` (`auth-context.tsx:81-82`). Đăng nhập lại thì provider mount mới, nên `syncUploads` chạy đúng 1 lần.
- Không có vòng lặp chuyển hướng: `RequireTranscribe` → `/record` là route không có điều kiện; `*` → `/` → index → `/record`.
- Mã hoá khoá đúng ở cả hai đầu: `transcriptPath` (`app-paths.ts:13`) và `docPath` (`transcript-docs-api.ts:76`). Khoá chỉ có dạng `s:<id>` hoặc `f:<id>` (`server/scribe-be/src/models/file.rs:261-272`).
- Deploy web không có vấn đề: Vite dùng base `/` (dist/index.html trỏ `/assets/...`); nginx có `try_files $uri $uri/ /index.html` (`deploy/nginx-scribe.conf:80-83`); các trang không lazy.

Rủi ro còn lại chủ yếu là UX và độ bền khi gặp sự cố, xếp từ Medium trở xuống.

## Critical
Không có.

## High
Không có.

## Medium

**M1. Lỗi render ở bất kỳ trang nào sẽ unmount `SignedInState` và cắt bản ghi web đang chạy.**
- `routes.tsx:38-69`: không route nào có `errorElement`. React Router 7 (bản đã cài 7.18.4) khi đó đặt error boundary mặc định ở route gốc. Route gốc ở đây là `/`, có element là `RequireAuth > SignedInState > AppShell`.
- Vì vậy chỉ cần một lỗi render trong `TranscriptReader`, `SplitPage` hay `QuotaBar` là cả provider bị thay bằng trang "Unexpected Application Error!". Khi đó `useRecorder` gọi cleanup → `Recorder.dispose()` (`recorder.ts:198-203`): mic bị trả, `MediaRecorder` bị bỏ mà không `stop()`, nên mất khoảng 1 giây chunk cuối và không có `finalize`; `recording_session` treo ở trạng thái chưa finalize.
- Trên iOS, `NativeRecorder.dispose` chỉ gỡ listener. Swift vẫn ghi, nhưng UI mất nút Dừng cho tới khi reload.
- Đây không phải regression: trước đây không có boundary, lỗi render cũng làm React unmount cả root. Nhưng giờ đã có data router, và đúng mục tiêu của phase là giữ bản ghi qua mọi thay đổi ở trang, nên nên sửa ở bước này.
- Cách sửa: thêm một layout route không path bọc các route con, với `errorElement` render trong `<Outlet/>` (thông báo lỗi trang và link về `/record`). Khi đó shell và provider vẫn sống:
  ```tsx
  children: [{ errorElement: <PageError />, children: [ /* index, record, split, transcripts… */ ] }]
  ```

**M2. Xoá transcript xong, bấm lùi hoặc vuốt lùi sẽ mở lại trang của bản vừa xoá.**
- `transcript-reader.tsx:96-98`, sau khi xoá, gọi `onBack()`, tức `navigate(PATHS.transcripts)` dạng PUSH (`transcript-page.tsx:21`). Lịch sử thành `…/transcripts → /transcripts/<key> → /transcripts`. Nút back của trình duyệt, hay cú vuốt lùi mới bật trên iPhone (`MainViewController.swift:11`), đưa người dùng về `/transcripts/<key>` đã xoá, và trang hiện lỗi 404 của API.
- Nút "Danh sách" cũng PUSH như vậy. Kết quả là lịch sử đi vòng list → reader → list, nên vuốt lùi sau khi bấm "Danh sách" lại mở reader.
- Cách sửa: sau khi xoá thì `navigate(PATHS.transcripts, { replace: true })`. Với "Danh sách": nếu mục trước trong lịch sử của app là danh sách (đọc `location.key !== "default"` hoặc một cờ trong `state`) thì `navigate(-1)`, nếu không thì `replace`.

## Low

**L1. `*` nằm ngoài layout đã đăng nhập** (`routes.tsx:90`). Mọi đường không khớp, ví dụ `/record/x` hay `/transcripts/a/b`, sẽ rời route `/`, unmount provider rồi mount lại. Hiện tại chỉ gặp khi tự gõ URL (khi đó trang cũng tải lại), nên rủi ro thấp. Đặt thêm `{ path: "*", element: <Navigate to={PATHS.record} replace /> }` làm route con của `/` để provider không bao giờ bị unmount vì đường sai.

**L2. Deep link khi chưa đăng nhập không quay lại được trang đích.** `require-auth.tsx:25` chuyển về `/login` mà không giữ đường cũ. `login-page.tsx:31` và `server/scribe-be/src/auth.rs:195` đều trả về `/`. Trước đây chưa có URL nên chưa có vấn đề này; giờ link `/transcripts/s%3A…` gửi cho người chưa đăng nhập sẽ rơi về `/record`. Nếu muốn hỗ trợ: lưu `location` vào `state` hoặc `sessionStorage`, và cho cả hai luồng đăng nhập (web và native) đọc lại.

**L3. Đăng xuất bỏ mất bản đang sửa mà không hỏi.**
- `session-activity.ts:52-68` chỉ tính bản ghi và upload; `LeaveGuard` không tham gia vào luồng đăng xuất.
- Trên iOS, `use-sign-out.ts:26-27` gọi `navigate("/login")` khi blocker vẫn đang đăng ký, nên lần điều hướng này bị chặn. Việc rời trang chỉ xảy ra vì `RequireAuth` unmount reader và `<Navigate>` điều hướng lại. Kết quả cuối cùng vẫn đúng, nhưng phụ thuộc vào thứ tự render.
- Nên cân nhắc thêm "đang sửa transcript" vào `SessionActivity`, hoặc chấp nhận và ghi rõ trong plan.

**L4. Upload ở Cắt file không bị huỷ khi đăng xuất.** `use-chunked-upload.ts:24-39` không có cleanup hay `AbortController`; provider (`split-state.tsx:44`) unmount nhưng vòng `while` (`chunked-upload.ts:51-70`) vẫn gửi chunk kế tiếp. Không lộ dữ liệu sang tài khoản khác: server kiểm chủ phiên (`server/scribe-be/src/upload.rs:90`), nên request đầu tiên sau đăng xuất bị từ chối và vòng lặp dừng. Đây là điểm còn lại từ H2 của red-team.

**L5. Context value không memo, nên header re-render mỗi giây khi đang ghi.** Bộ đếm giờ gọi `set()` mỗi giây (`recorder.ts:133-134`). Value của `BrowserRecordContext` là object mới mỗi lần render (`record-state.tsx:135-147`), nên `AppHeader` và `AccountControls` (qua `useSessionActivity`, `app-header.tsx:39`) render lại mỗi giây. `SplitPageContext` cũng vậy, mỗi lần tiến độ upload đổi (`split-state.tsx:67-78`). Chi phí hiện còn nhỏ, nhưng `useMemo` cho value và `useCallback` cho `toggle` là đủ sửa.

**L6. Test provider không dùng cây route thật.** `record-state.test.tsx:116-140` tự dựng router, nên nếu ai dời `SignedInState` khỏi `routes.tsx:42-46` thì vitest vẫn đạt; chỉ Playwright (`e2e/specs/routes.spec.ts:36`) bắt được. Nên dùng `appRoutes` với `createMemoryRouter` cho ít nhất một test.

**L7. Frontmatter của phase-03 vẫn ghi `status: pending`** (`phase-03-routes-and-state.md:4`), trong khi mục "Kết quả" đã báo làm xong. Người lead nên cập nhật; tôi không sửa.

## Edge case đã kiểm, không có lỗi
- Xoá transcript khi đang sửa: `setRemoved` → effect của `LeaveGuard` (con, chạy trước) cập nhật blocker thành `false` → effect của cha gọi `onBack`. Không hỏi lại. `useNavigate` của data router ổn định, nên effect không chạy lại.
- `LeaveGuard` không bị prompt đôi: `ConfirmModal.confirm` gọi `onOpenChange(false)` sau `proceed()`, và `proceeding` chặn `reset()` trên blocker đã bị xoá. Bấm lại vào NavLink đang active sẽ thành REPLACE, không thêm mục lịch sử.
- Quyền `canTranscribe` theo allowlist email (`server/scribe-be/src/auth.rs:469`), không đổi theo hạn mức, nên `refresh()` sau khi chép lời không đá người dùng ra khỏi reader.
- Reload trong WKWebView, ví dụ sau khi WebContent bị hệ điều hành dừng lúc khoá màn hình lâu: Capacitor trả `index.html` cho đường không có phần mở rộng. Khoá không chứa `.`, nên `/transcripts/s%3A…` mở lại được, và provider mount lại → `attach` + `syncUploads`.
- Listener và interval: `visibilitychange` được gỡ (`use-native-recorder.ts:66`); polling của `SplitJob` dừng khi unmount (`use-split-job.ts:20`); effect refetch của `ManagedFilesCard` đã bỏ, thay bằng `refetchOnMount: "always"` (`file-queries.ts:32`).

## Việc nên làm (theo thứ tự)
1. M1: thêm `errorElement` cho một layout route không path bao các route con.
2. M2: `replace: true` sau khi xoá; xem lại cách nút "Danh sách" ghi lịch sử.
3. L1: đưa `*` vào trong layout đã đăng nhập.
4. L5, L6: sửa nhỏ, có thể làm cùng lượt.
5. L2, L3: quyết định sản phẩm, hỏi Jev nếu cần.

## Theo dõi plan
- Các mục trong Files và Verification (vitest, Playwright) khớp với code đã kiểm.
- Còn mở như tác giả đã ghi: cài lại app iPhone và thử trên máy thật (ghi → đổi trang → khoá màn hình 2 phút → dừng; vuốt lùi), và deploy prod khi hàng đợi rỗng.

## Câu hỏi còn mở
- WKWebView có hiện ảnh chụp của trang tài khoản cũ khi vuốt lùi sau khi đăng xuất không? Tức là ảnh của các mục pushState trước `/login`, vì `use-sign-out.ts:27` chỉ thay mục hiện tại. Cần kiểm trên iPhone thật khi cài lại; nếu có thì nên `window.location.replace("/login")` sau khi đăng xuất trên iOS.
- Ngoài phạm vi diff, đã có từ trước: `download.ts:19` gán `window.location.href` tới `/api/...`. Nếu server trả lỗi mà không kèm `Content-Disposition`, trình duyệt rời SPA, và bản ghi web đang chạy bị cắt. Có cần xử lý không?

Status: DONE_WITH_CONCERNS
Summary: Không có lỗi Critical/High; bản ghi (web và native), lượt cắt, upload và đồng bộ native sống qua đổi route, auth refresh và StrictMode như plan khẳng định. Còn 2 Medium (thiếu `errorElement` nên lỗi render cắt bản ghi; xoá transcript xong vuốt lùi mở lại bản đã xoá) và 7 Low.
