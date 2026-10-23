package com.hoandat.scribe.recorder

import android.content.Context
import android.content.Intent
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.view.KeyEvent
import androidx.core.content.IntentCompat

/**
 * Headset buttons while recording (the AirPods part of NowPlayingBookmarks.swift): a media session
 * marked as playing takes play/pause/next/previous/hook presses and turns each into a bookmark instead
 * of controlling playback. Nothing here pauses or stops the recording.
 *
 * Android gives media buttons to the session of the app that last played audio, and Scribe plays none
 * while recording (it does not loop silence as iOS does). So presses may never arrive here, or may only
 * arrive until another app plays something: what works is checked on a device (docs/android-app.md)
 * and the Play listing does not promise headset bookmarks. The notification's "Đánh dấu" button is the
 * reliable path. Main thread only.
 */
class MediaButtonBookmarks(private val context: Context, private val onPress: () -> Unit) {
    private var session: MediaSession? = null

    fun start() {
        if (!ENABLED || session != null) return
        val next = MediaSession(context, "scribe-bookmarks")
        next.setCallback(object : MediaSession.Callback() {
            override fun onMediaButtonEvent(mediaButtonIntent: Intent): Boolean {
                val event = IntentCompat.getParcelableExtra(mediaButtonIntent, Intent.EXTRA_KEY_EVENT, KeyEvent::class.java)
                    ?: return false
                if (event.keyCode !in MARKING_KEYS) return false
                // One bookmark per press: the key down, not its repeat or release.
                if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) onPress()
                return true
            }

            // Controllers that send transport controls instead of key events (car, watch, lock screen).
            override fun onPlay() = onPress()
            override fun onPause() = onPress()
            override fun onSkipToNext() = onPress()
            override fun onSkipToPrevious() = onPress()
        })
        next.setPlaybackState(
            PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
                        PlaybackState.ACTION_SKIP_TO_NEXT or PlaybackState.ACTION_SKIP_TO_PREVIOUS,
                )
                .setState(PlaybackState.STATE_PLAYING, PlaybackState.PLAYBACK_POSITION_UNKNOWN, 1f)
                .build(),
        )
        next.isActive = true
        session = next
    }

    fun stop() {
        session?.let {
            it.isActive = false
            it.release()
        }
        session = null
    }

    private companion object {
        /** Switch for devices where holding the media buttons misbehaves. */
        const val ENABLED = true

        val MARKING_KEYS = setOf(
            KeyEvent.KEYCODE_HEADSETHOOK,
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
            KeyEvent.KEYCODE_MEDIA_PLAY,
            KeyEvent.KEYCODE_MEDIA_PAUSE,
            KeyEvent.KEYCODE_MEDIA_NEXT,
            KeyEvent.KEYCODE_MEDIA_PREVIOUS,
        )
    }
}
