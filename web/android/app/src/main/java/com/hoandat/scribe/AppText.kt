package com.hoandat.scribe

import android.content.Context
import androidx.annotation.StringRes

/**
 * Texts the native side shows (errors, notices) in the phone's language, for code that has no Context
 * of its own (auth, upload queue, stores). The strings live in res/values (Vietnamese, the default),
 * values-en and values-zh-rCN. Every entry point of the process ([MainActivity],
 * [com.hoandat.scribe.auth.AuthRedirectActivity], [com.hoandat.scribe.recorder.RecorderEngine]) calls
 * [init] before any of that code runs.
 */
object AppText {
    @Volatile
    private var lookup: ((Int, Array<out Any>) -> String)? = null

    fun init(context: Context) {
        if (lookup != null) return
        // The application context follows a change of the phone's language without a restart.
        val app = context.applicationContext
        lookup = { id, args -> app.getString(id, *args) }
    }

    /** Text [id] with its `%1$s`… placeholders filled from [args]. */
    fun get(@StringRes id: Int, vararg args: Any): String =
        // Only JVM unit tests run without init: the id keeps their assertions exact.
        lookup?.invoke(id, args) ?: "#$id${args.joinToString("") { "|$it" }}"
}
