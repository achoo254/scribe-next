package com.hoandat.scribe.recorder

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import android.util.Log
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.security.SecureRandom

/**
 * A bookmark pressed outside the WebView (the recording notification, a headset button). Kept on disk
 * until the web layer says it handed it to its bookmark queue for good, so a kill never loses one.
 */
data class NativeBookmark(
    /** 24 hex digits, the server's id shape: a resend never saves it twice. */
    val id: String,
    /** Recording session; its transcript is `s:<sessionId>`. */
    val sessionId: String,
    /** Seconds of audio kept in the session when pressed. */
    val positionS: Double,
    val createdAtMs: Long,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("id", id)
        .put("sessionId", sessionId)
        .put("positionS", positionS)
        .put("createdAt", StoreFiles.isoTime(createdAtMs))

    companion object {
        private val random = SecureRandom()

        /** 12 random bytes as 24 lowercase hex digits (ObjectId-shaped). */
        fun newId(): String {
            val bytes = ByteArray(12)
            random.nextBytes(bytes)
            return bytes.joinToString("") { "%02x".format(java.util.Locale.US, it.toInt() and 0xff) }
        }

        fun fromJson(json: JSONObject): NativeBookmark? {
            val id = json.optString("id")
            val sessionId = json.optString("sessionId")
            val position = json.optDouble("positionS", Double.NaN)
            if (id.isEmpty() || sessionId.isEmpty() || !position.isFinite()) return null
            return NativeBookmark(id, sessionId, position, StoreFiles.parseIsoTime(json.optString("createdAt")) ?: 0)
        }
    }
}

/**
 * `filesDir/bookmarks/<userKey>.json`, one small file per account, written atomically (Android twin
 * of NativeBookmarkStore in NowPlayingControls.swift). Every call is serialized.
 */
class BookmarkStore(private val root: File) {

    @Synchronized
    fun pending(userKey: String): List<NativeBookmark> {
        val file = file(userKey)
        if (!file.isFile) return emptyList()
        return try {
            val array = JSONArray(file.readText(Charsets.UTF_8))
            (0 until array.length()).mapNotNull { i -> array.optJSONObject(i)?.let { NativeBookmark.fromJson(it) } }
        } catch (e: JSONException) {
            Log.e(TAG, "read bookmarks", e)
            emptyList()
        } catch (e: IOException) {
            Log.e(TAG, "read bookmarks", e)
            emptyList()
        }
    }

    @Synchronized
    fun append(bookmark: NativeBookmark, userKey: String) {
        write(pending(userKey) + bookmark, userKey)
    }

    /** Forgets the bookmarks the web layer took care of. */
    @Synchronized
    fun remove(ids: Set<String>, userKey: String) {
        val all = pending(userKey)
        val kept = all.filter { it.id !in ids }
        if (kept.size == all.size) return
        if (kept.isEmpty()) {
            if (!file(userKey).delete()) throw IOException(AppText.get(R.string.recorder_bookmark_delete_failed))
        } else {
            write(kept, userKey)
        }
    }

    /** Account deleted: its pending bookmarks leave the phone. */
    @Synchronized
    fun removeAll(userKey: String) {
        val file = file(userKey)
        if (file.exists() && !file.delete()) throw IOException(AppText.get(R.string.recorder_bookmark_delete_failed))
    }

    private fun write(bookmarks: List<NativeBookmark>, userKey: String) {
        val array = JSONArray().apply { bookmarks.forEach { put(it.toJson()) } }
        StoreFiles.writeAtomically(file(userKey), array.toString())
    }

    private fun file(userKey: String) = File(root, "$userKey.json")

    private companion object {
        const val TAG = "ScribeBookmarks"
    }
}
