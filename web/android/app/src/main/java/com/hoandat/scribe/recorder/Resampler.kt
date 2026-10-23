package com.hoandat.scribe.recorder

/**
 * Linear resampler for mono PCM16, continuous across calls (the last input sample and the fractional
 * position carry over), used to turn the recorder's 48 / 44.1 kHz into the live transcript's 24 kHz.
 * No anti-aliasing filter: enough for speech recognition of a preview (iOS uses AVAudioConverter); the
 * m4a file is never resampled.
 */
class Resampler(private val inRate: Int, private val outRate: Int) {
    init {
        require(inRate > 0 && outRate > 0) { "sample rates must be positive" }
    }

    private val step = inRate.toDouble() / outRate
    /** Input position (in samples of the next chunk) of the next output sample; -1 is the carried sample. */
    private var position = 0.0
    private var last: Short = 0

    fun process(input: ShortArray, offset: Int = 0, count: Int = input.size - offset): ShortArray {
        if (count <= 0) return ShortArray(0)
        if (inRate == outRate) return input.copyOfRange(offset, offset + count)
        val capacity = ((count - position) / step).toInt() + 2
        val out = ShortArray(maxOf(capacity, 0))
        var produced = 0
        while (true) {
            val base = Math.floor(position).toInt()
            if (base + 1 > count - 1) break
            val fraction = position - base
            val s0 = if (base < 0) last.toDouble() else input[offset + base].toDouble()
            val s1 = input[offset + base + 1].toDouble()
            if (produced == out.size) break
            out[produced++] = Math.round(s0 + (s1 - s0) * fraction).toInt()
                .coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt()).toShort()
            position += step
        }
        position -= count
        last = input[offset + count - 1]
        return if (produced == out.size) out else out.copyOf(produced)
    }
}
