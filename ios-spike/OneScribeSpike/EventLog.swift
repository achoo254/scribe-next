import Foundation
import Combine

/// Nhật ký sự kiện của spike. Mục đích duy nhất: sau 30 phút khoá màn hình, đọc lại được chính xác
/// chuyện gì đã xảy ra (khoá máy lúc nào, có gián đoạn không, xoay đoạn lúc nào, pin tụt bao nhiêu).
/// Ghi song song ra file trong Documents để lấy được qua Finder/Files kể cả khi app bị kill.
final class EventLog: ObservableObject {
    static let shared = EventLog()

    /// Dòng log mới nhất ở đầu (UI hiển thị ngược thời gian, khỏi phải cuộn).
    @Published private(set) var lines: [String] = []

    private let queue = DispatchQueue(label: "vn.inet.onescribe.spike.log")
    private let fileURL: URL
    private let stamp: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss.SSS"
        return f
    }()

    private init() {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let name = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
        fileURL = docs.appendingPathComponent("spike-\(name).log")
        FileManager.default.createFile(atPath: fileURL.path, contents: nil)
        log("=== spike start === \(Date())")
    }

    var logPath: URL { fileURL }

    /// An toàn gọi từ mọi thread, kể cả audio render thread.
    func log(_ message: String) {
        let line = "[\(stamp.string(from: Date()))] \(message)"
        queue.async { [fileURL] in
            if let handle = try? FileHandle(forWritingTo: fileURL) {
                handle.seekToEndOfFile()
                handle.write(Data((line + "\n").utf8))
                try? handle.close()
            }
        }
        DispatchQueue.main.async {
            self.lines.insert(line, at: 0)
            if self.lines.count > 400 { self.lines.removeLast(self.lines.count - 400) }
        }
    }
}
