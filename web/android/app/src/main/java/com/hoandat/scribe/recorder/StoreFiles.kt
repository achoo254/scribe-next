package com.hoandat.scribe.recorder

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import java.io.File
import java.io.IOException
import java.text.ParseException
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/** File and time helpers shared by the recorder's stores (java.time needs API 26; minSdk is 24). */
internal object StoreFiles {
    /** Replaces [target] in one rename, so a kill never leaves half a JSON file. */
    fun writeAtomically(target: File, text: String) {
        target.parentFile?.mkdirs()
        val temp = File(target.parentFile, "${target.name}.tmp")
        temp.writeText(text, Charsets.UTF_8)
        if (!temp.renameTo(target)) {
            temp.delete()
            throw IOException(AppText.get(R.string.recorder_file_write_failed, target.name))
        }
    }

    private fun isoFormat(): SimpleDateFormat =
        SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }

    /** `2026-10-23T07:19:00Z`, the ISO 8601 shape of the iOS stores. */
    fun isoTime(millis: Long): String = isoFormat().format(Date(millis))

    /** RFC 3339 / ISO 8601 with `Z` or an offset (`+00:00`, chrono's to_rfc3339), optional milliseconds. */
    fun parseIsoTime(text: String?): Long? {
        if (text.isNullOrEmpty()) return null
        for (pattern in PARSE_PATTERNS) {
            val format = SimpleDateFormat(pattern, Locale.US).apply {
                timeZone = TimeZone.getTimeZone("UTC")
                isLenient = false
            }
            try {
                format.parse(text)?.let { return it.time }
            } catch (e: ParseException) {
                // Next pattern.
            }
        }
        return null
    }

    private val PARSE_PATTERNS = listOf("yyyy-MM-dd'T'HH:mm:ssXXX", "yyyy-MM-dd'T'HH:mm:ss.SSSXXX")

    /** `userKeyOf` of the web layer: 24 lowercase hex characters, never a path. */
    fun isUserKey(key: String?): Boolean =
        key != null && key.length == 24 && key.all { it in '0'..'9' || it in 'a'..'f' }
}
