import Foundation
import MediaPlayer
import os
import UIKit

/// A bookmark pressed outside the WebView (lock screen, Control Center,
/// AirPods) while its JavaScript was suspended. Kept on disk until the web
/// layer says it handed it to its bookmark queue for good (sent, refused or
/// dropped), so a crash or a kill never loses one.
struct NativeBookmark: Codable, Equatable {
    /// 24 hex digits, the server's id shape: a resend never saves it twice.
    let id: String
    /// Recording session; its transcript is `s:<sessionId>`.
    let sessionId: String
    /// Seconds of audio kept in the session when pressed (`recordedSeconds`).
    let positionS: Double
    let createdAt: Date

    var json: [String: Any] {
        [
            "id": id,
            "sessionId": sessionId,
            "positionS": positionS,
            "createdAt": ISO8601DateFormatter().string(from: createdAt),
        ]
    }

    /// 12 random bytes as 24 lowercase hex digits (ObjectId-shaped).
    static func newId() -> String {
        var generator = SystemRandomNumberGenerator()
        return (0..<12).map { _ in String(format: "%02x", UInt8.random(in: .min ... .max, using: &generator)) }
            .joined()
    }
}

/// `Documents/bookmarks/<userKey>.json`, one small file per account, written
/// atomically and readable while the phone is locked. Main thread only.
final class NativeBookmarkStore {
    private let root: URL
    private let log = Logger(subsystem: "com.hoandat.scribe", category: "bookmarks")
    private let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        return e
    }()
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    init(root: URL? = nil) {
        self.root = root ?? FileManager.default
            .urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("bookmarks", isDirectory: true)
    }

    func pending(userKey: String) -> [NativeBookmark] {
        let url = file(userKey)
        guard FileManager.default.fileExists(atPath: url.path) else { return [] }
        do {
            return try decoder.decode([NativeBookmark].self, from: Data(contentsOf: url))
        } catch {
            log.error("read bookmarks: \(error.localizedDescription, privacy: .public)")
            return []
        }
    }

    func append(_ bookmark: NativeBookmark, userKey: String) throws {
        try write(pending(userKey: userKey) + [bookmark], userKey: userKey)
    }

    /// Forgets the bookmarks the web layer took care of.
    func remove(ids: Set<String>, userKey: String) throws {
        let all = pending(userKey: userKey)
        let kept = all.filter { !ids.contains($0.id) }
        guard kept.count != all.count else { return }
        if kept.isEmpty {
            try FileManager.default.removeItem(at: file(userKey))
        } else {
            try write(kept, userKey: userKey)
        }
    }

    /// Account deleted: its pending bookmarks leave the phone.
    func removeAll(userKey: String) throws {
        let url = file(userKey)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try FileManager.default.removeItem(at: url)
    }

    private func write(_ bookmarks: [NativeBookmark], userKey: String) throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [
            .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication,
        ])
        try encoder.encode(bookmarks).write(
            to: file(userKey), options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }

    private func file(_ userKey: String) -> URL {
        root.appendingPathComponent("\(userKey).json")
    }
}

/// Lock screen / Control Center / AirPods while recording: the "Now Playing"
/// entry shows the recording clock. The middle button (play, pause,
/// play/pause — an AirPods single press) pauses or resumes the recording;
/// next and previous (AirPods double and triple press) add a bookmark.
/// Nothing there stops the recording. Feedback is visual only (a sound
/// would be recorded). Main thread only.
final class NowPlayingControls {
    struct Actions {
        /// Returns the bookmark position, or nil when it could not be saved.
        let bookmark: () -> Double?
        /// Pauses when recording, resumes when paused.
        let togglePause: () -> Void
    }

    /// Presses closer than this count once (a double press must not mark twice).
    private static let bookmarkDebounce: TimeInterval = 1
    /// Guards against one tap arriving twice and undoing itself.
    private static let pauseDebounce: TimeInterval = 0.4
    /// How long "Đã đánh dấu 12:34" stays in the title.
    private static let feedbackSeconds: TimeInterval = 3

    private var tokens: [(MPRemoteCommand, Any)] = []
    private var lastBookmark: Date?
    private var lastPauseToggle: Date?
    private var feedback: String?
    private var feedbackReset: DispatchWorkItem?
    private var elapsed: Double = 0
    private var paused = false

