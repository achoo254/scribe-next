import AVFoundation
import Combine
import UIKit

/// Engine ghi âm nền của spike — bản thu nhỏ của thiết kế dự kiến cho app iOS thật:
/// AVAudioSession (background audio) + AVAudioEngine tap → ghi thẳng AAC/m4a, **xoay đoạn theo phút**
/// để mỗi đoạn hoàn chỉnh là một file upload được ngay (crash-safe, khớp tính năng "Cắt theo phút").
///
/// Spike CỐ TÌNH dùng AVAudioEngine chứ không phải AVAudioRecorder: xoay đoạn không hở tiếng, và đây
/// đúng là phần dễ vỡ nhất (gián đoạn cuộc gọi, đổi route Bluetooth, media services reset) — cần đo thật.
///
/// Giới hạn đã biết của spike: ghi file ngay trên audio render thread (đơn giản, đúng thông lệ sample
/// code). App thật nên đẩy I/O sang serial queue riêng nếu đo thấy xoay đoạn gây glitch — log có ghi
/// thời gian từng lần xoay để đánh giá.
final class Recorder: ObservableObject {

    struct Segment: Identifiable {
        let id = UUID()
        let index: Int
        let url: URL
        /// Số frame đã ghi (đếm trong lúc chạy).
        var frames: AVAudioFramePosition = 0
        var sampleRate: Double = 0
        var bytes: Int64 = 0
        /// Thời lượng đọc lại từ chính file sau khi dừng — phép kiểm "file có nguyên không".
        var probedDuration: Double?

        var expectedDuration: Double { sampleRate > 0 ? Double(frames) / sampleRate : 0 }
        /// Lệch > 0.5s giữa số frame đã ghi và thời lượng đọc lại = file hỏng / mất đuôi.
        var isIntact: Bool? {
            guard let p = probedDuration else { return nil }
            return abs(p - expectedDuration) <= 0.5
        }
    }

    enum State: String { case idle, recording, stopping }

    @Published private(set) var state: State = .idle
    @Published private(set) var elapsed: TimeInterval = 0
    @Published private(set) var level: Float = 0
    @Published private(set) var segments: [Segment] = []
    @Published private(set) var interruptions = 0
    @Published private(set) var routeChanges = 0
    @Published private(set) var engineRestarts = 0
    @Published private(set) var writeErrors = 0
    @Published private(set) var batteryStart: Float = -1
    @Published private(set) var batteryNow: Float = -1
    @Published private(set) var thermal = "—"
    /// Sạc hay không — bắt buộc phải biết, nếu không thì con số pin vô nghĩa.
    @Published private(set) var powerState = "—"
    @Published private(set) var lastError = ""
    /// Độ dài mỗi đoạn (phút). 1 phút để test nhanh, 5–10 phút cho phiên 30 phút thật.
    @Published var segmentMinutes: Double = 5
    /// Ép dùng micro tích hợp của iPhone. Tắt để test kịch bản Bluetooth (AirPods) một cách chủ ý —
    /// mặc định bật vì iOS sẵn sàng chọn bất kỳ thiết bị HFP nào đang ghép đôi (kể cả đồng hồ) làm mic.
    @Published var preferBuiltInMic = true

    private let engine = AVAudioEngine()
    private let log = EventLog.shared
    /// Bảo vệ `file` / `currentFrames` giữa audio render thread và main thread (start/stop/gián đoạn).
    private let fileLock = NSLock()

    private var file: AVAudioFile?
    private var currentIndex = -1
    private var currentFrames: AVAudioFramePosition = 0
    private var currentRate: Double = 0
    /// Bản chụp của `segmentMinutes` lúc bắt đầu — audio thread không đọc @Published.
    private var segmentLimitSeconds: Double = 300
    private var sessionDir: URL?
    private var tapInstalled = false

    private var uiTimer: Timer?
    private var statsTimer: Timer?
    private var startedAt: Date?
    private var pausedTotal: TimeInterval = 0
    private var pausedAt: Date?
    private var levelTick = 0

    // MARK: - Vòng đời

    init() {
        UIDevice.current.isBatteryMonitoringEnabled = true
        observeSystemEvents()
    }

    func start() {
        guard state == .idle else { return }
        lastError = ""
        requestPermission { [weak self] granted in
            guard let self else { return }
            if granted { self.beginSession() } else { self.fail("Người dùng từ chối quyền micro") }
        }
    }

