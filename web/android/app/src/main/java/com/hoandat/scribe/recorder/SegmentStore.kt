package com.hoandat.scribe.recorder

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.util.Locale

/** One closed m4a segment of a session and its upload progress (same fields as SegmentStore.swift). */
data class Segment(
    val index: Int,
    val file: String,
    val durationS: Double,
    val bytes: Long,
    val uploaded: Boolean = false,
    val remoteFileId: String? = null,
    val attempts: Int = 0,
    /** Last terminal failure (403, unreadable audio); the file is kept. */
    val failure: String? = null,
    /** Server recording session of the upload in progress: a retry resends into it from offset 0. */
    val serverSession: String? = null,
    /** `finalize` was sent: before any new `init`, look the deterministic name up first. */
    val finalizeSent: Boolean = false,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("index", index)
        .put("file", file)
        .put("durationS", durationS)
        .put("bytes", bytes)
        .put("uploaded", uploaded)
        .put("attempts", attempts)
        .put("finalizeSent", finalizeSent)
        .apply {
            remoteFileId?.let { put("remoteFileId", it) }
            failure?.let { put("failure", it) }
            serverSession?.let { put("serverSession", it) }
        }

    companion object {
        /** Tolerant: missing keys never make a manifest unreadable (a rollback must not hide audio). */
        fun fromJson(json: JSONObject): Segment? {
            if (!json.has("index")) return null
            val index = json.optInt("index", -1).takeIf { it >= 0 } ?: return null
            return Segment(
                index = index,
                file = json.optString("file").ifEmpty { fileName(index) },
                durationS = json.optDouble("durationS", 0.0).takeIf { it.isFinite() } ?: 0.0,
                bytes = json.optLong("bytes", 0),
                uploaded = json.optBoolean("uploaded", false),
                remoteFileId = json.optStringOrNull("remoteFileId"),
                attempts = json.optInt("attempts", 0),
                failure = json.optStringOrNull("failure"),
                serverSession = json.optStringOrNull("serverSession"),
                finalizeSent = json.optBoolean("finalizeSent", false),
            )
        }

        /** `seg-007.m4a`, also past 999 (`seg-1000.m4a`). */
        fun fileName(index: Int): String = String.format(Locale.US, "seg-%03d.m4a", index)

        fun indexOf(name: String): Int? {
            if (!name.startsWith("seg-") || !name.endsWith(".m4a")) return null
            return name.removePrefix("seg-").removeSuffix(".m4a").toIntOrNull()
        }
    }
}

data class RecordingSession(
    val sessionId: String,
    val startedAtMs: Long,
    val segmentSeconds: Double,
    val segments: List<Segment>,
    val schemaVersion: Int = CURRENT_SCHEMA,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("schemaVersion", schemaVersion)
        .put("sessionId", sessionId)
        .put("startedAt", StoreFiles.isoTime(startedAtMs))
        .put("segmentSeconds", segmentSeconds)
        .put("segments", JSONArray().apply { segments.forEach { put(it.toJson()) } })

    companion object {
        const val CURRENT_SCHEMA = 1

        fun fromJson(json: JSONObject): RecordingSession? {
            val sessionId = json.optString("sessionId")
            if (sessionId.isEmpty()) return null
            val list = json.optJSONArray("segments")
            val segments = (0 until (list?.length() ?: 0)).mapNotNull { i -> list?.optJSONObject(i)?.let { Segment.fromJson(it) } }
            return RecordingSession(
                sessionId = sessionId,
                startedAtMs = StoreFiles.parseIsoTime(json.optString("startedAt")) ?: 0,
                segmentSeconds = json.optDouble("segmentSeconds", 60.0).takeIf { it.isFinite() && it > 0 } ?: 60.0,
                segments = segments,
                schemaVersion = json.optInt("schemaVersion", CURRENT_SCHEMA),
            )
        }
    }
}