    var isActive: Bool { !tokens.isEmpty }

    func start(_ actions: Actions) {
        guard tokens.isEmpty else { return }
        let center = MPRemoteCommandCenter.shared()
        // iOS picks which of play / pause / toggle to send from its own idea
        // of the playback state, which the silent loop keeps "playing": all
        // three toggle, so the button always does what it shows.
        let pausing: [MPRemoteCommand] = [center.togglePlayPauseCommand, center.playCommand, center.pauseCommand]
        for command in pausing {
            register(command) { [weak self] in self?.togglePause(actions.togglePause) ?? .commandFailed }
        }
        for command in [center.nextTrackCommand, center.previousTrackCommand] {
            register(command) { [weak self] in self?.bookmark(actions.bookmark) ?? .commandFailed }
        }
        let disabled: [MPRemoteCommand] = [
            center.stopCommand, center.skipForwardCommand, center.skipBackwardCommand,
            center.seekForwardCommand, center.seekBackwardCommand, center.changePlaybackPositionCommand,
            center.changePlaybackRateCommand,
        ]
        for command in disabled { command.isEnabled = false }
        UIApplication.shared.beginReceivingRemoteControlEvents()
        elapsed = 0
        paused = false
        lastBookmark = nil
        lastPauseToggle = nil
        publish()
    }

    /// Recorder clock (seconds, excluding interruptions and pauses), about once a second.
    func update(elapsed: Double) {
        guard isActive else { return }
        self.elapsed = elapsed
        publish()
    }

    /// The recording was paused or resumed (from here or from the app).
    func update(paused: Bool) {
        guard isActive, self.paused != paused else { return }
        self.paused = paused
        publish()
    }

    func stop() {
        guard isActive else { return }
        for (command, token) in tokens {
            command.removeTarget(token)
            command.isEnabled = false
        }
        tokens = []
        feedbackReset?.cancel()
        feedbackReset = nil
        feedback = nil
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        UIApplication.shared.endReceivingRemoteControlEvents()
    }

    private func register(_ command: MPRemoteCommand, _ handler: @escaping () -> MPRemoteCommandHandlerStatus) {
        command.isEnabled = true
        let token = command.addTarget { _ in handler() }
        tokens.append((command, token))
    }

    private func togglePause(_ action: () -> Void) -> MPRemoteCommandHandlerStatus {
        let now = Date()
        if let lastPauseToggle, now.timeIntervalSince(lastPauseToggle) < Self.pauseDebounce { return .success }
        lastPauseToggle = now
        action()
        return .success
    }

    private func bookmark(_ action: () -> Double?) -> MPRemoteCommandHandlerStatus {
        let now = Date()
        if let lastBookmark, now.timeIntervalSince(lastBookmark) < Self.bookmarkDebounce { return .success }
        lastBookmark = now
        guard let position = action() else { return .commandFailed }
        feedback = "Đã đánh dấu \(Self.clock(position))"
        feedbackReset?.cancel()
        let reset = DispatchWorkItem { [weak self] in
            self?.feedback = nil
            self?.publish()
        }
        feedbackReset = reset
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.feedbackSeconds, execute: reset)
        publish()
        return .success
    }

    /// Rate 1 shows a pause button while recording, rate 0 a play button
    /// while paused.
    private func publish() {
        MPNowPlayingInfoCenter.default().nowPlayingInfo = [
            MPMediaItemPropertyTitle: feedback ?? (paused ? "Scribe đã tạm dừng" : "Scribe đang ghi"),
            MPMediaItemPropertyArtist: paused
                ? "Bấm ▶ để ghi tiếp"
                : "Bấm ⏸ để tạm dừng · ⏩ để đánh dấu",
            MPNowPlayingInfoPropertyElapsedPlaybackTime: elapsed,
            MPNowPlayingInfoPropertyPlaybackRate: paused ? 0.0 : 1.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
            // A recording has no length: shown as live instead of an empty "--:--" progress bar.
            MPNowPlayingInfoPropertyIsLiveStream: true,
        ]
    }

    /// 754 → "12:34", 3_754 → "1:02:34".
    static func clock(_ seconds: Double) -> String {
        let total = Int(max(0, seconds.rounded(.down)))
        let h = total / 3600
        let m = total % 3600 / 60
        let s = total % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%02d:%02d", m, s)
    }
}
