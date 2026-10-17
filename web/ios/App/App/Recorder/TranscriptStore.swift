import Foundation
import os

/// Live transcript of a recording session, kept next to its segments as
/// `transcript.jsonl` (one JSON object per line, appended). Separate from
/// `SegmentStore` so the upload manifest is never touched by the stream.
///
/// Each line is written with a single append; a line torn by a kill is
/// skipped when the file is read back, so every other line survives.
/// Lines come in arrival order (OpenAI may finish sentences out of order);
/// readers get them sorted by session position.
actor TranscriptStore {
    static let shared = TranscriptStore()
    static let fileName = "transcript.jsonl"

    /// A finished sentence, or a stretch the stream missed (`gapUntilS` set,
    /// empty `text`): the full transcript after the recording fills it.
    struct Entry: Codable, Equatable {
        /// OpenAI item id of the sentence, or `gap-…`.
        let id: String
        /// Session position (seconds of recorded audio) where it starts.
        let atS: Double
        let text: String
        var gapUntilS: Double?

        var json: [String: Any] {
            var out: [String: Any] = ["id": id, "atS": atS, "text": text]
            if let gapUntilS { out["gapUntilS"] = gapUntilS }
            return out
        }
    }

    private let log = Logger(subsystem: "com.hoandat.scribe", category: "transcript")
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()
    /// Entries per session folder, loaded from disk on first use.
    private var cache: [URL: [Entry]] = [:]

    func append(_ entry: Entry, dir: URL) {
        var entries = load(dir)
        guard !entries.contains(where: { $0.id == entry.id }) else { return }
        entries.append(entry)
        cache[dir] = entries
        do {
            // Leading newline too: a line torn by a kill never swallows the next one.
            var line = Data([0x0A])
            line.append(try encoder.encode(entry))
            line.append(0x0A)
            try appendLine(line, to: dir.appendingPathComponent(Self.fileName))
        } catch {
            // The sentence stays in memory for this run; only a relaunch loses it.
            log.error("append transcript: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Every entry of the session, by session position.
    func entries(dir: URL) -> [Entry] {
        load(dir).sorted { $0.atS < $1.atS }
    }

    private func load(_ dir: URL) -> [Entry] {
        if let cached = cache[dir] { return cached }
        let url = dir.appendingPathComponent(Self.fileName)
        let data = (try? Data(contentsOf: url)) ?? Data()
        var seen = Set<String>()
        var entries: [Entry] = []
        for line in data.split(separator: 0x0A) {
            guard let entry = try? decoder.decode(Entry.self, from: Data(line)), seen.insert(entry.id).inserted
            else { continue }
            entries.append(entry)
        }
        cache[dir] = entries
        return entries
    }

    private func appendLine(_ line: Data, to url: URL) throws {
        if !FileManager.default.fileExists(atPath: url.path) {
            // Writable while the phone is locked, like the segments.
            FileManager.default.createFile(atPath: url.path, contents: nil, attributes: [
                .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication,
            ])
        }
        let handle = try FileHandle(forWritingTo: url)
        defer { try? handle.close() }
        try handle.seekToEnd()
        try handle.write(contentsOf: line)
    }
}
