import AuthenticationServices
import Capacitor
import CryptoKit
import Foundation

/// Sign-in through hoandat-auth (OIDC, public client `scribe-mobile`): the
/// system browser sheet runs Code + PKCE against the IdP; the app makes and
/// keeps the PKCE verifier and nonce, so a code intercepted by another app
/// cannot be exchanged. The backend then redeems the code, keeps the IdP
/// refresh token server-side and returns a device token of its own, stored
/// here in the Keychain (`SessionAuth`) so uploads can renew the backend
/// session later without the sheet.
@objc(HoandatAuthPlugin)
public class HoandatAuthPlugin: CAPPlugin, CAPBridgedPlugin, ASWebAuthenticationPresentationContextProviding {
    public let identifier = "HoandatAuthPlugin"
    public let jsName = "HoandatAuth"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "signIn", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "signOut", returnType: CAPPluginReturnPromise),
    ]

    private var session: ASWebAuthenticationSession?

    @objc func signIn(_ call: CAPPluginCall) {
        Task {
            do {
                let email = try await signIn()
                call.resolve(["email": email])
            } catch let error as ASWebAuthenticationSessionError where error.code == .canceledLogin {
                call.reject(localized("signIn.cancelled", "Cửa sổ đăng nhập đã đóng trước khi hoàn tất."), "CANCELLED")
            } catch {
                call.reject(error.localizedDescription, "SIGN_IN_FAILED")
            }
        }
    }

    /// Ends the backend session and forgets this device (the IdP grant goes with it).
    @objc func signOut(_ call: CAPPluginCall) {
        Task {
            await SessionAuth.signOut()
            call.resolve()
        }
    }

    public func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        bridge?.viewController?.view.window ?? ASPresentationAnchor()
    }

    private struct NativeConfig {
        let authorizationEndpoint: String
        let clientId: String
        let redirectUri: String
        let scope: String
    }

    private func signIn() async throws -> String {
        let config = try await loadConfig()
        let verifier = try Self.randomURLSafe(32)
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncoded()
        let state = try Self.randomURLSafe(16)
        let nonce = try Self.randomURLSafe(16)
        guard var components = URLComponents(string: config.authorizationEndpoint),
              let scheme = URL(string: config.redirectUri)?.scheme
        else { throw SessionAuth.AuthError(localized("signIn.badConfig", "Cấu hình đăng nhập không hợp lệ.")) }
        components.queryItems = [
            URLQueryItem(name: "client_id", value: config.clientId),
            URLQueryItem(name: "redirect_uri", value: config.redirectUri),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: config.scope),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "nonce", value: nonce),
        ]
        guard let authURL = components.url else { throw SessionAuth.AuthError(localized("signIn.badURL", "Đường dẫn đăng nhập không hợp lệ.")) }

        let callback = try await presentSession(url: authURL, scheme: scheme)
        let items = URLComponents(url: callback, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let value = { (name: String) in items.first(where: { $0.name == name })?.value }
        if let error = value("error") {
            throw SessionAuth.AuthError(error == "access_denied" ? localized("signIn.denied", "Bạn đã từ chối đăng nhập.") : localized("signIn.failed", "Đăng nhập thất bại (%@).", error))
        }
        guard value("state") == state, let code = value("code") else {
            throw SessionAuth.AuthError(localized("signIn.stateMismatch", "Phản hồi đăng nhập không khớp, thử lại."))
        }
        return try await SessionAuth.exchange(code: code, verifier: verifier, nonce: nonce)
    }

    private func loadConfig() async throws -> NativeConfig {
        guard let url = URL(string: "\(SessionAuth.backendOrigin)/api/auth/native/config") else {
            throw SessionAuth.AuthError(localized("server.badURL", "Đường dẫn máy chủ không hợp lệ."))
        }
        let json = try await SessionAuth.sendJSON(URLRequest(url: url), failure: localized("signIn.configFailed", "Không lấy được cấu hình đăng nhập"))
        guard let endpoint = json["authorizationEndpoint"] as? String,
              let clientId = json["clientId"] as? String,
              let redirectUri = json["redirectUri"] as? String,
              let scope = json["scope"] as? String
        else { throw SessionAuth.AuthError(localized("signIn.configIncomplete", "Cấu hình đăng nhập thiếu trường.")) }
        return NativeConfig(authorizationEndpoint: endpoint, clientId: clientId, redirectUri: redirectUri, scope: scope)
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
                    continuation.resume(throwing: SessionAuth.AuthError(localized("signIn.noResult", "Đăng nhập không trả kết quả.")))
                }
            }
            session.presentationContextProvider = self
            // Reuse the IdP browser session: a user already signed in at hoandat-auth taps once.
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: SessionAuth.AuthError(localized("signIn.sheetFailed", "Không mở được cửa sổ đăng nhập.")))
            }
        }
    }

    private static func randomURLSafe(_ count: Int) throws -> String {
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess else {
            throw SessionAuth.AuthError(localized("signIn.randomFailed", "Không tạo được mã ngẫu nhiên."))
        }
        return Data(bytes).base64URLEncoded()
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
