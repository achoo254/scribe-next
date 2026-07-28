# Phase 0 Spike — ghi âm nền khi khoá màn hình (iOS)

Mục đích **duy nhất**: chứng minh (hoặc bác bỏ) rằng iPhone ghi âm liên tục được khi **khoá màn hình**,
với **tài khoản Apple ID cá nhân miễn phí** — trước khi đầu tư vào app thật.

Đây là dụng cụ đo, **không phải sản phẩm**. Không có UI OneScribe, không có backend, không upload gì.
Xong việc thì xoá cả thư mục `ios-spike/`.

## Vì sao spike này tồn tại

Web (`web/`) không bao giờ ghi được khi khoá màn hình: iOS đình chỉ WebView và thu hồi mic.
`web/src/composables/use-recorder.ts` đã xử lý đúng cách duy nhất còn lại — phát hiện track `ended`
rồi dừng-và-lưu. Muốn ghi tiếp khi khoá màn hình thì bắt buộc phải là native.

Spike kiểm chứng đúng cơ chế mà app thật sẽ dùng:

| Cơ chế | Kiểm chứng điều gì |
|---|---|
| `UIBackgroundModes: [audio]` | Free personal team ký được, không cần trả $99 |
| `AVAudioSession(.playAndRecord)` | App không bị suspend khi khoá màn hình |
| `AVAudioEngine` tap → AAC/m4a | Xoay đoạn theo phút không hở tiếng (khớp tính năng "Cắt theo phút") |
| `interruptionNotification` | Cuộc gọi đến giữa chừng có làm hỏng file không |
| `routeChangeNotification` | Rút/cắm AirPods có làm chết engine không |
| `FileProtectionType` | Ghi file được khi máy đang khoá (gotcha kinh điển) |

## Cấu trúc

```
ios-spike/
  project.yml                     # XcodeGen — sinh .xcodeproj (không commit .xcodeproj)
  OneScribeSpike/
    App.swift                     # @main
    ContentView.swift             # UI đo đạc: nút Ghi/Dừng, chỉ số, danh sách đoạn, nhật ký
    Recorder.swift                # lõi: session + engine + xoay đoạn + xử lý gián đoạn
    EventLog.swift                # nhật ký ra file (đọc lại được sau khi máy khoá 30 phút)
    Info.plist                    # UIBackgroundModes=audio + NSMicrophoneUsageDescription
```

## Chạy trên Mac mini

Cần: macOS + **Xcode đầy đủ** (không phải Command Line Tools), iPhone thật + cáp.
Simulator **không dùng được** — không có mic thật, không khoá màn hình thật, không đo được pin.

```bash
brew install xcodegen          # nếu chưa có
cd ios-spike
xcodegen generate
open OneScribeSpike.xcodeproj
```

Trong Xcode:

1. Chọn target `OneScribeSpike` → tab **Signing & Capabilities**.
2. **Team** → chọn Apple ID cá nhân (`Add an Account…` nếu chưa có) → hiện `(Personal Team)`.
3. Nếu báo trùng bundle id: đổi **Bundle Identifier** thành `vn.inet.onescribe.spike.<tên-bạn>`.
4. Cắm iPhone, chọn nó làm run destination, bấm **Run**.
5. Lần đầu trên iPhone: **Cài đặt → Quyền riêng tư & Bảo mật → Chế độ nhà phát triển** → bật (máy khởi động lại),
   và **Cài đặt → Cài đặt chung → VPN & Quản lý thiết bị** → Tin cậy developer.

> Provisioning của tài khoản miễn phí **hết hạn sau 7 ngày** — app sẽ không mở được nữa,
> cắm lại vào Mac mini và Run lại là xong.

## Kịch bản đo

**Chạy chính (bắt buộc):**

