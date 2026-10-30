import Foundation
import Network
import os
import UIKit

/// Uploads closed segments one at a time, also while the next one is being
/// recorded (plan Phase 2). Each segment becomes one recording through
/// `/api/recordings/init → chunk → finalize`, named `<sessionId>#<index>`.
///
/// The backend has no idempotency, so a segment never gets a second `init`
/// blindly: the server session is kept in the manifest (a retry resends
/// into it from offset 0 — the backend accepts offset ≤ length), and once
/// `finalize` was sent the deterministic name is looked up first, because
/// the server may have committed while the reply was lost. A failed lookup
/// is transient, never a reason to start over.
///
/// Failure groups: transient (retry with backoff), redo (the server lost the
/// upload: drop its session, start the segment over), terminal (the backend
/// refused the audio: keep the file, stop that segment), 401 (renew the
/// session from the stored refresh token, else wait for sign-in) and 413
/// (quota full: wait).
actor UploadQueue {
    static let shared = UploadQueue()

    enum Event {
        case counts(waiting: Int, failed: Int)
        case uploaded(name: String)
        case needsSignIn
        case quotaFull(String)
        /// The account was deleted: its segments are gone from the phone.
        case accountDeleted(userKey: String)
    }

    private enum Outcome: Error {
        case transient(String)
        case redo(String)
        case terminal(String)
        case unauthorized
        case accountDeleted
        case quotaFull(String)
    }

    private static let chunkSize = 1 << 20

    private let store: SegmentStore
    private let session: URLSession
    private let log = Logger(subsystem: "com.hoandat.scribe", category: "upload")
    private var userKey: String?
    private var paused = false
    private var drainTask: Task<Void, Never>?
    /// A kick arrived while draining: run once more when the loop ends.
    private var rerun = false
    private var failures = 0
    private var retryTask: Task<Void, Never>?
    /// "Needs sign-in" is reported once until the next start.
    private var reportedSignIn = false
    private let monitor = NWPathMonitor()
    private var monitorStarted = false
    private var onEvent: (@Sendable (Event) -> Void)?

    init(store: SegmentStore = .shared, session: URLSession = .shared) {
        self.store = store
        self.session = session
    }

    func setEventHandler(_ handler: @escaping @Sendable (Event) -> Void) {
        onEvent = handler
    }

    /// Starts (or resumes) uploading for this user: at sign-in, at launch
    /// and when the app comes back to the foreground.
    func start(userKey: String) {
        if self.userKey != userKey {
            stopDrain()
            self.userKey = userKey
        }
        failures = 0
        paused = false
        reportedSignIn = false
        startMonitorOnce()
        kick(ifUser: userKey)
    }

    /// Sign-out: stop at once, cancel the upload in flight, and never send
    /// the previous user's segments.
    func stop() {
        userKey = nil
        paused = true
        stopDrain()
    }

    /// Wakes the queue for `user` only — a segment closing just before a
    /// sign-out must not restart uploads for the account that left.
    func kick(ifUser user: String) {
        guard user == userKey, !paused else { return }
        if drainTask != nil {
            rerun = true
            return
        }
        drainTask = Task { await self.runDrain(for: user) }
    }

    private func stopDrain() {
        drainTask?.cancel()
        drainTask = nil
        retryTask?.cancel()
        retryTask = nil
        rerun = false
    }

    private func startMonitorOnce() {
        guard !monitorStarted else { return }
        monitorStarted = true
        monitor.pathUpdateHandler = { [weak self] path in
            guard path.status == .satisfied, let self else { return }
            Task { await self.networkBack() }
        }
        monitor.start(queue: DispatchQueue(label: "com.hoandat.scribe.path"))
    }

    private func networkBack() {
        guard let userKey else { return }
        failures = 0
        retryTask?.cancel()
        retryTask = nil
        kick(ifUser: userKey)
    }

    // MARK: Loop

    private func runDrain(for user: String) async {
        // Keep uploading for a while if the user leaves the app.
        let background = await BackgroundTask.begin(name: "scribe-upload")
        repeat {
            rerun = false
            await drain(for: user)
        } while rerun && !Task.isCancelled && user == userKey && !paused
        await background.end()
        if user == userKey { drainTask = nil }
    }

    private func drain(for user: String) async {
        while !Task.isCancelled, user == userKey, !paused {
            let pending = await store.pending(userKey: user)
            await publishCounts(user)
            guard let next = pending.first else { return }
            do {
                let fileId = try await uploadWithRenewal(next)
                guard !Task.isCancelled, user == userKey else { return }
                try await store.markUploaded(userKey: user, sessionId: next.sessionId,
                                             index: next.segment.index, fileId: fileId)
                failures = 0
                onEvent?(.uploaded(name: Self.name(next)))
            } catch is CancellationError {
                return
            } catch let outcome as Outcome {
                guard !Task.isCancelled, user == userKey else { return }
                if await handle(outcome, for: next, user: user) == .stopLoop { return }
            } catch {
                if Task.isCancelled { return }
                failures += 1
                scheduleRetry(for: user)
                return
            }
        }
    }

    private enum Next { case continueLoop, stopLoop }

    private func handle(_ outcome: Outcome, for next: SegmentStore.Pending, user: String) async -> Next {
        let index = next.segment.index
        switch outcome {
        case .transient(let message):
            failures += 1
            log.warning("transient: \(message, privacy: .public)")
            scheduleRetry(for: user)
            return .stopLoop
        case .redo(let message):
            log.notice("redo: \(message, privacy: .public)")
            try? await store.setUploadState(userKey: user, sessionId: next.sessionId, index: index,
                                            serverSession: nil, finalizeSent: false)
            let failure: String? = next.segment.attempts >= 4 ? message : nil
            try? await store.markAttempt(userKey: user, sessionId: next.sessionId, index: index, failure: failure)
            return .continueLoop
        case .terminal(let message):
            try? await store.markAttempt(userKey: user, sessionId: next.sessionId, index: index, failure: message)
            return .continueLoop
        case .unauthorized:
            paused = true
            if !reportedSignIn {
                reportedSignIn = true
                onEvent?(.needsSignIn)
            }
            return .stopLoop
        case .quotaFull(let message):
            paused = true
            onEvent?(.quotaFull(message))
            return .stopLoop
        case .accountDeleted:
            // Never upload these into whatever account signs in next with the same email.
            paused = true
            try? await store.removeAll(userKey: user)
            log.notice("account deleted: local segments removed")
            onEvent?(.accountDeleted(userKey: user))
            return .stopLoop
        }
    }

    private func scheduleRetry(for user: String) {
        let delay = min(300, 5 * pow(2, Double(min(failures, 6))))
        retryTask?.cancel()
        retryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            guard !Task.isCancelled else { return }
            await self?.kick(ifUser: user)
        }
    }

    private func publishCounts(_ user: String) async {
        let counts = await store.queueCounts(userKey: user)
        onEvent?(.counts(waiting: counts.waiting, failed: counts.failed))
    }

    // MARK: One segment

    static func name(_ pending: SegmentStore.Pending) -> String {
        "\(pending.sessionId)#\(pending.segment.index)"
    }

    /// One attempt; a 401 first renews the session silently and retries once.
    private func uploadWithRenewal(_ pending: SegmentStore.Pending) async throws -> String {
        do {
            return try await upload(pending)
        } catch Outcome.unauthorized {
            do {
                try await SessionAuth.refreshSession(session: session)
                log.notice("session renewed from refresh token")
            } catch is SessionAuth.AccountDeleted {
                throw Outcome.accountDeleted
            } catch {
                throw Outcome.unauthorized
            }
            return try await upload(pending)
        }
    }

    private func upload(_ pending: SegmentStore.Pending) async throws -> String {
        let name = Self.name(pending)
        let user = pending.userKey
        let index = pending.segment.index
        // Re-read: the manifest has the latest server session / finalize flag.
        let segment = (try? await store.session(userKey: user, sessionId: pending.sessionId))?
            .segments.first(where: { $0.index == index }) ?? pending.segment

        if segment.finalizeSent, let existing = try await lookUp(name) {
            return existing
        }
        guard let data = try? Data(contentsOf: pending.url), !data.isEmpty else {
            throw Outcome.terminal(localized("upload.unreadableSegment", "Tệp đoạn ghi không đọc được."))
        }

        var serverSession = segment.serverSession
        if serverSession == nil {
            let reply = try await json("POST", "/api/recordings/init", body: ["name": name])
            guard let created = reply["sessionId"] as? String else {
                throw Outcome.transient("init không trả sessionId")
            }
            serverSession = created
            try? await store.setUploadState(userKey: user, sessionId: pending.sessionId, index: index,
                                            serverSession: created, finalizeSent: false)
        }
        guard let serverSession else { throw Outcome.transient("thiếu session") }

        // From offset 0 every time: the same bytes overwrite a partial upload.
        var offset = 0
        while offset < data.count {
            try Task.checkCancellation()
            let end = min(offset + Self.chunkSize, data.count)
            let reply = try await json("POST", "/api/recordings/chunk?session=\(serverSession)&offset=\(offset)",
                                       raw: data.subdata(in: offset..<end))
            guard let written = reply["written"] as? Int, written >= end else {
                throw Outcome.redo(localized("upload.chunkIncomplete", "chunk không ghi đủ dữ liệu"))
            }
            offset = end
        }

        try? await store.setUploadState(userKey: user, sessionId: pending.sessionId, index: index,
                                        serverSession: serverSession, finalizeSent: true)
        do {
            let reply = try await json("POST", "/api/recordings/finalize",
                                       body: ["session": serverSession, "durationS": pending.segment.durationS])
            guard let fileId = reply["fileId"] as? String else {
                throw Outcome.transient("finalize không trả fileId")
            }
            return fileId
        } catch Outcome.redo(let message) {
            // 404 may mean it WAS committed and only the reply was lost.
            if let existing = try await lookUp(name) { return existing }
            throw Outcome.redo(message)
        }
    }

    /// The recording called `name`, if the server has it. Any failure here
    /// is transient: guessing "not there" would upload it twice.
    private func lookUp(_ name: String) async throws -> String? {
        let reply = try await json("GET", "/api/files")
        guard let items = reply["items"] as? [[String: Any]] else {
            throw Outcome.transient("danh sách bản ghi không đọc được")
        }
        return items.first(where: { $0["name"] as? String == name })?["id"] as? String
    }

    // MARK: HTTP

    private func json(_ method: String, _ path: String, body: [String: Any]? = nil,
                      raw: Data? = nil) async throws -> [String: Any] {
        guard let url = URL(string: SessionAuth.backendOrigin + path) else {
            throw Outcome.terminal(localized("upload.badURL", "Đường dẫn không hợp lệ."))
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
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            if Task.isCancelled { throw CancellationError() }
            throw Outcome.transient(error.localizedDescription)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        let reply = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        // Only the backend's own `{ "error": … }` is trusted to be final; a
        // proxy page (Cloudflare, nginx) with the same status is transient.
        let backendMessage = reply?["error"] as? String
        switch status {
        case 200..<300: return reply ?? [:]
        case 401: throw Outcome.unauthorized
        case 413: throw Outcome.quotaFull(backendMessage ?? localized("upload.storageFull", "Hết dung lượng lưu trữ."))
        case 400, 403, 404:
            guard let message = backendMessage else { throw Outcome.transient("HTTP \(status) không từ máy chủ ứng dụng") }
            if status == 403 { throw Outcome.terminal(message) }
            throw Self.isRedo(message) ? Outcome.redo(message) : Outcome.terminal(message)
        default:
            throw Outcome.transient(backendMessage ?? "HTTP \(status)")
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

/// UIKit background task around an upload run: iOS gives the app some time
/// to finish after it leaves the screen (recording keeps it alive anyway).
@MainActor
final class BackgroundTask {
    private var id: UIBackgroundTaskIdentifier = .invalid

    static func begin(name: String) -> BackgroundTask {
        let task = BackgroundTask()
        task.id = UIApplication.shared.beginBackgroundTask(withName: name) { [weak task] in
            task?.end()
        }
        return task
    }

    func end() {
        guard id != .invalid else { return }
        UIApplication.shared.endBackgroundTask(id)
        id = .invalid
    }
}
