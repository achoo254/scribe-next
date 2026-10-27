package com.hoandat.scribe.auth

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.activity.result.ActivityResultLauncher
import androidx.browser.auth.AuthTabIntent
import androidx.browser.customtabs.CustomTabsClient
import androidx.browser.customtabs.CustomTabsIntent
import com.getcapacitor.JSObject
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin
import com.hoandat.scribe.BuildConfig
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException
import java.lang.ref.WeakReference

/*
 * Sign-in through hoandat-auth (OIDC, public client `scribe-mobile`), same JS contract as
 * HoandatAuthPlugin.swift: `signIn(): {email}`, `signOut()`, errors CANCELLED / SIGN_IN_FAILED.
 *
 * The app makes and keeps the PKCE verifier, `state` and `nonce` ([AuthRequest], memory only), opens
 * the authorization URL in the system browser — never a WebView — and gets the redirect
 * `com.hoandat.scribe:/auth/callback` back. The backend then redeems the code with the verifier and
 * nonce, sets the session cookie and returns a device token kept in [SecureStore] ([SessionAuth]).
 *
 * The browser part follows @achoo254/capacitor-system-auth (shared-next tag v0.3.0) (SystemAuthPlugin.kt, compiled
 * in Localman's CI), copied on purpose rather than depended on: a Capacitor plugin cannot call another
 * plugin's method from Kotlin, and using SystemAuth from JS would move the verifier into the WebView.
 * Auth Tab when the default browser supports it (the result comes back as an activity result);
 * otherwise a Custom Tab, with the redirect arriving through [AuthRedirectActivity], and a return to
 * the app without a redirect meaning the user closed it. The IdP's browser session is reused (no
 * ephemeral mode), so a user already signed in at hoandat-auth taps once.
 *
 * If Android kills the app while the browser is open, the attempt is gone (the verifier was only in
 * memory); the redirect then just reopens the app and the user taps "Đăng nhập" again.
 *
 * Every state change runs on the main thread.
 */
@CapacitorPlugin(name = "HoandatAuth")
class HoandatAuthPlugin : Plugin() {

    private class Pending(val call: PluginCall, val request: AuthRequest, val viaCustomTabs: Boolean) {
        /** Set once the app is paused behind the Custom Tab; the next resume without a redirect is a cancel. */
        var leftApp = false
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var pending: Pending? = null
    /** A sign-in between the config request and the browser opening (one attempt at a time). */
    private var starting = false
    private var authTabLauncher: ActivityResultLauncher<Intent>? = null

    private val session: SessionAuth get() = SessionAuth.shared(context)

    override fun load() {
        active = WeakReference(this)
        // Must be registered before the activity is STARTED; plugins load in BridgeActivity.onCreate.
        authTabLauncher = try {
            AuthTabIntent.registerActivityResultLauncher(activity) { result -> onAuthTabResult(result) }
        } catch (e: IllegalStateException) {
            Log.w(TAG, "Auth Tab launcher not registered; Custom Tabs only", e)
            null
        }
    }

    @PluginMethod
    fun signIn(call: PluginCall) {
        scope.launch {
            if (pending != null || starting) {
                call.reject("Đang mở trang đăng nhập.", SIGN_IN_FAILED)
                return@launch
            }
            starting = true
            try {
                val config = withContext(Dispatchers.IO) { session.loadConfig() }
                if (!config.redirectScheme.equals(REDIRECT_SCHEME, ignoreCase = true)) {
                    throw AuthException("Cấu hình đăng nhập không hợp lệ.")
                }
                openBrowser(call, AuthRequest.create(config))
            } catch (e: CancellationException) {
                throw e
            } catch (e: AuthException) {
                call.reject(e.message, SIGN_IN_FAILED)
            } catch (e: IOException) {
                call.reject("Không kết nối được máy chủ: ${e.message ?: e.javaClass.simpleName}", SIGN_IN_FAILED)
            } catch (e: Exception) {
                Log.e(TAG, "sign-in failed", e)
                call.reject("Đăng nhập thất bại: ${e.message ?: e.javaClass.simpleName}", SIGN_IN_FAILED)
            } finally {
                starting = false
            }
        }
    }

    /** Ends the backend session and forgets this device (the IdP grant goes with it). */
    @PluginMethod
    fun signOut(call: PluginCall) {
        scope.launch {
            withContext(Dispatchers.IO) { session.signOut() }
            call.resolve()
        }
    }

