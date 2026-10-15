import Foundation
import Security

/// Backend session of the native side (plan D5). Sign-in keeps the Firebase
/// refresh token in the app's own Keychain (no Keychain Sharing needed),
/// so when the 12-hour session cookie expires during a long recording the
/// upload queue gets a new one without any UI: refresh token → Firebase ID
/// token (securetoken API) → `POST /api/auth/session` → cookie in
/// HTTPCookieStorage, which CapacitorHttp and URLSession.shared both use.
enum SessionAuth {
    static let backendOrigin = "https://scribe.inetdev.io.vn"

    /// Google/Firebase client config, from GoogleService-Info.plist.
    struct Config {
        let clientId: String
        let reversedClientId: String
        let apiKey: String
        let bundleId: String

        static func load() throws -> Config {
            guard
                let url = Bundle.main.url(forResource: "GoogleService-Info", withExtension: "plist"),
                let dict = NSDictionary(contentsOf: url) as? [String: Any],
                let clientId = dict["CLIENT_ID"] as? String,
                let reversed = dict["REVERSED_CLIENT_ID"] as? String,
                let apiKey = dict["API_KEY"] as? String,
                let bundleId = dict["BUNDLE_ID"] as? String
            else { throw AuthError("Thiếu cấu hình GoogleService-Info.plist trong app.") }
            return Config(clientId: clientId, reversedClientId: reversed, apiKey: apiKey, bundleId: bundleId)
        }
    }

    struct AuthError: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }

    // MARK: Keychain

    private static let service = "vn.inet.onescribe.firebase"
    private static let account = "refreshToken"

    /// Readable after the first unlock, so a refresh works on the lock screen.
    static func saveRefreshToken(_ token: String) {
        let base: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(base as CFDictionary)
        var item = base
        item[kSecValueData as String] = Data(token.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        SecItemAdd(item as CFDictionary, nil)
    }

    static func refreshToken() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Sign-out: the next sign-in starts from the Google sheet again.
    static func clear() {
        SecItemDelete([
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ] as CFDictionary)
    }

    // MARK: Refresh

    /// Gets a new backend session cookie from the stored refresh token.
    /// Throws when there is none or Firebase/the backend refuses it; the
    /// caller then asks the user to sign in again.
    static func refreshSession(session: URLSession = .shared) async throws {
        guard let refresh = refreshToken() else { throw AuthError("Chưa có phiên đăng nhập để làm mới.") }
        let config = try Config.load()

        var components = URLComponents()
        components.scheme = "https"
        components.host = "securetoken.googleapis.com"
        components.path = "/v1/token"
        components.queryItems = [URLQueryItem(name: "key", value: config.apiKey)]
        guard let tokenURL = components.url, let sessionURL = URL(string: "\(backendOrigin)/api/auth/session")
        else { throw AuthError("Đường dẫn làm mới phiên không hợp lệ.") }

        var tokenRequest = URLRequest(url: tokenURL)
        tokenRequest.httpMethod = "POST"
        tokenRequest.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        tokenRequest.setValue(config.bundleId, forHTTPHeaderField: "X-Ios-Bundle-Identifier")
        tokenRequest.httpBody = formEncoded(["grant_type": "refresh_token", "refresh_token": refresh])
        let token = try await sendJSON(tokenRequest, session: session, failure: "Không làm mới được đăng nhập")
        guard let idToken = token["id_token"] as? String else { throw AuthError("Firebase không trả ID token.") }
        if let rotated = token["refresh_token"] as? String { saveRefreshToken(rotated) }

        var sessionRequest = URLRequest(url: sessionURL)
        sessionRequest.httpMethod = "POST"
        sessionRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        sessionRequest.httpBody = try JSONSerialization.data(withJSONObject: ["idToken": idToken])
        _ = try await sendJSON(sessionRequest, session: session, failure: "Máy chủ không cấp lại phiên")
    }

    // MARK: Helpers

    static func sendJSON(_ request: URLRequest, session: URLSession = .shared,
                         failure: String) async throws -> [String: Any] {
        let (data, response) = try await session.data(for: request)
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let detail = (json["error"] as? [String: Any])?["message"] as? String
                ?? json["error_description"] as? String
                ?? (json["error"] as? String)
                ?? "HTTP \((response as? HTTPURLResponse)?.statusCode ?? 0)"
            throw AuthError("\(failure): \(detail)")
        }
        return json
    }

    static func formEncoded(_ fields: [String: String]) -> Data {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        return fields
            .map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: allowed) ?? $0.value)" }
            .joined(separator: "&")
            .data(using: .utf8) ?? Data()
    }
}
