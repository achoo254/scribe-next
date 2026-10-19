# Scribe — hồ sơ App Store (bản nháp)

Bản nháp để dán vào App Store Connect khi tài khoản Apple Developer được duyệt. Cách tạo app, key và tải build: [ios-app.md](../ios-app.md#phát-hành-testflight--app-store). Ngôn ngữ chính của trang App Store: **Tiếng Việt**; thêm bản **English (U.S.)** bằng các dòng tiếng Anh dưới đây.

Ký tự đếm bằng số ký tự Unicode (App Store Connect đếm như vậy); số trong ngoặc là độ dài thật.

## Thông tin app

| Trường | Giá trị |
|---|---|
| Bundle ID | `com.hoandat.scribe` |
| SKU | `scribe-ios` |
| Tên (≤ 30) | `Scribe – Ghi âm & chép lời` (26) · EN: `Scribe – Meeting Transcripts` (28) |
| Phụ đề (≤ 30) | `Biên bản họp tiếng Việt` (23) · EN: `Record, transcribe, summarise` (29) |
| Danh mục chính / phụ | Productivity / Business |
| URL hỗ trợ | `https://scribe.hoandat.com/privacy#contact` (cần `VITE_SUPPORT_EMAIL` đã đặt để mục Liên hệ có email thật) |
| URL chính sách quyền riêng tư | `https://scribe.hoandat.com/privacy` |
| URL điều khoản (EULA tuỳ chỉnh, không bắt buộc) | `https://scribe.hoandat.com/terms` |
| Bản quyền | `2026 hoandat` |
| Giá | Miễn phí, không mua trong app (gói Plus do người vận hành cấp; trong app không có chỗ mua hay link mua — giữ như vậy để không vướng guideline 3.1.1) |
| Thiết bị | Chỉ iPhone (`TARGETED_DEVICE_FAMILY = 1`), iOS 15 trở lên |

### Văn bản quảng bá (≤ 170)

VI (128): `Ghi âm cuộc họp kể cả khi khoá màn hình, chép lời tiếng Việt có tên người nói, biên bản tự động, hỏi đáp và dịch sang tiếng Anh.`

EN (134): `Record meetings even with the screen locked, get Vietnamese transcripts with speakers, automatic minutes, Q&A and English translation.`

### Mô tả

VI:

```
Scribe ghi âm cuộc họp trên iPhone và chép lời tiếng Việt cho bạn.

GHI ÂM KHÔNG LO GIÁN ĐOẠN
• Ghi tiếp khi khoá màn hình hoặc chuyển sang app khác.
• Ghi theo từng đoạn và tải lên ngay, tắt app giữa chừng vẫn giữ được phần đã ghi.
• Đánh dấu khoảnh khắc quan trọng từ màn hình khoá hoặc tai nghe.
• Chép trực tiếp: xem chữ hiện ra ngay trong lúc họp.

CHÉP LỜI VÀ BIÊN BẢN
• Lời chép tiếng Việt có mốc thời gian và tên người nói; chạm một câu để nghe lại đúng đoạn đó.
• Biên bản tự động: tóm tắt, quyết định, việc cần làm, người làm và hạn.
• Sửa câu, đổi tên người nói, xuất TXT, SRT, VTT, Markdown hoặc Word.

TÌM LẠI MỌI THỨ
• Tìm trong mọi bản ghi, gõ có dấu hay không dấu đều được.
• Hỏi Scribe bằng câu tự nhiên; câu trả lời kèm nguồn để nghe lại.
• Dịch lời chép Việt ↔ Anh, xem song song từng câu.

RIÊNG TƯ
• Không quảng cáo, không theo dõi.
• Tệp tự xoá sau 7 ngày (gói miễn phí) hoặc 14 ngày (gói Plus).
• Xoá tài khoản và mọi dữ liệu ngay trong app.
• Có thể dùng key API của chính bạn (OpenAI, Gemini, Claude, ElevenLabs).

Đăng nhập bằng tài khoản hoandat. Hãy xin phép mọi người trước khi ghi âm.
```

EN:

```
Scribe records your meetings on iPhone and transcribes them in Vietnamese.

RECORD WITHOUT WORRY
• Keeps recording with the screen locked or while you use other apps.
• Records in segments and uploads as it goes, so nothing is lost if the app is closed.
• Bookmark key moments from the lock screen or your headphones.
• Live transcript: see the words as people speak.

TRANSCRIPTS AND MINUTES
• Vietnamese transcripts with timestamps and speakers; tap a sentence to hear it again.
• Automatic minutes: summary, decisions, action items with owners and due dates.
• Edit sentences, rename speakers, export TXT, SRT, VTT, Markdown or Word.

FIND ANYTHING
• Search every recording, with or without Vietnamese accents.
• Ask Scribe in plain words; answers cite the moments to replay.
• Translate transcripts between Vietnamese and English, side by side.

PRIVATE
• No ads, no tracking.
• Files are deleted after 7 days (free plan) or 14 days (Plus).
• Delete your account and all your data in the app.
• Bring your own API key (OpenAI, Gemini, Claude, ElevenLabs).

Sign in with a hoandat account. Ask everyone's permission before you record.
```

### Từ khoá (≤ 100, ngăn bằng dấu phẩy, không lặp tên app)

VI (95): `ghi âm,chép lời,biên bản,cuộc họp,họp,phiên âm,chuyển giọng nói,dịch,tóm tắt,tiếng Việt,ghi chú`

EN (94): `transcribe,transcription,recorder,meeting,minutes,notes,vietnamese,translate,summary,dictation`

## Phân loại độ tuổi (Age Rating)

Kết quả mong đợi: **4+**. Trả lời bảng câu hỏi:

| Câu hỏi | Trả lời |
|---|---|
| Bạo lực (hoạt hình, thực tế, kéo dài), nội dung tình dục, khoả thân, ngôn từ thô tục, kinh dị, thuốc, rượu, thuốc lá, mô phỏng cờ bạc, chủ đề y tế, nội dung người lớn | None |
| Cờ bạc bằng tiền thật, cuộc thi | No |
| Trình duyệt web không giới hạn | No (link ngoài mở trong Safari, app không có trình duyệt) |
| Nội dung do người dùng tạo được chia sẻ, chat giữa người dùng | No (nội dung chỉ chủ tài khoản thấy; gửi biên bản sang chat riêng của chính người dùng) |
| Quảng cáo | No |
| Kiểm soát của phụ huynh, xác minh tuổi | No |
| Made for Kids | No |

## App Privacy ("nhãn dinh dưỡng")

Khớp `web/ios/App/App/PrivacyInfo.xcprivacy` và `/privacy`. **Không theo dõi (tracking): No** cho mọi loại. Mọi loại dưới đây: thu thập = Yes, liên kết với danh tính = **Yes**, mục đích = **App Functionality** (gồm bảo mật và chống lạm dụng).

| Nhóm App Store Connect | Loại | Ghi chú |
|---|---|---|
| Contact Info | Name | Tên hiển thị từ tài khoản hoandat |
| Contact Info | Email Address | Email tài khoản hoandat |
| User Content | Audio Data | Bản ghi, tệp âm thanh tải lên; gửi ElevenLabs để chép lời |
| User Content | Photos or Videos | Video người dùng tự chọn hoặc quay để tải lên |
| User Content | Other User Content | Lời chép, câu sửa, tên người nói, biên bản, dấu mốc, bản dịch, Key riêng (mã hoá) |
| Identifiers | User ID | Mã tài khoản hoandat |
| Usage Data | Product Interaction | Nhật ký bảo mật hành động (đăng nhập, tải lên, xoá), số liệu mức dùng |

Không thu thập: Location, Contacts, Health, Financial, Browsing/Search History (câu tìm kiếm không lưu), Sensitive Info, Diagnostics, Purchases, Device ID.

## Ghi chú cho App Review

Dán vào **App Review Information → Notes** (tiếng Anh, người duyệt đọc tiếng Anh):

```
Scribe records meetings and transcribes Vietnamese speech.

Sign-in: tap "Đăng nhập bằng hoandat" (Sign in with hoandat). hoandat is our own account
service (auth.hoandat.com) shared by our apps. Use the demo account in the Sign-In
Information fields. It is on the Plus plan and already holds a transcribed sample meeting,
so every feature can be tried without recording.

Background audio (UIBackgroundModes: audio): recording continues while the screen is locked,
which is the core feature (meetings are long and phones lock). The app records only after the
user taps the record button, shows the elapsed time on the lock screen (Now Playing), and stops
when the user taps Stop. While recording, the app loops one second of silence only so iOS keeps
the lock-screen entry used to bookmark moments; nothing audible is played.

Microphone: used only while recording. Camera / Photos: only when the user chooses to film or
pick a video to upload for transcription.

Account deletion (5.1.1(v)): account menu (avatar, top right) → "Xoá tài khoản" (Delete
account) → type XOÁ → "Xoá tài khoản". All Scribe data is deleted at once. The shared hoandat
account itself can be deleted at auth.hoandat.com/account (linked in the same dialog).

Privacy policy and terms: account menu → "Quyền riêng tư" / "Điều khoản", also on the sign-in
page and at https://scribe.hoandat.com/privacy and /terms.

The interface is in Vietnamese. Main tabs: Ghi âm (Record), Cắt file (Split file),
Lời đã chép (Transcripts).
```

**Tài khoản demo (không bao giờ ghi vào repo):** đề xuất một tài khoản hoandat riêng cho người duyệt, ví dụ email `<review email do chủ dự án tạo>`, đăng nhập được không cần thiết bị thứ hai (không 2FA bằng điện thoại, không bị chặn khi đăng nhập từ máy lạ ở Mỹ). Thông tin đăng nhập chỉ nhập vào ô **Sign-In Information** của App Store Connect. Trước khi gửi duyệt:

1. Tạo tài khoản đó trên hoandat-auth, đăng nhập Scribe một lần.
2. `scribe-be admin set-plan <email> plus --until <ngày sau đợt duyệt>` để người duyệt không chạm hạn mức free.
3. Tải lên một bản ghi mẫu (vài phút, tiếng Việt), bấm Chép lời, Tóm tắt, Dịch sang tiếng Anh — để mọi màn hình có dữ liệu như ảnh chụp.
4. Sau đợt duyệt: đổi mật khẩu hoặc xoá tài khoản đó.

Rủi ro cần chủ dự án quyết: hoandat-auth hiện đăng nhập bằng Google hoặc GitHub. Guideline 4.8 yêu cầu một lựa chọn đăng nhập bảo vệ quyền riêng tư (ví dụ Sign in with Apple) khi app dùng đăng nhập mạng xã hội cho tài khoản chính, trừ khi app chỉ dùng hệ thống tài khoản của chính công ty. Lập luận ở ghi chú trên là "hoandat là hệ thống tài khoản riêng"; nếu bị từ chối theo 4.8 thì cần thêm Sign in with Apple (hoặc đăng nhập bằng email) vào hoandat-auth.

## Ảnh chụp màn hình

Tạo bằng `cd web && pnpm run screenshots:app-store` (Playwright, giao diện app iPhone với dữ liệu demo trong `web/e2e/app-store/demo-fixtures.ts`, theme sáng, PNG RGB không kênh alpha). Kết quả ở `docs/app-store/screenshots/<cỡ>/`:

| Thư mục | Nhóm App Store Connect | Kích thước |
|---|---|---|
| `6.9/` | iPhone 6.9" (Dynamic Island, lớn) | 1320 × 2868 |
| `6.5/` | iPhone 6.5" (Face ID, lớn) | 1284 × 2778 |
| `6.3/` | iPhone 6.3" (Dynamic Island, vừa) — trang yêu cầu của Apple (kiểm 19/10/2026) ghi đây là cỡ bắt buộc | 1206 × 2622 |

Thứ tự tải lên (giống nhau mọi cỡ):

1. `01-ghi-am` — đang ghi 25:23, Chép trực tiếp hiện câu. Chú thích gợi ý: "Ghi cả khi khoá màn hình".
2. `02-loi-chep` — lời chép có tên người nói và dấu mốc. "Lời chép tiếng Việt, rõ ai nói gì".
3. `03-bien-ban` — biên bản: tóm tắt, quyết định, việc cần làm. "Biên bản tự động sau mỗi buổi họp".
4. `04-hoi-scribe` — tìm kiếm và Hỏi Scribe có trích nguồn. "Hỏi lại mọi cuộc họp".
5. `05-ban-dich` — song song Việt ↔ Anh. "Dịch sang tiếng Anh từng câu".

Ảnh là giao diện thật của app (bản web trong vỏ iOS) với dữ liệu giả; khác máy thật ở chỗ không có thanh trạng thái iOS. Không có ảnh iPad vì app chỉ chạy trên iPhone.
