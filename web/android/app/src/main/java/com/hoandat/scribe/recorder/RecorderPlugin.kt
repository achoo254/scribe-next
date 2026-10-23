package com.hoandat.scribe.recorder

import android.Manifest
import android.os.Build
import com.getcapacitor.JSObject
import com.getcapacitor.PermissionState
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin
import com.getcapacitor.annotation.Permission
import com.getcapacitor.annotation.PermissionCallback
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException

/**
 * Bridge between the WebView and the native recording stack ([RecorderEngine]), same JS contract as
 * RecorderPlugin.swift (web/src/features/recorder/native-recorder.ts):
 * `start({userKey, segmentMinutes?, realtime?}) → {sessionId}`, `stop`, `pause`, `resume`, `getStatus`,
 * `syncUploads({userKey})`, `signOut({discardUserKey?})`, `takePendingBookmarks`, `ackBookmarks`,
 * `getTranscript`; events `state`, `paused`, `elapsed`, `level`, `segment`, `notice`, `failure`, `uploads`,
 * `uploaded`, `needsSignIn`, `quotaFull`, `bookmarkAdded`, `transcript`.
 *
 * Android-only step: `start` asks for the microphone (and, on Android 13+, notifications, so the
 * recording notification shows) before it records.
 */
@CapacitorPlugin(
    name = "Recorder",
    permissions = [
        Permission(alias = RecorderPlugin.MICROPHONE, strings = [Manifest.permission.RECORD_AUDIO]),
        Permission(alias = RecorderPlugin.NOTIFICATIONS, strings = [Manifest.permission.POST_NOTIFICATIONS]),
    ],
)
class RecorderPlugin : Plugin(), RecorderEngine.Listener {

    private lateinit var engine: RecorderEngine
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    /** The activity is on screen: frequent events (level, clock) are only worth sending then. */
    private var resumed = true

    override fun load() {
        engine = RecorderEngine.get(context)
        engine.listener = this
    }

    override fun emit(event: String, data: JSObject, frequent: Boolean) {
        if (frequent && !resumed) return
        notifyListeners(event, data)
    }

    override fun handleOnResume() {
        resumed = true
        engine.onForeground()
        // The clock was not sent while away; give the page the current one at once.
        val status = engine.status()
        notifyListeners("elapsed", JSObject().put("elapsedS", status.optDouble("elapsedS", 0.0)))
    }

    override fun handleOnPause() {
        resumed = false
    }

    override fun handleOnDestroy() {
        if (engine.listener === this) engine.listener = null
        scope.cancel()
    }

    // MARK: Methods

    @PluginMethod
    fun start(call: PluginCall) {
        if (userKey(call) == null) return
        if (getPermissionState(MICROPHONE) == PermissionState.GRANTED) {
            startRecording(call)
            return
        }
        val aliases = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            arrayOf(MICROPHONE, NOTIFICATIONS)
        } else {
            arrayOf(MICROPHONE)
        }
        requestPermissionForAliases(aliases, call, "afterPermission")
    }

    @PermissionCallback
    private fun afterPermission(call: PluginCall) {
        if (getPermissionState(MICROPHONE) == PermissionState.GRANTED) {
            startRecording(call)
        } else {
            val message = "Ứng dụng chưa được cấp quyền Micro — vào Cài đặt → Ứng dụng → Scribe → Quyền để bật."
            notifyListeners("failure", JSObject().put("message", message))
            call.reject(message, "PERMISSION")
        }
    }

    private fun startRecording(call: PluginCall) {
        val userKey = userKey(call) ?: return
        val minutes = call.getDouble("segmentMinutes") ?: 1.0
        val realtime = call.getBoolean("realtime", false) ?: false
        scope.launch {
            try {
                val sessionId = engine.start(userKey, minutes, realtime)
                call.resolve(JSObject().put("sessionId", sessionId))
            } catch (e: RecorderException) {
                call.reject(e.message, e.code)
            }
        }
    }

    @PluginMethod
    fun stop(call: PluginCall) {
        scope.launch { call.resolve(engine.stop()) }
    }

    @PluginMethod
    fun pause(call: PluginCall) {
        scope.launch { call.resolve(engine.pause()) }
    }

    @PluginMethod
    fun resume(call: PluginCall) {
        scope.launch { call.resolve(engine.resume()) }
    }

    @PluginMethod
    fun getStatus(call: PluginCall) {
        scope.launch { call.resolve(engine.status()) }
    }

    /** After sign-in and at launch: recover a killed session, report what was lost, upload what waits. */
    @PluginMethod
    fun syncUploads(call: PluginCall) {
        val userKey = userKey(call) ?: return
        scope.launch {
            val (lost, seconds) = engine.syncUploads(userKey)
            call.resolve(JSObject().put("lostSegments", lost).put("lostSecondsMax", seconds))
        }
    }

    @PluginMethod
    fun signOut(call: PluginCall) {
        val discard = call.getString("discardUserKey")
        scope.launch {
            engine.signOut(discard)
            call.resolve()
        }
    }

    /** `{ userKey }` → `{ bookmarks: [{ id, sessionId, positionS, createdAt }] }`, oldest first. */
    @PluginMethod
    fun takePendingBookmarks(call: PluginCall) {
        val userKey = userKey(call) ?: return
        scope.launch {
            val bookmarks = withContext(Dispatchers.IO) { engine.pendingBookmarks(userKey) }
            call.resolve(JSObject().put("bookmarks", bookmarks))
        }
    }

    /** `{ userKey, ids }`: the web queue took care of these bookmarks. */
    @PluginMethod
    fun ackBookmarks(call: PluginCall) {
        val userKey = userKey(call) ?: return
        val array = call.getArray("ids")
        val ids = (0 until (array?.length() ?: 0)).mapNotNull { array?.optString(it)?.takeIf { id -> id.isNotEmpty() } }.toSet()
        scope.launch {
            try {
                withContext(Dispatchers.IO) { engine.ackBookmarks(userKey, ids) }
                call.resolve()
            } catch (e: IOException) {
                call.reject("Không cập nhật được dấu mốc trên máy: ${e.message}", "STORE")
            }
        }
    }

    /** `{}` → `{ sessionId, entries: [{ id, atS, text, gapUntilS? }], status }`. */
    @PluginMethod
    fun getTranscript(call: PluginCall) {
        // Main thread: the engine's state; the entries are in memory after the first read.
        scope.launch { call.resolve(engine.transcript()) }
    }

    private fun userKey(call: PluginCall): String? {
        val userKey = call.getString("userKey")
        if (userKey.isNullOrEmpty()) {
            call.reject("Thiếu thông tin tài khoản.", "NO_USER")
            return null
        }
        return userKey
    }

    companion object {
        const val MICROPHONE = "microphone"
        const val NOTIFICATIONS = "notifications"
    }
}
