package com.hoandat.scribe.recorder

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioRecordingConfiguration
import android.media.MediaRecorder
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.content.ContextCompat
import java.io.File
import java.io.IOException
import kotlin.math.sqrt

enum class RecorderState(val js: String) {
    IDLE("idle"),
    STARTING("starting"),
    RECORDING("recording"),
    STOPPING("stopping"),
}

sealed interface RecorderEvent {
    data class State(val state: RecorderState) : RecorderEvent
    /** Seconds recorded so far, about once a second. */
    data class Elapsed(val seconds: Double) : RecorderEvent
    /** RMS level 0…1 of the input, a few times a second. */
    data class Level(val level: Float) : RecorderEvent
    /** A finished, readable segment of [sessionId]. */
    data class SegmentClosed(val sessionId: String, val segment: Segment) : RecorderEvent
    /** The recording keeps going after an interruption. */
    data class Notice(val message: String) : RecorderEvent
    /** Recording stopped by itself (microphone, encoder or disk failure). */
    data class Failure(val message: String) : RecorderEvent
}

/** Receives the recorded PCM16 mono with its rate and session position (live transcript); must not block. */
fun interface PcmSink {
    fun accept(samples: ShortArray, offset: Int, count: Int, sampleRate: Int, positionS: Double)
}

/**
 * Background recording engine (AudioRecorder.swift on iOS): AudioRecord PCM16 mono at 48 kHz (44.1 kHz
 * when the device refuses) → [AacSegmentWriter], rotated every N minutes exactly at the sample where
 * the segment is full, so no sample is lost between two segments and each closed segment is a complete
 * file that can be uploaded at once. The foreground service of type `microphone` (RecordingService)
 * keeps the process and the microphone while the screen is locked.
 *
 * A call or another app taking the microphone does not stop AudioRecord on Android: it delivers
 * silence. Android 10+ says so (AudioRecordingConfiguration.isClientSilenced); the recorder reports it
 * as a notice and keeps going. A dead input (read error, or no samples for 3 seconds) is reopened into
 * a new segment.
 *
 * Main thread: [start], [stop], [state], [sessionId]; [onEvent] is always called on the main thread.
 * The capture thread owns the AudioRecord loop and the segment writer.
 */
class AudioRecorder(private val context: Context, private val onEvent: (RecorderEvent) -> Unit) {
    private val main = Handler(Looper.getMainLooper())

    var state: RecorderState = RecorderState.IDLE
        private set
    /** Session being recorded (main thread). */
    var sessionId: String? = null
        private set

    private val lock = Any()
    // Guarded by [lock]: shared by the capture thread and readers of [recordedSeconds].
    private var closedSeconds = 0.0
    private var openFrames = 0L
    private var openRate = 0
    private var totalFrames = 0L
    private var sink: PcmSink? = null

    // Capture thread (set before it starts).
    private var thread: Thread? = null
    @Volatile private var running = false
    @Volatile private var restartReason: String? = null
    @Volatile private var record: AudioRecord? = null
    private var segmentLimitSeconds = 60.0
    private var segmentDir: File? = null
    private var segmentSessionId = ""
    private var segmentIndex = -1
    private var writer: AacSegmentWriter? = null
    private var checkedBrand = false
    private var writeFailures = 0
    private var levelTick = 0

    // Main thread.
    private var lastFrameCheck = -1L
    private var stalledTicks = 0
    private var silenced = false
    private var recordingCallback: Any? = null

    /** Seconds of audio kept in this session: closed segments plus the open one (bookmarks, transcript). */
    val recordedSeconds: Double
        get() = synchronized(lock) { closedSeconds + if (openRate > 0) openFrames.toDouble() / openRate else 0.0 }

    fun setPcmSink(next: PcmSink?) = synchronized(lock) { sink = next }

