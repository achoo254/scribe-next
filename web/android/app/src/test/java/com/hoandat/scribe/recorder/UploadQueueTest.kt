package com.hoandat.scribe.recorder

import com.hoandat.scribe.auth.AccountDeletedException
import com.hoandat.scribe.net.BackendHttp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.Collections
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

class UploadQueueTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val user = "0123456789abcdef01234567"
    private lateinit var server: MockWebServer
    private lateinit var store: SegmentStore
    private lateinit var scope: CoroutineScope
    private val events = LinkedBlockingQueue<UploadEvent>()
    private val paths: MutableList<String> = Collections.synchronizedList(ArrayList())

    /** Per-path replies; the default backend accepts everything. */
    private var unauthorizedUntilRefresh = false
    private var refreshFails: Exception? = null
    private val refreshes = AtomicInteger()
    private var existingFiles = JSONArray()

    @Before
    fun setUp() {
        server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val path = request.path ?: ""
                paths.add(path.substringBefore('?'))
                if (unauthorizedUntilRefresh && refreshes.get() == 0) return MockResponse().setResponseCode(401)
                return when {
                    path.startsWith("/api/recordings/init") -> ok(JSONObject().put("sessionId", "srv-1"))
                    path.startsWith("/api/recordings/chunk") -> {
                        val offset = path.substringAfter("offset=").toLong()
                        ok(JSONObject().put("written", offset + request.bodySize))
                    }
                    path.startsWith("/api/recordings/finalize") -> ok(JSONObject().put("fileId", "file-1"))
                    path.startsWith("/api/files") -> ok(JSONObject().put("items", existingFiles))
                    else -> MockResponse().setResponseCode(404)
                }
            }
        }
        server.start()
        store = SegmentStore(temp.root) { 60.0 }
        scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    }

    @After
    fun tearDown() {
        scope.cancel()
        server.shutdown()
    }

    private fun ok(json: JSONObject) = MockResponse().setResponseCode(200).setBody(json.toString())

    private fun queue() = UploadQueue(
        store,
        BackendHttp(server.url("/").toString().trimEnd('/')),
        refreshSession = {
            refreshes.incrementAndGet()
            refreshFails?.let { throw it }
        },
        scope = scope,
        onEvent = { events.add(it) },
    )

    private fun addSegment(sessionId: String, index: Int, bytes: Int = 3_000_000): File {
        val dir = store.sessionDir(user, sessionId).takeIf { it.exists() } ?: store.createSession(user, sessionId, 60.0)
        val file = File(dir, Segment.fileName(index)).apply { writeBytes(ByteArray(bytes) { (it % 251).toByte() }) }
        store.appendSegment(user, sessionId, Segment(index, file.name, 60.0, file.length()))
        return file
    }

    private inline fun <reified T : UploadEvent> awaitEvent(): T {
        val deadline = System.currentTimeMillis() + 10_000
        while (System.currentTimeMillis() < deadline) {
            val event = events.poll(deadline - System.currentTimeMillis(), TimeUnit.MILLISECONDS) ?: break
            if (event is T) return event
        }
        throw AssertionError("no ${T::class.simpleName} event")
    }

    @Test
    fun `uploads a segment in 1 MiB chunks, then frees it`() {
        val file = addSegment("s1", 0)
        queue().start(user)

        assertEquals(UploadEvent.Uploaded("s1#0"), awaitEvent<UploadEvent.Uploaded>())
        assertEquals(
            listOf("/api/recordings/init", "/api/recordings/chunk", "/api/recordings/chunk", "/api/recordings/chunk", "/api/recordings/finalize"),
            paths.toList(),
        )
        assertFalse(file.exists())
        assertEquals("file-1", store.session(user, "s1")?.segments?.single()?.remoteFileId)
        val init = JSONObject(server.takeRequest().body.readUtf8())
        assertEquals("s1#0", init.getString("name"))
    }

    @Test
    fun `a 401 renews the session once and retries`() {
        addSegment("s1", 0, bytes = 10)
        unauthorizedUntilRefresh = true
        queue().start(user)

        awaitEvent<UploadEvent.Uploaded>()
        assertEquals(1, refreshes.get())
    }

    @Test
    fun `a refused renewal waits for sign-in`() {
        addSegment("s1", 0, bytes = 10)
        unauthorizedUntilRefresh = true
        refreshFails = IllegalStateException("revoked")
        queue().start(user)

        awaitEvent<UploadEvent.NeedsSignIn>()
        assertEquals(1, store.pending(user).size)
    }

    @Test
    fun `a deleted account removes its segments from the phone`() {
        val file = addSegment("s1", 0, bytes = 10)
        unauthorizedUntilRefresh = true
        refreshFails = AccountDeletedException()
        queue().start(user)

        assertEquals(UploadEvent.AccountDeleted(user), awaitEvent<UploadEvent.AccountDeleted>())
        assertFalse(file.exists())
        assertTrue(store.pending(user).isEmpty())
    }

    @Test
    fun `after a lost finalize reply the name is looked up instead of uploading twice`() {
        addSegment("s1", 0, bytes = 10)
        store.setUploadState(user, "s1", 0, "srv-old", finalizeSent = true)
        existingFiles = JSONArray().put(JSONObject().put("name", "s1#0").put("id", "file-existing"))
        queue().start(user)

        awaitEvent<UploadEvent.Uploaded>()
        assertEquals(listOf("/api/files"), paths.toList())
        assertEquals("file-existing", store.session(user, "s1")?.segments?.single()?.remoteFileId)
    }

    @Test
    fun `stop never sends the previous user's segments`() {
        addSegment("s1", 0, bytes = 10)
        val queue = queue()
        queue.stop()
        queue.kick(user)
        Thread.sleep(300)
        assertTrue(paths.isEmpty())
    }

    @Test
    fun `server messages about lost uploads mean redo`() {
        assertTrue(UploadQueue.isRedo("offset không khớp"))
        assertTrue(UploadQueue.isRedo("Phiên tải lên không tồn tại"))
        assertFalse(UploadQueue.isRedo("bản ghi không phải audio/video hợp lệ"))
    }
}
