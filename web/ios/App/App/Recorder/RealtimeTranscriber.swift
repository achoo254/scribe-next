import AVFoundation
import Foundation
import os

/// Live preview of the words while recording: the recorder's input tap is
/// converted to PCM16 24 kHz mono, cut into 100 ms frames and streamed to
/// OpenAI Realtime (transcription session, server VAD) over a WebSocket.
/// Finished sentences go to `TranscriptStore` and to `onEvent`.
///
/// The m4a file stays the source of truth: nothing here can stop or slow
/// the recording. The tap only converts and hands the frame over; every
/// failure (network, token, quota) is contained, reported through
/// `.status`, and the audio of that stretch is simply not previewed.
///
/// Tokens come from the backend (`POST /api/realtime/token`, the native
/// session cookie, renewed like the upload queue does), which reserves
/// `reservedSeconds` of the user's transcription minutes per token. One
/// token streams at most that much audio: before it runs out a new token
/// opens a new WebSocket, audio switches over, and the old one is committed
/// and closed a few seconds later so its last sentence still arrives. The
/// seconds actually streamed with each token are reported back
/// (`POST /api/realtime/usage`) when it is retired.
///
/// All state lives on `queue`; `feed` runs on the audio tap thread.
final class RealtimeTranscriber: @unchecked Sendable {
    enum Event {
        case sentence(TranscriptStore.Entry)
        /// Text of a sentence still being transcribed (not stored).
        case partial(id: String, atS: Double, text: String)
        case status(Status)
    }

    struct Status {
        var connected = false
        /// Session position where the stream was lost; nil while connected.
        var disconnectedAtS: Double?
        /// Why the preview is not working (shown as is), nil when fine.
        var error: String?
        /// The stream ended for this recording (stopped, or a final error).
        var stopped = false

        var json: [String: Any] {
            var out: [String: Any] = ["type": "status", "connected": connected, "stopped": stopped]
            if let disconnectedAtS { out["disconnectedAtS"] = disconnectedAtS }
            if let error { out["error"] = error }
            return out
        }
    }

    let sessionId: String
    let dir: URL

    private static let realtimeURL = URL(string: "wss://api.openai.com/v1/realtime")
    private static let sampleRate = 24_000.0
    /// 100 ms of PCM16 mono at 24 kHz.
    private static let frameBytes = 4_800
    private static let frameSeconds = 0.1
    /// Frames kept while (re)connecting, sent once the new socket is open.
    private static let backlogLimit = 300
    /// Ask for the next token this long before the current one's budget ends.
    private static let rotateMarginS = 20.0
    /// A token closer than this to its expiry is not used to open a socket.
    private static let expiryMarginS = 30.0
    /// A retired socket stays open this long for its last sentence.
    private static let retireDelayS = 4.0
    private static let pingEveryS = 15.0
    private static let backoffS: [Double] = [1, 2, 5, 10, 20, 30, 60]
    /// A missed stretch shorter than this is not marked.
    private static let minGapS = 1.0

