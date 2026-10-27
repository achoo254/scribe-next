package com.hoandat.scribe.auth

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Target of `com.hoandat.scribe:/auth/callback` when the browser has no Auth Tab (Custom Tabs fallback,
 * as RedirectActivity of @achoo254/capacitor-system-auth). Hands the URL to [HoandatAuthPlugin],
 * then brings the app back to the front, which also closes the Custom Tab sitting above it.
 */
class AuthRedirectActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        HoandatAuthPlugin.handleRedirect(intent)
        packageManager.getLaunchIntentForPackage(packageName)?.let { launch ->
            launch.addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            startActivity(launch)
        }
        finish()
    }
}
