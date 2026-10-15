import Foundation
import Network
import os

/// Uploads closed segments one at a time, also while the next one is being
/// recorded (plan Phase 2). Each segment becomes one recording through
/// `/api/recordings/init → chunk → finalize`, named `<sessionId>#<index>`:
/// the backend has no idempotency, so that deterministic name is how a lost
/// `finalize` reply is recognised (and how the UI groups a session).
///
/// Failures fall in three groups — transient (retry with backoff), redo
/// (drop the server session and start the segment over) and terminal (keep
/// the file, stop that segment) — plus 401 (wait for sign-in) and 413
/// (quota full: wait). URLSession.shared carries the session cookie that
/// CapacitorHttp stored at sign-in.
actor UploadQueue {
    static let shared = UploadQueue()

    enum Event {
        case counts(waiting: Int, failed: Int)
        case uploaded(name: String)
        case needsSignIn
        case quotaFull(String)
    }

    private enum Outcome: Error {
        case transient(String)
        case redo(String)
        case terminal(String)
        case unauthorized
        case quotaFull(String)
    }

    static let baseURL = URL(string: "https://\(NativeFilesPlugin.backendHost)")!
    private static let chunkSize = 1 << 20

    private let store: SegmentStore
    private let session: URLSession
    private let log = Logger(subsystem: "vn.inet.onescribe", category: "upload")
    private var userKey: String?
    private var running = false
    private var paused = false
    private var failures = 0
    private var retryTask: Task<Void, Never>?
    private let monitor = NWPathMonitor()
    private var onEvent: (@Sendable (Event) -> Void)?

    init(store: SegmentStore = .shared, session: URLSession = .shared) {
        self.store = store
        self.session = session
    }

    func setEventHandler(_ handler: @escaping @Sendable (Event) -> Void) {
        onEvent = handler
    }

    /// Starts (or resumes) uploading for this user. Called at sign-in, at
    /// launch, when a segment closes and when the network comes back.
    func start(userKey: String) {
        if self.userKey != userKey {
            self.userKey = userKey
            failures = 0
        }
        paused = false
        startMonitorOnce()
        kick()
    }

    /// Sign-out: stop, and never send the previous user's segments.
    func stop() {
        userKey = nil
        paused = true
        retryTask?.cancel()
        retryTask = nil
    }

    func kick() {
        guard !running, !paused, userKey != nil else { return }
        running = true
        Task { await drain() }
    }

    private var monitorStarted = false
    private func startMonitorOnce() {
        guard !monitorStarted else { return }
        monitorStarted = true
        monitor.pathUpdateHandler = { [weak self] path in
            guard path.status == .satisfied, let self else { return }
            Task { await self.networkBack() }
        }
        monitor.start(queue: DispatchQueue(label: "vn.inet.onescribe.path"))
    }

    private func networkBack() {
        failures = 0
        retryTask?.cancel()
        retryTask = nil
        kick()
    }

    // MARK: Loop

    private func drain() async {
        defer { running = false }
        while let userKey, !paused {
            let pending = await store.pending(userKey: userKey)
            await publishCounts(userKey)
            guard let next = pending.first else { return }
            do {
                let fileId = try await upload(next)
                try await store.markUploaded(userKey: userKey, sessionId: next.sessionId,
                                             index: next.segment.index, fileId: fileId)
                failures = 0
                onEvent?(.uploaded(name: Self.name(next)))
            } catch let outcome as Outcome {
                switch outcome {
                case .transient(let message):
                    failures += 1
                    log.warning("transient: \(message, privacy: .public)")
                    scheduleRetry()
                    return
                case .redo(let message):
                    // The loop starts the segment over with a fresh init.
                    log.notice("redo: \(message, privacy: .public)")
                    try? await store.markAttempt(userKey: userKey, sessionId: next.sessionId,
                                                 index: next.segment.index, failure: nil)
                    if next.segment.attempts >= 5 {
                        try? await store.markAttempt(userKey: userKey, sessionId: next.sessionId,
                                                     index: next.segment.index, failure: message)
                    }
                case .terminal(let message):
                    try? await store.markAttempt(userKey: userKey, sessionId: next.sessionId,
                                                 index: next.segment.index, failure: message)
                case .unauthorized:
                    paused = true
                    onEvent?(.needsSignIn)
                    return
                case .quotaFull(let message):
                    paused = true
                    onEvent?(.quotaFull(message))
                    return
                }
            } catch {
                failures += 1
                scheduleRetry()
                return
            }
        }
    }

    private func scheduleRetry() {
        let delay = min(300, 5 * pow(2, Double(min(failures, 6))))
        retryTask?.cancel()
        retryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            guard !Task.isCancelled else { return }
            await self?.kick()
        }
    }

    private func publishCounts(_ userKey: String) async {
        let counts = await store.queueCounts(userKey: userKey)
        onEvent?(.counts(waiting: counts.waiting, failed: counts.failed))
    }

    // MARK: One segment

    static func name(_ pending: SegmentStore.Pending) -> String {
        "\(pending.sessionId)#\(pending.segment.index)"
    }

    private func upload(_ pending: SegmentStore.Pending) async throws -> String {
        let name = Self.name(pending)
        guard let data = try? Data(contentsOf: pending.url), !data.isEmpty else {
            throw Outcome.terminal("Tệp đoạn ghi không đọc được.")
        }
        let initReply = try await json("POST", "/api/recordings/init", body: ["name": name])
        guard let serverSession = initReply["sessionId"] as? String else {
            throw Outcome.transient("init không trả sessionId")
        }
        var offset = 0
        while offset < data.count {
            let end = min(offset + Self.chunkSize, data.count)
            let reply = try await json("POST", "/api/recordings/chunk?session=\(serverSession)&offset=\(offset)",
                                       raw: data.subdata(in: offset..<end))
            guard let written = reply["written"] as? Int, written > offset else {
                throw Outcome.redo("chunk không ghi thêm dữ liệu")
            }
            offset = written
        }
        do {
            let reply = try await json("POST", "/api/recordings/finalize",
                                       body: ["session": serverSession, "durationS": pending.segment.durationS])
            guard let fileId = reply["fileId"] as? String else { throw Outcome.transient("finalize không trả fileId") }
            return fileId
        } catch Outcome.redo(let message) {
            // 404 may mean the file WAS committed and only the reply was lost.
            if let existing = try await findRecording(named: name) { return existing }
            throw Outcome.redo(message)
        } catch Outcome.transient(let message) {
            if let existing = try? await findRecording(named: name) { return existing }
            throw Outcome.transient(message)
        }
    }

    private func findRecording(named name: String) async throws -> String? {
        let reply = try await json("GET", "/api/files")
        let items = reply["items"] as? [[String: Any]] ?? []
        return items.first(where: { $0["name"] as? String == name })?["id"] as? String
    }

    // MARK: HTTP

    private func json(_ method: String, _ path: String, body: [String: Any]? = nil,
                      raw: Data? = nil) async throws -> [String: Any] {
        guard let url = URL(string: path, relativeTo: Self.baseURL) else {
            throw Outcome.terminal("Đường dẫn không hợp lệ.")
        }
        var request = URLRequest(url: url, timeoutInterval: 60)
        request.httpMethod = method
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        } else if let raw {
            request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
            request.httpBody = raw
        }
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw Outcome.transient(error.localizedDescription)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        let reply = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        let message = reply["error"] as? String ?? "HTTP \(status)"
        switch status {
        case 200..<300: return reply
        case 401: throw Outcome.unauthorized
        case 413: throw Outcome.quotaFull(message)
        case 400, 404: throw Self.isRedo(message) ? Outcome.redo(message) : Outcome.terminal(message)
        case 403: throw Outcome.terminal(message)
        default: throw Outcome.transient(message)
        }
    }

    /// Messages of `recordings.rs` that mean "the server lost this upload":
    /// offset gap, unknown session (swept after 24 h), no data yet.
    static func isRedo(_ message: String) -> Bool {
        let lower = message.lowercased()
        return lower.contains("offset") || lower.contains("session") || lower.contains("phiên")
            || lower.contains("chưa có dữ liệu") || lower.contains("không tồn tại")
    }
}
