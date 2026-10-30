package com.hoandat.scribe.recorder

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.MediaMuxer
import java.io.File
import java.io.IOException
import java.nio.ByteOrder

/**
 * One segment file: mono PCM16 → AAC-LC (MediaCodec) → MPEG-4 audio (MediaMuxer), major brand set to
 * `M4A ` on close ([M4aBrand]). The muxer writes the moov atom only in [close], so a file still open
 * when the process dies is unreadable; SegmentStore.reconcile reports and removes it (same bound as
 * iOS: at most one segment, 1 minute by default).
 *
 * Not thread-safe: the recorder's capture thread owns it.
 */
class AacSegmentWriter(private val file: File, private val sampleRate: Int) {
    private val codec: MediaCodec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_AUDIO_AAC)
    private val info = MediaCodec.BufferInfo()
    private var track = -1
    private var muxerStarted = false
    private var samplesQueued = 0L
    private var closed = false

    /** Samples handed to the encoder so far. */
    val samples: Long get() = samplesQueued

    init {
        val format = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AAC, sampleRate, 1).apply {
            setInteger(MediaFormat.KEY_AAC_PROFILE, MediaCodecInfo.CodecProfileLevel.AACObjectLC)
            setInteger(MediaFormat.KEY_BIT_RATE, bitRate(sampleRate))
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, MAX_INPUT_BYTES)
        }
        try {
            codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            codec.start()
        } catch (e: Exception) {
            codec.release()
            throw IOException(AppText.get(R.string.recorder_encoder_failed, "${e.message}"), e)
        }
    }

    private val muxer: MediaMuxer = openMuxer()

    private fun openMuxer(): MediaMuxer = try {
        MediaMuxer(file.path, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)
    } catch (e: IOException) {
        codec.stop()
        codec.release()
        throw e
    }

    /** Encodes [count] samples of [pcm] from [offset]. Throws IOException when the encoder fails. */
    fun write(pcm: ShortArray, offset: Int, count: Int) {
        check(!closed) { "writer closed" }
        var position = offset
        val end = offset + count
        var stalls = 0
        try {
            while (position < end) {
                val index = codec.dequeueInputBuffer(TIMEOUT_US)
                if (index >= 0) {
                    val buffer = codec.getInputBuffer(index) ?: throw IOException("encoder input buffer missing")
                    buffer.clear()
                    val n = minOf(buffer.remaining() / 2, end - position)
                    buffer.order(ByteOrder.nativeOrder()).asShortBuffer().put(pcm, position, n)
                    codec.queueInputBuffer(index, 0, n * 2, presentationUs(samplesQueued), 0)
                    samplesQueued += n
                    position += n
                    stalls = 0
                } else if (++stalls > MAX_STALLS) {
                    throw IOException("encoder stalled")
                }
                drain(endOfStream = false)
            }
        } catch (e: IllegalStateException) {
            throw IOException("encoder failed: ${e.message}", e)
        }
    }

    /**
     * Ends the stream, writes the moov atom and releases the encoder. Returns false when the file is not
     * a complete segment (nothing encoded, or the encoder failed): the caller drops it.
     */
    fun close(): Boolean {
        if (closed) return false
        closed = true
        var complete = false
        try {
            var queued = false
            var tries = 0
            while (!queued && tries++ < MAX_STALLS) {
                val index = codec.dequeueInputBuffer(TIMEOUT_US)
                if (index >= 0) {
                    codec.queueInputBuffer(index, 0, 0, presentationUs(samplesQueued), MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                    queued = true
                } else {
                    drain(endOfStream = false)
                }
            }
            if (queued) drain(endOfStream = true)
            complete = queued && muxerStarted && samplesQueued > 0
        } catch (e: Exception) {
            complete = false
        } finally {
            try {
                codec.stop()
            } catch (e: IllegalStateException) {
                // Already in an error state: release below still frees it.
            }
            codec.release()
            if (muxerStarted) {
                try {
                    muxer.stop()
                } catch (e: IllegalStateException) {
                    complete = false
                }
            }
            try {
                muxer.release()
            } catch (e: IllegalStateException) {
                complete = false
            }
        }
        if (complete) {
            try {
                M4aBrand.apply(file)
            } catch (e: IOException) {
                complete = false
            }
        }
        return complete
    }

    private fun drain(endOfStream: Boolean) {
        var waits = 0
        while (true) {
            val index = codec.dequeueOutputBuffer(info, if (endOfStream) TIMEOUT_US else 0)
            when {
                index == MediaCodec.INFO_TRY_AGAIN_LATER -> {
                    if (!endOfStream || ++waits > MAX_STALLS) return
                }
                index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                    if (muxerStarted) throw IOException("encoder format changed twice")
                    track = muxer.addTrack(codec.outputFormat)
                    muxer.start()
                    muxerStarted = true
                }
                index >= 0 -> {
                    val buffer = codec.getOutputBuffer(index)
                    if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) info.size = 0
                    if (buffer != null && info.size > 0 && muxerStarted) {
                        buffer.position(info.offset)
                        buffer.limit(info.offset + info.size)
                        muxer.writeSampleData(track, buffer, info)
                    }
                    codec.releaseOutputBuffer(index, false)
                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) return
                }
            }
        }
    }

    private fun presentationUs(sample: Long): Long = sample * 1_000_000L / sampleRate

    companion object {
        private const val TIMEOUT_US = 10_000L
        private const val MAX_STALLS = 500
        private const val MAX_INPUT_BYTES = 16_384

        /** AAC bitrate must fit the sample rate (AudioRecorder.swift's table, mono). */
        fun bitRate(sampleRate: Int): Int = when {
            sampleRate <= 16_000 -> 32_000
            sampleRate <= 24_000 -> 48_000
            sampleRate <= 32_000 -> 64_000
            else -> 96_000
        }
    }
}
