import AVFoundation
import Foundation

/// The only owner of `session.json` files (plan Phase 2, fixes the race of
/// the first draft): the recorder (audio thread) and the upload queue both
/// go through this actor, so no segment or `uploaded` flag is lost.
///
/// Layout, per signed-in user so one account never uploads another's audio:
/// `Documents/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`.
actor SegmentStore {
    static let shared = SegmentStore()

    struct Segment: Codable, Equatable {
        var index: Int
        var file: String
        var durationS: Double
        var bytes: Int64
        var uploaded: Bool
        var remoteFileId: String?
        var attempts: Int
        /// Last terminal failure (403, unreadable audio); the file is kept.
        var failure: String?
        /// Server recording session of the upload in progress: a retry
        /// resends into it from offset 0 instead of leaving a new `.part`.
        var serverSession: String?
        /// `finalize` was sent: before any new `init`, look the deterministic
        /// name up — the server may have committed and only the reply was lost.
        var finalizeSent: Bool

        init(index: Int, file: String, durationS: Double, bytes: Int64) {
            self.index = index
            self.file = file
            self.durationS = durationS
            self.bytes = bytes
            uploaded = false
            remoteFileId = nil
            attempts = 0
            failure = nil
            serverSession = nil
            finalizeSent = false
        }

        // Tolerant decoding: missing or unknown keys never make a manifest
        // unreadable (a rollback must not hide recorded audio).
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            index = try c.decode(Int.self, forKey: .index)
            file = try c.decodeIfPresent(String.self, forKey: .file) ?? String(format: "seg-%03d.m4a", index)
            durationS = try c.decodeIfPresent(Double.self, forKey: .durationS) ?? 0
            bytes = try c.decodeIfPresent(Int64.self, forKey: .bytes) ?? 0
            uploaded = try c.decodeIfPresent(Bool.self, forKey: .uploaded) ?? false
            remoteFileId = try c.decodeIfPresent(String.self, forKey: .remoteFileId)
            attempts = try c.decodeIfPresent(Int.self, forKey: .attempts) ?? 0
            failure = try c.decodeIfPresent(String.self, forKey: .failure)
            serverSession = try c.decodeIfPresent(String.self, forKey: .serverSession)
            finalizeSent = try c.decodeIfPresent(Bool.self, forKey: .finalizeSent) ?? false
        }
    }

    struct Session: Codable {
        static let currentSchema = 1
        var schemaVersion: Int
        var sessionId: String
        var startedAt: Date
        var segmentSeconds: Double
        var segments: [Segment]

        init(sessionId: String, startedAt: Date, segmentSeconds: Double) {
            schemaVersion = Self.currentSchema
            self.sessionId = sessionId
            self.startedAt = startedAt
            self.segmentSeconds = segmentSeconds
            segments = []
        }

        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            schemaVersion = try c.decodeIfPresent(Int.self, forKey: .schemaVersion) ?? 1
            sessionId = try c.decode(String.self, forKey: .sessionId)
            startedAt = try c.decodeIfPresent(Date.self, forKey: .startedAt) ?? Date(timeIntervalSince1970: 0)
            segmentSeconds = try c.decodeIfPresent(Double.self, forKey: .segmentSeconds) ?? 60
            segments = try c.decodeIfPresent([Segment].self, forKey: .segments) ?? []
        }
    }

    /// A segment waiting for upload, with everything the queue needs.
    struct Pending: Equatable {
        let userKey: String
        let sessionId: String
        let segment: Segment
        let url: URL
    }

    /// A segment that was still open when the app died: unreadable, removed.
    struct LostSegment: Equatable {
        let sessionId: String
        let index: Int
        let approxSeconds: Double
    }

    private let root: URL
    private let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .iso8601
        e.outputFormatting = [.sortedKeys]
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
            .appendingPathComponent("recordings", isDirectory: true)
    }

    // MARK: Sessions

    /// Creates the folder of a new recording session. Protection must allow
    /// writes while the phone is locked (spike lesson 4).
    func createSession(userKey: String, sessionId: String, segmentSeconds: Double) throws -> URL {
        let dir = sessionDir(userKey: userKey, sessionId: sessionId)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [
            .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication,
        ])
        try write(Session(sessionId: sessionId, startedAt: Date(), segmentSeconds: segmentSeconds), to: dir)
        return dir
    }

    func appendSegment(userKey: String, sessionId: String, segment: Segment) throws {
        let dir = sessionDir(userKey: userKey, sessionId: sessionId)
        var session = try read(dir)
        session.segments.removeAll { $0.index == segment.index }
        session.segments.append(segment)
        session.segments.sort { $0.index < $1.index }
        try write(session, to: dir)
    }

    /// Uploaded: record the remote id, then free the phone's disk (manifest
    /// first: a failed write must never leave "not uploaded" with no file).
    func markUploaded(userKey: String, sessionId: String, index: Int, fileId: String) throws {
        let dir = sessionDir(userKey: userKey, sessionId: sessionId)
        var session = try read(dir)
        guard let i = session.segments.firstIndex(where: { $0.index == index }) else { return }
        session.segments[i].uploaded = true
        session.segments[i].remoteFileId = fileId
        session.segments[i].failure = nil
        session.segments[i].serverSession = nil
        session.segments[i].finalizeSent = false
        try write(session, to: dir)
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(session.segments[i].file))
    }

    /// Progress of the upload in flight (see `serverSession`, `finalizeSent`).
    func setUploadState(userKey: String, sessionId: String, index: Int,
                        serverSession: String?, finalizeSent: Bool) throws {
        let dir = sessionDir(userKey: userKey, sessionId: sessionId)
        var session = try read(dir)
        guard let i = session.segments.firstIndex(where: { $0.index == index }) else { return }
        session.segments[i].serverSession = serverSession
        session.segments[i].finalizeSent = finalizeSent
        try write(session, to: dir)
    }

    /// Gives segments stopped by an error another chance (once per launch).
    func clearFailures(userKey: String) {
        for dir in sessionDirs(userKey: userKey) {
            guard var session = try? read(dir), session.segments.contains(where: { $0.failure != nil }) else {
                continue
            }
            for i in session.segments.indices { session.segments[i].failure = nil }
            try? write(session, to: dir)
        }
    }

    func markAttempt(userKey: String, sessionId: String, index: Int, failure: String?) throws {
        let dir = sessionDir(userKey: userKey, sessionId: sessionId)
        var session = try read(dir)
        guard let i = session.segments.firstIndex(where: { $0.index == index }) else { return }
        session.segments[i].attempts += 1
        session.segments[i].failure = failure
        try write(session, to: dir)
    }

    func session(userKey: String, sessionId: String) throws -> Session {
        try read(sessionDir(userKey: userKey, sessionId: sessionId))
    }

    /// Every segment of this user not yet uploaded and not failed for good, oldest first.
    func pending(userKey: String) -> [Pending] {
        sessionDirs(userKey: userKey).flatMap { dir -> [Pending] in
            guard let session = try? read(dir) else { return [] }
            return session.segments
                .filter { !$0.uploaded && $0.failure == nil }
                .map { Pending(userKey: userKey, sessionId: session.sessionId, segment: $0,
                               url: dir.appendingPathComponent($0.file)) }
        }
    }

    /// Counts for the UI: segments waiting, and segments stopped by a terminal error.
    func queueCounts(userKey: String) -> (waiting: Int, failed: Int) {
        var waiting = 0
        var failed = 0
        for dir in sessionDirs(userKey: userKey) {
            guard let session = try? read(dir) else { continue }
            for seg in session.segments where !seg.uploaded {
                if seg.failure == nil { waiting += 1 } else { failed += 1 }
            }
        }
        return (waiting, failed)
    }

    // MARK: Startup reconcile

    /// Merges every manifest with the `seg-*.m4a` files really on disk: a
    /// file missing from the manifest is added (a broken manifest never hides
    /// audio); a file AVAudioFile cannot read was the segment still open when
    /// the app was killed — it is removed and reported. Sessions with nothing
    /// left are deleted.
    func reconcile(userKey: String, activeSessionId: String?) -> [LostSegment] {
        var lost: [LostSegment] = []
        for dir in sessionDirs(userKey: userKey) {
            let sessionId = dir.lastPathComponent
            if sessionId == activeSessionId { continue }
            var session = (try? read(dir))
                ?? Session(sessionId: sessionId, startedAt: Date(timeIntervalSince1970: 0), segmentSeconds: 60)
            let files = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
            for name in files where name.hasPrefix("seg-") && name.hasSuffix(".m4a") {
                let url = dir.appendingPathComponent(name)
                guard let readable = try? AVAudioFile(forReading: url), readable.processingFormat.sampleRate > 0,
                      readable.length > 0
                else {
                    let index = Self.segmentIndex(name) ?? -1
                    lost.append(LostSegment(sessionId: sessionId, index: index,
                                            approxSeconds: session.segmentSeconds))
                    try? FileManager.default.removeItem(at: url)
                    session.segments.removeAll { $0.file == name && !$0.uploaded }
                    continue
                }
                if !session.segments.contains(where: { $0.file == name }) {
                    let index = Self.segmentIndex(name) ?? session.segments.count
                    let seconds = Double(readable.length) / readable.processingFormat.sampleRate
                    session.segments.append(Segment(index: index, file: name, durationS: seconds,
                                                    bytes: Self.fileSize(url)))
                }
            }
            // Manifest entries whose file vanished and were never uploaded cannot be sent.
            session.segments.removeAll { !$0.uploaded && !files.contains($0.file) }
            session.segments.sort { $0.index < $1.index }
            if session.segments.allSatisfy(\.uploaded) {
                try? FileManager.default.removeItem(at: dir)
            } else {
                try? write(session, to: dir)
            }
        }
        return lost
    }

    // MARK: Files

    private func sessionDir(userKey: String, sessionId: String) -> URL {
        root.appendingPathComponent(userKey, isDirectory: true)
            .appendingPathComponent(sessionId, isDirectory: true)
    }

    /// Sessions oldest first (by creation date; names are UUIDs).
    private func sessionDirs(userKey: String) -> [URL] {
        let userDir = root.appendingPathComponent(userKey, isDirectory: true)
        let dirs = (try? FileManager.default.contentsOfDirectory(
            at: userDir, includingPropertiesForKeys: [.creationDateKey])) ?? []
        func created(_ url: URL) -> Date {
            (try? url.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast
        }
        return dirs.filter(\.hasDirectoryPath).sorted { created($0) < created($1) }
    }

    /// `seg-007.m4a` → 7, also past 999 (`seg-1000.m4a`).
    static func segmentIndex(_ name: String) -> Int? {
        guard name.hasPrefix("seg-"), name.hasSuffix(".m4a") else { return nil }
        return Int(name.dropFirst(4).dropLast(4))
    }

    private func read(_ dir: URL) throws -> Session {
        try decoder.decode(Session.self, from: Data(contentsOf: dir.appendingPathComponent("session.json")))
    }

    private func write(_ session: Session, to dir: URL) throws {
        try encoder.encode(session).write(
            to: dir.appendingPathComponent("session.json"),
            options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }

    static func fileSize(_ url: URL) -> Int64 {
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        return (attrs?[.size] as? NSNumber)?.int64Value ?? 0
    }
}