/** A segment waiting for upload, with everything the queue needs. */
data class PendingSegment(val userKey: String, val sessionId: String, val segment: Segment, val file: File)

/** A segment that was still open when the app died: unreadable, removed. */
data class LostSegment(val sessionId: String, val index: Int, val approxSeconds: Double)

/**
 * The only owner of `session.json` files (Android twin of SegmentStore.swift): the recorder and the
 * upload queue both go through it, one call at a time, so no segment or `uploaded` flag is lost.
 *
 * Layout, per signed-in user so one account never uploads another's audio:
 * `filesDir/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`.
 *
 * [probe] returns the duration of a readable m4a, or null when it cannot be read (the segment that was
 * still being written when the app was killed has no moov atom). Android passes MediaExtractor; tests
 * pass a fake.
 */
class SegmentStore(private val root: File, private val probe: (File) -> Double?) {

    @Synchronized
    fun createSession(userKey: String, sessionId: String, segmentSeconds: Double, nowMs: Long = System.currentTimeMillis()): File {
        val dir = sessionDir(userKey, sessionId)
        if (!dir.mkdirs() && !dir.isDirectory) throw IOException(AppText.get(R.string.recorder_session_folder_failed))
        write(RecordingSession(sessionId, nowMs, segmentSeconds, emptyList()), dir)
        return dir
    }

    @Synchronized
    fun appendSegment(userKey: String, sessionId: String, segment: Segment) {
        val dir = sessionDir(userKey, sessionId)
        val session = read(dir) ?: throw IOException(AppText.get(R.string.recorder_session_missing, sessionId))
        val segments = (session.segments.filter { it.index != segment.index } + segment).sortedBy { it.index }
        write(session.copy(segments = segments), dir)
    }

    /** Uploaded: record the remote id, then free the phone's disk (manifest first). */
    @Synchronized
    fun markUploaded(userKey: String, sessionId: String, index: Int, fileId: String) {
        val dir = sessionDir(userKey, sessionId)
        val segment = update(dir, index) {
            it.copy(uploaded = true, remoteFileId = fileId, failure = null, serverSession = null, finalizeSent = false)
        } ?: return
        File(dir, segment.file).delete()
    }

    @Synchronized
    fun setUploadState(userKey: String, sessionId: String, index: Int, serverSession: String?, finalizeSent: Boolean) {
        update(sessionDir(userKey, sessionId), index) { it.copy(serverSession = serverSession, finalizeSent = finalizeSent) }
    }

    @Synchronized
    fun markAttempt(userKey: String, sessionId: String, index: Int, failure: String?) {
        update(sessionDir(userKey, sessionId), index) { it.copy(attempts = it.attempts + 1, failure = failure) }
    }

    /** Gives segments stopped by an error another chance (once per launch). */
    @Synchronized
    fun clearFailures(userKey: String) {
        for (dir in sessionDirs(userKey)) {
            val session = read(dir) ?: continue
            if (session.segments.none { it.failure != null }) continue
            tryWrite(session.copy(segments = session.segments.map { it.copy(failure = null) }), dir)
        }
    }

    @Synchronized
    fun session(userKey: String, sessionId: String): RecordingSession? = read(sessionDir(userKey, sessionId))

    /** Every segment of this user not yet uploaded and not failed for good, oldest session first. */
    @Synchronized
    fun pending(userKey: String): List<PendingSegment> = sessionDirs(userKey).flatMap { dir ->
        val session = read(dir) ?: return@flatMap emptyList()
        session.segments.filter { !it.uploaded && it.failure == null }
            .map { PendingSegment(userKey, session.sessionId, it, File(dir, it.file)) }
    }

    /** Segments waiting, and segments stopped by a terminal error. */
    @Synchronized
    fun queueCounts(userKey: String): Pair<Int, Int> {
        var waiting = 0
        var failed = 0
        for (dir in sessionDirs(userKey)) {
            val session = read(dir) ?: continue
            for (segment in session.segments) {
                if (segment.uploaded) continue
                if (segment.failure == null) waiting++ else failed++
            }
        }
        return waiting to failed
    }

