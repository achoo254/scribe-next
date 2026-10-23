package com.hoandat.scribe.recorder

import java.io.File
import java.io.IOException
import java.io.RandomAccessFile

/**
 * MediaMuxer writes MPEG-4 files with the major brand `mp42`. The backend recognises an upload by its
 * magic bytes (`infer`, which checks video before audio), so `mp42` would store the segment as an mp4
 * video. iOS writes `M4A ` (AudioRecorder.swift checks it); this sets the same major brand in place —
 * same length, nothing else moves, decoders read the file the same way.
 */
object M4aBrand {
    const val AUDIO = "M4A "

    /** Sets the major brand to `M4A `; returns the brand found before, or null when the file is not ISO BMFF. */
    fun apply(file: File): String? {
        RandomAccessFile(file, "rw").use { raf ->
            if (raf.length() < 12) return null
            val header = ByteArray(12)
            raf.readFully(header)
            if (String(header, 4, 4, Charsets.US_ASCII) != "ftyp") return null
            val brand = String(header, 8, 4, Charsets.US_ASCII)
            if (brand != AUDIO) {
                raf.seek(8)
                raf.write(AUDIO.toByteArray(Charsets.US_ASCII))
            }
            return brand
        }
    }

    /** The major brand of [file], or null. */
    fun read(file: File): String? = try {
        RandomAccessFile(file, "r").use { raf ->
            if (raf.length() < 12) return null
            val header = ByteArray(12)
            raf.readFully(header)
            if (String(header, 4, 4, Charsets.US_ASCII) != "ftyp") null else String(header, 8, 4, Charsets.US_ASCII)
        }
    } catch (e: IOException) {
        null
    }
}
