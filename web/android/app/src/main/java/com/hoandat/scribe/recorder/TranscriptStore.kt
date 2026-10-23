package com.hoandat.scribe.recorder

import android.util.Log
import org.json.JSONException
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.io.IOException

/**
 * A finished sentence of the live transcript, or a stretch the stream missed ([gapUntilS] set, empty
 * [text]): the full transcript after the recording fills it.
 */
data class TranscriptEntry(
    /** Provider id (OpenAI item id, `<tokenId>-<n>` for ElevenLabs) or `gap-…`. */
    val id: String,
    /** Session position (seconds of recorded audio) where it starts. */
    val atS: Double,
    val text: String,
    val gapUntilS: Double? = null,
) {
    fun toJson(): JSONObject = JSONObject().put("id", id).put("atS", atS).put("text", text)
        .apply { gapUntilS?.let { put("gapUntilS", it) } }

    companion object {
        fun fromJson(json: JSONObject): TranscriptEntry? {
            val id = json.optString("id")
            val atS = json.optDouble("atS", Double.NaN)
            if (id.isEmpty() || !atS.isFinite() || !json.has("text")) return null
            val gap = json.optDouble("gapUntilS", Double.NaN).takeIf { it.isFinite() }
            return TranscriptEntry(id, atS, json.optString("text"), gap)
        }
    }
}

/**
 * Live transcript of a recording session, kept next to its segments as `transcript.jsonl` (one JSON
 * object per line, appended), like TranscriptStore.swift. Each line is written with a single append and
 * a leading newline, so a line torn by a kill never swallows the next one and is skipped when read back.
 * Readers get entries sorted by session position.
 */
class TranscriptStore {
    /** Entries per session folder, loaded from disk on first use. */
    private val cache = HashMap<String, MutableList<TranscriptEntry>>()

    @Synchronized
    fun append(entry: TranscriptEntry, dir: File) {
        val entries = load(dir)
        if (entries.any { it.id == entry.id }) return
        entries.add(entry)
        try {
            dir.mkdirs()
            FileOutputStream(File(dir, FILE_NAME), true).use { out ->
                out.write("\n${entry.toJson()}\n".toByteArray(Charsets.UTF_8))
            }
        } catch (e: IOException) {
            // The sentence stays in memory for this run; only a relaunch loses it.
            Log.e(TAG, "append transcript", e)
        }
    }

    /** Every entry of the session, by session position. */
    @Synchronized
    fun entries(dir: File): List<TranscriptEntry> = load(dir).sortedBy { it.atS }

    private fun load(dir: File): MutableList<TranscriptEntry> {
        cache[dir.path]?.let { return it }
        val file = File(dir, FILE_NAME)
        val seen = HashSet<String>()
        val entries = ArrayList<TranscriptEntry>()
        val text = try {
            if (file.isFile) file.readText(Charsets.UTF_8) else ""
        } catch (e: IOException) {
            ""
        }
        for (line in text.split('\n')) {
            if (line.isBlank()) continue
            val entry = try {
                TranscriptEntry.fromJson(JSONObject(line))
            } catch (e: JSONException) {
                null
            } ?: continue
            if (seen.add(entry.id)) entries.add(entry)
        }
        cache[dir.path] = entries
        return entries
    }

    companion object {
        const val FILE_NAME = "transcript.jsonl"
        private const val TAG = "ScribeTranscript"
    }
}