1. Mở app, chọn **5 phút**/đoạn.
2. Bấm **Bắt đầu ghi**, nói vài câu để chắc mic sống.
3. **Bấm nút nguồn khoá màn hình** (không phải chỉ chờ tắt màn hình tự động — phải test khoá chủ động).
4. Để yên **30 phút**. Không mở app.
5. Mở khoá, mở app, bấm **Dừng ghi**.
6. Đọc kết quả trên màn hình + bấm nút chia sẻ (góc trên phải) để lấy file log.

**Chạy phụ (mỗi cái một lần, 5 phút là đủ):**

- **Cuộc gọi**: đang ghi → gọi vào máy → nghe 30 giây → cúp → chờ 1 phút → dừng.
- **AirPods**: đang ghi → cắm/kết nối AirPods → chờ 1 phút → ngắt → chờ 1 phút → dừng.

## Tiêu chí PASS / FAIL (đây là cái gate)

| # | Tiêu chí | PASS khi |
|---|---|---|
| 1 | **Ghi liên tục khi khoá** | `elapsed` ≈ 30:00 (lệch < 10s), có dòng `LOCK` trong log và **không** có `APP willTerminate` |
| 2 | **File nguyên vẹn** | Mọi đoạn hiện `file OK`. Không đoạn nào `LỆCH` |
| 3 | **Đủ số đoạn** | 30 phút / 5 phút = 6 đoạn (đoạn cuối ngắn hơn là bình thường) |
| 4 | **Không lỗi ghi** | `Lỗi ghi file` = 0 |
| 5 | **Sống sót gián đoạn** | Sau cuộc gọi: `Engine dựng lại` ≥ 1, các đoạn sau đó vẫn `file OK` |
| 6 | **Pin chấp nhận được** | Tụt ≤ 8% cho 30 phút. Nhiệt không lên `serious` |
| 7 | **Xoay đoạn không đắt** | Log `SEG rotate mất …ms` < 50ms (nếu cao hơn → app thật phải đẩy I/O sang queue riêng) |

**Tiêu chí 1 và 2 là bắt buộc.** Fail một trong hai → dừng toàn bộ hướng native hybrid, quay lại
bàn thiết kế. 3–7 fail thì chỉ là điều chỉnh tham số, không phải chặn.

## Lấy file ra khỏi máy

- **Trong app**: nút chia sẻ góc trên phải → gửi file `.log` qua AirDrop.
- **File ghi**: Files.app → *Trên iPhone của tôi* → `OneScribeSpike` → `recordings/<timestamp>/seg-*.m4a`.
- **Qua Mac**: Finder → chọn iPhone → tab *Files* → `OneScribeSpike`.

## Đọc log

```
[10:31:02.418] SESSION active — 48000Hz, in=1ch, route=[iPhone Microphone]
[10:31:02.501] ENGINE start — tap 48000Hz 1ch
[10:31:02.502] SEG open #0 — 48000Hz 1ch
[10:31:14.883] LOCK — màn hình khoá (protected data unavailable)   ← bằng chứng đã khoá thật
[10:31:14.884] APP → background
[10:32:02.110] STATS pin=87% nhiệt=nominal elapsed=60s đoạn=1 engineRunning=true
[10:36:02.114] SEG close #0 (đủ 5 phút) — 300.0s, 4812345 bytes
[10:36:02.117] SEG rotate mất 3.2ms
...
[11:01:20.005] PROBE #0 file=300.0s ghi=300.0s OK
```

Dòng cần soi: `LOCK`, `STATS … engineRunning=true` (phải `true` suốt phiên), `PROBE … OK`,
và tuyệt đối không được có `APP willTerminate` giữa chừng.

## Giới hạn đã biết của spike (cố ý)

- Ghi file **trên audio render thread** — đúng thông lệ sample code, đơn giản. Log đo thời gian xoay
  đoạn để biết app thật có cần đẩy I/O sang serial queue riêng không.
- Không upload, không auth, không UI OneScribe — những thứ đó là Phase 1–3, không thuộc gate này.
- Không có Live Activity (nút Dừng trên màn hình khoá) — cần Widget Extension, ăn quota App ID của
  tài khoản miễn phí. Đánh giá riêng sau khi gate này pass.
