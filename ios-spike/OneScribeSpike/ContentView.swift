import SwiftUI

/// UI tối giản — spike này là dụng cụ đo, không phải sản phẩm. Mọi thứ hiển thị đều phục vụ việc
/// đọc kết quả sau phiên 30 phút khoá màn hình.
struct ContentView: View {
    @StateObject private var rec = Recorder()
    @ObservedObject private var log = EventLog.shared

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    controls
                    Divider()
                    stats
                    Divider()
                    segmentList
                    Divider()
                    logList
                }
                .padding()
            }
            .navigationTitle("OneScribe Spike")
            .toolbar {
                ShareLink(item: log.logPath) { Image(systemName: "square.and.arrow.up") }
            }
        }
    }

    // MARK: - Điều khiển

    private var controls: some View {
        VStack(alignment: .leading, spacing: 12) {
            Picker("Độ dài đoạn", selection: $rec.segmentMinutes) {
                Text("1 phút").tag(1.0)
                Text("5 phút").tag(5.0)
                Text("10 phút").tag(10.0)
            }
            .pickerStyle(.segmented)
            .disabled(rec.state != .idle)

            Toggle("Ép dùng micro iPhone", isOn: $rec.preferBuiltInMic)
                .font(.subheadline)
                .disabled(rec.state != .idle)

            Button {
                if rec.state == .recording { rec.stop() } else { rec.start() }
            } label: {
                Label(rec.state == .recording ? "Dừng ghi" : "Bắt đầu ghi",
                      systemImage: rec.state == .recording ? "stop.circle.fill" : "mic.circle.fill")
                    .font(.title3.weight(.semibold))
                    .frame(maxWidth: .infinity, minHeight: 52)
            }
            .buttonStyle(.borderedProminent)
            .tint(rec.state == .recording ? .red : .green)
            .disabled(rec.state == .stopping)

            if !rec.lastError.isEmpty {
                Text(rec.lastError).font(.footnote).foregroundStyle(.red)
            }

            HStack {
                Text(Self.clock(rec.elapsed)).font(.system(size: 34, weight: .bold, design: .monospaced))
                Spacer()
                Text(rec.state.rawValue).font(.caption).foregroundStyle(.secondary)
            }

            // Mức âm thanh — cách nhanh nhất để biết mic còn sống khi vừa mở khoá máy ra xem.
            ProgressView(value: Double(min(1, rec.level * 4)))
                .tint(.green)

            Text("Bấm Ghi → khoá màn hình → chờ 30 phút → mở khoá → bấm Dừng. "
                 + "Muốn thử gián đoạn: gọi vào máy giữa chừng, hoặc rút/cắm AirPods.")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Chỉ số

    private var stats: some View {
        VStack(alignment: .leading, spacing: 6) {
            row("Gián đoạn (cuộc gọi/Siri)", "\(rec.interruptions)")
            row("Đổi route thiết bị", "\(rec.routeChanges)")
            row("Engine dựng lại", "\(rec.engineRestarts)")
            row("Lỗi ghi file", "\(rec.writeErrors)", warn: rec.writeErrors > 0)
            row("Pin đầu → hiện tại", Self.battery(rec.batteryStart, rec.batteryNow))
            row("Nguồn", rec.powerState, warn: rec.powerState != "KHÔNG-sạc")
            row("Trạng thái nhiệt", rec.thermal, warn: rec.thermal == "serious" || rec.thermal == "critical")
        }
    }

    private func row(_ label: String, _ value: String, warn: Bool = false) -> some View {
        HStack {
            Text(label).font(.subheadline).foregroundStyle(.secondary)
            Spacer()
            Text(value).font(.subheadline.monospacedDigit()).foregroundStyle(warn ? Color.red : Color.primary)
        }
    }

    // MARK: - Danh sách đoạn

    private var segmentList: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Đoạn ghi (\(rec.segments.count))").font(.headline)
            if rec.segments.isEmpty {
                Text("Chưa có đoạn nào.").font(.footnote).foregroundStyle(.secondary)
            }
            ForEach(rec.segments) { seg in
                HStack(alignment: .firstTextBaseline) {
                    Text(seg.url.lastPathComponent).font(.subheadline.monospaced())
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("\(Self.secs(seg.expectedDuration)) · \(Self.mb(seg.bytes))")
                            .font(.caption.monospacedDigit())
                        if let intact = seg.isIntact {
                            Text(intact
                                 ? "file OK (\(Self.secs(seg.probedDuration ?? 0)))"
                                 : "LỆCH — file \(Self.secs(seg.probedDuration ?? 0))")
                                .font(.caption2)
                                .foregroundStyle(intact ? Color.green : Color.red)
                        }
                    }
                }
            }
            if rec.state == .idle && !rec.segments.isEmpty {
                Text("Kiểm tra ở Files → Trên iPhone của tôi → OneScribeSpike → recordings/")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Log

    private var logList: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Nhật ký").font(.headline)
            ForEach(Array(log.lines.prefix(120).enumerated()), id: \.offset) { item in
                Text(item.element)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(Self.tint(for: item.element))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    // MARK: - Định dạng

    private static func tint(for line: String) -> Color {
        if line.contains("ERROR") || line.contains("FAIL") || line.contains("LỆCH") { return .red }
        if line.contains("WARN") || line.contains("INTERRUPT") { return .orange }
        if line.contains("LOCK") || line.contains("UNLOCK") { return .blue }
        return .primary
    }

    private static func clock(_ t: TimeInterval) -> String {
        let total = Int(t)
        return String(format: "%02d:%02d:%02d", total / 3600, (total % 3600) / 60, total % 60)
    }

    private static func secs(_ t: Double) -> String { String(format: "%.1fs", t) }

    private static func mb(_ bytes: Int64) -> String {
        bytes >= 1_048_576
            ? String(format: "%.1f MB", Double(bytes) / 1_048_576)
            : "\(max(0, bytes / 1024)) KB"
    }

    private static func battery(_ start: Float, _ now: Float) -> String {
        guard start >= 0, now >= 0 else { return "n/a" }
        return String(format: "%.2f%% → %.2f%%  (−%.2f%%)", start * 100, now * 100, (start - now) * 100)
    }
}
