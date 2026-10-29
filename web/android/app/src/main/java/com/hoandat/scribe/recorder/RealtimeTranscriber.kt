package com.hoandat.scribe.recorder

import android.os.Handler
import android.os.Looper
import android.util.Log
import com.hoandat.scribe.net.Base64Text
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONException
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

/** Status of the live transcript (the `transcript` event with `type: "status"`). */
data class TranscriptStatus(
    val connected: Boolean = false,
    /** Session position where the stream was lost; null while connected. */
    val disconnectedAtS: Double? = null,
    /** Why the preview is not working (shown as is), null when fine. */
    val error: String? = null,
    /** The stream ended for this recording (stopped, or a final error). */
    val stopped: Boolean = false,
) {
    fun toJson(): JSONObject = JSONObject().put("type", "status").put("connected", connected).put("stopped", stopped)
        .apply {
            disconnectedAtS?.let { put("disconnectedAtS", it) }
            error?.let { put("error", it) }
        }
}

sealed interface TranscriptEvent {
    data class Sentence(val entry: TranscriptEntry) : TranscriptEvent
    /** Text of a sentence still being transcribed (not stored). */
    data class Partial(val id: String, val atS: Double, val text: String) : TranscriptEvent
    data class Status(val status: TranscriptStatus) : TranscriptEvent
}

/**
 * Live preview of the words while recording (RealtimeTranscriber.swift): the recorder's PCM is
 * resampled to PCM16 24 kHz mono ([Resampler]), cut into 100 ms frames and streamed over a WebSocket
 * (OkHttp) to the provider the backend picks per token: ElevenLabs Scribe v2 Realtime (default) or
 * OpenAI Realtime. Finished sentences go to [TranscriptStore] and to [onEvent].
 *
 * The m4a file stays the source of truth: nothing here can stop or slow the recording. [accept] only
 * converts and hands the frame over; every failure (network, token, quota) is contained, reported
 * through the status, and the audio of that stretch is simply not previewed.
 *
 * Same rules as iOS: one token streams at most its `reservedSeconds`; 20 s before that a new token opens
 * a new socket and the old one is committed and closed 4 s later; an ElevenLabs token opens one socket,
 * so every reconnection asks for a new token; reconnection backs off 1…60 s and keeps up to 30 s of
 * audio to send once connected; the seconds streamed per token are reported when it is retired.
 * OkHttp pings every 15 s and fails a socket that stops answering.
 *
 * All state lives on [queue]; [accept] runs on the capture thread; [onEvent] is called on the main thread.
 */