    /** Records into [dir] (created by the store) as `seg-000.m4a`, `seg-001.m4a`… Microphone permission granted. */
    fun start(sessionId: String, dir: File, segmentSeconds: Double) {
        if (state != RecorderState.IDLE) return
        this.sessionId = sessionId
        segmentSessionId = sessionId
        segmentLimitSeconds = maxOf(10.0, segmentSeconds)
        segmentDir = dir
        segmentIndex = -1
        checkedBrand = false
        writeFailures = 0
        restartReason = null
        synchronized(lock) {
            closedSeconds = 0.0
            openFrames = 0
            openRate = 0
            totalFrames = 0
        }
        setState(RecorderState.STARTING)
        val opened = openRecord()
        if (opened == null) {
            fail("Không mở được micro — kiểm tra quyền Micro của Scribe trong Cài đặt.")
            return
        }
        try {
            opened.startRecording()
        } catch (e: IllegalStateException) {
            opened.release()
            fail("Không bắt đầu ghi được: ${e.message}")
            return
        }
        if (opened.recordingState != AudioRecord.RECORDSTATE_RECORDING) {
            opened.release()
            fail("Micro đang bị cuộc gọi hoặc ứng dụng khác dùng — chưa ghi được.")
            return
        }
        record = opened
        watchSilence(opened)
        running = true
        thread = Thread({ captureLoop() }, "scribe-capture").also { it.start() }
        lastFrameCheck = -1
        stalledTicks = 0
        setState(RecorderState.RECORDING)
        main.postDelayed(tick, TICK_MS)
    }

    /** Stops recording and closes the last segment. */
    fun stop() {
        when (state) {
            RecorderState.STARTING -> {
                sessionId = null
                setState(RecorderState.IDLE)
            }
            RecorderState.RECORDING -> {
                setState(RecorderState.STOPPING)
                shutdownCapture()
                onEvent(RecorderEvent.Level(0f))
                sessionId = null
                setState(RecorderState.IDLE)
            }
            RecorderState.IDLE, RecorderState.STOPPING -> Unit
        }
    }

    private fun fail(message: String) {
        if (state == RecorderState.IDLE && sessionId == null) {
            onEvent(RecorderEvent.Failure(message))
            return
        }
        shutdownCapture()
        sessionId = null
        setState(RecorderState.IDLE)
        onEvent(RecorderEvent.Failure(message))
    }

    /** Ends the capture thread (it closes the open segment on its way out) and frees the microphone. */
    private fun shutdownCapture() {
        main.removeCallbacks(tick)
        running = false
        val current = record
        try {
            current?.stop()
        } catch (e: IllegalStateException) {
            // Never started: release below.
        }
        val capture = thread
        if (capture != null && capture !== Thread.currentThread()) {
            try {
                capture.join(JOIN_MS)
            } catch (e: InterruptedException) {
                Thread.currentThread().interrupt()
            }
        }
        thread = null
        unwatchSilence()
        record = null
        current?.release()
    }

    // MARK: Capture thread

    private fun captureLoop() {
        var current = record ?: return
        var rate = current.sampleRate
        var buffer = ShortArray(rate / 10)
        try {
            openSegment(rate)
            while (running) {
                val read = current.read(buffer, 0, buffer.size)
                if (!running) break
                val restart = restartReason
                if (read < 0 || restart != null) {
                    restartReason = null
                    current = reopen(current, restart ?: "micro ngừng gửi âm thanh") ?: return
                    rate = current.sampleRate
                    buffer = ShortArray(rate / 10)
                    continue
                }
                if (read == 0) continue
                updateLevel(buffer, read)
                write(buffer, read, rate)
            }
        } catch (e: IOException) {
            Log.e(TAG, "capture failed", e)
            post { fail("Không ghi được vào bộ nhớ máy (có thể đã đầy) — đã dừng, giữ phần đã ghi.") }
        } finally {
            closeSegment("stop")
        }
    }

