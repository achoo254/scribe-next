import AVFoundation
import Foundation
import os
import UIKit

/// Background recording engine, ported from the background-recording spike (in git history; 63 minutes measured
/// on the phone: locked screen, rotation in ~10 ms, 0 broken segments).
/// AVAudioSession (background audio) + AVAudioEngine tap → AAC/m4a written
/// directly, rotated every N minutes so each closed segment is a complete
/// file that can be uploaded at once. A segment still open when iOS kills
/// the app has no moov atom and is lost; the 1-minute default bounds that.
///
/// Does not import Capacitor: the UI layer subscribes through `onEvent`
/// (always delivered on the main queue).
final class AudioRecorder {
    enum State: String { case idle, starting, recording, stopping }

    enum Event {
        case state(State)
        /// Seconds recorded, excluding interruptions and pauses; about once a second.
        case elapsed(Double)
        /// The user paused (true) or resumed (false) the recording.
        case paused(Bool)
        /// RMS level 0…1 of the input, about ten times a second.
        case level(Float)
        /// A finished, readable segment of `sessionId`.
        case segmentClosed(sessionId: String, segment: SegmentStore.Segment, url: URL)
        /// The recording keeps going after an interruption or route change.
        case notice(String)
        /// Recording stopped by itself (permission, engine or disk failure).
        case failure(String)
    }

    var onEvent: ((Event) -> Void)?

    private(set) var state: State = .idle
    /// Session being recorded (main thread).
    private(set) var sessionId: String?
    /// Paused by the user (lock screen, AirPods, the app); main thread. The
    /// engine keeps running so the app stays alive and the Now Playing entry
    /// can resume it, but nothing is written until `resume()`.
    private(set) var isPaused = false

    /// Recreated after a media services reset (Apple: every audio object
    /// must be rebuilt then).
    private var engine = AVAudioEngine()
    private let log = Logger(subsystem: "com.hoandat.scribe", category: "recorder")

    /// Guards the fields below up to `recordingFlag`, shared by the audio
    /// render thread and main.
    private let fileLock = NSLock()
    private var file: AVAudioFile?
    private var currentIndex = -1
    private var currentFrames: AVAudioFramePosition = 0
    private var currentRate: Double = 0
    /// Session and folder of this recording, set at start under the lock,
    /// so the audio thread never reads main-thread state.
    private var segmentSessionId: String?
    private var segmentDir: URL?
    private var checkedBrand = false
    private var writeErrors = 0
    /// Frames the input tap delivered, written or not: the watchdog's sign of
    /// a live engine, also while paused.
    private var receivedFrames: AVAudioFramePosition = 0
    /// Mirror of `isPaused` for the audio thread: no segment opens while set.
    private var writingPaused = false
    /// Length of the session's kept (closed, ≥ 0.5 s) segments: with the open
    /// segment's frames, the session position the transcript counts.
    private var closedSeconds: Double = 0
    private var recordingFlag = false
    /// Receives every buffer written to the file with its session position
    /// (live transcript). Called on the audio thread; must not block.
    private var pcmSink: ((AVAudioPCMBuffer, Double) -> Void)?

    private var segmentLimitSeconds: Double = 60
    private var tapInstalled = false
    /// iOS gives the lock-screen Now Playing entry and AirPods presses only to an app that plays audio. A
    /// recorder plays nothing, so a looped silent buffer keeps Scribe eligible while it records (lock-screen
    /// and AirPods bookmarks). Set to false to switch it off if it misbehaves on a device.
    private static let playsSilenceWhileRecording = true
    private var silencePlayer: AVAudioPlayerNode?
    private var timer: Timer?
    private var startedAt: Date?
    private var pausedTotal: TimeInterval = 0
    /// Start of the current interruption (call, another app took the mic).
    private var pausedAt: Date?
    /// Since when the clock stands still (interruption or user pause).
    private var clockPausedAt: Date?
    /// Input format of the installed tap, for the segment opened on resume.
    private var tapFormat: AVAudioFormat?
    private var lastFrameCheck: AVAudioFramePosition = -1
    private var stalledTicks = 0
    private var levelTick = 0
    private var observers: [NSObjectProtocol] = []
    private var engineObserver: NSObjectProtocol?

