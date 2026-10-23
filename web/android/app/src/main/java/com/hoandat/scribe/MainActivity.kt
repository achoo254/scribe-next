package com.hoandat.scribe

import android.os.Bundle
import androidx.activity.OnBackPressedCallback
import com.getcapacitor.BridgeActivity
import com.hoandat.scribe.auth.HoandatAuthPlugin
import com.hoandat.scribe.auth.SecretsPlugin
import com.hoandat.scribe.files.NativeFilesPlugin
import com.hoandat.scribe.net.WebViewCookies
import com.hoandat.scribe.recorder.RecorderPlugin

/**
 * Capacitor bridge of the app (MainViewController.swift on iOS): registers the plugins that live in
 * this module (Capacitor only discovers packaged plugins on its own) and handles Back.
 *
 * Links that leave the app's origin (the IdP's account page, "Mở chat") already open in the system
 * browser: Capacitor's Bridge.launchIntent sends every navigation to another host to ACTION_VIEW.
 * Edge-to-edge (required at target 35+) is Capacitor 8's SystemBars: it passes the system bar insets to
 * the page's `env(safe-area-inset-*)` on recent WebViews and pads the WebView on older ones.
 */
class MainActivity : BridgeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        registerPlugin(HoandatAuthPlugin::class.java)
        registerPlugin(NativeFilesPlugin::class.java)
        registerPlugin(RecorderPlugin::class.java)
        registerPlugin(SecretsPlugin::class.java)
        WebViewCookies.install()
        super.onCreate(savedInstanceState)
        onBackPressedDispatcher.addCallback(this, backThroughHistory)
    }

    /**
     * Back (button or predictive gesture) walks the web app's history, like the iOS edge swipe; with
     * nothing left it sends the app to the background instead of closing it, so a recording keeps going
     * and the page is still there when the user comes back.
     */
    private val backThroughHistory = object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
            val webView = bridge?.webView
            if (webView != null && webView.canGoBack()) {
                webView.goBack()
            } else {
                moveTaskToBack(true)
            }
        }
    }
}
