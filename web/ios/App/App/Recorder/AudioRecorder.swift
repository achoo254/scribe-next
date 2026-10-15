import AVFoundation
import Foundation
import os

/// Background recording engine, ported from ios-spike (63 minutes measured
/// on the phone: locked screen, rotation in ~10 ms, 0 broken segments).
/// AVAudioSession (background audio) + AVAudioEngine tap → AAC/m4a written
/// directly, rotated every N minutes so each closed segment is a complete
/// file that can be uploaded at once. A segment still open when iOS kills
/// the app has no moov atom and is lost; the 1-minute default bounds that.
///
/// Does not import Capacitor: the UI layer subscribes through `onEvent`.
final class AudioRecorder {
    enum State: String { case idle, recording, stopping }

    enum Event {
        case state(State)
        /// Seconds recorded, excluding interruptions; about once a second.
        case elapsed(Double)
        /// RMS level 0…1 of the input, about ten times a second.
        case level(Float)
        /// A finished, readable segment of `sessionId`.
        case segmentClosed(sessionId: String, segment: SegmentStore.Segment, url: URL)
        /// The recording keeps going after an interruption or route change.
        case notice(String)
        /// Recording stopped by itself (permission, engine failure).
        case failure(String)
    }

    var onEvent: ((Event) -> Void)?

    private(set) var state: State = .idle
    private(set) var sessionId: String?
    private var sessionDir: URL?

    private let engine = AVAudioEngine()
    private let log = Logger(subsystem: "vn.inet.onescribe", category: "recorder")
    /// Guards `file`, `currentFrames` between the audio thread and main.
    private let fileLock = NSLock()

    private var file: AVAudioFile?
    private var currentIndex = -1
    private var currentFrames: AVAudioFramePosition = 0
    private var currentRate: Double = 0
    private var segmentLimitSeconds: Double = 60
    private var tapInstalled = false
    private var checkedBrand = false

    private var timer: Timer?
    private var startedAt: Date?
    private var pausedTotal: TimeInterval = 0
    private var pausedAt: Date?
    private var levelTick = 0
    private var observers: [NSObjectProtocol] = []

    init() {
        observeSystemEvents()
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
    }

    // MARK: - Start / stop

    /// Asks for the microphone, then records into `dir` (created by the
    /// store) as `seg-000.m4a`, `seg-001.m4a`…
    func start(sessionId: String, dir: URL, segmentSeconds: Double) {
        guard state == .idle else { return }
        self.sessionId = sessionId
        sessionDir = dir
        segmentLimitSeconds = max(10, segmentSeconds)
        currentIndex = -1
        checkedBrand = false
        requestPermission { [weak self] granted in
            guard let self else { return }
            if granted {
                self.attemptStart(attempt: 1)
            } else {
                self.fail("Ứng dụng chưa được cấp quyền Micro — vào Cài đặt → OneScribe → Micro để bật.")
            }
        }
    }

    func stop() {
        guard state == .recording else { return }
        setState(.stopping)
        stopEngineChain()
        closeSegment(reason: "stop")
        timer?.invalidate()
        timer = nil
        do {
            try AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        } catch {
            log.warning("deactivate session: \(error.localizedDescription, privacy: .public)")
        }
        onEvent?(.level(0))
        sessionId = nil
        sessionDir = nil
        setState(.idle)
    }

