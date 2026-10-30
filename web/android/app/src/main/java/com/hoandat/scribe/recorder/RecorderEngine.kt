package com.hoandat.scribe.recorder

import android.content.Context
import android.media.MediaExtractor
import android.media.MediaFormat
import android.net.ConnectivityManager
import android.net.Network
import android.os.Handler
import android.os.Looper
import android.util.Log
import com.getcapacitor.JSArray
import com.getcapacitor.JSObject
import com.hoandat.scribe.R
import com.hoandat.scribe.auth.SessionAuth
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.util.Locale
import java.util.UUID

/** A refusal of the recorder, with the JS error code. */
class RecorderException(message: String, val code: String) : Exception(message)

/**
 * The native recording stack of the process (RecorderPlugin.swift's state on iOS), kept outside the
 * plugin so it outlives the activity: with the foreground service running, Android may destroy and
 * recreate the activity (and its plugin instance) while a recording continues. The plugin attaches as
 * [listener] and reads [status] when it comes back. The native side is the source of truth (plan D2).
 *
 * Main thread for every public method, [listener] callbacks included.
 */
class RecorderEngine private constructor(private val app: Context) {

    /** Receives the plugin events; [frequent] ones (level, elapsed) only matter while on screen. */
    interface Listener {
        fun emit(event: String, data: JSObject, frequent: Boolean = false)
    }

    var listener: Listener? = null
    /** Running foreground service, set by it once it called startForeground. */
    var service: RecordingService? = null

    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val session = SessionAuth.shared(app)
    private val store = SegmentStore(File(app.filesDir, "recordings")) { probeDuration(it) }
    private val bookmarks = BookmarkStore(File(app.filesDir, "bookmarks"))
    private val transcripts = TranscriptStore()
    private val realtimeApi = RealtimeApi(session.http, session::refreshSession, scope)
    private val queue = UploadQueue(store, session.http, session::refreshSession, scope) { event ->
        main.post { handle(event) }
    }
    private val recorder = AudioRecorder(app) { handle(it) }
    private val mediaButtons = MediaButtonControls(app, onBookmark = { addNativeBookmark() }, onTogglePause = { togglePause() })

    /** Signed-in account. */
    private var userKey: String? = null
    /** Account the current recording belongs to: its segments go there even if the user signs out meanwhile. */
    private var recordingUserKey: String? = null
    private var elapsed = 0.0
    private var waiting = 0
    private var failed = 0
    private var lastNotice = ""
    /** Origin of the notification's clock: moved on resume so it shows the recorded time, not the wall time. */
    private var startedAtWallMs = 0L
    private var realtime: RealtimeTranscriber? = null
    private var transcriptSession: Pair<String, File>? = null
    private var transcriptStatus = TranscriptStatus()
    private var lastBookmarkAtMs = 0L
    private var feedback: String? = null
    private val clearFeedback = Runnable {
        feedback = null
        refreshNotification()
    }
    private var watchingNetwork = false

    val isRecording: Boolean
        get() = recorder.state == RecorderState.RECORDING || recorder.state == RecorderState.STARTING

    // MARK: Methods of the plugin

    /** Starts a session (microphone permission granted); returns its id. */
    fun start(userKey: String, segmentMinutes: Double, wantsRealtime: Boolean): String {
        if (recorder.state != RecorderState.IDLE) throw RecorderException("Đang ghi rồi.", "BUSY")
        val segmentSeconds = segmentMinutes.takeIf { it.isFinite() && it > 0 }?.let { it * 60 } ?: 60.0
        val sessionId = UUID.randomUUID().toString().lowercase(Locale.US)
        val dir = try {
            store.createSession(userKey, sessionId, segmentSeconds)
        } catch (e: IOException) {
            throw RecorderException("Không tạo được phiên ghi: ${e.message}", "STORE")
        }
        this.userKey = userKey
        recordingUserKey = userKey
        elapsed = 0.0
        lastNotice = ""
        stopRealtime()
        transcriptSession = null
        transcriptStatus = TranscriptStatus()
        if (wantsRealtime) startRealtime(sessionId, dir)
        startedAtWallMs = System.currentTimeMillis()
        recorder.start(sessionId, dir, segmentSeconds)
        if (recorder.state != RecorderState.RECORDING) {
            // The failure event already told the page why.
            throw RecorderException(lastNotice.ifEmpty { "Không bắt đầu ghi được." }, "START_FAILED")
        }
        try {
            RecordingService.start(app)
        } catch (e: RuntimeException) {
            // ForegroundServiceStartNotAllowedException: only possible if the app left the screen meanwhile.
            val message = "Android không cho Scribe ghi âm nền: ${e.message ?: e.javaClass.simpleName}"
            recorder.stop()
            lastNotice = message
            throw RecorderException(message, "START_FAILED")
        }
        mediaButtons.start()
        return sessionId
    }

