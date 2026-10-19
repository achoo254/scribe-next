import Foundation

/// Backend session of the native side. Sign-in (HoandatAuthPlugin) leaves
/// a device token of the backend in the app's own Keychain; when the
/// 12-hour session cookie expires during a long recording the upload queue
/// trades it for a new cookie (`POST /api/auth/native/refresh`), which the
/// backend grants only while hoandat-auth still accepts this device. The
/// cookie lands in HTTPCookieStorage, which CapacitorHttp and
/// URLSession.shared both use.
enum SessionAuth {
    /// Backend origin, set at build time (`SCRIBE_BACKEND_ORIGIN` → Info.plist
    /// `ScribeBackendOrigin`; `pnpm run ios:device` fills it from web/.env).
    static let backendOrigin: String = {
        guard let origin = Bundle.main.object(forInfoDictionaryKey: "ScribeBackendOrigin") as? String,
              let host = URL(string: origin)?.host, !host.isEmpty else {
            fatalError("ScribeBackendOrigin is missing: build with SCRIBE_BACKEND_ORIGIN set (pnpm run ios:device)")
        }
        return origin
    }()

    /// Host part of `backendOrigin`, for matching request URLs.
    static var backendHost: String { URL(string: backendOrigin)?.host ?? "" }

    /// The backend says the account behind this device was deleted (410 on refresh): what the phone still
    /// keeps for it (segments not uploaded, bookmarks) must go, never into a later account.
    struct AccountDeleted: LocalizedError {
        var errorDescription: String? { "Tài khoản Scribe đã bị xoá." }
    }

    struct AuthError: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }

    // MARK: Keychain

    private static let service = "com.hoandat.scribe.session"
    private static let account = "deviceToken"
    /// Firebase refresh token of older builds: removed on the next sign-in or sign-out.
    private static let legacyService = "com.hoandat.scribe.firebase"

    static func deviceToken() -> String? {
        Keychain.read(service: service, account: account)
    }

    /// Sign-out (or a revoked device): the next sign-in starts from the IdP sheet again.
    static func clear() {
        Keychain.delete(service: service, account: account)
        Keychain.delete(service: legacyService, account: "refreshToken")
    }

    // MARK: Sign-in

    /// Redeems the IdP code at the backend (session cookie set by the
    /// response) and keeps the device token. Returns the signed-in email.
    static func exchange(code: String, verifier: String, nonce: String) async throws -> String {
        let json = try await post("/api/auth/native/exchange",
                                  ["code": code, "codeVerifier": verifier, "nonce": nonce],
                                  failure: "Máy chủ không cấp phiên đăng nhập")
        guard let token = json["deviceToken"] as? String else {
            throw AuthError("Máy chủ không trả mã thiết bị.")
        }
        clear()
        Keychain.save(token, service: service, account: account)
        return json["email"] as? String ?? ""
    }

    static func signOut() async {
        var body: [String: Any] = [:]
        if let token = deviceToken() { body["deviceToken"] = token }
        _ = try? await post("/api/auth/logout", body, failure: "Đăng xuất")
        clear()
    }

    // MARK: Refresh

    private static let renewal = Renewal()

    /// Gets a new backend session cookie from the stored device token. Throws
    /// when there is none or the backend refuses it (device revoked at the
    /// IdP); the caller then asks the user to sign in again.
    static func refreshSession(session: URLSession = .shared) async throws {
        try await renewal.run { try await renew(session: session) }
    }

    private static func renew(session: URLSession) async throws {
        guard let token = deviceToken() else { throw AuthError("Chưa có phiên đăng nhập để làm mới.") }
        do {
            _ = try await post("/api/auth/native/refresh", ["deviceToken": token],
                               session: session, failure: "Máy chủ không cấp lại phiên")
        } catch let error as HTTPError where error.status == 409 {
            // Another renewal of this device is running (e.g. from the WebView): it sets the cookie.
            try await Task.sleep(nanoseconds: 2_000_000_000)
        } catch let error as HTTPError where error.status == 401 {
            clear()
            throw AuthError(error.message)
        } catch let error as HTTPError where error.status == 410 {
            clear()
            throw AccountDeleted()
        }
    }

    /// One renewal at a time: the IdP refresh token behind the device is single-use.
    private actor Renewal {
        private var running: Task<Void, Error>?

        func run(_ work: @escaping @Sendable () async throws -> Void) async throws {
            if let running { return try await running.value }
            let task = Task { try await work() }
            running = task
            defer { running = nil }
            try await task.value
        }
    }

    // MARK: Helpers

    /// HTTP failure with its status (renewal branches on 401/409).
    struct HTTPError: LocalizedError {
        let status: Int
        let message: String
        var errorDescription: String? { message }
    }

    static func sendJSON(_ request: URLRequest, session: URLSession = .shared,
                         failure: String) async throws -> [String: Any] {
        let (data, response) = try await session.data(for: request)
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(status) else {
            let detail = (json["error"] as? String) ?? "HTTP \(status)"
            throw HTTPError(status: status, message: "\(failure): \(detail)")
        }
        return json
    }

    static func post(_ path: String, _ body: [String: Any], session: URLSession = .shared,
                     failure: String) async throws -> [String: Any] {
        guard let url = URL(string: backendOrigin + path) else { throw AuthError("Đường dẫn máy chủ không hợp lệ.") }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        return try await sendJSON(request, session: session, failure: failure)
    }
}