    private func requestPermission(_ done: @escaping (Bool) -> Void) {
        let reply: (Bool) -> Void = { granted in DispatchQueue.main.async { done(granted) } }
        if #available(iOS 17.0, *) {
            AVAudioApplication.requestRecordPermission(completionHandler: reply)
        } else {
            AVAudioSession.sharedInstance().requestRecordPermission(reply)
        }
    }

    /// Route changes (Bluetooth in/out, speaker override) land asynchronously
    /// right after the session activates, so the input format may not be
    /// stable on the first try: retry once after 0.5 s (spike lesson 3).
    private func attemptStart(attempt: Int) {
        do {
            try configureAudioSession()
            try startEngineChain()
        } catch {
            stopEngineChain()
            fileLock.lock()
            file = nil
            currentIndex = -1
            fileLock.unlock()
            if attempt == 1 {
                log.warning("start failed once: \(error.localizedDescription, privacy: .public); retrying")
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
        setState(.recording)
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.tickElapsed()
        }
    }

    private func configureAudioSession() throws {
        let session = AVAudioSession.sharedInstance()
        // .playAndRecord: the app also plays recordings back.
        try session.setCategory(.playAndRecord, mode: .default,
                                options: [.allowBluetoothHFP, .allowBluetoothA2DP, .defaultToSpeaker])
        try session.setActive(true)
        // iOS picks ANY paired HFP device as the mic, even a smartwatch
        // (measured: a watch took the input at 16 kHz). Prefer the built-in
        // mic (spike lesson 2).
        if let builtIn = session.availableInputs?.first(where: { $0.portType == .builtInMic }) {
            try? session.setPreferredInput(builtIn)
        }
    }

    // MARK: - Engine

    private func startEngineChain() throws {
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else {
            throw RecorderError("Định dạng micro không hợp lệ.")
        }
        try openSegment(format: format)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            self?.process(buffer)
        }
        tapInstalled = true
        engine.prepare()
        try engine.start()
    }

    private func stopEngineChain() {
        if tapInstalled {
            engine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        if engine.isRunning { engine.stop() }
    }

    /// Rebuilds the chain after an interruption, route change or media
    /// services reset. Always opens a NEW segment: rate or channels may have
    /// changed (AirPods → HFP 16 kHz).
    private func restartEngineChain(reason: String) {
        guard state == .recording else { return }
        stopEngineChain()
        closeSegment(reason: reason)
        do {
            try configureAudioSession()
            try startEngineChain()
            onEvent?(.notice("Ghi tiếp sau gián đoạn (\(reason))."))
        } catch {
            fail("Không ghi tiếp được sau gián đoạn: \(error.localizedDescription)")
        }
    }

    // MARK: - Segments

    private func openSegment(format: AVAudioFormat) throws {
        guard let dir = sessionDir else { throw RecorderError("Chưa có thư mục phiên ghi.") }
        let index = currentIndex + 1
        let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
        let newFile = try makeAACFile(url: url, format: format)
        fileLock.lock()
        file = newFile
        currentFrames = 0
        currentRate = format.sampleRate
        currentIndex = index
        fileLock.unlock()
    }

    /// AAC bitrate must fit (sample rate × channels): 128 kbps is rejected at
    /// 16 kHz mono (HFP) with '!dat' (spike lesson 1). Pick by rate, and if
    /// the encoder still refuses, let it choose.
    private func makeAACFile(url: URL, format: AVAudioFormat) throws -> AVAudioFile {
        var settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: format.sampleRate,
            AVNumberOfChannelsKey: format.channelCount,
            AVEncoderBitRateKey: Self.aacBitRate(sampleRate: format.sampleRate, channels: format.channelCount),
        ]
        do {
            return try AVAudioFile(forWriting: url, settings: settings)
        } catch {
            settings.removeValue(forKey: AVEncoderBitRateKey)
            return try AVAudioFile(forWriting: url, settings: settings)
        }
    }

    static func aacBitRate(sampleRate: Double, channels: AVAudioChannelCount) -> Int {
        let perChannel: Int
        switch sampleRate {
        case ..<16_001: perChannel = 32_000
        case ..<24_001: perChannel = 48_000
        case ..<32_001: perChannel = 64_000
        default: perChannel = 96_000
        }
        return perChannel * Int(max(1, channels))
    }

    /// Releases the file (AVAudioFile writes the moov atom on release) and
    /// reports the finished segment.
    private func closeSegment(reason: String) {
        fileLock.lock()
        let hadFile = file != nil
        let frames = currentFrames
        let index = currentIndex
        let rate = currentRate
        // The only reference: releasing it writes the moov atom, so the file
        // is complete on disk once the lock is released.
        file = nil
        currentFrames = 0
        fileLock.unlock()

        guard index >= 0, hadFile, let dir = sessionDir, let sessionId else { return }
        let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
        let seconds = rate > 0 ? Double(frames) / rate : 0
        guard seconds >= 0.5 else {
            // An empty tail (stop right after a rotation) is not worth a record.
            try? FileManager.default.removeItem(at: url)
            return
        }
        if !checkedBrand {
            checkedBrand = true
            checkM4ABrand(url)
        }
        let segment = SegmentStore.Segment(index: index, file: url.lastPathComponent, durationS: seconds,
                                           bytes: SegmentStore.fileSize(url))
        log.info("segment \(index) closed (\(reason, privacy: .public)): \(seconds, format: .fixed(precision: 1))s")
        // Rotation runs on the audio render thread: deliver on main, where
        // the plugin keeps its state.
        let event = Event.segmentClosed(sessionId: sessionId, segment: segment, url: url)
        if Thread.isMainThread {
            onEvent?(event)
        } else {
            DispatchQueue.main.async { self.onEvent?(event) }
        }
    }

    /// The backend recognises the upload by magic bytes; `infer` registers
    /// video before audio, so a brand other than `M4A ` could be taken for
    /// mp4. Check the first segment once.
    private func checkM4ABrand(_ url: URL) {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return }
        defer { try? handle.close() }
        let header = (try? handle.read(upToCount: 12)) ?? Data()
        let brand = header.count >= 12 ? String(decoding: header[8..<12], as: UTF8.self) : ""
        if brand != "M4A " {
            log.error("unexpected ftyp brand '\(brand, privacy: .public)'")
            onEvent?(.notice("Định dạng tệp ghi bất thường (\(brand)) — máy chủ có thể không nhận."))
        }
    }

    private func rotate(format: AVAudioFormat, reason: String) {
        closeSegment(reason: reason)
        do {
            try openSegment(format: format)
        } catch {
            fileLock.lock()
            file = nil
            fileLock.unlock()
            DispatchQueue.main.async { self.fail("Không mở được đoạn ghi mới: \(error.localizedDescription)") }
        }
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
            rotate(format: buffer.format, reason: "format-change")
            return
        }
        var needRotate = false
        do {
            try target.write(from: buffer)
            currentFrames += AVAudioFramePosition(buffer.frameLength)
            needRotate = currentRate > 0 && Double(currentFrames) / currentRate >= segmentLimitSeconds
        } catch {
            fileLock.unlock()
            log.error("write: \(error.localizedDescription, privacy: .public)")
            return
        }
        fileLock.unlock()
        if needRotate { rotate(format: buffer.format, reason: "rotation") }
    }

    private func updateLevel(_ buffer: AVAudioPCMBuffer) {
        guard let data = buffer.floatChannelData, buffer.frameLength > 0 else { return }
        levelTick += 1
        guard levelTick % 6 == 0 else { return }
        let n = Int(buffer.frameLength)
        var sum: Float = 0
        for i in 0..<n {
            let v = data[0][i]
            sum += v * v
        }
        let rms = (sum / Float(n)).squareRoot()
        DispatchQueue.main.async { self.onEvent?(.level(rms)) }
    }

    // MARK: - System events

    private func observeSystemEvents() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil,
                                            queue: .main) { [weak self] note in
            self?.handleInterruption(note)
        })
        observers.append(center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil,
                                            queue: .main) { [weak self] note in
            self?.handleRouteChange(note)
        })
        observers.append(center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification,
                                            object: nil, queue: .main) { [weak self] _ in
            self?.restartEngineChain(reason: "dịch vụ âm thanh khởi động lại")
        })
    }

    private func handleInterruption(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw), state == .recording
        else { return }
        switch type {
        case .began:
            pausedAt = Date()
            stopEngineChain()
            closeSegment(reason: "interrupt")
            onEvent?(.notice("Tạm dừng: cuộc gọi hoặc ứng dụng khác đang dùng micro."))
        case .ended:
            if let pausedAt {
                pausedTotal += Date().timeIntervalSince(pausedAt)
                self.pausedAt = nil
            }
            // Try to rebuild even without .shouldResume; fail clearly if impossible.
            restartEngineChain(reason: "hết cuộc gọi/gián đoạn")
        @unknown default:
            break
        }
    }

    private func handleRouteChange(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw), state == .recording
        else { return }
        // Only when the input device really changed (AirPods, headset).
        if reason == .oldDeviceUnavailable || reason == .newDeviceAvailable {
            restartEngineChain(reason: "đổi thiết bị âm thanh")
        }
    }

    // MARK: - Helpers

    private func tickElapsed() {
        guard let startedAt else { return }
        let paused = pausedTotal + (pausedAt.map { Date().timeIntervalSince($0) } ?? 0)
        onEvent?(.elapsed(max(0, Date().timeIntervalSince(startedAt) - paused)))
    }

    private func setState(_ next: State) {
        state = next
        onEvent?(.state(next))
    }

    private func fail(_ message: String) {
        stopEngineChain()
        closeSegment(reason: "failure")
        timer?.invalidate()
        timer = nil
        sessionId = nil
        sessionDir = nil
        setState(.idle)
        onEvent?(.failure(message))
    }

    struct RecorderError: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }
}