    fun stop(): JSObject {
        recorder.stop()
        return status()
    }

    fun pause(): JSObject {
        recorder.pause()
        return status()
    }

    fun resume(): JSObject {
        recorder.resume()
        return status()
    }

    /** The notification's "Tạm dừng"/"Ghi tiếp" or a headset play/pause press. */
    fun togglePause() {
        if (recorder.isPaused) recorder.resume() else recorder.pause()
    }

    fun status(): JSObject = JSObject()
        .put("state", recorder.state.js)
        // Also tells the web layer this build can pause.
        .put("paused", recorder.isPaused)
        .put("elapsedS", elapsed)
        // Session position of the audio kept so far (bookmarks).
        .put("positionS", recorder.recordedSeconds)
        .put("sessionId", recorder.sessionId ?: JSONObject.NULL)
        .put("uploadsWaiting", waiting)
        .put("uploadsFailed", failed)
        .put("notice", lastNotice)

    /**
     * After sign-in and at launch: recover leftovers of a killed session, report the open segment that
     * was lost, and upload what is waiting. Returns (lost segments, their seconds at most).
     */
    suspend fun syncUploads(userKey: String): Pair<Int, Double> {
        this.userKey = userKey
        val active = recorder.sessionId
        val lost = withContext(Dispatchers.IO) {
            store.reconcile(userKey, active).also { store.clearFailures(userKey) }
        }
        queue.start(userKey)
        watchNetwork()
        return lost.size to lost.sumOf { it.approxSeconds }
    }

    /**
     * Sign-out: stop recording and uploads so nothing reaches the old account. [discardUserKey] (the
     * account was deleted): also remove what the phone keeps for it.
     *
     * The device token is left to `HoandatAuth.signOut()`, which the page calls right after this
     * (auth-context.tsx) and which must send it to `/api/auth/logout` so the backend revokes the device
     * at the IdP. Clearing it here first (as RecorderPlugin.swift does) would leave the device listed at
     * auth.hoandat.com/account. With the queue stopped nothing renews the session meanwhile.
     */
    suspend fun signOut(discardUserKey: String?) {
        userKey = null
        recorder.stop()
        queue.stop()
        val discard = discardUserKey?.takeIf { StoreFiles.isUserKey(it) } ?: return
        withContext(Dispatchers.IO) {
            try {
                store.removeAll(discard)
                bookmarks.removeAll(discard)
            } catch (e: IOException) {
                Log.e(TAG, "discard local data failed", e)
            }
        }
    }

    /** Oldest first; they stay stored until acknowledged. */
    fun pendingBookmarks(userKey: String): JSArray =
        JSArray().apply { bookmarks.pending(userKey).forEach { put(it.toJson()) } }

    fun ackBookmarks(userKey: String, ids: Set<String>) {
        bookmarks.remove(ids, userKey)
    }

    /** `{ sessionId, entries, status }` of the current or last recording with a live transcript. */
    fun transcript(): JSObject {
        val result = JSObject().put("status", transcriptStatus.toJson())
        val current = transcriptSession
        if (current == null) {
            result.put("sessionId", JSONObject.NULL)
            result.put("entries", JSArray())
            return result
        }
        result.put("sessionId", current.first)
        result.put("entries", JSArray().apply { transcripts.entries(current.second).forEach { put(it.toJson()) } })
        return result
    }