    func stop() {
        guard state == .recording else { return }
        state = .stopping
        log.log("STOP — người dùng bấm dừng")

        stopEngineChain()
        closeSegment(reason: "stop")

        uiTimer?.invalidate(); uiTimer = nil
        statsTimer?.invalidate(); statsTimer = nil

        do {
            try AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        } catch {
            log.log("WARN không deactivate được session: \(error.localizedDescription)")
        }

        probeSegments()
        sampleStats()
        level = 0
        state = .idle
        log.log("=== phiên kết thúc: \(segments.count) đoạn, tổng \(String(format: "%.1f", elapsed))s ===")
    }

    // MARK: - Khởi động

    private func requestPermission(_ done: @escaping (Bool) -> Void) {
        let hand: (Bool) -> Void = { granted in DispatchQueue.main.async { done(granted) } }
        if #available(iOS 17.0, *) {
            AVAudioApplication.requestRecordPermission(completionHandler: hand)
        } else {
            AVAudioSession.sharedInstance().requestRecordPermission(hand)
        }
    }

    private func beginSession() {
        segments = []
        currentIndex = -1
        sessionDir = nil
        segmentLimitSeconds = max(10, segmentMinutes * 60)
        attemptStart(attempt: 1)
    }

    /// Đổi route (Bluetooth vào/ra, override `.defaultToSpeaker`) xảy ra bất đồng bộ ngay sau khi kích
    /// hoạt session → format của input node có thể chưa ổn định ở lần thử đầu. Thử lại một lần sau 0.5s
    /// thay vì bắt người dùng bấm lại — một phiên đo 30 phút hỏng vì lỗi này là quá đắt.
    private func attemptStart(attempt: Int) {
        do {
            try configureAudioSession()
            if sessionDir == nil { sessionDir = try makeSessionDir() }
            try startEngineChain()
        } catch {
            stopEngineChain()
            fileLock.lock(); file = nil; currentIndex = -1; fileLock.unlock()
            segments = []
            if attempt == 1 {
                log.log("WARN khởi động lỗi lần 1: \(error.localizedDescription) → thử lại sau 0.5s")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                    self?.attemptStart(attempt: 2)
                }
                return
            }
            fail("Không bắt đầu ghi được: \(error.localizedDescription)")
            return
        }

        startedAt = Date()
        pausedTotal = 0
        pausedAt = nil
        elapsed = 0
        interruptions = 0; routeChanges = 0; engineRestarts = 0; writeErrors = 0
        state = .recording

        UIDevice.current.isBatteryMonitoringEnabled = true
        batteryStart = UIDevice.current.batteryLevel
        batteryNow = batteryStart
        sampleStats()

        uiTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.tickElapsed()
        }
        // Mẫu pin/nhiệt mỗi 60s — cơ sở đánh giá chi phí pin của phiên 30 phút.
        statsTimer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { [weak self] _ in
            self?.sampleStats()
        }
        log.log("START — segment \(Int(segmentMinutes)) phút, thư mục \(sessionDir?.lastPathComponent ?? "?")")
    }

    private func configureAudioSession() throws {
        let s = AVAudioSession.sharedInstance()
        // .playAndRecord (không phải .record) vì app thật còn phải phát lại bản ghi; đây cũng là ca khó
        // hơn nên đáng kiểm chứng ngay ở spike.
        try s.setCategory(.playAndRecord,
                          mode: .default,
                          options: [.allowBluetoothHFP, .allowBluetoothA2DP, .defaultToSpeaker])
        try s.setActive(true)
        // iOS sẵn sàng chọn BẤT KỲ thiết bị HFP nào đang ghép đôi làm mic — kể cả đồng hồ thông minh
        // (đo thực tế: HUAWEI WATCH FIT 4 chiếm input ở 16kHz). Ép về mic tích hợp cho phép đo cơ sở.
        if preferBuiltInMic, let builtIn = s.availableInputs?.first(where: { $0.portType == .builtInMic }) {
            try? s.setPreferredInput(builtIn)
            log.log("INPUT ép về micro tích hợp")
        }
        let route = s.currentRoute.inputs.map(\.portName).joined(separator: ", ")
        let available = (s.availableInputs ?? []).map(\.portName).joined(separator: " | ")
        log.log("SESSION active — \(Int(s.sampleRate))Hz, in=\(s.inputNumberOfChannels)ch, route=[\(route)]")
        log.log("INPUT khả dụng: \(available)")
    }

    private func makeSessionDir() throws -> URL {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let dir = docs.appendingPathComponent("recordings/\(Self.stampNow())", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [
            // Gotcha kinh điển: mức bảo vệ .complete sẽ KHÔNG ghi được khi máy đang khoá.
            // .completeUntilFirstUserAuthentication vẫn ghi được sau lần mở khoá đầu tiên sau reboot.
            .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication,
        ])
        return dir
    }

    // MARK: - Engine + tap

    private func startEngineChain() throws {
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else {
            throw NSError(domain: "spike", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "input format không hợp lệ (\(format))"])
        }
        log.log("INPUT format \(Int(format.sampleRate))Hz \(format.channelCount)ch")
        try openSegment(format: format)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            self?.process(buffer)
        }
        tapInstalled = true
        engine.prepare()
        try engine.start()
        log.log("ENGINE start — tap \(Int(format.sampleRate))Hz \(format.channelCount)ch")
    }

    private func stopEngineChain() {
        if tapInstalled {
            engine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        if engine.isRunning { engine.stop() }
    }

    /// Dựng lại toàn bộ chuỗi engine (sau gián đoạn / đổi route / media services reset).
    /// Luôn mở đoạn MỚI vì sample rate hoặc số kênh có thể đã đổi (vd cắm AirPods → HFP 16kHz).
    private func restartEngineChain(reason: String) {
        guard state == .recording else { return }
        stopEngineChain()
        closeSegment(reason: reason)
        do {
            try configureAudioSession()
            try startEngineChain()
            engineRestarts += 1
            log.log("ENGINE restart OK (\(reason)) — lần thứ \(engineRestarts)")
        } catch {
            fail("ENGINE restart THẤT BẠI (\(reason)): \(error.localizedDescription)")
        }
    }

    // MARK: - Đoạn ghi

    private func openSegment(format: AVAudioFormat) throws {
        guard let dir = sessionDir else { return }
        let index = currentIndex + 1
        let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
        let f = try makeAACFile(url: url, format: format)

        fileLock.lock()
        file = f
        currentFrames = 0
        currentRate = format.sampleRate
        currentIndex = index
        fileLock.unlock()

        let seg = Segment(index: index, url: url, sampleRate: format.sampleRate)
        onMain { self.segments.append(seg) }
        log.log("SEG open #\(index) — \(Int(format.sampleRate))Hz \(format.channelCount)ch")
    }

    /// Tạo file AAC khớp format của tap. AVAudioFile tự encode PCM → AAC; sample rate và số kênh phải
    /// khớp format tap, nếu không `processingFormat` lệch và `write(from:)` ném lỗi.
    ///
    /// Bitrate phải nằm trong dải AAC cho phép ứng với (sample rate × số kênh). 128 kbps hợp lệ ở
    /// 44.1/48kHz nhưng bị từ chối ở 16kHz mono (route HFP Bluetooth) — encoder trả
    /// `kAudioCodecUnsupportedFormatError` ('!dat'). Vì vậy tính bitrate theo sample rate, và nếu vẫn
    /// bị từ chối thì bỏ hẳn key để encoder tự chọn.
    private func makeAACFile(url: URL, format: AVAudioFormat) throws -> AVAudioFile {
        var settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: format.sampleRate,
            AVNumberOfChannelsKey: format.channelCount,
        ]
        let bitRate = Self.aacBitRate(sampleRate: format.sampleRate, channels: format.channelCount)
        settings[AVEncoderBitRateKey] = bitRate
        do {
            return try AVAudioFile(forWriting: url, settings: settings)
        } catch {
            log.log("WARN AAC từ chối \(bitRate)bps @\(Int(format.sampleRate))Hz "
                    + "(\(error.localizedDescription)) → để encoder tự chọn bitrate")
            settings.removeValue(forKey: AVEncoderBitRateKey)
            return try AVAudioFile(forWriting: url, settings: settings)
        }
    }

    static func aacBitRate(sampleRate: Double, channels: AVAudioChannelCount) -> Int {
        let perChannel: Int
        switch sampleRate {
        case ..<16_001: perChannel = 32_000   // HFP Bluetooth (8/16kHz)
        case ..<24_001: perChannel = 48_000
        case ..<32_001: perChannel = 64_000
        default: perChannel = 96_000          // mic tích hợp (48kHz)
        }
        return perChannel * Int(max(1, channels))
    }

    private func closeSegment(reason: String) {
        fileLock.lock()
        let frames = currentFrames
        let index = currentIndex
        let rate = currentRate
        file = nil // AVAudioFile flush + đóng file khi nhả tham chiếu
        currentFrames = 0
        fileLock.unlock()

        guard index >= 0 else { return }
        let dur = rate > 0 ? Double(frames) / rate : 0
        // Chạy đồng bộ khi đã ở main (stop/gián đoạn) để probeSegments() ngay sau đó thấy số frame đúng.
        onMain {
            guard let i = self.segments.firstIndex(where: { $0.index == index }) else { return }
            self.segments[i].frames = frames
            let size = Self.fileSize(self.segments[i].url)
            self.segments[i].bytes = size
            self.log.log("SEG close #\(index) (\(reason)) — \(String(format: "%.1f", dur))s, \(size) bytes")
        }
    }

    /// Cập nhật state cho UI: đồng bộ nếu đang ở main thread, async nếu đang ở audio render thread.
    /// Giữ đúng thứ tự các thao tác trên `segments` giữa hai luồng gọi.
    private func onMain(_ work: @escaping () -> Void) {
        if Thread.isMainThread { work() } else { DispatchQueue.main.async(execute: work) }
    }

    /// Xoay sang đoạn mới. Đo luôn thời gian xoay: đây là lúc duy nhất có syscall mở file trên audio
    /// thread → tốn nhiều ms thì app thật phải đẩy I/O sang queue riêng.
    private func rotate(format: AVAudioFormat, reason: String) {
        let t0 = CFAbsoluteTimeGetCurrent()
        closeSegment(reason: reason)
        do {
            try openSegment(format: format)
        } catch {
            fileLock.lock(); file = nil; fileLock.unlock()
            log.log("ERROR mở đoạn mới thất bại: \(error.localizedDescription)")
            DispatchQueue.main.async { self.lastError = "Không mở được đoạn mới: \(error.localizedDescription)" }
            return
        }
        log.log("SEG rotate mất \(String(format: "%.1f", (CFAbsoluteTimeGetCurrent() - t0) * 1000))ms")
    }

    // MARK: - Audio render thread

    private func process(_ buffer: AVAudioPCMBuffer) {
        updateLevel(buffer)

        fileLock.lock()
        guard let target = file else {
            fileLock.unlock()
            return
        }
        if buffer.format != target.processingFormat {
            fileLock.unlock()
            log.log("WARN format buffer khác processingFormat → xoay đoạn")
            rotate(format: buffer.format, reason: "format-change")
            return
        }
        var needRotate = false
        do {
            try target.write(from: buffer)
            currentFrames += AVAudioFramePosition(buffer.frameLength)
            if currentRate > 0, Double(currentFrames) / currentRate >= segmentLimitSeconds { needRotate = true }
        } catch {
            fileLock.unlock()
            DispatchQueue.main.async { self.writeErrors += 1 }
            log.log("ERROR write: \(error.localizedDescription)")
            return
        }
        fileLock.unlock()

        if needRotate { rotate(format: buffer.format, reason: "đủ \(Int(segmentLimitSeconds / 60)) phút") }
    }

    private func updateLevel(_ buffer: AVAudioPCMBuffer) {
        guard let data = buffer.floatChannelData, buffer.frameLength > 0 else { return }
        var sum: Float = 0
        let n = Int(buffer.frameLength)
        for i in 0..<n {
            let v = data[0][i]
            sum += v * v
        }
        let rms = (sum / Float(n)).squareRoot()
        levelTick += 1
        guard levelTick % 6 == 0 else { return } // ~10 lần/giây là đủ cho UI
        DispatchQueue.main.async { self.level = rms }
    }

    // MARK: - Sự kiện hệ thống

    private func observeSystemEvents() {
        let nc = NotificationCenter.default
        _ = nc.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] n in
            self?.handleInterruption(n)
        }
        _ = nc.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] n in
            self?.handleRouteChange(n)
        }
        _ = nc.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("MEDIA SERVICES RESET")
            self?.restartEngineChain(reason: "media-services-reset")
        }
        // Bằng chứng máy đã THỰC SỰ khoá màn hình (không chỉ là chuyển app).
        _ = nc.addObserver(forName: UIApplication.protectedDataWillBecomeUnavailableNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("LOCK — màn hình khoá (protected data unavailable)")
        }
        _ = nc.addObserver(forName: UIApplication.protectedDataDidBecomeAvailableNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("UNLOCK — protected data available")
        }
        _ = nc.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("APP → background")
        }
        _ = nc.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("APP → foreground")
        }
        _ = nc.addObserver(forName: UIApplication.willTerminateNotification, object: nil, queue: .main) { [weak self] _ in
            self?.log.log("APP willTerminate — hệ thống kill app")
        }
    }

    private func handleInterruption(_ n: Notification) {
        guard let raw = n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        switch type {
        case .began:
            guard state == .recording else { return }
            interruptions += 1
            pausedAt = Date()
            log.log("INTERRUPT began (lần \(interruptions)) — cuộc gọi/Siri/app khác chiếm mic")
            stopEngineChain()
            closeSegment(reason: "interrupt")
        case .ended:
            guard state == .recording else { return }
            let opts = AVAudioSession.InterruptionOptions(
                rawValue: n.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0)
            log.log("INTERRUPT ended — shouldResume=\(opts.contains(.shouldResume))")
            if let p = pausedAt {
                pausedTotal += Date().timeIntervalSince(p)
                pausedAt = nil
            }
            if !opts.contains(.shouldResume) {
                log.log("WARN hệ thống KHÔNG gợi ý resume — vẫn thử dựng lại engine")
            }
            restartEngineChain(reason: "interrupt-resume")
        @unknown default:
            break
        }
    }

    private func handleRouteChange(_ n: Notification) {
        guard let raw = n.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw) else { return }
        routeChanges += 1
        let route = AVAudioSession.sharedInstance().currentRoute.inputs.map(\.portName).joined(separator: ", ")
        log.log("ROUTE change #\(routeChanges) reason=\(reason.rawValue) → in=[\(route)]")
        guard state == .recording else { return }
        // Chỉ dựng lại khi thiết bị input đổi thật (rút/cắm AirPods, tai nghe) — tránh restart thừa.
        if reason == .oldDeviceUnavailable || reason == .newDeviceAvailable {
            restartEngineChain(reason: "route-change")
        }
    }

    // MARK: - Đo đạc

    private func tickElapsed() {
        guard let s = startedAt else { return }
        let paused = pausedTotal + (pausedAt.map { Date().timeIntervalSince($0) } ?? 0)
        elapsed = Date().timeIntervalSince(s) - paused
    }

    private func sampleStats() {
        batteryNow = UIDevice.current.batteryLevel
        thermal = Self.thermalName(ProcessInfo.processInfo.thermalState)
        powerState = Self.powerName(UIDevice.current.batteryState)
        // In 2 chữ số thập phân: batteryLevel thường chỉ có độ phân giải 1%, nhưng ghi âm nền tốn
        // dưới 1%/giờ nên cần thấy được mọi bước nhảy nhỏ nhất mà API cấp.
        let pct = batteryNow < 0 ? "n/a" : String(format: "%.2f%%", batteryNow * 100)
        let lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled ? " tiếtKiệmPin=BẬT" : ""
        log.log("STATS pin=\(pct) nguồn=\(powerState)\(lowPower) nhiệt=\(thermal) "
                + "elapsed=\(Int(elapsed))s đoạn=\(segments.count) engineRunning=\(engine.isRunning)")
    }

    private static func powerName(_ s: UIDevice.BatteryState) -> String {
        switch s {
        case .unplugged: return "KHÔNG-sạc"
        case .charging: return "ĐANG-SẠC"
        case .full: return "SẠC-đầy"
        case .unknown: return "?"
        @unknown default: return "?"
        }
    }

    /// Sau khi dừng: đọc lại từng file bằng AVAudioFile (parse container thật) và đối chiếu thời lượng
    /// với số frame đã ghi. Đây là phép kiểm "file có nguyên không" của Phase 0.
    private func probeSegments() {
        for (i, seg) in segments.enumerated() {
            var probed = 0.0
            if let f = try? AVAudioFile(forReading: seg.url), f.processingFormat.sampleRate > 0 {
                probed = Double(f.length) / f.processingFormat.sampleRate
            }
            segments[i].probedDuration = probed
            segments[i].bytes = Self.fileSize(seg.url)
            let verdict = segments[i].isIntact == true ? "OK" : "LỆCH"
            log.log("PROBE #\(seg.index) file=\(String(format: "%.1f", probed))s "
                    + "ghi=\(String(format: "%.1f", segments[i].expectedDuration))s \(verdict)")
        }
    }

    private func fail(_ message: String) {
        lastError = message
        log.log("FAIL \(message)")
        stopEngineChain()
        state = .idle
    }

    // MARK: - Tiện ích

    static func fileSize(_ url: URL) -> Int64 {
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        return (attrs?[.size] as? NSNumber)?.int64Value ?? 0
    }

    static func stampNow() -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyyMMdd-HHmmss"
        return f.string(from: Date())
    }

    private static func thermalName(_ s: ProcessInfo.ThermalState) -> String {
        switch s {
        case .nominal: return "nominal"
        case .fair: return "fair"
        case .serious: return "serious"
        case .critical: return "critical"
        @unknown default: return "?"
        }
    }
}