class RealtimeTranscriber(
    val sessionId: String,
    val dir: File,
    private val store: TranscriptStore,
    private val api: RealtimeApi,
    private val scope: CoroutineScope,
    private val onEvent: (TranscriptEvent) -> Unit,
    private val client: OkHttpClient = sharedClient,
) : PcmSink {

    private class Token(val token: RealtimeToken) {
        var sentS = 0.0
        /** An ElevenLabs token is consumed by the socket it opens. */
        var used = false
    }

    private inner class Connection(val tokenId: String, val provider: RealtimeProvider) : WebSocketListener() {
        var socket: WebSocket? = null
        var open = false
        /** Session position of the first frame sent on this socket (provider timestamps count from there). */
        var baseS: Double? = null
        var sentS = 0.0
        val speechStart = HashMap<String, Double>()
        /** ElevenLabs: index of the segment being transcribed (id `<tokenId>-<segment>`) and its start. */
        var segment = 0
        var segmentStartS = 0.0

        override fun onMessage(webSocket: WebSocket, text: String) {
            onQueue { handle(text, this) }
        }

        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
            onQueue { lost(this, "closed $code") }
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            onQueue { lost(this, t.message ?: t.javaClass.simpleName) }
        }
    }

    private val queue: ScheduledExecutorService = Executors.newSingleThreadScheduledExecutor { runnable ->
        Thread(runnable, "scribe-realtime")
    }
    private val main = Handler(Looper.getMainLooper())

    // Capture thread only.
    private var resampler: Resampler? = null
    @Volatile private var accepting = true

    // [queue] only.
    private var token: Token? = null
    private var active: Connection? = null
    private var opening: Connection? = null
    private val retiring = ArrayList<Connection>()
    private var pending = ByteArray(0)
    private var pendingStartS = 0.0
    private val backlog = ArrayDeque<Pair<ByteArray, Double>>()
    /** End of the last frame sent on any socket. */
    private var lastSentEndS: Double? = null
    private val partials = HashMap<String, String>()
    private var status = TranscriptStatus()
    private var fetching = false
    private var retryScheduled = false
    private var attempt = 0
    private var finished = false
    /** ElevenLabs said the server account is out of credit: told to the backend with the last usage report. */
    private var quotaExceeded = false
    /** A final token refusal while the current token still has budget. */
    private var finalAfterBudget: String? = null

    fun start() {
        onQueue {
            publish()
            requestToken()
        }
    }

    /** The recording ended: commit what is buffered, close, report usage. */
    fun finish() {
        accepting = false
        onQueue { end(null) }
        later(RETIRE_DELAY_MS + 1_000) { queue.shutdown() }
    }

    /** Runs [block] on [queue]; dropped once the transcriber has shut down (late socket callbacks). */
    private fun onQueue(block: () -> Unit): Boolean = try {
        queue.execute(Runnable { block() })
        true
    } catch (e: RejectedExecutionException) {
        false
    }

    private fun later(delayMs: Long, block: () -> Unit) {
        try {
            queue.schedule(Runnable { block() }, delayMs, TimeUnit.MILLISECONDS)
        } catch (e: RejectedExecutionException) {
            // Shut down: nothing left to do.
        }
    }

    // MARK: Audio (capture thread)

    override fun accept(samples: ShortArray, offset: Int, count: Int, sampleRate: Int, positionS: Double) {
        if (!accepting || count <= 0) return
        val converter = resampler?.takeIf { rate == sampleRate } ?: Resampler(sampleRate, SAMPLE_RATE).also {
            resampler = it
            rate = sampleRate
        }
        val out = converter.process(samples, offset, count)
        if (out.isEmpty()) return
        val bytes = ByteArray(out.size * 2)
        for (i in out.indices) {
            val v = out[i].toInt()
            bytes[2 * i] = (v and 0xff).toByte()
            bytes[2 * i + 1] = (v shr 8 and 0xff).toByte()
        }
        if (!onQueue { acceptBytes(bytes, positionS) }) accepting = false
    }

    private var rate = 0

    // MARK: Frames (queue)

    private fun acceptBytes(data: ByteArray, startS: Double) {
        if (finished) return
        if (pending.isEmpty()) pendingStartS = startS
        pending += data
        while (pending.size >= FRAME_BYTES) {
            val frame = pending.copyOfRange(0, FRAME_BYTES)
            pending = pending.copyOfRange(FRAME_BYTES, pending.size)
            enqueue(frame, pendingStartS)
            pendingStartS += FRAME_SECONDS
        }
    }

    private fun enqueue(frame: ByteArray, startS: Double) {
        val connection = active
        val current = token
        if (connection != null && current != null && current.token.id == connection.tokenId && backlog.isEmpty()) {
            if (current.sentS + FRAME_SECONDS <= current.token.reservedS) {
                send(frame, startS, connection)
                current.sentS += FRAME_SECONDS
                if (current.sentS >= current.token.reservedS - ROTATE_MARGIN_S) requestToken()
                return
            }
            // Budget used up and no further token allowed (out of minutes): wind down.
            finalAfterBudget?.let { return end(it) }
        }
        backlog.addLast(frame to startS)
        while (backlog.size > BACKLOG_LIMIT) backlog.removeFirst()
        requestToken()
    }

    private fun send(frame: ByteArray, startS: Double, connection: Connection) {
        if (connection.baseS == null) connection.baseS = startS
        connection.sentS += FRAME_SECONDS
        lastSentEndS = startS + FRAME_SECONDS
        val sent = connection.socket?.send(audioMessage(frame, connection.provider, commit = false)) ?: false
        if (!sent) lost(connection, "send refused")
    }

    // MARK: Tokens (queue)

    /** Fetches a token unless one is on the way; with a usable token in hand, opens a socket with it. */
    private fun requestToken() {
        if (finished || fetching || retryScheduled || opening != null || finalAfterBudget != null) return
        val current = token
        val needNew = current == null || current.used ||
            current.sentS >= current.token.reservedS - ROTATE_MARGIN_S ||
            current.token.expiresAtMs - System.currentTimeMillis() < EXPIRY_MARGIN_MS
        if (!needNew && current != null) {
            connect(current)
            return
        }
        fetching = true
        scope.launch {
            val result = api.fetchToken(sessionId)
            if (!onQueue { tokenArrived(result) } && result is TokenResult.Ok) {
                // Finished meanwhile: the reservation of a fresh token is settled with 0 seconds.
                api.reportUsage(result.token.id, 0.0)
            }
        }
    }

    private fun tokenArrived(result: TokenResult) {
        fetching = false
        when (result) {
            is TokenResult.Ok -> {
                val fresh = result.token
                if (finished) {
                    api.reportUsage(fresh.id, 0.0)
                    return
                }
                // From here on audio only goes out with the new token: the old one is done.
                token?.takeIf { it.token.id != fresh.id }?.let { api.reportUsage(it.token.id, it.sentS) }
                val next = Token(fresh)
                token = next
                connect(next)
            }
            is TokenResult.Transient -> {
                Log.w(TAG, "token: ${result.message}")
                setError(result.message)
                scheduleRetry()
            }
            is TokenResult.Final -> {
                Log.w(TAG, "token refused: ${result.message}")
                // Out of minutes: the current token streams to its end, nothing after.
                val current = token
                if (current != null && active != null && current.sentS < current.token.reservedS) {
                    setError(result.message)
                    finalAfterBudget = result.message
                } else {
                    end(result.message)
                }
            }
        }
    }

    private fun scheduleRetry() {
        if (finished || retryScheduled) return
        retryScheduled = true
        val delay = BACKOFF_S[minOf(attempt, BACKOFF_S.size - 1)]
        attempt++
        later(delay * 1_000) {
            retryScheduled = false
            requestToken()
        }
    }

    // MARK: Sockets (queue)

    private fun connect(current: Token) {
        val request = when (current.token.provider) {
            RealtimeProvider.OPENAI -> Request.Builder().url(current.token.url)
                .header("Authorization", "Bearer ${current.token.value}").build()
            RealtimeProvider.ELEVENLABS -> {
                // The single-use token rides in the query (the API key never leaves the server).
                val url = current.token.url.replaceFirst("wss://", "https://").toHttpUrlOrNull()
                    ?.newBuilder()?.addQueryParameter("token", current.token.value)?.build()
                    ?: return end("Địa chỉ chép trực tiếp không hợp lệ.")
                current.used = true
                Request.Builder().url(url).build()
            }
        }
        val connection = Connection(current.token.id, current.token.provider)
        opening = connection
        connection.socket = client.newWebSocket(request, connection)
    }

    private fun handle(text: String, connection: Connection) {
        val message = try {
            JSONObject(text)
        } catch (e: JSONException) {
            return
        }
        when (connection.provider) {
            RealtimeProvider.OPENAI -> handleOpenAi(message, connection)
            RealtimeProvider.ELEVENLABS -> handleElevenLabs(message, connection)
        }
    }

    private fun handleOpenAi(message: JSONObject, connection: Connection) {
        val type = message.optString("type")
        val itemId = message.optString("item_id")
        when (type) {
            "session.created", "session.updated" -> if (opening === connection) promote(connection)
            "input_audio_buffer.speech_started" -> {
                val ms = message.optDouble("audio_start_ms", 0.0)
                connection.speechStart[itemId] = (connection.baseS ?: lastSentEndS ?: 0.0) + ms / 1000
            }
            "conversation.item.input_audio_transcription.delta" -> {
                val text = (partials[itemId] ?: "") + message.optString("delta")
                partials[itemId] = text
                emit(TranscriptEvent.Partial(itemId, startOf(itemId, connection), text))
            }
            "conversation.item.input_audio_transcription.completed" -> {
                partials.remove(itemId)
                val text = message.optString("transcript").trim()
                if (itemId.isEmpty() || text.isEmpty()) return
                save(TranscriptEntry(itemId, startOf(itemId, connection), text))
            }
            "error" -> {
                val error = message.optJSONObject("error")
                val code = error?.optString("code").orEmpty()
                // An empty commit when retiring a quiet socket is expected.
                if (code != "input_audio_buffer_commit_empty") Log.w(TAG, "realtime error $code: ${error?.optString("message")}")
            }
        }
    }

    /**
     * ElevenLabs: `partial_transcript` carries the whole text of the current segment so far;
     * `committed_transcript_with_timestamps` closes it with word timestamps counted from the socket's
     * first frame (the plain `committed_transcript` before it is skipped).
     */
    private fun handleElevenLabs(message: JSONObject, connection: Connection) {
        val type = message.optString("message_type")
        val segmentId = "${connection.tokenId}-${connection.segment}"
        when (type) {
            "session_started" -> if (opening === connection) promote(connection)
            "partial_transcript" -> {
                val text = message.optString("text").trim()
                if (text.isEmpty()) return
                val atS = (connection.baseS ?: lastSentEndS ?: 0.0) + connection.segmentStartS
                emit(TranscriptEvent.Partial(segmentId, atS, text))
            }
            "committed_transcript_with_timestamps" -> {
                val all = message.optJSONArray("words")
                val words = (0 until (all?.length() ?: 0)).mapNotNull { all?.optJSONObject(it) }
                    .filter { it.optString("type") == "word" }
                val firstS = words.firstOrNull()?.optDouble("start", Double.NaN)?.takeIf { it.isFinite() } ?: connection.segmentStartS
                val lastS = words.lastOrNull()?.optDouble("end", Double.NaN)?.takeIf { it.isFinite() }
                connection.segment++
                connection.segmentStartS = lastS ?: connection.sentS
                val text = message.optString("text").trim()
                if (text.isEmpty()) return
                save(TranscriptEntry(segmentId, (connection.baseS ?: lastSentEndS ?: 0.0) + firstS, text))
            }
            "quota_exceeded" -> {
                Log.w(TAG, "realtime quota_exceeded: ${message.optString("error")}")
                quotaExceeded = true
                end("Hệ thống đã dùng hết giờ chép lời chung — chép trực tiếp dừng, bản ghi vẫn tiếp tục.")
            }
            "committed_transcript", "warning", "commit_throttled", "insufficient_audio_activity" -> Unit
            else -> {
                // Any other message with an `error` is a failure of this socket (auth, rate limit, time
                // limit…): drop it and reconnect with a new token; [lost] ignores retiring sockets.
                val error = message.optString("error")
                if (error.isEmpty()) return
                Log.w(TAG, "realtime $type: $error")
                lost(connection, type)
            }
        }
    }

    private fun startOf(itemId: String, connection: Connection): Double =
        connection.speechStart[itemId] ?: connection.baseS?.let { it + connection.sentS } ?: lastSentEndS ?: 0.0

    /** The new socket is open: audio goes there, the previous one retires. */
    private fun promote(connection: Connection) {
        opening = null
        connection.open = true
        active?.let { retire(it) }
        active = connection
        attempt = 0
        val first = backlog.firstOrNull()?.second
        val last = lastSentEndS
        if (first != null && last != null && first - last >= MIN_GAP_S) {
            save(TranscriptEntry("gap-${(last * 10).toLong()}", last, "", gapUntilS = first))
        }
        status = status.copy(connected = true, disconnectedAtS = null, error = if (finalAfterBudget == null) null else status.error)
        publish()
        val waiting = backlog.toList()
        backlog.clear()
        for ((frame, startS) in waiting) enqueue(frame, startS)
    }

    private fun retire(connection: Connection) {
        val commit = when (connection.provider) {
            RealtimeProvider.OPENAI -> "{\"type\":\"input_audio_buffer.commit\"}"
            RealtimeProvider.ELEVENLABS -> audioMessage(ByteArray(0), RealtimeProvider.ELEVENLABS, commit = true)
        }
        connection.socket?.send(commit)
        retiring.add(connection)
        later(RETIRE_DELAY_MS) {
            connection.socket?.close(1000, null)
            retiring.remove(connection)
        }
    }

    private fun lost(connection: Connection, reason: String) {
        if (retiring.contains(connection)) return
        val wasActive = active === connection
        if (!wasActive && opening !== connection) return
        Log.w(TAG, "socket lost: $reason")
        connection.socket?.cancel()
        if (wasActive) active = null else opening = null
        if (finished) return
        if (status.disconnectedAtS == null) status = status.copy(disconnectedAtS = lastSentEndS ?: pendingStartS)
        status = status.copy(connected = false)
        publish()
        finalAfterBudget?.let { return end(it) }
        scheduleRetry()
    }

    // MARK: End (queue)

    private fun end(error: String?) {
        if (finished) return
        finished = true
        accepting = false
        opening?.socket?.cancel()
        opening = null
        active?.let { retire(it) }
        active = null
        token?.let { api.reportUsage(it.token.id, it.sentS, quotaExceeded) }
        token = null
        backlog.clear()
        status = status.copy(connected = false, stopped = true, error = error)
        publish()
    }

    // MARK: Output

    private fun save(entry: TranscriptEntry) {
        store.append(entry, dir)
        emit(TranscriptEvent.Sentence(entry))
    }

    private fun setError(message: String) {
        status = status.copy(error = message)
        publish()
    }

    private fun publish() = emit(TranscriptEvent.Status(status))

    private fun emit(event: TranscriptEvent) {
        main.post { onEvent(event) }
    }

    companion object {
        private const val TAG = "ScribeRealtime"
        /** Both providers take PCM16 mono at 24 kHz (ElevenLabs `pcm_24000`). */
        const val SAMPLE_RATE = 24_000
        /** 100 ms of PCM16 mono at 24 kHz. */
        private const val FRAME_BYTES = 4_800
        private const val FRAME_SECONDS = 0.1
        /** Frames kept while (re)connecting (30 s), sent once the new socket is open. */
        private const val BACKLOG_LIMIT = 300
        /** Ask for the next token this long before the current one's budget ends. */
        private const val ROTATE_MARGIN_S = 20.0
        /** A token closer than this to its expiry is not used to open a socket. */
        private const val EXPIRY_MARGIN_MS = 30_000L
        /** A retired socket stays open this long for its last sentence. */
        private const val RETIRE_DELAY_MS = 4_000L
        private val BACKOFF_S = longArrayOf(1, 2, 5, 10, 20, 30, 60)
        /** A missed stretch shorter than this is not marked. */
        private const val MIN_GAP_S = 1.0

        private val sharedClient: OkHttpClient by lazy {
            OkHttpClient.Builder()
                .pingInterval(15, TimeUnit.SECONDS)
                .connectTimeout(20, TimeUnit.SECONDS)
                .readTimeout(0, TimeUnit.SECONDS)
                .build()
        }

        /** One audio frame (or, with an empty frame and [commit], a bare commit). Base64 needs no JSON escaping. */
        fun audioMessage(frame: ByteArray, provider: RealtimeProvider, commit: Boolean): String {
            val audio = Base64Text.standard(frame)
            return when (provider) {
                RealtimeProvider.OPENAI -> "{\"type\":\"input_audio_buffer.append\",\"audio\":\"$audio\"}"
                RealtimeProvider.ELEVENLABS ->
                    "{\"message_type\":\"input_audio_chunk\",\"audio_base_64\":\"$audio\",\"commit\":$commit,\"sample_rate\":$SAMPLE_RATE}"
            }
        }
    }
}
