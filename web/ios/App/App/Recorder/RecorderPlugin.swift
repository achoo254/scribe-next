import AVFoundation
import Capacitor
import Foundation
import UIKit

/// Bridge between the WebView and the native recording stack. Swift is the
/// source of truth (plan D2): the WebView is suspended while the phone is
/// locked, so it re-reads `getStatus()` when it comes back instead of
/// merging its own state.
///
/// Events: `state`, `paused`, `elapsed`, `level`, `segment`, `notice`, `failure`,
/// `uploads`, `needsSignIn`, `quotaFull`, `bookmarkAdded`, `transcript`.
///
/// Live transcript (`start({ realtime: true })`, `RealtimeTranscriber`):
/// `transcript` events (`type` sentence | partial | status) reach the page
/// only while it is on screen; when it comes back it replaces its state
/// with `getTranscript()`.
///
/// The lock screen and AirPods (`NowPlayingControls`) pause and resume the
/// recording, and add bookmarks. Those bookmarks are saved here first; the web layer pulls them (`takePendingBookmarks`),
/// sends them through its own queue, then `ackBookmarks` forgets them.
@objc(RecorderPlugin)
public class RecorderPlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "RecorderPlugin"
    public let jsName = "Recorder"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "start", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "stop", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "pause", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "resume", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "getStatus", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "syncUploads", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "signOut", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "takePendingBookmarks", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "ackBookmarks", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "getTranscript", returnType: CAPPluginReturnPromise),
    ]

    private let recorder = AudioRecorder()
    private let store = SegmentStore.shared
    private let queue = UploadQueue.shared
    private let bookmarks = NativeBookmarkStore()
    private let nowPlaying = NowPlayingControls()
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
    /// Live transcript of the current (or last) recording, if it was asked for.
    private var realtime: RealtimeTranscriber?
    private var transcriptSession: (id: String, dir: URL)?
    private var transcriptStatus = RealtimeTranscriber.Status()

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
                // Account deleted: nothing of it stays on the phone.
                try await store.removeAll(userKey: userKey)
                NSLog("%@", "SELFTEST discard pending=\(await store.pending(userKey: userKey).count)")
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

    /// `{ userKey, segmentMinutes?, realtime? }` → starts a session (1-minute
    /// segments by default: a segment still open when iOS kills the app is
    /// lost). `realtime: true` also streams the audio for a live transcript.
    @objc func start(_ call: CAPPluginCall) {
        guard let userKey = call.getString("userKey"), !userKey.isEmpty else {
            call.reject(localized("recorder.noUser", "Thiếu thông tin tài khoản."), "NO_USER")
            return
        }
        let minutes = call.getDouble("segmentMinutes") ?? 1
        let wantsRealtime = call.getBool("realtime") ?? false
        let sessionId = UUID().uuidString.lowercased()
        Task { @MainActor in
            guard recorder.state == .idle else {
                call.reject(localized("recorder.busy", "Đang ghi rồi."), "BUSY")
                return
            }
            self.userKey = userKey
            recordingUserKey = userKey
            do {
                let dir = try await store.createSession(userKey: userKey, sessionId: sessionId,
                                                        segmentSeconds: minutes * 60)
                elapsed = 0
                lastNotice = ""
                stopRealtime()
                transcriptSession = nil
                transcriptStatus = RealtimeTranscriber.Status()
                if wantsRealtime { startRealtime(sessionId: sessionId, dir: dir) }
                recorder.start(sessionId: sessionId, dir: dir, segmentSeconds: minutes * 60)
                call.resolve(["sessionId": sessionId])
            } catch {
                call.reject(localized("recorder.sessionCreateFailed", "Không tạo được phiên ghi: %@", error.localizedDescription), "STORE")
            }
        }
    }

    @objc func stop(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            self.recorder.stop()
            call.resolve(self.status())
        }
    }

    @objc func pause(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            self.recorder.pause()
            call.resolve(self.status())
        }
    }

    @objc func resume(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            self.recorder.resume()
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
            call.reject(localized("recorder.noUser", "Thiếu thông tin tài khoản."), "NO_USER")
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
    /// `{ discardUserKey }` (the account was deleted): also remove what the phone
    /// keeps for that account — segments not uploaded, live transcripts, bookmarks —
    /// so a later sign-in with the same email never uploads them.
    @objc func signOut(_ call: CAPPluginCall) {
        let discard = call.getString("discardUserKey").flatMap { Self.isUserKey($0) ? $0 : nil }
        DispatchQueue.main.async {
            self.userKey = nil
            self.recorder.stop()
            SessionAuth.clear()
            Task {
                await self.queue.stop()
                if let discard { await self.discardLocalData(userKey: discard) }
                call.resolve()
            }
        }
    }

    /// `userKeyOf` of the web layer: 24 lowercase hex characters (never a path).
    static func isUserKey(_ key: String) -> Bool {
        key.count == 24 && key.allSatisfy { $0.isHexDigit && !$0.isUppercase }
    }

    @MainActor private func discardLocalData(userKey: String) async {
        do {
            try await store.removeAll(userKey: userKey)
            try bookmarks.removeAll(userKey: userKey)
        } catch {
            NSLog("%@", "discard local data failed: \(error.localizedDescription)")
        }
    }

    /// `{ userKey }` → `{ bookmarks: [{ id, sessionId, positionS, createdAt }] }`,
    /// oldest first. They stay stored until acknowledged.
    @objc func takePendingBookmarks(_ call: CAPPluginCall) {
        guard let userKey = call.getString("userKey"), !userKey.isEmpty else {
            call.reject(localized("recorder.noUser", "Thiếu thông tin tài khoản."), "NO_USER")
            return
        }
        DispatchQueue.main.async {
            call.resolve(["bookmarks": self.bookmarks.pending(userKey: userKey).map(\.json)])
        }
    }

    /// `{ userKey, ids }`: the web queue took care of these bookmarks.
    @objc func ackBookmarks(_ call: CAPPluginCall) {
        guard let userKey = call.getString("userKey"), !userKey.isEmpty else {
            call.reject(localized("recorder.noUser", "Thiếu thông tin tài khoản."), "NO_USER")
            return
        }
        let ids = Set((call.getArray("ids") ?? []).compactMap { $0 as? String })
        DispatchQueue.main.async {
            do {
                try self.bookmarks.remove(ids: ids, userKey: userKey)
                call.resolve()
            } catch {
                call.reject(localized("recorder.bookmarkUpdateFailed", "Không cập nhật được dấu mốc trên máy: %@", error.localizedDescription), "STORE")
            }
        }
    }

    /// `{}` → `{ sessionId, entries: [{ id, atS, text, gapUntilS? }], status }`
    /// of the current or last recording with a live transcript (sorted by
    /// position); `sessionId` null and no entries when there is none.
    @objc func getTranscript(_ call: CAPPluginCall) {
        DispatchQueue.main.async {
            let status = self.transcriptStatus.json
            guard let session = self.transcriptSession else {
                call.resolve(["sessionId": NSNull(), "entries": [], "status": status])
                return
            }
            Task {
                let entries = await TranscriptStore.shared.entries(dir: session.dir)
                call.resolve(["sessionId": session.id, "entries": entries.map(\.json), "status": status])
            }
        }
    }

    // MARK: Live transcript

    /// Its failures stay inside it: the recording never depends on it.
    private func startRealtime(sessionId: String, dir: URL) {
        let transcriber = RealtimeTranscriber(sessionId: sessionId, dir: dir) { [weak self] event in
            self?.handle(event, sessionId: sessionId)
        }
        realtime = transcriber
        transcriptSession = (sessionId, dir)
        recorder.setPCMSink { buffer, position in transcriber.feed(buffer, at: position) }
        transcriber.start()
    }

    private func stopRealtime() {
        recorder.setPCMSink(nil)
        realtime?.finish()
        realtime = nil
    }

    private func handle(_ event: RealtimeTranscriber.Event, sessionId: String) {
        guard transcriptSession?.id == sessionId else { return }
        var data: [String: Any]
        switch event {
        case .sentence(let entry):
            data = entry.json
            data["type"] = "sentence"
        case .partial(let id, let atS, let text):
            data = ["type": "partial", "id": id, "atS": atS, "text": text]
        case .status(let status):
            transcriptStatus = status
            data = status.json
        }
        // Off screen the WebView is suspended: it reads getTranscript() when back.
        guard UIApplication.shared.applicationState == .active else { return }
        data["sessionId"] = sessionId
        notifyListeners("transcript", data: data)
    }

    /// A lock-screen or AirPods press: saved to disk at once at the recorder
    /// clock. Returns the position for the Now Playing feedback, nil if not saved.
    private func addNativeBookmark() -> Double? {
        guard recorder.state == .recording, let sessionId = recorder.sessionId, let owner = recordingUserKey
        else { return nil }
        let bookmark = NativeBookmark(id: NativeBookmark.newId(), sessionId: sessionId,
                                      positionS: recorder.recordedSeconds, createdAt: Date())
        do {
            try bookmarks.append(bookmark, userKey: owner)
        } catch {
            lastNotice = localized("recorder.bookmarkSaveFailed", "Không lưu được dấu mốc: %@", error.localizedDescription)
            notifyListeners("notice", data: ["message": lastNotice])
            return nil
        }
        // Off screen the WebView is suspended: it pulls when it comes back.
        if UIApplication.shared.applicationState == .active {
            notifyListeners("bookmarkAdded", data: bookmark.json)
        }
        return bookmark.positionS
    }

    /// The lock screen's middle button or an AirPods press.
    private func togglePause() {
        if recorder.isPaused { recorder.resume() } else { recorder.pause() }
    }

    // MARK: Events

    private func status() -> [String: Any] {
        [
            "state": recorder.state.rawValue,
            // Also tells the web layer this build can pause.
            "paused": recorder.isPaused,
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
            if state == .recording {
                nowPlaying.start(NowPlayingControls.Actions(
                    bookmark: { [weak self] in self?.addNativeBookmark() },
                    togglePause: { [weak self] in self?.togglePause() }
                ))
            } else if state == .idle {
                nowPlaying.stop()
                stopRealtime()
            }
            notifyListeners("state", data: ["state": state.rawValue])
        case .paused(let paused):
            nowPlaying.update(paused: paused)
            notifyListeners("paused", data: ["paused": paused])
        case .elapsed(let seconds):
            elapsed = seconds
            nowPlaying.update(elapsed: seconds)
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
        case .accountDeleted(let userKey):
            try? bookmarks.removeAll(userKey: userKey)
            notifyListeners("needsSignIn", data: [:])
        }
    }
}
