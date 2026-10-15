import AuthenticationServices
import Capacitor
import CryptoKit
import Foundation

/// Google sign-in without the Firebase iOS SDK (plan D5.3): the system
/// browser sheet runs Google OAuth (authorization code + PKCE) with the iOS
/// OAuth client of the Firebase project, then Firebase's REST
/// `accounts:signInWithIdp` trades the Google ID token for a Firebase ID
/// token — the only token the backend accepts (`jwks.rs` pins securetoken).
/// The Firebase refresh token is kept (SessionAuth) so uploads can renew the
/// backend session later without the sheet.
@objc(GoogleAuthPlugin)
public class GoogleAuthPlugin: CAPPlugin, CAPBridgedPlugin, ASWebAuthenticationPresentationContextProviding {
    public let identifier = "GoogleAuthPlugin"
    public let jsName = "GoogleAuth"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "signIn", returnType: CAPPluginReturnPromise)
    ]

    private var session: ASWebAuthenticationSession?

    @objc func signIn(_ call: CAPPluginCall) {
        Task {
            do {
                let config = try SessionAuth.Config.load()
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

    private func googleIdToken(_ config: SessionAuth.Config) async throws -> String {
        let verifier = try Self.randomURLSafe(32)
        let challenge = Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncoded()
        let state = try Self.randomURLSafe(16)
        let redirectURI = "\(config.reversedClientId):/oauth2redirect"

        var components = URLComponents()
        components.scheme = "https"
        components.host = "accounts.google.com"
        components.path = "/o/oauth2/v2/auth"
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
        guard let authURL = components.url, let tokenURL = URL(string: "https://oauth2.googleapis.com/token")
        else { throw SessionAuth.AuthError("Không dựng được URL đăng nhập.") }

        let callbackURL = try await presentSession(url: authURL, scheme: config.reversedClientId)
        let items = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false)?.queryItems ?? []
        guard items.first(where: { $0.name == "state" })?.value == state else {
            throw SessionAuth.AuthError("Phản hồi đăng nhập không hợp lệ (state).")
        }
        if let error = items.first(where: { $0.name == "error" })?.value {
            throw SessionAuth.AuthError("Google từ chối đăng nhập: \(error)")
        }
        guard let code = items.first(where: { $0.name == "code" })?.value else {
            throw SessionAuth.AuthError("Google không trả mã đăng nhập.")
        }

        var request = URLRequest(url: tokenURL)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.httpBody = SessionAuth.formEncoded([
            "client_id": config.clientId,
            "code": code,
            "code_verifier": verifier,
            "redirect_uri": redirectURI,
            "grant_type": "authorization_code",
        ])
        let json = try await SessionAuth.sendJSON(request, failure: "Không đổi được mã đăng nhập Google")
        guard let idToken = json["id_token"] as? String else {
            throw SessionAuth.AuthError("Google không trả ID token.")
        }
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
                    continuation.resume(throwing: SessionAuth.AuthError("Đăng nhập không trả kết quả."))
                }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: SessionAuth.AuthError("Không mở được cửa sổ đăng nhập."))
            }
        }
    }

    // MARK: Firebase REST

    private func firebaseIdToken(_ googleIdToken: String, _ config: SessionAuth.Config) async throws -> String {
        var components = URLComponents()
        components.scheme = "https"
        components.host = "identitytoolkit.googleapis.com"
        components.path = "/v1/accounts:signInWithIdp"
        components.queryItems = [URLQueryItem(name: "key", value: config.apiKey)]
        guard let url = components.url else { throw SessionAuth.AuthError("Đường dẫn Firebase không hợp lệ.") }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        // The iOS API key is restricted to this bundle id.
        request.setValue(config.bundleId, forHTTPHeaderField: "X-Ios-Bundle-Identifier")
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "postBody": "id_token=\(googleIdToken)&providerId=google.com",
            "requestUri": "http://localhost",
            "returnSecureToken": true,
            "returnIdpCredential": true,
        ])
        let json = try await SessionAuth.sendJSON(request, failure: "Firebase không chấp nhận đăng nhập Google")
        guard let idToken = json["idToken"] as? String else {
            throw SessionAuth.AuthError("Firebase không trả ID token.")
        }
        if let refresh = json["refreshToken"] as? String { SessionAuth.saveRefreshToken(refresh) }
        return idToken
    }

    private static func randomURLSafe(_ count: Int) throws -> String {
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, count, &bytes) == errSecSuccess else {
            throw SessionAuth.AuthError("Không tạo được khoá ngẫu nhiên.")
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
