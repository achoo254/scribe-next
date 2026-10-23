package com.hoandat.scribe.recorder

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class SegmentStoreTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val user = "0123456789abcdef01234567"

    /** "ok:<seconds>" is a readable segment of that length; anything else is a truncated m4a. */
    private val probe: (File) -> Double? = { file ->
        file.readText().takeIf { it.startsWith("ok:") }?.removePrefix("ok:")?.toDoubleOrNull()
    }

    private fun store() = SegmentStore(temp.root, probe)

    private fun segmentFile(dir: File, index: Int, content: String): File =
        File(dir, Segment.fileName(index)).apply { writeText(content) }

    @Test
    fun `segments are pending until uploaded, then their file is freed`() {
        val store = store()
        val dir = store.createSession(user, "s1", 60.0)
        val file = segmentFile(dir, 0, "ok:60")
        store.appendSegment(user, "s1", Segment(0, file.name, 60.0, file.length()))
        assertEquals(listOf(0), store.pending(user).map { it.segment.index })
        assertEquals(1 to 0, store.queueCounts(user))

        store.markUploaded(user, "s1", 0, "file-1")
        assertTrue(store.pending(user).isEmpty())
        assertFalse(file.exists())
        assertEquals("file-1", store.session(user, "s1")?.segments?.single()?.remoteFileId)
    }

    @Test
    fun `upload state and failures round-trip through session json`() {
        val store = store()
        val dir = store.createSession(user, "s1", 60.0)
        val file = segmentFile(dir, 0, "ok:60")
        store.appendSegment(user, "s1", Segment(0, file.name, 60.0, file.length()))
        store.setUploadState(user, "s1", 0, "server-1", true)
        store.markAttempt(user, "s1", 0, "refused")

        val segment = SegmentStore(temp.root, probe).session(user, "s1")?.segments?.single()
        assertNotNull(segment)
        assertEquals("server-1", segment?.serverSession)
        assertEquals(true, segment?.finalizeSent)
        assertEquals(1, segment?.attempts)
        assertEquals(0 to 1, store.queueCounts(user))
        assertTrue(store.pending(user).isEmpty())

        store.clearFailures(user)
        assertEquals(1, store.pending(user).size)
    }

    @Test
    fun `reconcile reports the killed segment and recovers files missing from the manifest`() {
        val store = store()
        val dir = store.createSession(user, "s1", 60.0)
        val first = segmentFile(dir, 0, "ok:60")
        store.appendSegment(user, "s1", Segment(0, first.name, 60.0, first.length()))
        segmentFile(dir, 1, "ok:59.5") // closed but never added to the manifest
        segmentFile(dir, 2, "truncated") // open when the app was killed

        val lost = store.reconcile(user, activeSessionId = null)

        assertEquals(listOf(LostSegment("s1", 2, 60.0)), lost)
        assertFalse(File(dir, Segment.fileName(2)).exists())
        assertEquals(listOf(0, 1), store.pending(user).map { it.segment.index })
        assertEquals(59.5, store.session(user, "s1")?.segments?.get(1)?.durationS ?: 0.0, 0.001)
    }

    @Test
    fun `reconcile rebuilds a broken manifest and skips the session being recorded`() {
        val store = store()
        val broken = store.createSession(user, "broken", 60.0)
        segmentFile(broken, 0, "ok:60")
        File(broken, SegmentStore.MANIFEST).writeText("{not json")
        val active = store.createSession(user, "active", 60.0)
        segmentFile(active, 0, "truncated")

        val lost = store.reconcile(user, activeSessionId = "active")

        assertTrue(lost.isEmpty())
        assertTrue(File(active, Segment.fileName(0)).exists())
        assertEquals(listOf("broken" to 0), store.pending(user).map { it.sessionId to it.segment.index })
    }

    @Test
    fun `reconcile deletes sessions with everything uploaded`() {
        val store = store()
        val dir = store.createSession(user, "done", 60.0)
        val file = segmentFile(dir, 0, "ok:60")
        store.appendSegment(user, "done", Segment(0, file.name, 60.0, file.length()))
        store.markUploaded(user, "done", 0, "file-1")

        store.reconcile(user, null)

        assertFalse(dir.exists())
    }

    @Test
    fun `pending lists older sessions first and removeAll forgets the account`() {
        val store = store()
        for ((id, started) in listOf("late" to 2_000L, "early" to 1_000L)) {
            val dir = store.createSession(user, id, 60.0, nowMs = started)
            val file = segmentFile(dir, 0, "ok:60")
            store.appendSegment(user, id, Segment(0, file.name, 60.0, file.length()))
        }
        assertEquals(listOf("early", "late"), store.pending(user).map { it.sessionId })

        store.removeAll(user)
        assertTrue(store.pending(user).isEmpty())
        assertFalse(File(temp.root, user).exists())
    }

    @Test
    fun `segment names follow the iOS pattern`() {
        assertEquals("seg-007.m4a", Segment.fileName(7))
        assertEquals("seg-1000.m4a", Segment.fileName(1000))
        assertEquals(1000, Segment.indexOf("seg-1000.m4a"))
        assertEquals(null, Segment.indexOf("session.json"))
    }

    @Test
    fun `user keys are 24 lowercase hex characters`() {
        assertTrue(StoreFiles.isUserKey(user))
        assertFalse(StoreFiles.isUserKey("0123456789ABCDEF01234567"))
        assertFalse(StoreFiles.isUserKey("../../../../../../shared"))
        assertFalse(StoreFiles.isUserKey(null))
    }

    @Test
    fun `iso times parse with Z, an offset or milliseconds`() {
        val expected = 1_792_739_940_000L // 2026-10-23T07:19:00Z
        assertEquals(expected, StoreFiles.parseIsoTime("2026-10-23T07:19:00Z"))
        assertEquals(expected, StoreFiles.parseIsoTime("2026-10-23T07:19:00+00:00"))
        assertEquals(expected, StoreFiles.parseIsoTime("2026-10-23T14:19:00+07:00"))
        assertEquals(expected + 250, StoreFiles.parseIsoTime("2026-10-23T07:19:00.250Z"))
        assertEquals("2026-10-23T07:19:00Z", StoreFiles.isoTime(expected))
        assertEquals(null, StoreFiles.parseIsoTime("yesterday"))
    }
}