    init() {
        observeSystemEvents()
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
        if let engineObserver { NotificationCenter.default.removeObserver(engineObserver) }
    }

    // MARK: - Start / stop

    /// Asks for the microphone, then records into `dir` (created by the
    /// store) as `seg-000.m4a`, `seg-001.m4a`…
    func start(sessionId: String, dir: URL, segmentSeconds: Double) {
        guard state == .idle else { return }
        self.sessionId = sessionId
        segmentLimitSeconds = max(10, segmentSeconds)
        fileLock.lock()
        segmentSessionId = sessionId
        segmentDir = dir
        currentIndex = -1
        checkedBrand = false
        writeErrors = 0
        receivedFrames = 0
        closedSeconds = 0
        writingPaused = false
        fileLock.unlock()
        isPaused = false
        setState(.starting)
        requestPermission { [weak self] granted in
            guard let self, self.state == .starting else { return }
            if granted {
                self.attemptStart(attempt: 1)
            } else {
                self.fail(localized("recorder.micDenied", "Ứng dụng chưa được cấp quyền Micro — vào Cài đặt → Scribe → Micro để bật."))
            }
        }
    }

    /// Stops recording; also cancels a start still waiting for permission.
    func stop() {
        switch state {
        case .starting:
            sessionId = nil
            setState(.idle)
        case .recording:
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
            isPaused = false
            setState(.idle)
        case .idle, .stopping:
            break
        }
    }

    /// Stops writing without ending the session: the open segment closes (and
    /// uploads), the clock stands still, and the audio until `resume()` is
    /// never kept. Bookmarks and the transcript continue at the same position.
    func pause() {
        guard state == .recording, !isPaused else { return }
        isPaused = true
        fileLock.lock()
        writingPaused = true
        fileLock.unlock()
        closeSegment(reason: "pause")
        updateClock()
        onEvent?(.level(0))
        onEvent?(.paused(true))
    }

