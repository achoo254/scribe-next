package com.hoandat.scribe.net

import org.json.JSONException
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL

/** Status, headers and body of one backend call. */
class HttpReply(val status: Int, val body: ByteArray, private val headers: Map<String, List<String>>) {
    val ok: Boolean get() = status in 200..299

    /** The body as a JSON object, or null when it is not one (a proxy error page, an empty body). */
    fun json(): JSONObject? = try {
        JSONObject(String(body, Charsets.UTF_8))
    } catch (e: JSONException) {
        null
    }

    /** The backend's own `{ "error": … }`; a proxy page with the same status has none. */
    fun backendError(): String? = json()?.optString("error")?.takeIf { it.isNotEmpty() }

    fun header(name: String): String? =
        headers.entries.firstOrNull { it.key.equals(name, ignoreCase = true) }?.value?.firstOrNull()
}

/**
 * Blocking HTTP to the Scribe backend over HttpURLConnection, so the session cookie comes from the
 * process-wide java.net.CookieHandler: Capacitor installs one backed by the WebView's cookie store
 * (CapacitorCookies), and [prepare] installs the same kind when the bridge has not (a recording service
 * running without the activity). Call it off the main thread.
 *
 * Tests pass a MockWebServer origin and no [prepare].
 */
class BackendHttp(
    val origin: String,
    private val prepare: () -> Unit = {},
) {
    fun get(path: String, timeoutMs: Int = DEFAULT_TIMEOUT_MS): HttpReply =
        send("GET", path, null, null, timeoutMs)

    fun postJson(path: String, body: JSONObject, timeoutMs: Int = DEFAULT_TIMEOUT_MS): HttpReply =
        send("POST", path, body.toString().toByteArray(Charsets.UTF_8), "application/json", timeoutMs)

    fun postBytes(path: String, bytes: ByteArray, timeoutMs: Int = DEFAULT_TIMEOUT_MS): HttpReply =
        send("POST", path, bytes, "application/octet-stream", timeoutMs)

    /** `origin + path`; [path] must be an absolute path on the backend. */
    fun url(path: String): URL {
        require(path.startsWith("/")) { "backend path must start with /: $path" }
        return URL(origin + path)
    }

    /**
     * Downloads [url] (already checked to be on the backend) into [target]. Returns the response with
     * an empty body; the caller reads the file name from `Content-Disposition`.
     */
    fun download(url: URL, target: File, timeoutMs: Int = DOWNLOAD_TIMEOUT_MS): HttpReply {
        prepare()
        val connection = open(url, "GET", timeoutMs)
        try {
            val status = connection.responseCode
            if (status !in 200..299) {
                return HttpReply(status, readAll(connection.errorStream), connection.headerFields.withoutNullKeys())
            }
            connection.inputStream.use { input -> target.outputStream().use { output -> input.copyTo(output) } }
            return HttpReply(status, ByteArray(0), connection.headerFields.withoutNullKeys())
        } finally {
            connection.disconnect()
        }
    }

    private fun send(method: String, path: String, body: ByteArray?, contentType: String?, timeoutMs: Int): HttpReply {
        prepare()
        val connection = open(url(path), method, timeoutMs)
        try {
            if (body != null && contentType != null) {
                connection.doOutput = true
                connection.setRequestProperty("Content-Type", contentType)
                connection.setFixedLengthStreamingMode(body.size)
                connection.outputStream.use { it.write(body) }
            }
            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            return HttpReply(status, readAll(stream), connection.headerFields.withoutNullKeys())
        } finally {
            connection.disconnect()
        }
    }

    private fun open(url: URL, method: String, timeoutMs: Int): HttpURLConnection {
        val connection = url.openConnection() as? HttpURLConnection
            ?: throw IOException("Not an HTTP URL: $url")
        connection.requestMethod = method
        connection.connectTimeout = minOf(timeoutMs, CONNECT_TIMEOUT_MS)
        connection.readTimeout = timeoutMs
        connection.instanceFollowRedirects = false
        connection.useCaches = false
        connection.setRequestProperty("Accept", "application/json")
        return connection
    }

    private fun readAll(stream: InputStream?): ByteArray {
        if (stream == null) return ByteArray(0)
        return stream.use { input ->
            val out = ByteArrayOutputStream()
            input.copyTo(out)
            out.toByteArray()
        }
    }

    private fun Map<String?, List<String>>.withoutNullKeys(): Map<String, List<String>> =
        entries.mapNotNull { (key, value) -> key?.let { it to value } }.toMap()

    companion object {
        const val DEFAULT_TIMEOUT_MS = 60_000
        private const val CONNECT_TIMEOUT_MS = 20_000
        private const val DOWNLOAD_TIMEOUT_MS = 120_000
    }
}
