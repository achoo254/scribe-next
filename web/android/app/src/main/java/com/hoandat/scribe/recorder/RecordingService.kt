package com.hoandat.scribe.recorder

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.hoandat.scribe.AppText
import com.hoandat.scribe.MainActivity
import com.hoandat.scribe.R

/**
 * Foreground service of type `microphone` while a recording runs (iOS: UIBackgroundModes audio). It
 * keeps the process and the microphone when the screen is locked or the app is in the background, and
 * shows the required notification: "Scribe đang ghi" with a running clock, a "Tạm dừng"/"Ghi tiếp"
 * button, an "Đánh dấu" button that adds a bookmark (both also on the lock screen) and a "Dừng" button
 * that opens the app — a recording is never stopped silently from the shade.
 *
 * Started only by [RecorderEngine] when the user taps Record, i.e. while the app is on screen (Android
 * 14+ forbids starting a microphone service from the background). Not sticky: if Android kills the
 * process the recording ends there, and the next launch reconciles the segment that was open.
 */
class RecordingService : Service() {

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val engine = RecorderEngine.get(this)
        when (intent?.action) {
            ACTION_BOOKMARK -> {
                engine.addNativeBookmark()
                return START_NOT_STICKY
            }
            ACTION_TOGGLE_PAUSE -> {
                engine.togglePause()
                return START_NOT_STICKY
            }
            ACTION_START -> Unit
            else -> {
                // Restarted without a recording (should not happen with START_NOT_STICKY).
                stopSelf()
                return START_NOT_STICKY
            }
        }
        // startForeground first, always: a service started with startForegroundService that stops
        // before calling it crashes the app.
        try {
            showNotification(engine.notificationState())
        } catch (e: RuntimeException) {
            // ForegroundServiceStartNotAllowedException / SecurityException (no microphone permission).
            Log.e(TAG, "startForeground refused", e)
            engine.foregroundRefused(AppText.get(R.string.recorder_foreground_refused, e.message ?: e.javaClass.simpleName))
            stopSelf()
            return START_NOT_STICKY
        }
        if (engine.isRecording) {
            engine.service = this
        } else {
            // The recording ended before the service came up.
            finish()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        RecorderEngine.peek()?.let { if (it.service === this) it.service = null }
        super.onDestroy()
    }

    /** The recording ended: remove the notification and stop. */
    fun finish() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    /**
     * Shows (or updates, same id) the foreground notification. The `microphone` type exists from Android 11;
     * Android 10 would reject it (the manifest attribute is unknown there), so no type is passed below R.
     */
    @SuppressLint("WrongConstant") // 0 = no type, below Android 11 only
    fun showNotification(state: NotificationState) {
        val notification = buildNotification(state)
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE else 0
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, type)
    }

    private fun buildNotification(state: NotificationState): Notification {
        ensureChannel(this)
        val open = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val bookmark = PendingIntent.getService(
            this, 1,
            Intent(this, RecordingService::class.java).setAction(ACTION_BOOKMARK),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val togglePause = PendingIntent.getService(
            this, 2,
            Intent(this, RecordingService::class.java).setAction(ACTION_TOGGLE_PAUSE),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val text = state.feedback
            ?: getString(if (state.paused) R.string.recording_paused_text else R.string.recording_text)
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_recording)
            .setContentTitle(getString(if (state.paused) R.string.recording_paused_title else R.string.recording_title))
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            // Paused, the clock would keep running: hidden until resumed.
            .setUsesChronometer(!state.paused)
            .setShowWhen(!state.paused)
            .setWhen(state.startedAtWallMs)
            .addAction(0, getString(if (state.paused) R.string.recording_resume else R.string.recording_pause), togglePause)
            .addAction(0, getString(R.string.recording_bookmark), bookmark)
            .addAction(0, getString(R.string.recording_stop), open)
            .build()
    }

    /** What the notification shows: the clock's origin, whether paused, and an optional "Đã đánh dấu 12:34". */
    data class NotificationState(val startedAtWallMs: Long, val paused: Boolean, val feedback: String?)

    companion object {
        private const val TAG = "ScribeRecordingService"
        private const val CHANNEL_ID = "recording"
        private const val NOTIFICATION_ID = 1
        private const val ACTION_START = "com.hoandat.scribe.recorder.START"
        private const val ACTION_BOOKMARK = "com.hoandat.scribe.recorder.BOOKMARK"
        private const val ACTION_TOGGLE_PAUSE = "com.hoandat.scribe.recorder.TOGGLE_PAUSE"

        fun start(context: Context) {
            val intent = Intent(context, RecordingService::class.java).setAction(ACTION_START)
            ContextCompat.startForegroundService(context, intent)
        }

        private fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            if (manager.getNotificationChannel(CHANNEL_ID) != null) return
            val channel = NotificationChannel(CHANNEL_ID, context.getString(R.string.recording_channel), NotificationManager.IMPORTANCE_LOW)
            channel.description = context.getString(R.string.recording_channel_description)
            channel.lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            channel.setShowBadge(false)
            manager.createNotificationChannel(channel)
        }
    }
}
