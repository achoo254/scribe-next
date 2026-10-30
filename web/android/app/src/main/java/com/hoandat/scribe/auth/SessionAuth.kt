package com.hoandat.scribe.auth

import android.content.Context
import com.hoandat.scribe.AppText
import com.hoandat.scribe.BuildConfig
import com.hoandat.scribe.R
import com.hoandat.scribe.net.BackendHttp
import com.hoandat.scribe.net.WebViewCookies
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject
import java.io.IOException

/** Where the device token (and the page's allowed secrets) are kept: [SecureStore] on the phone. */
interface TokenStore {
    fun read(name: String): String?

    /** False when the value could not be stored. */
    fun write(name: String, value: String): Boolean

    fun delete(name: String)
}

/** A refusal the user can act on (shown as is); the text is Vietnamese like the backend's. */
open class AuthException(message: String) : Exception(message)

/** The backend failed with [status]; renewal branches on 401/409/410. */
class HttpStatusException(val status: Int, message: String) : AuthException(message)

/**
 * The account behind this device was deleted (410 on refresh): what the phone still keeps for it
 * (segments not uploaded, bookmarks) must go, never into a later account.
 */
class AccountDeletedException : AuthException(AppText.get(R.string.account_deleted))

/**
 * Backend session of the native side, the Android twin of SessionAuth.swift. Sign-in leaves a device
 * token of the backend in [store]; when the session cookie expires during a long recording the upload
 * queue trades it for a new cookie (`POST /api/auth/native/refresh`), which the backend grants only
 * while hoandat-auth still accepts this device. The cookie lands in the WebView's cookie store
 * (WebViewCookies), which the page's CapacitorHttp uses too.
 *
 * Every call blocks on the network: call it from Dispatchers.IO.
 */
class SessionAuth(
    val http: BackendHttp,
    private val store: TokenStore,
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
    private val conflictWaitMs: Long = 2_000,
) {
    fun deviceToken(): String? = store.read(DEVICE_TOKEN)

    /** Sign-out (or a revoked device): the next sign-in starts from the browser again. */
    fun clear() = store.delete(DEVICE_TOKEN)

    fun loadConfig(): NativeAuthConfig {
        val reply = http.get("/api/auth/native/config")
        if (!reply.ok) throw HttpStatusException(reply.status, failure(AppText.get(R.string.sign_in_config_failed), reply.status, reply.backendError()))
        return NativeAuthConfig.parse(reply.json()) ?: throw AuthException(AppText.get(R.string.sign_in_config_incomplete))
    }

    /**
     * Redeems the IdP code at the backend (the response sets the session cookie) and keeps the device
     * token. Returns the signed-in email.
     */
    fun exchange(code: String, verifier: String, nonce: String): String {
        val body = JSONObject().put("code", code).put("codeVerifier", verifier).put("nonce", nonce)
        val json = post("/api/auth/native/exchange", body, AppText.get(R.string.session_exchange_failed))
        val token = json.optString("deviceToken")
        if (token.isEmpty()) throw AuthException(AppText.get(R.string.session_no_device_token))
        clear()
        if (!store.write(DEVICE_TOKEN, token)) throw AuthException(AppText.get(R.string.session_device_token_not_saved))
        return json.optString("email")
    }

    /** Ends the backend session and forgets this device (the IdP grant goes with it). Never throws. */
    fun signOut() {
        val body = JSONObject()
        deviceToken()?.let { body.put("deviceToken", it) }
        try {
            post("/api/auth/logout", body, AppText.get(R.string.session_sign_out_failed))
        } catch (e: IOException) {
            // Offline: the device is forgotten here; the backend session expires on its own.
        } catch (e: AuthException) {
            // Already signed out on the server.
        }
        clear()
    }

    private val renewalLock = Mutex()
    private var renewal: Deferred<Unit>? = null

    /**
     * Gets a new backend session cookie from the stored device token. One renewal at a time (the IdP
     * refresh token behind the device is single-use): concurrent callers wait for the running one.
     * Throws [AccountDeletedException] on 410, [AuthException] when there is no token or the backend
     * refuses it (the caller then asks the user to sign in again), IOException when offline.
     */
    suspend fun refreshSession() {
        val running = renewalLock.withLock {
            renewal ?: scope.async { renew() }.also { renewal = it }
        }
        try {
            running.await()
        } finally {
            renewalLock.withLock { if (renewal === running) renewal = null }
        }
    }

    private suspend fun renew() {
        val token = deviceToken() ?: throw AuthException(AppText.get(R.string.session_none))
        try {
            post("/api/auth/native/refresh", JSONObject().put("deviceToken", token), AppText.get(R.string.session_refresh_failed))
        } catch (e: HttpStatusException) {
            when (e.status) {
                // Another renewal of this device is running (from the WebView): it sets the cookie.
                409 -> delay(conflictWaitMs)
                401 -> {
                    clear()
                    throw AuthException(e.message ?: AppText.get(R.string.session_expired))
                }
                410 -> {
                    clear()
                    throw AccountDeletedException()
                }
                else -> throw e
            }
        }
    }

    private fun post(path: String, body: JSONObject, failure: String): JSONObject {
        val reply = http.postJson(path, body)
        if (!reply.ok) throw HttpStatusException(reply.status, failure(failure, reply.status, reply.backendError()))
        return reply.json() ?: JSONObject()
    }

    private fun failure(prefix: String, status: Int, detail: String?): String = "$prefix: ${detail ?: "HTTP $status"}"

    companion object {
        private const val DEVICE_TOKEN = "deviceToken"

        @Volatile
        private var shared: SessionAuth? = null

        /** The app's instance: backend origin from the build, token in the AndroidKeyStore-backed store. */
        fun shared(context: Context): SessionAuth = shared ?: synchronized(this) {
            shared ?: SessionAuth(
                BackendHttp(backendOrigin(), WebViewCookies::install),
                SecureStore.shared(context),
            ).also { shared = it }
        }

        /** `BuildConfig.SCRIBE_BACKEND_ORIGIN`; the build fails without it (app/build.gradle). */
        fun backendOrigin(): String {
            val origin = BuildConfig.SCRIBE_BACKEND_ORIGIN
            check(origin.startsWith("https://")) { "SCRIBE_BACKEND_ORIGIN is missing from this build" }
            return origin
        }
    }
}
