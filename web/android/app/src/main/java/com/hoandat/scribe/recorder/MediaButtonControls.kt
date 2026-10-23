package com.hoandat.scribe.recorder

import android.content.Context
import android.content.Intent
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.SystemClock
import android.view.KeyEvent
import androidx.core.content.IntentCompat

/**
 * Headset buttons while recording (the AirPods part of NowPlayingControls.swift): a media session takes
 * the presses instead of controlling playback. Play, pause, play/pause and the wired headset's hook (a
 * single press) pause or resume the recording; next and previous (a double or triple press) add a
 * bookmark. Nothing here stops the recording.
 *
 * Android gives media buttons to the session of the app that last played audio, and Scribe plays none
 * while recording (it does not loop silence as iOS does). So presses may never arrive here, or may only
 * arrive until another app plays something: what works is checked on a device (docs/android-app.md)
 * and the Play listing does not promise headset controls. The notification's buttons are the reliable
 * path. Main thread only.
 */
class MediaButtonControls(
    private val context: Context,
    private val onBookmark: () -> Unit,
    private val onTogglePause: () -> Unit,
) {
    private var session: MediaSession? = null
    private var lastToggleAtMs = 0L

    fun start() {
        if (!ENABLED || session != null) return
        val next = MediaSession(context, "scribe-controls")
        next.setCallback(object : MediaSession.Callback() {
            override fun onMediaButtonEvent(mediaButtonIntent: Intent): Boolean {
                val event = IntentCompat.getParcelableExtra(mediaButtonIntent, Intent.EXTRA_KEY_EVENT, KeyEvent::class.java)
                    ?: return false
                val action: () -> Unit = when (event.keyCode) {
                    in PAUSING_KEYS -> { -> togglePause() }
                    in MARKING_KEYS -> onBookmark
                    else -> return false
                }
                // One action per press: the key down, not its repeat or release.
                if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) action()
                return true
            }

            // Controllers that send transport controls instead of key events (car, watch, lock screen).
            override fun onPlay() = togglePause()
            override fun onPause() = togglePause()
            override fun onSkipToNext() = onBookmark()
            override fun onSkipToPrevious() = onBookmark()
        })
        next.isActive = true
        session = next
        lastToggleAtMs = 0L
        update(paused = false)
    }

    /** The recording was paused or resumed (from here, the notification or the app). */
    fun update(paused: Boolean) {
        val current = session ?: return
        current.setPlaybackState(
            PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
                        PlaybackState.ACTION_SKIP_TO_NEXT or PlaybackState.ACTION_SKIP_TO_PREVIOUS,
                )
                .setState(
                    if (paused) PlaybackState.STATE_PAUSED else PlaybackState.STATE_PLAYING,
                    PlaybackState.PLAYBACK_POSITION_UNKNOWN,
                    if (paused) 0f else 1f,
                )
                .build(),
        )
    }

    fun stop() {
        session?.let {
            it.isActive = false
            it.release()
        }
        session = null
    }

    /** A controller may send play and pause for one tap: closer presses count once. */
    private fun togglePause() {
        val now = SystemClock.elapsedRealtime()
        if (now - lastToggleAtMs < TOGGLE_DEBOUNCE_MS) return
        lastToggleAtMs = now
        onTogglePause()
    }

    private companion object {
        /** Switch for devices where holding the media buttons misbehaves. */
        const val ENABLED = true
        const val TOGGLE_DEBOUNCE_MS = 400L

        val PAUSING_KEYS = setOf(
            KeyEvent.KEYCODE_HEADSETHOOK,
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
            KeyEvent.KEYCODE_MEDIA_PLAY,
            KeyEvent.KEYCODE_MEDIA_PAUSE,
        )
        val MARKING_KEYS = setOf(
            KeyEvent.KEYCODE_MEDIA_NEXT,
            KeyEvent.KEYCODE_MEDIA_PREVIOUS,
        )
    }
}
