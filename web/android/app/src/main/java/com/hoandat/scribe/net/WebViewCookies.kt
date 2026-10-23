package com.hoandat.scribe.net

import android.util.Log
import android.webkit.CookieManager
import java.net.CookieHandler
import java.net.URI

/**
 * The backend session cookie lives in the WebView's cookie store (android.webkit.CookieManager): the
 * page's fetch goes through CapacitorHttp, whose HttpURLConnection uses the CookieHandler that
 * Capacitor's CapacitorCookies plugin installs over that same store. The native side (sign-in exchange,
 * refresh, uploads, downloads) uses HttpURLConnection too, so both read and write one cookie jar, as
 * HTTPCookieStorage does on iOS.
 *
 * [install] covers the moment before the bridge loads, or a process where only the recording service
 * runs: it sets an equivalent handler when none is set. Capacitor replaces it with its own on load,
 * which reads the same store.
 */
object WebViewCookies {
    private const val TAG = "ScribeCookies"

    fun install() {
        if (CookieHandler.getDefault() != null) return
        synchronized(this) {
            if (CookieHandler.getDefault() == null) CookieHandler.setDefault(Handler())
        }
    }

    private class Handler : CookieHandler() {
        override fun get(uri: URI, requestHeaders: Map<String, List<String>>): Map<String, List<String>> {
            val cookie = try {
                CookieManager.getInstance().getCookie(uri.toString())
            } catch (e: RuntimeException) {
                // No WebView provider (being updated): send the request without the cookie.
                Log.w(TAG, "cookie store unavailable", e)
                null
            }
            return if (cookie.isNullOrEmpty()) emptyMap() else mapOf("Cookie" to listOf(cookie))
        }

        override fun put(uri: URI, responseHeaders: Map<String, List<String>>) {
            val cookies = responseHeaders.entries
                .filter { it.key.equals("Set-Cookie", ignoreCase = true) }
                .flatMap { it.value }
            if (cookies.isEmpty()) return
            try {
                val manager = CookieManager.getInstance()
                for (cookie in cookies) manager.setCookie(uri.toString(), cookie)
                manager.flush()
            } catch (e: RuntimeException) {
                Log.w(TAG, "cookie store unavailable", e)
            }
        }
    }
}
