package com.hoandat.scribe.recorder

import android.util.Log
import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import com.hoandat.scribe.auth.AccountDeletedException
import com.hoandat.scribe.net.BackendHttp
import com.hoandat.scribe.net.HttpReply
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.IOException
import java.net.URLEncoder

sealed interface UploadEvent {
    data class Counts(val waiting: Int, val failed: Int) : UploadEvent
    data class Uploaded(val name: String) : UploadEvent
    data object NeedsSignIn : UploadEvent
    data class QuotaFull(val message: String) : UploadEvent
    /** The account was deleted: its segments are gone from the phone. */
    data class AccountDeleted(val userKey: String) : UploadEvent
}

/**
 * Uploads closed segments one at a time, also while the next one is being recorded (Android twin of
 * UploadQueue.swift). Each segment becomes one recording through `/api/recordings/init → chunk →
 * finalize`, named `<sessionId>#<index>`.
 *
 * The backend has no idempotency, so a segment never gets a second `init` blindly: the server session
 * is kept in the manifest (a retry resends into it from offset 0), and once `finalize` was sent the
 * deterministic name is looked up first, because the server may have committed while the reply was
 * lost. A failed lookup is transient, never a reason to start over.
 *
 * Failure groups: transient (retry with backoff), redo (the server lost the upload: drop its session,
 * start the segment over), terminal (the backend refused the audio: keep the file, stop that segment),
 * 401 (renew the session from the device token, else wait for sign-in), 410 on renewal (account
 * deleted: remove its segments) and 413 (quota full: wait).
 *
 * HTTP calls block inside [scope] (Dispatchers.IO); a sign-out cancels the drain, and a reply that
 * arrives after it is dropped.
 */
