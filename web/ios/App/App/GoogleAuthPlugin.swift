import AuthenticationServices
import Capacitor
import CryptoKit
import Foundation

/// Google sign-in without the Firebase iOS SDK (plan D5.3): the system
/// browser sheet runs Google OAuth (authorization code + PKCE) with the iOS
/// OAuth client of the Firebase project, then Firebase's REST
/// `accounts:signInWithIdp` trades the Google ID token for a Firebase ID
/// token — the only token the backend accepts (`jwks.rs` pins securetoken).
/// Client id, reversed scheme and API key come from GoogleService-Info.plist.
@objc(GoogleAuthPlugin)
public class GoogleAuthPlugin: CAPPlugin, CAPBridgedPlugin, ASWebAuthenticationPresentationContextProviding {
    public let identifier = "GoogleAuthPlugin"
    public let jsName = "GoogleAuth"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "signIn", returnType: CAPPluginReturnPromise)
    ]

    private var session: ASWebAuthenticationSession?

    private struct Config {
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

    @objc func signIn(_ call: CAPPluginCall) {
        Task {
            do {
                let config = try Config.load()
                let googleIdToken = try await googleIdToken(config)
                let firebaseIdToken = try await firebaseIdToken(googleIdToken, config)
                call.resolve(["idToken": firebaseIdToken])
            } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
                call.reject("Cửa sổ đăng nhập đã đóng trước khi hoàn tất.", "CANCELLED")
            } catch {
                call.reject(error.localizedDescription, "SIGN_IN_FAILED")
            }
        }
    }

    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        bridge?.viewController?.view.window ?? ASPresentationAnchor()
    }

    // MARK: Google OAuth (authorization code + PKCE)

    private func googleIdToken(_ config: Config) async throws -> String {
        let verifier = Self.randomURLSafe(32)
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncoded()
        let state = Self.randomURLSafe(16)
        let redirectURI = "\(config.reversedClientId):/oauth2redirect"

        var components = URLComponents(string: "https://accounts.google.com/o/oauth2/v2/auth")!
        components.queryItems = [
            URLQueryItem(name: "client_id", value: config.clientId),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: "openid email profile"),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "prompt", value: "select_account"),
        ]
        guard let authURL = components.url else { throw AuthError("Không dựng được URL đăng nhập.") }

        let callbackURL = try await presentSession(url: authURL, scheme: config.reversedClientId)
        let items = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false)?.queryItems ?? []
        guard items.first(where: { $0.name == "state" })?.value == state else {
            throw AuthError("Phản hồi đăng nhập không hợp lệ (state).")
        }
        if let error = items.first(where: { $0.name == "error" })?.value {
            throw AuthError("Google từ chối đăng nhập: \(error)")
        }
        guard let code = items.first(where: { $0.name == "code" })?.value else {
            throw AuthError("Google không trả mã đăng nhập.")
        }

        let body = Self.formEncoded([
            "client_id": config.clientId,
            "code": code,
            "code_verifier": verifier,
            "redirect_uri": redirectURI,
            "grant_type": "authorization_code",
        ])
        var request = URLRequest(url: URL(string: "https://oauth2.googleapis.com/token")!)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        let json = try await Self.sendJSON(request, failure: "Không đổi được mã đăng nhập Google")
        guard let idToken = json["id_token"] as? String else { throw AuthError("Google không trả ID token.") }
        return idToken
    }

    @MainActor
    private func presentSession(url: URL, scheme: String) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(url: url, callbackURLScheme: scheme) { callbackURL, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let callbackURL {
                    continuation.resume(returning: callbackURL)
                } else {
                    continuation.resume(throwing: AuthError("Đăng nhập không trả kết quả."))
                }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: AuthError("Không mở được cửa sổ đăng nhập."))
            }
        }
    }

    // MARK: Firebase REST

    private func firebaseIdToken(_ googleIdToken: String, _ config: Config) async throws -> String {
        var components = URLComponents(string: "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp")!
        components.queryItems = [URLQueryItem(name: "key", value: config.apiKey)]
        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        // The iOS API key is restricted to this bundle id.
        request.setValue(config.bundleId, forHTTPHeaderField: "X-Ios-Bundle-Identifier")
        let postBody = "id_token=\(googleIdToken)&providerId=google.com"
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "postBody": postBody,
            "requestUri": "http://localhost",
            "returnSecureToken": true,
            "returnIdpCredential": true,
        ])
        let json = try await Self.sendJSON(request, failure: "Firebase không chấp nhận đăng nhập Google")
        guard let idToken = json["idToken"] as? String else { throw AuthError("Firebase không trả ID token.") }
        return idToken
    }

    // MARK: Helpers

    private static func sendJSON(_ request: URLRequest, failure: String) async throws -> [String: Any] {
        let (data, response) = try await URLSession.shared.data(for: request)
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

    private static func formEncoded(_ fields: [String: String]) -> Data {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        return fields
            .map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: allowed) ?? $0.value)" }
            .joined(separator: "&")
            .data(using: .utf8) ?? Data()
    }

    private static func randomURLSafe(_ bytes: Int) -> String {
        var data = Data(count: bytes)
        _ = data.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, bytes, $0.baseAddress!) }
        return data.base64URLEncoded()
    }
}

private extension Data {
    func base64URLEncoded() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