    /**
     * Merges every manifest with the `seg-*.m4a` files really on disk: a readable file missing from the
     * manifest is added (a broken manifest never hides audio); an unreadable one was the segment still
     * open when the app was killed — removed and reported. Sessions with nothing left are deleted.
     */
    @Synchronized
    fun reconcile(userKey: String, activeSessionId: String?): List<LostSegment> {
        val lost = ArrayList<LostSegment>()
        for (dir in sessionDirs(userKey)) {
            val sessionId = dir.name
            if (sessionId == activeSessionId) continue
            val session = read(dir) ?: RecordingSession(sessionId, 0, 60.0, emptyList())
            val segments = session.segments.toMutableList()
            val files = dir.list()?.toSet().orEmpty()
            for (name in files.sorted()) {
                val index = Segment.indexOf(name) ?: continue
                val file = File(dir, name)
                val seconds = probe(file)?.takeIf { it > 0 }
                if (seconds == null) {
                    lost.add(LostSegment(sessionId, index, session.segmentSeconds))
                    file.delete()
                    segments.removeAll { it.file == name && !it.uploaded }
                    continue
                }
                if (segments.none { it.file == name }) {
                    segments.add(Segment(index, name, seconds, file.length()))
                }
            }
            // Manifest entries whose file vanished and were never uploaded cannot be sent.
            val remaining = dir.list()?.toSet().orEmpty()
            segments.removeAll { !it.uploaded && it.file !in remaining }
            segments.sortBy { it.index }
            if (segments.all { it.uploaded }) {
                dir.deleteRecursively()
            } else {
                tryWrite(session.copy(segments = segments), dir)
            }
        }
        return lost
    }

    /** Account deleted: every session of [userKey] (segments, manifests, live transcripts) leaves the phone. */
    @Synchronized
    fun removeAll(userKey: String) {
        val dir = File(root, userKey)
        if (dir.exists() && !dir.deleteRecursively()) throw IOException(AppText.get(R.string.recorder_account_delete_failed))
    }

    fun sessionDir(userKey: String, sessionId: String): File = File(File(root, userKey), sessionId)

    /** Sessions oldest first, by the start time in their manifest (folder names are UUIDs). */
    private fun sessionDirs(userKey: String): List<File> {
        val dirs = File(root, userKey).listFiles()?.filter { it.isDirectory }.orEmpty()
        return dirs.sortedWith(compareBy<File>({ read(it)?.startedAtMs ?: it.lastModified() }, { it.name }))
    }

    private fun update(dir: File, index: Int, change: (Segment) -> Segment): Segment? {
        val session = read(dir) ?: return null
        val current = session.segments.firstOrNull { it.index == index } ?: return null
        val next = change(current)
        write(session.copy(segments = session.segments.map { if (it.index == index) next else it }), dir)
        return next
    }

    private fun read(dir: File): RecordingSession? {
        val file = File(dir, MANIFEST)
        if (!file.isFile) return null
        return try {
            RecordingSession.fromJson(JSONObject(file.readText(Charsets.UTF_8)))
        } catch (e: JSONException) {
            null
        } catch (e: IOException) {
            null
        }
    }

    private fun write(session: RecordingSession, dir: File) {
        StoreFiles.writeAtomically(File(dir, MANIFEST), session.toJson().toString())
    }

    private fun tryWrite(session: RecordingSession, dir: File) {
        try {
            write(session, dir)
        } catch (e: IOException) {
            // Disk full: the files stay; the next reconcile tries again.
        }
    }

    companion object {
        const val MANIFEST = "session.json"
    }
}

internal fun JSONObject.optStringOrNull(name: String): String? =
    if (has(name) && !isNull(name)) optString(name).takeIf { it.isNotEmpty() } else null