class UploadQueue(
    private val store: SegmentStore,
    private val http: BackendHttp,
    private val refreshSession: suspend () -> Unit,
    private val scope: CoroutineScope,
    private val onEvent: (UploadEvent) -> Unit,
) {
    private sealed class Outcome(message: String) : Exception(message) {
        class Transient(message: String) : Outcome(message)
        class Redo(message: String) : Outcome(message)
        class Terminal(message: String) : Outcome(message)
        class Unauthorized : Outcome("unauthorized")
        class AccountDeleted : Outcome("account deleted")
        class QuotaFull(message: String) : Outcome(message)
    }

    private val lock = Any()
    private var userKey: String? = null
    private var paused = false
    private var drainJob: Job? = null
    /** A kick arrived while draining: run once more when the loop ends. */
    private var rerun = false
    private var failures = 0
    private var retryJob: Job? = null
    /** "Needs sign-in" is reported once until the next start. */
    private var reportedSignIn = false

    /** Starts (or resumes) uploading for this user: at sign-in, at launch and back in the foreground. */
    fun start(userKey: String) = synchronized(lock) {
        if (this.userKey != userKey) {
            stopDrainLocked()
            this.userKey = userKey
        }
        failures = 0
        paused = false
        reportedSignIn = false
        kickLocked(userKey)
    }

    /** Sign-out: stop at once, cancel the upload in flight, and never send the previous user's segments. */
    fun stop() = synchronized(lock) {
        userKey = null
        paused = true
        stopDrainLocked()
    }

    /** Wakes the queue for [user] only: a segment closing right before a sign-out must not restart uploads. */
    fun kick(user: String) = synchronized(lock) { kickLocked(user) }

    /** The network came back: retry now instead of waiting for the backoff. */
    fun networkBack() = synchronized(lock) {
        val user = userKey ?: return@synchronized
        failures = 0
        retryJob?.cancel()
        retryJob = null
        kickLocked(user)
    }

    private fun kickLocked(user: String) {
        if (user != userKey || paused) return
        if (drainJob?.isActive == true) {
            rerun = true
            return
        }
        drainJob = scope.launch { runDrain(user) }
    }

    private fun stopDrainLocked() {
        drainJob?.cancel()
        drainJob = null
        retryJob?.cancel()
        retryJob = null
        rerun = false
    }

    private fun isCurrent(user: String): Boolean = synchronized(lock) { user == userKey && !paused }

    private suspend fun runDrain(user: String) {
        val job = currentCoroutineContext()[Job]
        while (true) {
            synchronized(lock) { rerun = false }
            drain(user)
            val again = synchronized(lock) {
                val more = rerun && user == userKey && !paused && job?.isActive != false
                if (!more && drainJob === job) drainJob = null
                more
            }
            if (!again) return
        }
    }

    private suspend fun drain(user: String) {
        while (currentCoroutineContext().isActive && isCurrent(user)) {
            val pending = store.pending(user)
            publishCounts(user)
            val next = pending.firstOrNull() ?: return
            try {
                val fileId = uploadWithRenewal(next)
                if (!isCurrent(user)) return
                store.markUploaded(user, next.sessionId, next.segment.index, fileId)
                synchronized(lock) { failures = 0 }
                onEvent(UploadEvent.Uploaded(name(next)))
            } catch (e: CancellationException) {
                throw e
            } catch (outcome: Outcome) {
                if (!isCurrent(user)) return
                if (!handle(outcome, next, user)) return
            } catch (e: Exception) {
                Log.w(TAG, "upload failed", e)
                synchronized(lock) { failures++ }
                scheduleRetry(user)
                return
            }
        }
    }

    /** False when the loop must stop. */
    private fun handle(outcome: Outcome, next: PendingSegment, user: String): Boolean {
        val index = next.segment.index
        return when (outcome) {
            is Outcome.Transient -> {
                Log.w(TAG, "transient: ${outcome.message}")
                synchronized(lock) { failures++ }
                scheduleRetry(user)
                false
            }
            is Outcome.Redo -> {
                Log.i(TAG, "redo: ${outcome.message}")
                ignoringIo { store.setUploadState(user, next.sessionId, index, null, false) }
                val failure = if (next.segment.attempts >= 4) outcome.message else null
                ignoringIo { store.markAttempt(user, next.sessionId, index, failure) }
                true
            }
            is Outcome.Terminal -> {
                ignoringIo { store.markAttempt(user, next.sessionId, index, outcome.message) }
                true
            }
            is Outcome.Unauthorized -> {
                val report = synchronized(lock) {
                    paused = true
                    (!reportedSignIn).also { reportedSignIn = true }
                }
                if (report) onEvent(UploadEvent.NeedsSignIn)
                false
            }
            is Outcome.QuotaFull -> {
                synchronized(lock) { paused = true }
                onEvent(UploadEvent.QuotaFull(outcome.message ?: AppText.get(R.string.upload_storage_full)))
                false
            }
            is Outcome.AccountDeleted -> {
                // Never upload these into whatever account signs in next with the same email.
                synchronized(lock) { paused = true }
                ignoringIo { store.removeAll(user) }
                Log.i(TAG, "account deleted: local segments removed")
                onEvent(UploadEvent.AccountDeleted(user))
                false
            }
        }
    }

    private fun scheduleRetry(user: String) = synchronized(lock) {
        val exponent = minOf(failures, 6)
        val delaySeconds = minOf(300L, 5L shl exponent)
        retryJob?.cancel()
        retryJob = scope.launch {
            delay(delaySeconds * 1_000)
            kick(user)
        }
    }

    private fun publishCounts(user: String) {
        val (waiting, failed) = store.queueCounts(user)
        onEvent(UploadEvent.Counts(waiting, failed))
    }

    /** One attempt; a 401 first renews the session silently and retries once. */
    private suspend fun uploadWithRenewal(pending: PendingSegment): String {
        try {
            return upload(pending)
        } catch (e: Outcome.Unauthorized) {
            try {
                refreshSession()
                Log.i(TAG, "session renewed from the device token")
            } catch (e: CancellationException) {
                throw e
            } catch (e: AccountDeletedException) {
                throw Outcome.AccountDeleted()
            } catch (e: Exception) {
                throw Outcome.Unauthorized()
            }
            return upload(pending)
        }
    }

    private suspend fun upload(pending: PendingSegment): String {
        val name = name(pending)
        val user = pending.userKey
        val index = pending.segment.index
        // Re-read: the manifest has the latest server session / finalize flag.
        val segment = store.session(user, pending.sessionId)?.segments?.firstOrNull { it.index == index } ?: pending.segment

        if (segment.finalizeSent) lookUp(name)?.let { return it }
        val data = try {
            pending.file.readBytes()
        } catch (e: IOException) {
            null
        }
        if (data == null || data.isEmpty()) throw Outcome.Terminal(AppText.get(R.string.upload_unreadable_segment))

        var serverSession = segment.serverSession
        if (serverSession == null) {
            val reply = json(http.runCatchingIo { postJson("/api/recordings/init", JSONObject().put("name", name)) })
            val created = reply.optString("sessionId").ifEmpty { throw Outcome.Transient("init không trả sessionId") }
            serverSession = created
            ignoringIo { store.setUploadState(user, pending.sessionId, index, created, false) }
        }

        // From offset 0 every time: the same bytes overwrite a partial upload.
        var offset = 0
        val session = encode(serverSession)
        while (offset < data.size) {
            currentCoroutineContext().ensureActive()
            val end = minOf(offset + CHUNK_SIZE, data.size)
            val reply = json(
                http.runCatchingIo { postBytes("/api/recordings/chunk?session=$session&offset=$offset", data.copyOfRange(offset, end)) },
            )
            val written = reply.optLong("written", -1)
            if (written < end) throw Outcome.Redo(AppText.get(R.string.upload_chunk_incomplete))
            offset = end
        }

        ignoringIo { store.setUploadState(user, pending.sessionId, index, serverSession, true) }
        try {
            val body = JSONObject().put("session", serverSession).put("durationS", pending.segment.durationS)
            val reply = json(http.runCatchingIo { postJson("/api/recordings/finalize", body) })
            return reply.optString("fileId").ifEmpty { throw Outcome.Transient("finalize không trả fileId") }
        } catch (e: Outcome.Redo) {
            // 404 may mean it WAS committed and only the reply was lost.
            lookUp(name)?.let { return it }
            throw e
        }
    }

    /** The recording called [name], if the server has it. Any failure here is transient. */
    private fun lookUp(name: String): String? {
        val reply = json(http.runCatchingIo { get("/api/files") })
        val items = reply.optJSONArray("items") ?: throw Outcome.Transient("danh sách bản ghi không đọc được")
        for (i in 0 until items.length()) {
            val item = items.optJSONObject(i) ?: continue
            if (item.optString("name") == name) return item.optString("id").ifEmpty { null }
        }
        return null
    }

    /** Network failures are transient. */
    private inline fun BackendHttp.runCatchingIo(call: BackendHttp.() -> HttpReply): HttpReply = try {
        call()
    } catch (e: IOException) {
        throw Outcome.Transient(e.message ?: e.javaClass.simpleName)
    }

    /**
     * Maps a reply to its JSON or the failure group. Only the backend's own `{ "error": … }` is trusted
     * to be final; a proxy page (Cloudflare, nginx) with the same status is transient.
     */
    private fun json(reply: HttpReply): JSONObject {
        val message = reply.backendError()
        return when (reply.status) {
            in 200..299 -> reply.json() ?: JSONObject()
            401 -> throw Outcome.Unauthorized()
            413 -> throw Outcome.QuotaFull(message ?: AppText.get(R.string.upload_storage_full))
            400, 403, 404 -> {
                if (message == null) throw Outcome.Transient("HTTP ${reply.status} không từ máy chủ ứng dụng")
                if (reply.status == 403) throw Outcome.Terminal(message)
                throw if (isRedo(message)) Outcome.Redo(message) else Outcome.Terminal(message)
            }
            else -> throw Outcome.Transient(message ?: "HTTP ${reply.status}")
        }
    }

    private inline fun ignoringIo(block: () -> Unit) {
        try {
            block()
        } catch (e: IOException) {
            Log.w(TAG, "manifest write failed", e)
        }
    }

    companion object {
        private const val TAG = "ScribeUpload"
        private const val CHUNK_SIZE = 1 shl 20

        fun name(pending: PendingSegment): String = "${pending.sessionId}#${pending.segment.index}"

        private fun encode(value: String): String = URLEncoder.encode(value, "UTF-8")

        /** Messages of `recordings.rs` that mean "the server lost this upload". */
        fun isRedo(message: String): Boolean {
            val lower = message.lowercase()
            return lower.contains("offset") || lower.contains("session") || lower.contains("phiên") ||
                lower.contains("chưa có dữ liệu") || lower.contains("không tồn tại")
        }
    }
}