    /** Back on screen: resume uploads (they may have waited on a backoff). */
    fun onForeground() {
        userKey?.let { queue.kick(it) }
    }

    /** The service could not go to the foreground: the recording cannot continue in the background. */
    fun foregroundRefused(message: String) {
        recorder.stop()
        lastNotice = message
        listener?.emit("failure", JSObject().put("message", message))
    }

    // MARK: Bookmarks

    /**
     * A press on the notification or a headset next/previous button: saved to disk at once at the recorder clock.
     * Presses closer than a second count once.
     */
    fun addNativeBookmark() {
        val now = System.currentTimeMillis()
        if (now - lastBookmarkAtMs < DEBOUNCE_MS) return
        lastBookmarkAtMs = now
        val sessionId = recorder.sessionId
        val owner = recordingUserKey
        if (recorder.state != RecorderState.RECORDING || sessionId == null || owner == null) return
        val bookmark = NativeBookmark(NativeBookmark.newId(), sessionId, recorder.recordedSeconds, now)
        try {
            bookmarks.append(bookmark, owner)
        } catch (e: IOException) {
            lastNotice = "Không lưu được dấu mốc: ${e.message}"
            listener?.emit("notice", JSObject().put("message", lastNotice))
            return
        }
        listener?.emit("bookmarkAdded", JSObject.fromJSONObject(bookmark.toJson()))
        feedback = app.getString(R.string.recording_bookmarked, clock(bookmark.positionS))
        refreshNotification()
        main.removeCallbacks(clearFeedback)
        main.postDelayed(clearFeedback, FEEDBACK_MS)
    }

    fun notificationState() = RecordingService.NotificationState(startedAtWallMs, recorder.isPaused, feedback)

    private fun refreshNotification() {
        val running = service ?: return
        try {
            running.showNotification(notificationState())
        } catch (e: RuntimeException) {
            Log.w(TAG, "notification update failed", e)
        }
    }

    // MARK: Live transcript

    /** Its failures stay inside it: the recording never depends on it. */
    private fun startRealtime(sessionId: String, dir: File) {
        val transcriber = RealtimeTranscriber(sessionId, dir, transcripts, realtimeApi, scope, { handle(it, sessionId) })
        realtime = transcriber
        transcriptSession = sessionId to dir
        recorder.setPcmSink(transcriber)
        transcriber.start()
    }

    private fun stopRealtime() {
        recorder.setPcmSink(null)
        realtime?.finish()
        realtime = null
    }

    private fun handle(event: TranscriptEvent, sessionId: String) {
        if (transcriptSession?.first != sessionId) return
        val data = when (event) {
            is TranscriptEvent.Sentence -> JSObject.fromJSONObject(event.entry.toJson()).put("type", "sentence")
            is TranscriptEvent.Partial -> JSObject().put("type", "partial").put("id", event.id).put("atS", event.atS).put("text", event.text)
            is TranscriptEvent.Status -> {
                transcriptStatus = event.status
                JSObject.fromJSONObject(event.status.toJson())
            }
        }
        // Sent even off screen (a few per minute): Android's WebView keeps running, and the page then has
        // them without waiting for a visibilitychange it might not get.
        listener?.emit("transcript", data.put("sessionId", sessionId))
    }

    // MARK: Events

