package com.hoandat.scribe.files

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import com.getcapacitor.FileUtils
import com.getcapacitor.JSObject
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin
import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import com.hoandat.scribe.auth.SessionAuth
import com.hoandat.scribe.net.BackendHttp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException
import java.net.URI
import java.util.UUID

/**
 * Downloads the WebView cannot do itself, same contract as NativeFilesPlugin.swift: `<audio src>` and
 * `window.location` would leave the app's origin without the session cookie. The download goes over
 * HttpURLConnection with the WebView's cookie store (WebViewCookies). Only the backend host is
 * accepted ([MediaFiles.backendUrl]), so the cookie never goes anywhere else.
 *
 * - `cache({url}) → {url}`: once into `cacheDir/recordings-cache/`, returned as the WebView-readable
 *   `https://localhost/_capacitor_file_/…` URL (the local server answers range requests, so seeking works).
 * - `share({url})`: downloads under the server's file name, then opens the Android share sheet through
 *   the FileProvider (Drive, Files, Photos…).
 * - `clear()`: deletes the cached recordings (sign-out: the next user must not play them).
 */
@CapacitorPlugin(name = "NativeFiles")
class NativeFilesPlugin : Plugin() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    /** One download per file at a time: the reader asks for the next segment ahead, then again on play. */
    private val downloads = HashMap<String, Deferred<File>>()

    private val http: BackendHttp get() = SessionAuth.shared(context).http

    @PluginMethod
    fun cache(call: PluginCall) {
        val url = backendUrl(call) ?: return
        scope.launch {
            try {
                val file = cachedFile(url)
                val portable = FileUtils.getPortablePath(context, bridge.localUrl, Uri.fromFile(file))
                call.resolve(JSObject().put("url", portable))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                call.reject(e.message ?: AppText.get(R.string.files_download_failed), DOWNLOAD_FAILED)
            }
        }
    }

    @PluginMethod
    fun share(call: PluginCall) {
        val url = backendUrl(call) ?: return
        scope.launch {
            try {
                val file = withContext(Dispatchers.IO) { downloadForSharing(url) }
                val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                val type = MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase())
                    ?: "application/octet-stream"
                val send = Intent(Intent.ACTION_SEND)
                    .setType(type)
                    .putExtra(Intent.EXTRA_STREAM, uri)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                val chooser = Intent.createChooser(send, null).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                activity.startActivity(chooser)
                call.resolve()
            } catch (e: CancellationException) {
                throw e
            } catch (e: ActivityNotFoundException) {
                call.reject(AppText.get(R.string.files_share_unavailable), "NO_VIEW")
            } catch (e: Exception) {
                call.reject(e.message ?: AppText.get(R.string.files_download_failed), DOWNLOAD_FAILED)
            }
        }
    }

    @PluginMethod
    fun clear(call: PluginCall) {
        scope.launch {
            withContext(Dispatchers.IO) {
                File(context.cacheDir, CACHE_DIR).deleteRecursively()
                File(context.cacheDir, SHARE_DIR).deleteRecursively()
            }
            call.resolve()
        }
    }

    override fun handleOnDestroy() {
        scope.cancel()
    }

    private fun backendUrl(call: PluginCall): URI? {
        val url = MediaFiles.backendUrl(call.getString("url"), http.origin)
        if (url == null) call.reject(AppText.get(R.string.files_bad_download_url), DOWNLOAD_FAILED)
        return url
    }

    /** The cached copy of [url], downloading it first when missing (main thread: [downloads]). */
    private suspend fun cachedFile(url: URI): File {
        val key = MediaFiles.cacheKey(url)
        val running = downloads[key] ?: scope.async(Dispatchers.IO) { cacheFile(url, key) }.also { downloads[key] = it }
        try {
            return running.await()
        } finally {
            if (downloads[key] === running) downloads.remove(key)
        }
    }

    /**
     * Downloads under a unique name, then gives the file its final name in one rename, so a reader
     * never sees a half-written file.
     */
    private fun cacheFile(url: URI, key: String): File {
        val dir = directory(CACHE_DIR)
        dir.listFiles()?.firstOrNull { it.nameWithoutExtension == key && it.isFile }?.let { return it }
        val staged = File(dir, "${UUID.randomUUID()}.part")
        try {
            val reply = http.download(url.toURL(), staged)
            requireOk(reply.status, staged)
            val name = MediaFiles.fileName(reply.header("Content-Disposition"))
            val extension = MediaFiles.extension(name ?: url.rawPath, reply.header("Content-Type"))
            val target = File(dir, if (extension.isEmpty()) key else "$key.$extension")
            if (!staged.renameTo(target)) throw IOException(AppText.get(R.string.files_cache_failed))
            return target
        } finally {
            staged.delete()
        }
    }

    private fun downloadForSharing(url: URI): File {
        // Shared copies of earlier shares: the receiving app had its time to read them.
        File(context.cacheDir, SHARE_DIR).deleteRecursively()
        val dir = File(directory(SHARE_DIR), UUID.randomUUID().toString()).apply { mkdirs() }
        val staged = File(dir, "download.part")
        try {
            val reply = http.download(url.toURL(), staged)
            requireOk(reply.status, staged)
            val name = MediaFiles.fileName(reply.header("Content-Disposition"))
                ?: url.rawPath.substringAfterLast('/').ifEmpty { "Scribe" }
            val target = File(dir, name)
            if (!staged.renameTo(target)) throw IOException(AppText.get(R.string.files_share_prepare_failed))
            return target
        } finally {
            staged.delete()
        }
    }

    private fun requireOk(status: Int, staged: File) {
        if (status in 200..299) return
        staged.delete()
        throw IOException(
            if (status == 401) {
                AppText.get(R.string.files_session_expired)
            } else {
                AppText.get(R.string.files_download_failed_http, status.toString())
            },
        )
    }

    private fun directory(name: String): File = File(context.cacheDir, name).apply { mkdirs() }

    companion object {
        private const val DOWNLOAD_FAILED = "DOWNLOAD_FAILED"
        private const val CACHE_DIR = "recordings-cache"
        /** Matches res/xml/file_paths.xml, the only folder the FileProvider exposes. */
        private const val SHARE_DIR = "share"
    }
}
