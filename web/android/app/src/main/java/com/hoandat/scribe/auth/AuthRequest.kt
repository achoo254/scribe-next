package com.hoandat.scribe.auth

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import com.hoandat.scribe.net.Base64Text
import org.json.JSONObject
import java.net.URI
import java.net.URISyntaxException
import java.net.URLDecoder
import java.net.URLEncoder
import java.security.MessageDigest
import java.security.SecureRandom

/** `GET /api/auth/native/config`: where and how the app signs in (client `scribe-mobile`). */
data class NativeAuthConfig(
    val authorizationEndpoint: String,
    val clientId: String,
    val redirectUri: String,
    val scope: String,
) {
    /** Scheme of [redirectUri] (`com.hoandat.scribe`), the one the browser returns to. */
    val redirectScheme: String? get() = try {
        URI(redirectUri).scheme
    } catch (e: URISyntaxException) {
        null
    }

    companion object {
        fun parse(json: JSONObject?): NativeAuthConfig? {
            if (json == null) return null
            val endpoint = json.optString("authorizationEndpoint")
            val clientId = json.optString("clientId")
            val redirectUri = json.optString("redirectUri")
            val scope = json.optString("scope")
            if (endpoint.isEmpty() || clientId.isEmpty() || redirectUri.isEmpty() || scope.isEmpty()) return null
            return NativeAuthConfig(endpoint, clientId, redirectUri, scope)
        }
    }
}

/**
 * One sign-in attempt: Code + PKCE (S256) with `state` and `nonce`, all made and kept on the phone
 * (never written to disk, never handed to the WebView), like HoandatAuthPlugin.swift. A code
 * intercepted by another app is useless without [verifier] and [nonce].
 */
class AuthRequest(
    val config: NativeAuthConfig,
    val verifier: String,
    val state: String,
    val nonce: String,
) {
    val challenge: String = Pkce.challenge(verifier)

    /** The authorization URL opened in the system browser. */
    fun authorizationUrl(): String {
        val params = listOf(
            "client_id" to config.clientId,
            "redirect_uri" to config.redirectUri,
            "response_type" to "code",
            "scope" to config.scope,
            "code_challenge" to challenge,
            "code_challenge_method" to "S256",
            "state" to state,
            "nonce" to nonce,
        )
        val query = params.joinToString("&") { (name, value) -> "${encode(name)}=${encode(value)}" }
        val separator = if (config.authorizationEndpoint.contains('?')) '&' else '?'
        return config.authorizationEndpoint + separator + query
    }

    /** Reads the redirect the browser returned: the code to exchange, or why sign-in failed. */
    fun parseCallback(url: String): CallbackResult {
        val base = url.substringBefore('?').substringBefore('#')
        if (!base.equals(config.redirectUri, ignoreCase = true)) return CallbackResult.Failure(AppText.get(R.string.sign_in_state_mismatch))
        val query = try {
            URI(url).rawQuery
        } catch (e: URISyntaxException) {
            return CallbackResult.Failure(AppText.get(R.string.sign_in_state_mismatch))
        }
        val values = parseQuery(query)
        values["error"]?.let { error ->
            return CallbackResult.Failure(
                if (error == "access_denied") AppText.get(R.string.sign_in_denied) else AppText.get(R.string.sign_in_failed_code, error),
            )
        }
        val code = values["code"]
        if (values["state"] != state || code.isNullOrEmpty()) return CallbackResult.Failure(AppText.get(R.string.sign_in_state_mismatch))
        return CallbackResult.Code(code)
    }

    companion object {
        fun create(config: NativeAuthConfig, random: SecureRandom = SecureRandom()): AuthRequest =
            AuthRequest(
                config = config,
                verifier = Pkce.randomUrlSafe(32, random),
                state = Pkce.randomUrlSafe(16, random),
                nonce = Pkce.randomUrlSafe(16, random),
            )

        /** Form encoding with `%20` for spaces (the scope), which every parser reads the same way. */
        private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8").replace("+", "%20")

        /** First value of each parameter of a raw query string. */
        fun parseQuery(rawQuery: String?): Map<String, String> {
            if (rawQuery.isNullOrEmpty()) return emptyMap()
            val values = LinkedHashMap<String, String>()
            for (pair in rawQuery.split('&')) {
                if (pair.isEmpty()) continue
                val name = decode(pair.substringBefore('='))
                val value = if (pair.contains('=')) decode(pair.substringAfter('=')) else ""
                if (name != null && value != null && !values.containsKey(name)) values[name] = value
            }
            return values
        }

        private fun decode(value: String): String? = try {
            URLDecoder.decode(value, "UTF-8")
        } catch (e: IllegalArgumentException) {
            null
        }
    }
}

sealed interface CallbackResult {
    data class Code(val code: String) : CallbackResult
    data class Failure(val message: String) : CallbackResult
}

/** RFC 7636 helpers; base64url without padding, as the IdP expects. */
object Pkce {
    fun randomUrlSafe(byteCount: Int, random: SecureRandom = SecureRandom()): String {
        val bytes = ByteArray(byteCount)
        random.nextBytes(bytes)
        return base64Url(bytes)
    }

    fun challenge(verifier: String): String =
        base64Url(MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.US_ASCII)))

    fun base64Url(bytes: ByteArray): String = Base64Text.urlSafe(bytes)
}
