import Capacitor
import Foundation
import Security

/// Generic passwords in this app's own Keychain (no Keychain Sharing needed,
/// so it works with free provisioning). Readable after the first unlock,
/// so background uploads on the lock screen can still use them.
enum Keychain {
    private static func query(_ service: String, _ account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    /// False when the Keychain refused the item.
    @discardableResult
    static func save(_ value: String, service: String, account: String) -> Bool {
        let base = query(service, account)
        SecItemDelete(base as CFDictionary)
        var item = base
        item[kSecValueData as String] = Data(value.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        return SecItemAdd(item as CFDictionary, nil) == errSecSuccess
    }

    static func read(service: String, account: String) -> String? {
        var item = query(service, account)
        item[kSecReturnData as String] = true
        item[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?
        guard SecItemCopyMatching(item as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(service: String, account: String) {
        SecItemDelete(query(service, account) as CFDictionary)
    }
}

/// Values the WebView asks the app to keep in the Keychain: the user's own
/// OpenAI key for transcription. Only names in `allowed` are accepted, so
/// the page cannot read other Keychain items (the Firebase refresh token).
@objc(SecretsPlugin)
public class SecretsPlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "SecretsPlugin"
    public let jsName = "Secrets"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "get", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "set", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "remove", returnType: CAPPluginReturnPromise),
    ]

    private static let service = "vn.inet.onescribe.secrets"
    private static let allowed: Set<String> = ["openaiApiKey"]

    private func name(_ call: CAPPluginCall) -> String? {
        guard let name = call.getString("name"), Self.allowed.contains(name) else {
            call.reject("Tên khoá không hợp lệ.", "BAD_NAME")
            return nil
        }
        return name
    }

    @objc func get(_ call: CAPPluginCall) {
        guard let name = name(call) else { return }
        if let value = Keychain.read(service: Self.service, account: name) {
            call.resolve(["value": value])
        } else {
            call.resolve([:])
        }
    }

    @objc func set(_ call: CAPPluginCall) {
        guard let name = name(call) else { return }
        guard let value = call.getString("value"), !value.isEmpty else {
            call.reject("Thiếu giá trị.", "NO_VALUE")
            return
        }
        if Keychain.save(value, service: Self.service, account: name) {
            call.resolve()
        } else {
            call.reject("Không lưu được vào Keychain.", "KEYCHAIN")
        }
    }

    @objc func remove(_ call: CAPPluginCall) {
        guard let name = name(call) else { return }
        Keychain.delete(service: Self.service, account: name)
        call.resolve()
    }
}