    private fun write(buffer: ShortArray, count: Int, rate: Int) {
        var offset = 0
        val limit = (segmentLimitSeconds * rate).toLong()
        while (offset < count) {
            val target = writer ?: run {
                openSegment(rate)
                writer ?: throw IOException("no segment writer")
            }
            val (frames, position, receiver) = synchronized(lock) {
                Triple(openFrames, closedSeconds + if (openRate > 0) openFrames.toDouble() / openRate else 0.0, sink)
            }
            val take = minOf((limit - frames).coerceAtLeast(1L), (count - offset).toLong()).toInt()
            try {
                target.write(buffer, offset, take)
                writeFailures = 0
            } catch (e: IOException) {
                // Keep what the segment has, then try a fresh one; give up after a few in a row.
                Log.w(TAG, "segment write failed", e)
                closeSegment("write-error")
                if (++writeFailures >= MAX_WRITE_FAILURES) throw e
                post { onEvent(RecorderEvent.Notice("Không ghi được vào bộ nhớ máy — kiểm tra dung lượng trống.")) }
                openSegment(rate)
                return
            }
            synchronized(lock) {
                openFrames += take
                totalFrames += take
            }
            receiver?.accept(buffer, offset, take, rate, position)
            offset += take
            if (frames + take >= limit) {
                closeSegment("rotation")
                openSegment(rate)
            }
        }
    }

    private fun openSegment(rate: Int) {
        val dir = segmentDir ?: throw IOException("Chưa có thư mục phiên ghi.")
        val index = segmentIndex + 1
        val file = File(dir, Segment.fileName(index))
        val next = AacSegmentWriter(file, rate)
        segmentIndex = index
        writer = next
        synchronized(lock) {
            openFrames = 0
            openRate = rate
        }
    }

    /** Finishes the open segment and reports it (shorter tails are dropped, not kept). */
    private fun closeSegment(reason: String) {
        val current = writer ?: return
        writer = null
        // Copied before close(), which can take a while: a new recording must not change where this segment goes.
        val dir = segmentDir ?: return
        val index = segmentIndex
        val session = segmentSessionId
        val (frames, rate) = synchronized(lock) { openFrames to openRate }
        val seconds = if (rate > 0) frames.toDouble() / rate else 0.0
        val complete = current.close()
        val keep = complete && seconds >= MIN_SEGMENT_SECONDS
        synchronized(lock) {
            if (keep) closedSeconds += seconds
            openFrames = 0
        }
        val file = File(dir, Segment.fileName(index))
        if (!keep) {
            file.delete()
            return
        }
        if (!checkedBrand) {
            checkedBrand = true
            val brand = M4aBrand.read(file)
            if (brand != M4aBrand.AUDIO) {
                post { onEvent(RecorderEvent.Notice("Định dạng tệp ghi bất thường ($brand) — máy chủ có thể không nhận.")) }
            }
        }
        val segment = Segment(index, file.name, seconds, file.length())
        Log.i(TAG, "segment $index closed ($reason): ${"%.1f".format(java.util.Locale.US, seconds)}s")
        post { onEvent(RecorderEvent.SegmentClosed(session, segment)) }
    }

    /** A dead input: new AudioRecord, new segment (the rate may differ). Null after reporting a failure. */
    private fun reopen(old: AudioRecord, reason: String): AudioRecord? {
        closeSegment(reason)
        try {
            old.stop()
        } catch (e: IllegalStateException) {
            // Already stopped.
        }
        old.release()
        post { unwatchSilence() }
        val next = openRecord()
        if (next == null || !startRecording(next)) {
            next?.release()
            record = null
            post { fail("Không ghi tiếp được sau gián đoạn ($reason).") }
            return null
        }
        record = next
        if (!running) {
            // Stopped while reopening: the stop already ran, so free this microphone here.
            record = null
            try {
                next.stop()
            } catch (e: IllegalStateException) {
                // Not recording.
            }
            next.release()
            return null
        }
        return try {
            openSegment(next.sampleRate)
            post {
                watchSilence(next)
                onEvent(RecorderEvent.Notice("Ghi tiếp sau gián đoạn ($reason)."))
            }
            next
        } catch (e: IOException) {
            post { fail("Không mở được đoạn ghi mới: ${e.message}") }
            null
        }
    }

    private fun startRecording(next: AudioRecord): Boolean = try {
        next.startRecording()
        next.recordingState == AudioRecord.RECORDSTATE_RECORDING
    } catch (e: IllegalStateException) {
        false
    }