    private let store: TranscriptStore
    private let onEvent: (Event) -> Void
    private let queue = DispatchQueue(label: "com.hoandat.scribe.realtime")
    private let log = Logger(subsystem: "com.hoandat.scribe", category: "realtime")
    private let sockets = URLSession(configuration: .default)
    private let outFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: sampleRate,
                                          channels: 1, interleaved: true)

    // Tap thread only.
    private var converter: AVAudioConverter?
    // Shared by the tap thread and `queue`.
    private let acceptLock = NSLock()
    private var accepting = true

    // `queue` only.
    private struct Token {
        let value: String
        let id: String
        let expiresAt: Date
        let reservedS: Double
        var sentS: Double = 0
    }

    private final class Connection: @unchecked Sendable {
        let task: URLSessionWebSocketTask
        let tokenId: String
        var open = false
        /// Session position of the first frame sent on this socket: OpenAI's
        /// `audio_start_ms` counts from there.
        var baseS: Double?
        var sentS = 0.0
        var speechStart: [String: Double] = [:]
        init(task: URLSessionWebSocketTask, tokenId: String) {
            self.task = task
            self.tokenId = tokenId
        }
    }

    private var token: Token?
    /// Socket receiving the audio.
    private var active: Connection?
    /// Socket waiting for `session.created` (first connection, reconnection or rotation).
    private var opening: Connection?
    private var retiring: [Connection] = []
    private var pending = Data()
    private var pendingStartS = 0.0
    private var backlog: [(frame: Data, startS: Double)] = []
    /// End of the last frame sent on any socket.
    private var lastSentEndS: Double?
    private var partials: [String: String] = [:]
    private var status = Status()
    private var fetching = false
    private var retryScheduled = false
    private var attempt = 0
    private var finished = false
    private var ping: DispatchSourceTimer?

    init(sessionId: String, dir: URL, store: TranscriptStore = .shared,
         onEvent: @escaping (Event) -> Void) {
        self.sessionId = sessionId
        self.dir = dir
        self.store = store
        self.onEvent = onEvent
    }

    // MARK: - Public

    func start() {
        queue.async {
            self.startPing()
            self.publish()
            self.requestToken()
        }
    }

    /// The recording ended: commit what is buffered, close, report usage.
    func finish() {
        queue.async { self.end(error: nil) }
    }

    /// Audio tap thread: converts and hands the audio over; never blocks.
    func feed(_ buffer: AVAudioPCMBuffer, at positionS: Double) {
        acceptLock.lock()
        let open = accepting
        acceptLock.unlock()
        guard open, let data = convert(buffer) else { return }
        queue.async { self.accept(data, startS: positionS) }
    }

    // MARK: - Audio

    private func convert(_ buffer: AVAudioPCMBuffer) -> Data? {
        guard let outFormat, buffer.frameLength > 0, buffer.format.sampleRate > 0 else { return nil }
        if converter?.inputFormat != buffer.format {
            converter = AVAudioConverter(from: buffer.format, to: outFormat)
        }
        guard let converter else { return nil }
        let ratio = outFormat.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard let out = AVAudioPCMBuffer(pcmFormat: outFormat, frameCapacity: capacity) else { return nil }
        var given = false
        var error: NSError?
        let result = converter.convert(to: out, error: &error) { _, inputStatus in
            if given {
                inputStatus.pointee = .noDataNow
                return nil
            }
            given = true
            inputStatus.pointee = .haveData
            return buffer
        }
        guard result != .error, out.frameLength > 0, let samples = out.int16ChannelData else {
            if let error { log.error("convert: \(error.localizedDescription, privacy: .public)") }
            return nil
        }
        return Data(bytes: samples[0], count: Int(out.frameLength) * MemoryLayout<Int16>.size)
    }

    private func accept(_ data: Data, startS: Double) {
        guard !finished else { return }
        if pending.isEmpty { pendingStartS = startS }
        pending.append(data)
        while pending.count >= Self.frameBytes {
            let frame = Data(pending.prefix(Self.frameBytes))
            pending = Data(pending.dropFirst(Self.frameBytes))
            enqueue(frame, startS: pendingStartS)
            pendingStartS += Self.frameSeconds
        }
    }

    private func enqueue(_ frame: Data, startS: Double) {
        if let connection = active, var current = token, current.id == connection.tokenId, backlog.isEmpty {
            if current.sentS + Self.frameSeconds <= current.reservedS {
                send(frame, startS: startS, on: connection)
                current.sentS += Self.frameSeconds
                token = current
                if current.sentS >= current.reservedS - Self.rotateMarginS { requestToken() }
                return
            }
            // Budget used up and no further token allowed (out of minutes): wind down.
            if let message = finalAfterBudget { return end(error: message) }
        }
        backlog.append((frame, startS))
        if backlog.count > Self.backlogLimit { backlog.removeFirst(backlog.count - Self.backlogLimit) }
        requestToken()
    }

    private func send(_ frame: Data, startS: Double, on connection: Connection) {
        if connection.baseS == nil { connection.baseS = startS }
        connection.sentS += Self.frameSeconds
        lastSentEndS = startS + Self.frameSeconds
        // Base64 needs no JSON escaping.
        let message = "{\"type\":\"input_audio_buffer.append\",\"audio\":\"\(frame.base64EncodedString())\"}"
        connection.task.send(.string(message)) { [weak self] error in
            guard let self, let error else { return }
            self.queue.async { self.lost(connection, reason: error.localizedDescription) }
        }
    }

    // MARK: - Tokens

    private enum TokenFailure: Error {
        /// Try again later (network, server busy).
        case transient(String)
        /// Will not work for this recording (not allowed, out of minutes, signed out).
        case final(String)
    }

    /// Fetches a token unless one is on the way; with a usable token in hand
    /// (reconnection), opens a socket with it instead.
    private func requestToken() {
        guard !finished, !fetching, !retryScheduled, opening == nil, finalAfterBudget == nil else { return }
        let needNew = token.map { t in
            t.sentS >= t.reservedS - Self.rotateMarginS
                || t.expiresAt.timeIntervalSinceNow < Self.expiryMarginS
        } ?? true
        if !needNew, let current = token {
            connect(with: current)
            return
        }
        fetching = true
        let sessionId = self.sessionId
        Task {
            let result = await Self.fetchToken(sessionId: sessionId)
            self.queue.async { self.tokenArrived(result) }
        }
    }

    private func tokenArrived(_ result: Result<Token, TokenFailure>) {
        fetching = false
        switch result {
        case .success(let fresh):
            if finished {
                Self.report(tokenId: fresh.id, seconds: 0, log: log)
                return
            }
            // From here on audio only goes out with the new token (frames wait
            // in the backlog until its socket is open): the old one is done.
            if let old = token, old.id != fresh.id {
                Self.report(tokenId: old.id, seconds: old.sentS, log: log)
            }
            token = fresh
            connect(with: fresh)
        case .failure(.transient(let message)):
            log.warning("token: \(message, privacy: .public)")
            setError(message)
            scheduleRetry()
        case .failure(.final(let message)):
            log.warning("token refused: \(message, privacy: .public)")
            // Out of minutes: the current token streams to its end, nothing after.
            if let current = token, active != nil, current.sentS < current.reservedS {
                setError(message)
                finalAfterBudget = message
            } else {
                end(error: message)
            }
        }
    }

    /// A final token refusal while the current token still has budget.
    private var finalAfterBudget: String?

    private func scheduleRetry() {
        guard !finished, !retryScheduled else { return }
        retryScheduled = true
        let delay = Self.backoffS[min(attempt, Self.backoffS.count - 1)]
        attempt += 1
        queue.asyncAfter(deadline: .now() + delay) {
            self.retryScheduled = false
            self.requestToken()
        }
    }

    // MARK: - Sockets

    private func connect(with token: Token) {
        guard let url = Self.realtimeURL else { return end(error: "Địa chỉ chép trực tiếp không hợp lệ.") }
        var request = URLRequest(url: url)
        request.setValue("Bearer \(token.value)", forHTTPHeaderField: "Authorization")
        let connection = Connection(task: sockets.webSocketTask(with: request), tokenId: token.id)
        opening = connection
        connection.task.resume()
        receive(on: connection)
    }

    private func receive(on connection: Connection) {
        connection.task.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                switch result {
                case .success(.string(let text)):
                    self.handle(text, on: connection)
                    self.receive(on: connection)
                case .success:
                    self.receive(on: connection)
                case .failure(let error):
                    self.lost(connection, reason: error.localizedDescription)
                }
            }
        }
    }

    private func handle(_ text: String, on connection: Connection) {
        guard let data = text.data(using: .utf8),
              let message = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let type = message["type"] as? String
        else { return }
        let itemId = message["item_id"] as? String ?? ""
        switch type {
        case "session.created", "session.updated":
            if opening === connection { promote(connection) }
        case "input_audio_buffer.speech_started":
            let ms = (message["audio_start_ms"] as? NSNumber)?.doubleValue ?? 0
            connection.speechStart[itemId] = (connection.baseS ?? lastSentEndS ?? 0) + ms / 1000
        case "conversation.item.input_audio_transcription.delta":
            let text = (partials[itemId] ?? "") + (message["delta"] as? String ?? "")
            partials[itemId] = text
            emit(.partial(id: itemId, atS: startOf(itemId, on: connection), text: text))
        case "conversation.item.input_audio_transcription.completed":
            partials[itemId] = nil
            let text = (message["transcript"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !itemId.isEmpty, !text.isEmpty else { return }
            let entry = TranscriptStore.Entry(id: itemId, atS: startOf(itemId, on: connection), text: text)
            save(entry)
        case "error":
            let error = message["error"] as? [String: Any]
            let code = error?["code"] as? String ?? "-"
            // An empty commit when retiring a quiet socket is expected.
            if code != "input_audio_buffer_commit_empty" {
                log.warning("realtime error \(code, privacy: .public): \(error?["message"] as? String ?? "", privacy: .public)")
            }
        default:
            break
        }
    }

    private func startOf(_ itemId: String, on connection: Connection) -> Double {
        connection.speechStart[itemId] ?? connection.baseS.map { $0 + connection.sentS } ?? lastSentEndS ?? 0
    }

    /// The new socket is open: audio goes there, the previous one retires.
    private func promote(_ connection: Connection) {
        opening = nil
        connection.open = true
        if let previous = active { retire(previous) }
        active = connection
        attempt = 0
        if let first = backlog.first?.startS, let last = lastSentEndS, first - last >= Self.minGapS {
            let gap = TranscriptStore.Entry(id: "gap-\(Int(last * 10))", atS: last, text: "", gapUntilS: first)
            save(gap)
        }
        status.connected = true
        status.disconnectedAtS = nil
        if finalAfterBudget == nil { status.error = nil }
        publish()
        let waiting = backlog
        backlog.removeAll()
        for item in waiting { enqueue(item.frame, startS: item.startS) }
    }

    private func retire(_ connection: Connection) {
        connection.task.send(.string("{\"type\":\"input_audio_buffer.commit\"}")) { _ in }
        retiring.append(connection)
        queue.asyncAfter(deadline: .now() + Self.retireDelayS) {
            connection.task.cancel(with: .normalClosure, reason: nil)
            self.retiring.removeAll { $0 === connection }
        }
    }

    private func lost(_ connection: Connection, reason: String) {
        if retiring.contains(where: { $0 === connection }) { return }
        let wasActive = active === connection
        guard wasActive || opening === connection else { return }
        log.warning("socket lost: \(reason, privacy: .public)")
        connection.task.cancel(with: .goingAway, reason: nil)
        if wasActive { active = nil } else { opening = nil }
        guard !finished else { return }
        if status.disconnectedAtS == nil { status.disconnectedAtS = lastSentEndS ?? pendingStartS }
        status.connected = false
        publish()
        if let message = finalAfterBudget { return end(error: message) }
        scheduleRetry()
    }

    private func startPing() {
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + Self.pingEveryS, repeating: Self.pingEveryS)
        timer.setEventHandler { [weak self] in
            guard let self, let connection = self.active else { return }
            connection.task.sendPing { error in
                guard let error else { return }
                self.queue.async { self.lost(connection, reason: "ping: \(error.localizedDescription)") }
            }
        }
        timer.resume()
        ping = timer
    }

    // MARK: - End

    private func end(error: String?) {
        guard !finished else { return }
        finished = true
        acceptLock.lock()
        accepting = false
        acceptLock.unlock()
        ping?.cancel()
        ping = nil
        opening?.task.cancel(with: .goingAway, reason: nil)
        opening = nil
        if let connection = active { retire(connection) }
        active = nil
        if let current = token { Self.report(tokenId: current.id, seconds: current.sentS, log: log) }
        token = nil
        backlog.removeAll()
        status.connected = false
        status.stopped = true
        status.error = error
        publish()
    }

    // MARK: - Output

    private func save(_ entry: TranscriptStore.Entry) {
        let store = self.store
        let dir = self.dir
        Task { await store.append(entry, dir: dir) }
        emit(.sentence(entry))
    }

    private func setError(_ message: String) {
        status.error = message
        publish()
    }

    private func publish() {
        emit(.status(status))
    }

    private func emit(_ event: Event) {
        DispatchQueue.main.async { self.onEvent(event) }
    }

    // MARK: - Backend

    private static func fetchToken(sessionId: String) async -> Result<Token, TokenFailure> {
        do {
            var (status, json) = try await post("/api/realtime/token", ["sessionId": sessionId])
            if status == 401 {
                do {
                    try await SessionAuth.refreshSession()
                } catch {
                    return .failure(.final("Phiên đăng nhập đã hết hạn — chép trực tiếp dừng, bản ghi vẫn tiếp tục."))
                }
                (status, json) = try await post("/api/realtime/token", ["sessionId": sessionId])
            }
            let message = json["error"] as? String
            switch status {
            case 200..<300:
                guard let value = json["token"] as? String, let id = json["tokenId"] as? String,
                      let expires = (json["expiresAt"] as? String).flatMap(parseDate),
                      let reserved = (json["reservedSeconds"] as? NSNumber)?.doubleValue, reserved > 0
                else { return .failure(.transient("Máy chủ trả token chép trực tiếp không đọc được.")) }
                return .success(Token(value: value, id: id, expiresAt: expires, reservedS: reserved))
            case 401:
                return .failure(.final("Phiên đăng nhập đã hết hạn — chép trực tiếp dừng, bản ghi vẫn tiếp tục."))
            // Only the backend's own `{ "error": … }` is final; a proxy page with the same status is not.
            case 400, 403:
                if let message { return .failure(.final(message)) }
                return .failure(.transient("Không lấy được token chép trực tiếp (HTTP \(status))."))
            default:
                return .failure(.transient(message ?? "Không lấy được token chép trực tiếp (HTTP \(status))."))
            }
        } catch {
            return .failure(.transient("Mất kết nối tới máy chủ: \(error.localizedDescription)"))
        }
    }

    /// Reports the seconds streamed with a retired token; a few tries, then
    /// the reservation simply stands (the backend counts it in full).
    private static func report(tokenId: String, seconds: Double, log: Logger) {
        Task {
            for attempt in 1...3 {
                do {
                    let (status, _) = try await post("/api/realtime/usage", ["tokenId": tokenId, "seconds": seconds])
                    if status == 401 { try? await SessionAuth.refreshSession() }
                    // 404/409: unknown or already settled — nothing left to do.
                    if (200..<300).contains(status) || status == 404 || status == 409 { return }
                } catch {
                    log.warning("usage report: \(error.localizedDescription, privacy: .public)")
                }
                try? await Task.sleep(nanoseconds: UInt64(attempt) * 5_000_000_000)
            }
        }
    }

    private static func post(_ path: String, _ body: [String: Any]) async throws -> (Int, [String: Any]) {
        guard let url = URL(string: SessionAuth.backendOrigin + path) else {
            throw SessionAuth.AuthError("Đường dẫn máy chủ không hợp lệ.")
        }
        var request = URLRequest(url: url, timeoutInterval: 20)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        let (data, response) = try await URLSession.shared.data(for: request)
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        return ((response as? HTTPURLResponse)?.statusCode ?? 0, json)
    }

    private static func parseDate(_ text: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: text)
    }
}
