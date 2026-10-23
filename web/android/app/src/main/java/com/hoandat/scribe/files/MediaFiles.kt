package com.hoandat.scribe.files

import java.net.URI
import java.net.URISyntaxException
import java.net.URLDecoder

/** Pure rules of NativeFilesPlugin, kept apart so JVM tests cover them. */
object MediaFiles {
    /**
     * [raw] when it is an `https` URL of the backend's own host under `/api/`, else null: the session
     * cookie goes with the download, so a page asking for another host must never get it.
     */
    fun backendUrl(raw: String?, backendOrigin: String): URI? {
        if (raw.isNullOrEmpty()) return null
        val url = try {
            URI(raw)
        } catch (e: URISyntaxException) {
            return null
        }
        val backend = try {
            URI(backendOrigin)
        } catch (e: URISyntaxException) {
            return null
        }
        val sameOrigin = url.scheme == "https" && backend.scheme == "https" &&
            url.host != null && url.host.equals(backend.host, ignoreCase = true) && url.port == backend.port &&
            url.rawUserInfo == null
        val path = url.rawPath ?: return null
        return if (sameOrigin && path.startsWith("/api/") && !path.contains("/../")) url else null
    }

    /** Cache name of a backend file: its path with `/` replaced, as NativeFilesPlugin.swift. */
    fun cacheKey(url: URI): String = (url.rawPath ?: "").replace('/', '_')

    /**
     * File name from `Content-Disposition` (`filename*=UTF-8''…` first, then `filename="…"`), reduced to
     * a safe single path segment; null when the header has none.
     */
    fun fileName(contentDisposition: String?): String? {
        if (contentDisposition.isNullOrEmpty()) return null
        val parts = contentDisposition.split(';').map { it.trim() }
        val extended = parts.firstOrNull { it.startsWith("filename*=", ignoreCase = true) }
            ?.substringAfter('=')
            ?.let { value ->
                val encoded = value.substringAfter("''", value)
                try {
                    URLDecoder.decode(encoded.replace("+", "%2B"), "UTF-8")
                } catch (e: IllegalArgumentException) {
                    null
                }
            }
        val plain = parts.firstOrNull { it.startsWith("filename=", ignoreCase = true) }
            ?.substringAfter('=')
            ?.trim('"')
        return safeName(extended ?: plain)
    }

    /** Extension for a file name, else for the content type (the WebView picks the MIME type by it). */
    fun extension(name: String?, contentType: String?): String {
        val fromName = name?.substringAfterLast('.', "")?.lowercase().orEmpty()
        if (fromName.isNotEmpty() && fromName.length <= 5 && fromName.all { it.isLetterOrDigit() }) return fromName
        return when (contentType?.substringBefore(';')?.trim()?.lowercase()) {
            "audio/mp4", "audio/m4a", "audio/x-m4a" -> "m4a"
            "audio/mpeg" -> "mp3"
            "audio/webm" -> "webm"
            "video/webm" -> "webm"
            "audio/ogg" -> "ogg"
            "audio/wav", "audio/x-wav" -> "wav"
            "video/mp4" -> "mp4"
            "video/quicktime" -> "mov"
            "text/plain" -> "txt"
            "application/pdf" -> "pdf"
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" -> "docx"
            else -> ""
        }
    }

    private fun safeName(name: String?): String? {
        val cleaned = name?.substringAfterLast('/')?.substringAfterLast('\\')
            ?.filter { it >= ' ' && it != '\u007f' }
            ?.trim()
        return cleaned?.takeIf { it.isNotEmpty() && it != "." && it != ".." }
    }
}
