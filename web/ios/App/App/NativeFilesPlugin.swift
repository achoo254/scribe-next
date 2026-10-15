import Capacitor
import Foundation
import UIKit

/// Downloads that the WebView cannot do itself (plan D3): `<audio src>` and
/// `window.location` would leave the app's origin without the session
/// cookie. URLSession.shared carries the cookie CapacitorHttp stored at
/// sign-in. Only the backend host is accepted, so the cookie never goes
/// anywhere else.
@objc(NativeFilesPlugin)
public class NativeFilesPlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "NativeFilesPlugin"
    public let jsName = "NativeFiles"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "cache", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "share", returnType: CAPPluginReturnPromise),
    ]

    static let backendHost = "scribe.inetdev.io.vn"

    struct FilesError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Downloads once into Caches/media and returns a WebView-playable URL.
    @objc func cache(_ call: CAPPluginCall) {
        Task {
            do {
                let url = try backendURL(call)
                let dir = try Self.directory("media")
                let key = url.path.replacingOccurrences(of: "/", with: "_")
                if let cached = try? FileManager.default
                    .contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
                    .first(where: { $0.deletingPathExtension().lastPathComponent == key }) {
                    call.resolve(["url": portable(cached)])
                    return
                }
                let (temp, response) = try await download(url)
                let ext = (response.suggestedFilename as NSString?)?.pathExtension ?? ""
                let target = dir.appendingPathComponent(key).appendingPathExtension(ext)
                try? FileManager.default.removeItem(at: target)
                try FileManager.default.moveItem(at: temp, to: target)
                call.resolve(["url": portable(target)])
            } catch {
                call.reject(error.localizedDescription, "DOWNLOAD_FAILED")
            }
        }
    }

    /// Downloads under the server's file name, then opens the share sheet.
    @objc func share(_ call: CAPPluginCall) {
        Task {
            do {
                let url = try backendURL(call)
                let (temp, response) = try await download(url)
                let name = response.suggestedFilename ?? url.lastPathComponent
                let dir = try Self.directory("share-\(UUID().uuidString)")
                let target = dir.appendingPathComponent(name)
                try FileManager.default.moveItem(at: temp, to: target)
                await MainActor.run { presentShareSheet(for: target, call: call) }
            } catch {
                call.reject(error.localizedDescription, "DOWNLOAD_FAILED")
            }
        }
    }

    @MainActor
    private func presentShareSheet(for file: URL, call: CAPPluginCall) {
        guard let controller = bridge?.viewController else {
            call.reject("Không mở được bảng chia sẻ.", "NO_VIEW")
            return
        }
        let sheet = UIActivityViewController(activityItems: [file], applicationActivities: nil)
        // iPad needs an anchor for the popover.
        sheet.popoverPresentationController?.sourceView = controller.view
        sheet.popoverPresentationController?.sourceRect = CGRect(
            x: controller.view.bounds.midX, y: controller.view.bounds.maxY, width: 0, height: 0)
        sheet.completionWithItemsHandler = { _, _, _, _ in
            try? FileManager.default.removeItem(at: file.deletingLastPathComponent())
        }
        controller.present(sheet, animated: true)
        call.resolve()
    }

    private func backendURL(_ call: CAPPluginCall) throws -> URL {
        guard
            let raw = call.getString("url"),
            let url = URL(string: raw),
            url.scheme == "https",
            url.host == Self.backendHost,
            url.path.hasPrefix("/api/")
        else { throw FilesError(message: "Đường dẫn tải không hợp lệ.") }
        return url
    }

    private func download(_ url: URL) async throws -> (URL, URLResponse) {
        let (temp, response) = try await URLSession.shared.download(from: url)
        guard let http = response as? HTTPURLResponse else {
            throw FilesError(message: "Máy chủ không trả lời.")
        }
        guard (200..<300).contains(http.statusCode) else {
            try? FileManager.default.removeItem(at: temp)
            throw FilesError(message: http.statusCode == 401
                ? "Phiên đăng nhập đã hết — đăng nhập lại rồi thử lại."
                : "Tải tệp thất bại (HTTP \(http.statusCode)).")
        }
        return (temp, response)
    }

    private func portable(_ file: URL) -> String {
        bridge?.portablePath(fromLocalURL: file)?.absoluteString ?? file.absoluteString
    }

    private static func directory(_ name: String) throws -> URL {
        let base = try FileManager.default.url(
            for: .cachesDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        let dir = base.appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
}