    private fun updateLevel(buffer: ShortArray, count: Int) {
        if (++levelTick % 2 != 0) return
        var sum = 0.0
        for (i in 0 until count) {
            val v = buffer[i] / 32768.0
            sum += v * v
        }
        val level = sqrt(sum / count).toFloat()
        post { onEvent(RecorderEvent.Level(level)) }
    }

    // MARK: Main thread

    /** Clock + watchdog: if no sample arrives for 3 seconds the input died silently; reopen it. */
    private val tick: Runnable = object : Runnable {
        override fun run() {
            if (state != RecorderState.RECORDING) return
            onEvent(RecorderEvent.Elapsed(recordedSeconds))
            val frames = synchronized(lock) { totalFrames }
            stalledTicks = if (frames == lastFrameCheck) stalledTicks + 1 else 0
            lastFrameCheck = frames
            if (stalledTicks >= 3) {
                stalledTicks = 0
                restartReason = "micro ngừng gửi âm thanh"
                try {
                    record?.stop() // unblocks the capture thread's read
                } catch (e: IllegalStateException) {
                    // Reopened meanwhile.
                }
            }
            main.postDelayed(this, TICK_MS)
        }
    }

    private fun watchSilence(target: AudioRecord) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) watchSilenceQ(target)
    }

    @RequiresApi(Build.VERSION_CODES.Q)
    private fun watchSilenceQ(target: AudioRecord) {
        val manager = context.getSystemService(AudioManager::class.java) ?: return
        val audioSession = target.audioSessionId
        silenced = false
        val callback = object : AudioManager.AudioRecordingCallback() {
            override fun onRecordingConfigChanged(configs: List<AudioRecordingConfiguration>?) {
                val mine = configs?.firstOrNull { it.clientAudioSessionId == audioSession } ?: return
                if (mine.isClientSilenced == silenced || state != RecorderState.RECORDING) return
                silenced = mine.isClientSilenced
                onEvent(
                    RecorderEvent.Notice(
                        if (silenced) {
                            "Micro đang bị cuộc gọi hoặc ứng dụng khác dùng — đoạn này sẽ im lặng, bản ghi vẫn chạy."
                        } else {
                            "Ghi tiếp sau gián đoạn (hết cuộc gọi/gián đoạn)."
                        },
                    ),
                )
            }
        }
        manager.registerAudioRecordingCallback(callback, main)
        recordingCallback = callback
    }

    private fun unwatchSilence() {
        val callback = recordingCallback ?: return
        recordingCallback = null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && callback is AudioManager.AudioRecordingCallback) {
            context.getSystemService(AudioManager::class.java)?.unregisterAudioRecordingCallback(callback)
        }
    }

    @SuppressLint("MissingPermission") // checked on the first line
    private fun openRecord(): AudioRecord? {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            return null
        }
        for (rate in SAMPLE_RATES) {
            val min = AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
            if (min <= 0) continue
            // At least two seconds: a segment rotation (encoder drain + new file) must never overflow it.
            val size = maxOf(min * 4, rate * 2 * 2)
            val candidate = try {
                AudioRecord(MediaRecorder.AudioSource.MIC, rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, size)
            } catch (e: IllegalArgumentException) {
                continue
            } catch (e: SecurityException) {
                return null
            }
            if (candidate.state == AudioRecord.STATE_INITIALIZED) return candidate
            candidate.release()
        }
        return null
    }

    private fun setState(next: RecorderState) {
        state = next
        onEvent(RecorderEvent.State(next))
    }

    private fun post(block: () -> Unit) {
        main.post(block)
    }

    companion object {
        private const val TAG = "ScribeRecorder"
        private val SAMPLE_RATES = intArrayOf(48_000, 44_100)
        private const val MIN_SEGMENT_SECONDS = 0.5
        private const val MAX_WRITE_FAILURES = 3
        private const val TICK_MS = 1_000L
        private const val JOIN_MS = 3_000L
    }
}