    private fun handle(event: RecorderEvent) {
        when (event) {
            is RecorderEvent.State -> {
                if (event.state == RecorderState.IDLE) {
                    mediaButtons.stop()
                    stopRealtime()
                    main.removeCallbacks(clearFeedback)
                    feedback = null
                    service?.finish()
                    service = null
                }
                listener?.emit("state", JSObject().put("state", event.state.js))
            }
            is RecorderEvent.Paused -> {
                if (!event.paused) {
                    // The clock counts recorded seconds only: start it again where it stood.
                    startedAtWallMs = System.currentTimeMillis() - (recorder.recordedSeconds * 1000).toLong()
                }
                mediaButtons.update(event.paused)
                refreshNotification()
                listener?.emit("paused", JSObject().put("paused", event.paused))
            }
            is RecorderEvent.Elapsed -> {
                elapsed = event.seconds
                listener?.emit("elapsed", JSObject().put("elapsedS", event.seconds), frequent = true)
            }
            is RecorderEvent.Level -> listener?.emit("level", JSObject().put("level", event.level.toDouble()), frequent = true)
            is RecorderEvent.SegmentClosed -> {
                val owner = recordingUserKey ?: return
                listener?.emit("segment", JSObject().put("index", event.segment.index).put("durationS", event.segment.durationS))
                scope.launch {
                    try {
                        store.appendSegment(owner, event.sessionId, event.segment)
                    } catch (e: IOException) {
                        // The file stays on disk; the next reconcile adds it to the manifest.
                        Log.e(TAG, "append segment failed", e)
                    }
                    // Only wakes the queue if `owner` is still the signed-in user.
                    queue.kick(owner)
                }
            }
            is RecorderEvent.Notice -> {
                lastNotice = event.message
                listener?.emit("notice", JSObject().put("message", event.message))
            }
            is RecorderEvent.Failure -> {
                lastNotice = event.message
                listener?.emit("failure", JSObject().put("message", event.message))
            }
        }
    }

    private fun handle(event: UploadEvent) {
        when (event) {
            is UploadEvent.Counts -> {
                waiting = event.waiting
                failed = event.failed
                listener?.emit("uploads", JSObject().put("waiting", event.waiting).put("failed", event.failed))
            }
            is UploadEvent.Uploaded -> listener?.emit("uploaded", JSObject().put("name", event.name))
            UploadEvent.NeedsSignIn -> listener?.emit("needsSignIn", JSObject())
            is UploadEvent.QuotaFull -> listener?.emit("quotaFull", JSObject().put("message", event.message))
            is UploadEvent.AccountDeleted -> {
                scope.launch {
                    try {
                        bookmarks.removeAll(event.userKey)
                    } catch (e: IOException) {
                        Log.e(TAG, "remove bookmarks failed", e)
                    }
                }
                listener?.emit("needsSignIn", JSObject())
            }
        }
    }

    /** Uploads waiting on a backoff retry at once when the network comes back. */
    private fun watchNetwork() {
        if (watchingNetwork) return
        val manager = app.getSystemService(ConnectivityManager::class.java) ?: return
        try {
            manager.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    queue.networkBack()
                }
            })
            watchingNetwork = true
        } catch (e: RuntimeException) {
            Log.w(TAG, "network callback not registered", e)
        }
    }

    companion object {
        private const val TAG = "ScribeRecorderEngine"
        private const val DEBOUNCE_MS = 1_000L
        /** How long "Đã đánh dấu 12:34" stays in the notification. */
        private const val FEEDBACK_MS = 3_000L

        @Volatile
        private var instance: RecorderEngine? = null

        fun get(context: Context): RecorderEngine = instance ?: synchronized(this) {
            instance ?: RecorderEngine(context.applicationContext).also { instance = it }
        }

        fun peek(): RecorderEngine? = instance

        /** Duration of a readable m4a, null when it cannot be read (no moov atom: killed mid-write). */
        fun probeDuration(file: File): Double? {
            val extractor = MediaExtractor()
            return try {
                extractor.setDataSource(file.path)
                if (extractor.trackCount == 0) return null
                val format = extractor.getTrackFormat(0)
                if (!format.containsKey(MediaFormat.KEY_DURATION)) return null
                format.getLong(MediaFormat.KEY_DURATION) / 1_000_000.0
            } catch (e: IOException) {
                null
            } catch (e: RuntimeException) {
                null
            } finally {
                extractor.release()
            }
        }

        /** 754 → "12:34", 3_754 → "1:02:34". */
        fun clock(seconds: Double): String {
            val total = maxOf(0L, seconds.toLong())
            val h = total / 3600
            val m = total % 3600 / 60
            val s = total % 60
            return if (h > 0) String.format(Locale.US, "%d:%02d:%02d", h, m, s) else String.format(Locale.US, "%02d:%02d", m, s)
        }
    }
}
