package com.hoandat.scribe.recorder

import android.util.Log
import com.hoandat.scribe.net.BackendHttp
import com.hoandat.scribe.net.HttpReply
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.IOException
import java.net.URI
import java.net.URISyntaxException

enum class RealtimeProvider { OPENAI, ELEVENLABS }

/** A token of `POST /api/realtime/token`: one WebSocket's worth of live transcript. */
data class RealtimeToken(
    val value: String,
    val id: String,
    val expiresAtMs: Long,
    val reservedS: Double,
    val provider: RealtimeProvider,
    val url: String,
) {
    companion object {
        /** Socket address for a backend that does not send `url` (OpenAI). */
        private const val OPENAI_URL = "wss://api.openai.com/v1/realtime"

        /** The token in [json], or null when it is not usable (wrong rate, not a wss URL, fields missing). */
        fun parse(json: JSONObject?): RealtimeToken? {
            if (json == null) return null
            // No `provider` = a backend from before ElevenLabs: OpenAI.
            val provider = if (json.optString("provider") == "elevenlabs") RealtimeProvider.ELEVENLABS else RealtimeProvider.OPENAI
            val url = json.optString("url").ifEmpty { if (provider == RealtimeProvider.OPENAI) OPENAI_URL else "" }
            val scheme = try {
                URI(url).scheme
            } catch (e: URISyntaxException) {
                null
            }
            val rate = json.optDouble("sampleRate", RealtimeTranscriber.SAMPLE_RATE.toDouble())
            val value = json.optString("token")
            val id = json.optString("tokenId")
            val expires = StoreFiles.parseIsoTime(json.optString("expiresAt"))
            val reserved = json.optDouble("reservedSeconds", Double.NaN)
            if (value.isEmpty() || id.isEmpty() || expires == null || !(reserved > 0) || scheme != "wss" ||
                rate != RealtimeTranscriber.SAMPLE_RATE.toDouble()
            ) {
                return null
            }
            return RealtimeToken(value, id, expires, reserved, provider, url)
        }
    }
}

sealed interface TokenResult {
    data class Ok(val token: RealtimeToken) : TokenResult
    /** Try again later (network, server busy). */
    data class Transient(val message: String) : TokenResult
    /** Will not work for this recording (not allowed, out of minutes, signed out). */
    data class Final(val message: String) : TokenResult
}

/**
 * Backend calls of the live transcript (RealtimeTranscriber.swift's fetchToken/report): tokens with the
 * native session cookie, renewed from the device token on 401, and usage reports.
 */
class RealtimeApi(
    private val http: BackendHttp,
    private val refreshSession: suspend () -> Unit,
    private val scope: CoroutineScope,
) {
    suspend fun fetchToken(sessionId: String): TokenResult {
        val body = JSONObject().put("sessionId", sessionId)
        return try {
            var reply = http.postJson(TOKEN_PATH, body, TIMEOUT_MS)
            if (reply.status == 401) {
                try {
                    refreshSession()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: IOException) {
                    // Offline, not signed out: try again later.
                    return TokenResult.Transient("Mất kết nối tới máy chủ: ${e.message ?: e.javaClass.simpleName}")
                } catch (e: Exception) {
                    return TokenResult.Final(SIGNED_OUT)
                }
                reply = http.postJson(TOKEN_PATH, body, TIMEOUT_MS)
            }
            tokenResult(reply)
        } catch (e: IOException) {
            TokenResult.Transient("Mất kết nối tới máy chủ: ${e.message ?: e.javaClass.simpleName}")
        }
    }

    /**
     * Reports the seconds streamed with a retired token; a few tries, then the reservation simply stands
     * (the backend counts it in full). [quotaExceeded] tells the backend ElevenLabs is out of credit.
     */
    fun reportUsage(tokenId: String, seconds: Double, quotaExceeded: Boolean = false) {
        val body = JSONObject().put("tokenId", tokenId).put("seconds", seconds)
        if (quotaExceeded) body.put("quotaExceeded", true)
        scope.launch {
            for (attempt in 1..3) {
                try {
                    val reply = http.postJson("/api/realtime/usage", body, TIMEOUT_MS)
                    if (reply.status == 401) {
                        try {
                            refreshSession()
                        } catch (e: CancellationException) {
                            throw e
                        } catch (e: Exception) {
                            Log.w(TAG, "usage report: session not renewed", e)
                        }
                    }
                    // 404/409: unknown or already settled — nothing left to do.
                    if (reply.ok || reply.status == 404 || reply.status == 409) return@launch
                } catch (e: IOException) {
                    Log.w(TAG, "usage report", e)
                }
                delay(attempt * 5_000L)
            }
        }
    }

    companion object {
        private const val TAG = "ScribeRealtime"
        private const val TOKEN_PATH = "/api/realtime/token"
        private const val TIMEOUT_MS = 20_000
        private const val SIGNED_OUT = "Phiên đăng nhập đã hết hạn — chép trực tiếp dừng, bản ghi vẫn tiếp tục."

        /** Only the backend's own `{ "error": … }` is final; a proxy page with the same status is not. */
        fun tokenResult(reply: HttpReply): TokenResult {
            val message = reply.backendError()
            return when (reply.status) {
                in 200..299 -> RealtimeToken.parse(reply.json())?.let { TokenResult.Ok(it) }
                    ?: TokenResult.Transient("Máy chủ trả token chép trực tiếp không đọc được.")
                401 -> TokenResult.Final(SIGNED_OUT)
                400, 403 -> if (message != null) {
                    TokenResult.Final(message)
                } else {
                    TokenResult.Transient("Không lấy được token chép trực tiếp (HTTP ${reply.status}).")
                }
                else -> TokenResult.Transient(message ?: "Không lấy được token chép trực tiếp (HTTP ${reply.status}).")
            }
        }
    }
}