    /// Writes again, into a new segment. During an interruption the segment
    /// opens when the engine restarts at its end.
    func resume() {
        guard state == .recording, isPaused else { return }
        isPaused = false
        fileLock.lock()
        writingPaused = false
        fileLock.unlock()
        updateClock()
        if pausedAt == nil, let tapFormat {
            do {
                try openSegment(format: tapFormat)
            } catch {
                fail(localized("recorder.resumeFailed", "Không ghi tiếp được sau khi tạm dừng: %@", error.localizedDescription))
                return
            }
        }
        onEvent?(.paused(false))
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
        guard state == .starting else { return }
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
            fail(localized("recorder.startFailed", "Không bắt đầu ghi được: %@", error.localizedDescription))
            return
        }
        startedAt = Date()
        pausedTotal = 0
        pausedAt = nil
        clockPausedAt = nil
        lastFrameCheck = -1
        stalledTicks = 0
        setState(.recording)
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.tick()
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
            throw RecorderError(localized("recorder.badFormat", "Định dạng micro không hợp lệ."))
        }
        try openSegment(format: format)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            self?.process(buffer)
        }
        tapInstalled = true
        tapFormat = format
        attachSilence()
        engine.prepare()
        try engine.start()
        silencePlayer?.play()
        fileLock.lock()
        recordingFlag = true
        fileLock.unlock()
    }

    private func stopEngineChain() {
        fileLock.lock()
        recordingFlag = false
        fileLock.unlock()
        if tapInstalled {
            engine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
            tapFormat = nil
        }
        if engine.isRunning { engine.stop() }
        if let player = silencePlayer {
            player.stop()
            engine.detach(player)
            silencePlayer = nil
        }
    }

    /// One second of zeros looped into the main mixer: inaudible, never reaches the input tap.
    private func attachSilence() {
        guard Self.playsSilenceWhileRecording else { return }
        let format = engine.mainMixerNode.outputFormat(forBus: 0)
        guard format.sampleRate > 0,
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(format.sampleRate))
        else { return }
        buffer.frameLength = buffer.frameCapacity
        if let channels = buffer.floatChannelData {
            for channel in 0..<Int(format.channelCount) {
                channels[channel].update(repeating: 0, count: Int(buffer.frameLength))
            }
        }
        let player = AVAudioPlayerNode()
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)
        player.scheduleBuffer(buffer, at: nil, options: .loops)
        silencePlayer = player
    }

    /// Rebuilds the chain after an interruption, route or configuration
    /// change. Always opens a NEW segment: rate or channels may have
    /// changed (AirPods → HFP 16 kHz).
    private func restartEngineChain(reason: String, newEngine: Bool = false) {
        guard state == .recording, pausedAt == nil else { return }
        stopEngineChain()
        closeSegment(reason: reason)
        if newEngine {
            engine = AVAudioEngine()
            observeEngineConfiguration()
        }
        do {
            try configureAudioSession()
            try startEngineChain()
            lastFrameCheck = -1
            stalledTicks = 0
            onEvent?(.notice(localized("recorder.resumedAfterInterruption", "Ghi tiếp sau gián đoạn (%@).", reason)))
        } catch {
            fail(localized("recorder.interruptionResumeFailed", "Không ghi tiếp được sau gián đoạn: %@", error.localizedDescription))
        }
    }

    /// Sets (or clears) the receiver of the recorded audio. The sink only
    /// ever sees buffers that were written, at the position they start, so
    /// its timeline matches the transcript's.
    func setPCMSink(_ sink: ((AVAudioPCMBuffer, Double) -> Void)?) {
        fileLock.lock()
        pcmSink = sink
        fileLock.unlock()
    }

    // MARK: - Segments

    /// Shorter tails (a stop right after a rotation) are dropped, not kept.
    private static let minSegmentSeconds = 0.5

    /// Seconds of audio kept in this session so far: closed segments plus
    /// the open one. Bookmarks use it — it is what the transcript's
    /// timeline counts, and unlike the WebView's copy it is never stale.
    var recordedSeconds: Double {
        fileLock.lock()
        defer { fileLock.unlock() }
        let open = file != nil && currentRate > 0 ? Double(currentFrames) / currentRate : 0
        return closedSeconds + open
    }

    /// Does nothing while paused: `resume()` opens the next segment.
    private func openSegment(format: AVAudioFormat) throws {
        fileLock.lock()
        let dir = segmentDir
        let index = currentIndex + 1
        let paused = writingPaused
        fileLock.unlock()
        guard !paused else { return }
        guard let dir else { throw RecorderError(localized("recorder.noSessionFolder", "Chưa có thư mục phiên ghi.")) }
        let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
        let newFile = try makeAACFile(url: url, format: format)
        fileLock.lock()
        // Paused while the file was being created (a rotation on the audio
        // thread): drop it, the index is used again on resume.
        if writingPaused {
            fileLock.unlock()
            try? FileManager.default.removeItem(at: url)
            return
        }
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

    /// Releases the file (AVAudioFile writes the moov atom on release; this
    /// is the only reference) and reports the finished segment on main.
    private func closeSegment(reason: String) {
        fileLock.lock()
        let hadFile = file != nil
        let frames = currentFrames
        let index = currentIndex
        let rate = currentRate
        let dir = segmentDir
        let session = segmentSessionId
        let checkBrand = !checkedBrand && hadFile
        if checkBrand { checkedBrand = true }
        let seconds = rate > 0 ? Double(frames) / rate : 0
        // Counted in the same lock that empties the open segment, so
        // `recordedSeconds` never misses a segment between the two.
        if hadFile && seconds >= Self.minSegmentSeconds { closedSeconds += seconds }
        file = nil
        currentFrames = 0
        fileLock.unlock()

        guard index >= 0, hadFile, let dir, let session else { return }
        let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
        guard seconds >= Self.minSegmentSeconds else {
            // An empty tail (stop right after a rotation) is not worth a record.
            try? FileManager.default.removeItem(at: url)
            return
        }
        if checkBrand { checkM4ABrand(url) }
        let segment = SegmentStore.Segment(index: index, file: url.lastPathComponent, durationS: seconds,
                                           bytes: SegmentStore.fileSize(url))
        log.info("segment \(index) closed (\(reason, privacy: .public)): \(seconds, format: .fixed(precision: 1))s")
        emit(.segmentClosed(sessionId: session, segment: segment, url: url))
    }

    /// The backend recognises the upload by magic bytes; `infer` registers
    /// video before audio, so a brand other than `M4A ` could be taken for
    /// mp4. Checked once, on the first segment.
    private func checkM4ABrand(_ url: URL) {
        guard let handle = try? FileHandle(forReadingFrom: url) else { return }
        defer { try? handle.close() }
        let header = (try? handle.read(upToCount: 12)) ?? Data()
        let brand = header.count >= 12 ? String(decoding: header[8..<12], as: UTF8.self) : ""
        if brand != "M4A " {
            log.error("unexpected ftyp brand '\(brand, privacy: .public)'")
            emit(.notice(localized("recorder.oddFileFormat", "Định dạng tệp ghi bất thường (%@) — máy chủ có thể không nhận.", brand)))
        }
    }

    /// Called on the audio thread, after the buffer's file reference is gone.
    private func rotate(format: AVAudioFormat, reason: String) {
        fileLock.lock()
        let active = recordingFlag
        fileLock.unlock()
        guard active else { return }
        closeSegment(reason: reason)
        do {
            try openSegment(format: format)
        } catch {
            fileLock.lock()
            file = nil
            fileLock.unlock()
            DispatchQueue.main.async { self.fail(localized("recorder.segmentOpenFailed", "Không mở được đoạn ghi mới: %@", error.localizedDescription)) }
        }
    }

    // MARK: - Audio render thread

    private enum WriteResult { case ok, rotate, formatChanged, failed(String), noFile }

    private func process(_ buffer: AVAudioPCMBuffer) {
        updateLevel(buffer)
        fileLock.lock()
        receivedFrames += AVAudioFramePosition(buffer.frameLength)
        let sink = pcmSink
        let position = closedSeconds + (currentRate > 0 ? Double(currentFrames) / currentRate : 0)
        fileLock.unlock()
        // The file reference lives only inside `write`, so it is released
        // before a rotation reads the closed file back.
        switch write(buffer) {
        case .ok:
            sink?(buffer, position)
        case .noFile:
            break
        case .rotate:
            sink?(buffer, position)
            rotate(format: buffer.format, reason: "rotation")
        case .formatChanged:
            rotate(format: buffer.format, reason: "format-change")
        case .failed(let message):
            log.error("write: \(message, privacy: .public)")
        }
    }

    private func write(_ buffer: AVAudioPCMBuffer) -> WriteResult {
        fileLock.lock()
        defer { fileLock.unlock() }
        guard let target = file else { return .noFile }
        if buffer.format != target.processingFormat { return .formatChanged }
        do {
            try target.write(from: buffer)
            currentFrames += AVAudioFramePosition(buffer.frameLength)
            writeErrors = 0
            let full = currentRate > 0 && Double(currentFrames) / currentRate >= segmentLimitSeconds
            return full ? .rotate : .ok
        } catch {
            writeErrors += 1
            if writeErrors == 1 || writeErrors % 50 == 0 {
                let count = writeErrors
                DispatchQueue.main.async { self.diskTrouble(count) }
            }
            return .failed(error.localizedDescription)
        }
    }

    private func updateLevel(_ buffer: AVAudioPCMBuffer) {
        fileLock.lock()
        let paused = writingPaused
        fileLock.unlock()
        guard !paused, let data = buffer.floatChannelData, buffer.frameLength > 0 else { return }
        levelTick += 1
        guard levelTick % 6 == 0 else { return }
        let n = Int(buffer.frameLength)
        var sum: Float = 0
        for i in 0..<n {
            let v = data[0][i]
            sum += v * v
        }
        emit(.level((sum / Float(n)).squareRoot()))
    }

    /// Writes failing (disk full): say so at once, and stop after ~100
    /// failed buffers instead of showing "Đang ghi" while saving nothing.
    private func diskTrouble(_ count: Int) {
        guard state == .recording else { return }
        if count >= 100 {
            fail(localized("recorder.diskFullStopped", "Không ghi được vào bộ nhớ máy (có thể đã đầy) — đã dừng, giữ phần đã ghi."))
        } else {
            onEvent?(.notice(localized("recorder.diskWriteFailed", "Không ghi được vào bộ nhớ máy — kiểm tra dung lượng trống.")))
        }
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
            self?.restartEngineChain(reason: localized("recorder.reason.mediaReset", "dịch vụ âm thanh khởi động lại"), newEngine: true)
        })
        // An interruption whose "ended" never arrives: try again when the
        // user is back in the app.
        observers.append(center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil,
                                            queue: .main) { [weak self] _ in
            self?.resumeIfStillInterrupted()
        })
        observeEngineConfiguration()
    }

    /// The engine stops itself when the hardware format changes; without
    /// this the UI would keep counting while nothing is recorded.
    private func observeEngineConfiguration() {
        if let engineObserver { NotificationCenter.default.removeObserver(engineObserver) }
        engineObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main
        ) { [weak self] _ in
            self?.restartEngineChain(reason: localized("recorder.reason.configChange", "đổi cấu hình âm thanh"))
        }
    }

    private func handleInterruption(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw), state == .recording
        else { return }
        switch type {
        case .began:
            guard pausedAt == nil else { return }
            pausedAt = Date()
            updateClock()
            stopEngineChain()
            closeSegment(reason: "interrupt")
            onEvent?(.notice(localized("recorder.pausedByCall", "Tạm dừng: cuộc gọi hoặc ứng dụng khác đang dùng micro.")))
        case .ended:
            endInterruption()
        @unknown default:
            break
        }
    }

    private func endInterruption() {
        guard pausedAt != nil else { return }
        pausedAt = nil
        updateClock()
        // Try to rebuild even without .shouldResume; fail clearly if impossible.
        restartEngineChain(reason: localized("recorder.reason.interruptionEnded", "hết cuộc gọi/gián đoạn"))
    }

    private func resumeIfStillInterrupted() {
        guard state == .recording, pausedAt != nil else { return }
        endInterruption()
    }

    private func handleRouteChange(_ note: Notification) {
        guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw),
              state == .recording,
              pausedAt == nil // during a call the restart happens when it ends
        else { return }
        if reason == .oldDeviceUnavailable || reason == .newDeviceAvailable {
            restartEngineChain(reason: localized("recorder.reason.routeChange", "đổi thiết bị âm thanh"))
        }
    }

    // MARK: - Helpers

    /// The clock stands still while interrupted or paused by the user; an
    /// overlap of both counts once.
    private func updateClock() {
        let halted = pausedAt != nil || isPaused
        if halted, clockPausedAt == nil {
            clockPausedAt = Date()
        } else if !halted, let since = clockPausedAt {
            pausedTotal += Date().timeIntervalSince(since)
            clockPausedAt = nil
        }
    }

    /// Clock + watchdog: if the input stops delivering frames while not
    /// interrupted (paused or not), the engine died silently — rebuild it.
    private func tick() {
        guard let startedAt else { return }
        let paused = pausedTotal + (clockPausedAt.map { Date().timeIntervalSince($0) } ?? 0)
        emit(.elapsed(max(0, Date().timeIntervalSince(startedAt) - paused)))

        guard state == .recording, pausedAt == nil else { return }
        fileLock.lock()
        let frames = receivedFrames
        fileLock.unlock()
        if !engine.isRunning || frames == lastFrameCheck {
            stalledTicks += 1
        } else {
            stalledTicks = 0
        }
        lastFrameCheck = frames
        if stalledTicks >= 3 {
            stalledTicks = 0
            restartEngineChain(reason: localized("recorder.reason.silentMic", "micro ngừng gửi âm thanh"))
        }
    }

    private func setState(_ next: State) {
        state = next
        onEvent?(.state(next))
    }

    private func emit(_ event: Event) {
        if Thread.isMainThread {
            onEvent?(event)
        } else {
            DispatchQueue.main.async { self.onEvent?(event) }
        }
    }

    private func fail(_ message: String) {
        stopEngineChain()
        closeSegment(reason: "failure")
        timer?.invalidate()
        timer = nil
        sessionId = nil
        isPaused = false
        setState(.idle)
        onEvent?(.failure(message))
    }

    struct RecorderError: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }
}
