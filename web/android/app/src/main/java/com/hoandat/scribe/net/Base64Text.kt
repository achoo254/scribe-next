package com.hoandat.scribe.net

/**
 * Base64 without android.util.Base64 (not in JVM unit tests) or java.util.Base64 (API 26; minSdk is 24).
 */
object Base64Text {
    private const val STANDARD = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    private const val URL_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

    /** RFC 4648 base64 with padding (audio frames of the live transcript). */
    fun standard(bytes: ByteArray, offset: Int = 0, length: Int = bytes.size - offset): String =
        encode(bytes, offset, length, STANDARD, pad = true)

    /** RFC 4648 base64url without padding (PKCE). */
    fun urlSafe(bytes: ByteArray): String = encode(bytes, 0, bytes.size, URL_SAFE, pad = false)

    private fun encode(bytes: ByteArray, offset: Int, length: Int, alphabet: String, pad: Boolean): String {
        val out = StringBuilder((length + 2) / 3 * 4)
        var i = offset
        val end = offset + length
        while (i + 3 <= end) {
            val n = (bytes[i].toInt() and 0xff shl 16) or (bytes[i + 1].toInt() and 0xff shl 8) or
                (bytes[i + 2].toInt() and 0xff)
            out.append(alphabet[n shr 18 and 63]).append(alphabet[n shr 12 and 63])
                .append(alphabet[n shr 6 and 63]).append(alphabet[n and 63])
            i += 3
        }
        when (end - i) {
            1 -> {
                val n = bytes[i].toInt() and 0xff shl 16
                out.append(alphabet[n shr 18 and 63]).append(alphabet[n shr 12 and 63])
                if (pad) out.append("==")
            }
            2 -> {
                val n = (bytes[i].toInt() and 0xff shl 16) or (bytes[i + 1].toInt() and 0xff shl 8)
                out.append(alphabet[n shr 18 and 63]).append(alphabet[n shr 12 and 63]).append(alphabet[n shr 6 and 63])
                if (pad) out.append('=')
            }
        }
        return out.toString()
    }
}
