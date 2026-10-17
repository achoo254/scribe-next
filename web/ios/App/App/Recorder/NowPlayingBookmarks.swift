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
/// entry shows the recording clock, and every media command (play, pause,
/// play/pause, next, previous — AirPods single, double, triple press) adds a
/// bookmark instead of controlling playback. Nothing there pauses or stops
/// the recording. Feedback is visual only (a sound would be recorded).
/// Main thread only.
final class NowPlayingBookmarks {
    /// Presses closer than this count once (a double press must not mark twice).
    private static let debounce: TimeInterval = 1
    /// How long "Đã đánh dấu 12:34" stays in the title.
    private static let feedbackSeconds: TimeInterval = 3

    private var tokens: [(MPRemoteCommand, Any)] = []
    private var lastPress: Date?
    private var feedback: String?
    private var feedbackReset: DispatchWorkItem?
    private var elapsed: Double = 0

    var isActive: Bool { !tokens.isEmpty }

    /// Registers the commands; `onPress` returns the bookmark position, or
    /// nil when it could not be saved.
    func start(onPress: @escaping () -> Double?) {
        guard tokens.isEmpty else { return }
        let center = MPRemoteCommandCenter.shared()
        let marking: [MPRemoteCommand] = [
            center.togglePlayPauseCommand, center.playCommand, center.pauseCommand,
            center.nextTrackCommand, center.previousTrackCommand,
        ]
        for command in marking {
            command.isEnabled = true
            let token = command.addTarget { [weak self] _ in
                guard let self else { return .commandFailed }
                return self.press(onPress)
            }
            tokens.append((command, token))
        }
        let disabled: [MPRemoteCommand] = [
            center.stopCommand, center.skipForwardCommand, center.skipBackwardCommand,
            center.seekForwardCommand, center.seekBackwardCommand, center.changePlaybackPositionCommand,
            center.changePlaybackRateCommand,
        ]
        for command in disabled { command.isEnabled = false }
        UIApplication.shared.beginReceivingRemoteControlEvents()
        elapsed = 0
        lastPress = nil
        publish()
    }

    /// Recorder clock (seconds, excluding interruptions), about once a second.
    func update(elapsed: Double) {
        guard isActive else { return }
        self.elapsed = elapsed
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

    private func press(_ onPress: () -> Double?) -> MPRemoteCommandHandlerStatus {
        let now = Date()
        if let lastPress, now.timeIntervalSince(lastPress) < Self.debounce { return .success }
        lastPress = now
        guard let position = onPress() else { return .commandFailed }
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

    /// Rate 1 keeps the lock screen showing a "pause" button: pressing it
    /// arrives here as a bookmark, never as a pause.
    private func publish() {
        MPNowPlayingInfoCenter.default().nowPlayingInfo = [
            MPMediaItemPropertyTitle: feedback ?? "Scribe đang ghi",
            MPMediaItemPropertyArtist: "Bấm nút hoặc tai nghe để đánh dấu",
            MPNowPlayingInfoPropertyElapsedPlaybackTime: elapsed,
            MPNowPlayingInfoPropertyPlaybackRate: 1.0,
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
