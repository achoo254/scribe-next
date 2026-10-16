import AVFoundation
import Capacitor
import Foundation
import UIKit

/// Bridge between the WebView and the native recording stack. Swift is the
/// source of truth (plan D2): the WebView is suspended while the phone is
/// locked, so it re-reads `getStatus()` when it comes back instead of
/// merging its own state.
///
/// Events: `state`, `elapsed`, `level`, `segment`, `notice`, `failure`,
/// `uploads`, `needsSignIn`, `quotaFull`.
@objc(RecorderPlugin)
public class RecorderPlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "RecorderPlugin"
    public let jsName = "Recorder"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "start", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "stop", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "getStatus", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "syncUploads", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "signOut", returnType: CAPPluginReturnPromise),
    ]

    private let recorder = AudioRecorder()
    private let store = SegmentStore.shared
    private let queue = UploadQueue.shared
    /// Signed-in account (main thread).
    private var userKey: String?
    /// Account the current recording belongs to: its segments go there even
    /// if the user signs out meanwhile.
    private var recordingUserKey: String?
    private var elapsed: Double = 0
    private var foregroundObserver: NSObjectProtocol?
    private var waiting = 0
    private var failed = 0
    private var lastNotice = ""

    override public func load() {
        recorder.onEvent = { [weak self] event in self?.handle(event) }
        #if DEBUG
        runSelfTestIfAsked()
        #endif
        let target = WeakPlugin(self)
        Task {
            await queue.setEventHandler { event in
                DispatchQueue.main.async { target.plugin?.handle(event) }
            }
        }
        // Back on screen: resume uploads (the queue sleeps while suspended).
        foregroundObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
        ) { _ in
            guard let plugin = target.plugin, let user = plugin.userKey else { return }
            Task { await plugin.queue.kick(ifUser: user) }
        }
    }

    deinit {
        if let foregroundObserver { NotificationCenter.default.removeObserver(foregroundObserver) }
    }

    #if DEBUG
    /// Simulator check of the storage and upload stack without a microphone
    /// (the Mac mini running the simulator has none: the engine's input
    /// aborts there). Launch with `-OSSelfTest`: writes two synthetic m4a
    /// segments plus one truncated "still open when killed" file, runs the
    /// startup reconcile, then the upload queue (signed out, it must stop at
    /// "needs sign-in"). Not compiled into release builds.
    private func runSelfTestIfAsked() {
        guard ProcessInfo.processInfo.arguments.contains("-OSSelfTest") else { return }
        let userKey = "selftest"
        let sessionId = "selftest-\(Int(Date().timeIntervalSince1970))"
        Task {
            do {
                let dir = try await store.createSession(userKey: userKey, sessionId: sessionId, segmentSeconds: 10)
                for index in 0..<2 {
                    let url = dir.appendingPathComponent(String(format: "seg-%03d.m4a", index))
                    let seconds = try Self.writeTone(url: url, seconds: 3)
                    try await store.appendSegment(userKey: userKey, sessionId: sessionId, segment:
                        SegmentStore.Segment(index: index, file: url.lastPathComponent, durationS: seconds,
                                             bytes: SegmentStore.fileSize(url)))
                }
                // A segment iOS killed mid-write: bytes without a moov atom.
                try Data(repeating: 0, count: 4096).write(to: dir.appendingPathComponent("seg-002.m4a"))
                let lost = await store.reconcile(userKey: userKey, activeSessionId: nil)
                NSLog("%@", "SELFTEST reconcile lost=\(lost.map { "\($0.index)" }.joined(separator: ","))")
                let pending = await store.pending(userKey: userKey)
                NSLog("%@", "SELFTEST pending=\(pending.map { UploadQueue.name($0) }.joined(separator: " "))")
                await queue.start(userKey: userKey)
                try await Task.sleep(nanoseconds: 8_000_000_000)
                let session = try await store.session(userKey: userKey, sessionId: sessionId)
                for seg in session.segments {
                    NSLog("%@", "SELFTEST seg \(seg.index) \(String(format: "%.1f", seg.durationS))s bytes=\(seg.bytes) "
                        + "uploaded=\(seg.uploaded) attempts=\(seg.attempts) failure=\(seg.failure ?? "-")")
                }
                await queue.stop()
            } catch {
                NSLog("%@", "SELFTEST error \(error.localizedDescription)")
            }
        }
    }

    /// 440 Hz mono AAC file through the same AVAudioFile writer settings.
    private static func writeTone(url: URL, seconds: Double) throws -> Double {
        let rate = 44_100.0
        guard let format = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1),
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(rate * seconds)),
              let samples = buffer.floatChannelData?[0]
        else { throw NSError(domain: "selftest", code: 1) }
        buffer.frameLength = buffer.frameCapacity
        for i in 0..<Int(buffer.frameLength) {
            samples[i] = 0.2 * sinf(Float(i) * 2 * .pi * 440 / Float(rate))
        }
        let file = try AVAudioFile(forWriting: url, settings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: rate,
            AVNumberOfChannelsKey: 1,
            AVEncoderBitRateKey: AudioRecorder.aacBitRate(sampleRate: rate, channels: 1),
        ])
        try file.write(from: buffer)
        return Double(buffer.frameLength) / rate
    }
    #endif

    /// Lets the upload actor call back without holding the plugin strongly;
    /// the plugin is only touched on the main queue.
    private final class WeakPlugin: @unchecked Sendable {
        weak var plugin: RecorderPlugin?
        init(_ plugin: RecorderPlugin) { self.plugin = plugin }
    }

    // MARK: Methods

    /// `{ userKey, segmentMinutes? }` → starts a session (1-minute segments
    /// by default: a segment still open when iOS kills the app is lost).
    @objc func start(_ call: CAPPluginCall) {
        guard let userKey = call.getString("userKey"), !userKey.isEmpty else {
            call.reject("Thiếu thông tin tài khoản.", "NO_USER")
            return
        }
        let minutes = call.getDouble("segmentMinutes") ?? 1
        let sessionId = UUID().uuidString.lowercased()
        Task { @MainActor in
            guard recorder.state == .idle else {
                call.reject("Đang ghi rồi.", "BUSY")
                return
            }
            self.userKey = userKey
            recordingUserKey = userKey
            do {
                let dir = try await store.createSession(userKey: userKey, sessionId: sessionId,
                                                        segmentSeconds: minutes * 60)
                elapsed = 0
                lastNotice = ""
                recorder.start(sessionId: sessionId, dir: dir, segmentSeconds: minutes * 60)
                call.resolve(["sessionId": sessionId])
            } catch {
                call.reject("Không tạo được phiên ghi: \(error.localizedDescription)", "STORE")
            }
        }
    }

    @objc func stop(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            self.recorder.stop()
            call.resolve(self.status())
        }
    }

    @objc func getStatus(_ call: CAPPluginCall) {
        DispatchQueue.main.async { call.resolve(self.status()) }
    }

    /// After sign-in and at launch: recover leftovers of a killed session,
    /// report the open segment that was lost, and upload what is waiting.
    @objc func syncUploads(_ call: CAPPluginCall) {
        guard let userKey = call.getString("userKey"), !userKey.isEmpty else {
            call.reject("Thiếu thông tin tài khoản.", "NO_USER")
            return
        }
        Task { @MainActor in
            self.userKey = userKey
            let active = recorder.sessionId
            let lost = await store.reconcile(userKey: userKey, activeSessionId: active)
            await store.clearFailures(userKey: userKey)
            await queue.start(userKey: userKey)
            let lostSeconds = lost.reduce(0) { $0 + $1.approxSeconds }
            call.resolve(["lostSegments": lost.count, "lostSecondsMax": lostSeconds])
        }
    }

    /// Sign-out: stop recording and uploads so nothing reaches the old account.
    @objc func signOut(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            self.userKey = nil
            self.recorder.stop()
            SessionAuth.clear()
            Task {
                await self.queue.stop()
                call.resolve()
            }
        }
    }

    // MARK: Events

    private func status() -> [String: Any] {
        [
            "state": recorder.state.rawValue,
            "elapsedS": elapsed,
            // Session position of the audio kept so far (bookmarks).
            "positionS": recorder.recordedSeconds,
            "sessionId": recorder.sessionId ?? NSNull(),
            "uploadsWaiting": waiting,
            "uploadsFailed": failed,
            "notice": lastNotice,
        ]
    }

    private func handle(_ event: AudioRecorder.Event) {
        switch event {
        case .state(let state):
            notifyListeners("state", data: ["state": state.rawValue])
        case .elapsed(let seconds):
            elapsed = seconds
            // The WebView is suspended off screen; it re-reads getStatus()
            // when visible, so do not queue thousands of evaluations.
            if UIApplication.shared.applicationState == .active {
                notifyListeners("elapsed", data: ["elapsedS": seconds])
            }
        case .level(let level):
            if UIApplication.shared.applicationState == .active {
                notifyListeners("level", data: ["level": level])
            }
        case .segmentClosed(let sessionId, let segment, _):
            guard let owner = recordingUserKey else { return }
            notifyListeners("segment", data: ["index": segment.index, "durationS": segment.durationS])
            Task {
                try? await store.appendSegment(userKey: owner, sessionId: sessionId, segment: segment)
                // Only wakes the queue if `owner` is still the signed-in user.
                await queue.kick(ifUser: owner)
            }
        case .notice(let message):
            lastNotice = message
            notifyListeners("notice", data: ["message": message])
        case .failure(let message):
            lastNotice = message
            notifyListeners("failure", data: ["message": message])
        }
    }

    private func handle(_ event: UploadQueue.Event) {
        switch event {
        case .counts(let waiting, let failed):
            self.waiting = waiting
            self.failed = failed
            notifyListeners("uploads", data: ["waiting": waiting, "failed": failed])
        case .uploaded(let name):
            notifyListeners("uploaded", data: ["name": name])
        case .needsSignIn:
            #if DEBUG
            NSLog("%@", "SELFTEST upload queue → needsSignIn")
            #endif
            notifyListeners("needsSignIn", data: [:])
        case .quotaFull(let message):
            notifyListeners("quotaFull", data: ["message": message])
        }
    }
}