    private fun openBrowser(call: PluginCall, request: AuthRequest) {
        val url = Uri.parse(request.authorizationUrl())
        val browser = CustomTabsClient.getPackageName(context, emptyList())
        val launcher = authTabLauncher
        try {
            if (launcher != null && browser != null && !forceCustomTabs() &&
                CustomTabsClient.isAuthTabSupported(context, browser)
            ) {
                pending = Pending(call, request, viaCustomTabs = false)
                val intent = AuthTabIntent.Builder().build()
                intent.intent.setPackage(browser)
                intent.launch(launcher, url, REDIRECT_SCHEME)
            } else {
                pending = Pending(call, request, viaCustomTabs = true)
                val intent = CustomTabsIntent.Builder().build()
                // No Custom Tabs browser: launchUrl falls back to a plain ACTION_VIEW.
                browser?.let { intent.intent.setPackage(it) }
                intent.launchUrl(activity, url)
            }
        } catch (e: ActivityNotFoundException) {
            pending = null
            call.reject("Máy chưa có trình duyệt để mở trang đăng nhập.", SIGN_IN_FAILED)
        }
    }

    /** Debug builds only: `adb shell run-as com.hoandat.scribe touch files/force-custom-tabs` tests the fallback. */
    private fun forceCustomTabs(): Boolean =
        BuildConfig.DEBUG && File(context.filesDir, "force-custom-tabs").exists()

    private fun onAuthTabResult(result: AuthTabIntent.AuthResult) {
        val current = pending?.takeIf { !it.viaCustomTabs } ?: return
        pending = null
        val uri = result.resultUri
        when (result.resultCode) {
            AuthTabIntent.RESULT_OK ->
                if (uri != null) {
                    finish(current, uri.toString())
                } else {
                    current.call.reject("Đăng nhập không trả kết quả.", SIGN_IN_FAILED)
                }
            AuthTabIntent.RESULT_CANCELED -> current.call.reject(CLOSED, CANCELLED)
            else -> current.call.reject("Đăng nhập thất bại (Auth Tab ${result.resultCode}).", SIGN_IN_FAILED)
        }
    }

    /** Resolves the pending sign-in when [uri] is its redirect. Main thread. */
    private fun consumeRedirect(uri: Uri): Boolean {
        val current = pending?.takeIf { it.viaCustomTabs } ?: return false
        if (!uri.scheme.equals(REDIRECT_SCHEME, ignoreCase = true)) return false
        pending = null
        finish(current, uri.toString())
        return true
    }

    /** Checks the redirect, then lets the backend redeem the code. */
    private fun finish(current: Pending, callbackUrl: String) {
        when (val result = current.request.parseCallback(callbackUrl)) {
            is CallbackResult.Failure -> current.call.reject(result.message, SIGN_IN_FAILED)
            is CallbackResult.Code -> scope.launch {
                try {
                    val request = current.request
                    val email = withContext(Dispatchers.IO) { session.exchange(result.code, request.verifier, request.nonce) }
                    current.call.resolve(JSObject().put("email", email))
                } catch (e: CancellationException) {
                    throw e
                } catch (e: AuthException) {
                    current.call.reject(e.message, SIGN_IN_FAILED)
                } catch (e: IOException) {
                    current.call.reject("Không kết nối được máy chủ: ${e.message ?: e.javaClass.simpleName}", SIGN_IN_FAILED)
                } catch (e: Exception) {
                    Log.e(TAG, "code exchange failed", e)
                    current.call.reject("Đăng nhập thất bại: ${e.message ?: e.javaClass.simpleName}", SIGN_IN_FAILED)
                }
            }
        }
    }

    // A BridgeActivity forwards new intents to plugins (a redirect delivered to MainActivity itself).
    override fun handleOnNewIntent(intent: Intent) {
        handleRedirect(intent)
    }

    override fun handleOnPause() {
        pending?.takeIf { it.viaCustomTabs }?.leftApp = true
    }

    override fun handleOnResume() {
        val current = pending?.takeIf { it.viaCustomTabs && it.leftApp } ?: return
        pending = null
        current.call.reject(CLOSED, CANCELLED)
    }

    override fun handleOnDestroy() {
        pending?.call?.reject("Ứng dụng đã đóng trong lúc đăng nhập — thử lại.", SIGN_IN_FAILED)
        pending = null
        scope.cancel()
        if (active?.get() === this) active = null
    }

    companion object {
        private const val TAG = "ScribeAuth"
        private const val CANCELLED = "CANCELLED"
        private const val SIGN_IN_FAILED = "SIGN_IN_FAILED"
        private const val CLOSED = "Cửa sổ đăng nhập đã đóng trước khi hoàn tất."

        /** Scheme of `com.hoandat.scribe:/auth/callback`, the only one AuthRedirectActivity receives. */
        const val REDIRECT_SCHEME = "com.hoandat.scribe"

        @Volatile
        private var active: WeakReference<HoandatAuthPlugin>? = null

        /**
         * Hands a redirect to the plugin; call it on the main thread. Returns true when the intent was
         * the redirect of the pending sign-in.
         */
        @JvmStatic
        fun handleRedirect(intent: Intent?): Boolean {
            val uri = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.data ?: return false
            return active?.get()?.consumeRedirect(uri) ?: false
        }
    }
}
