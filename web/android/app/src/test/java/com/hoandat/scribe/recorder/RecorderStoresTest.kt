package com.hoandat.scribe.recorder

import com.hoandat.scribe.net.HttpReply
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

/** Bookmarks, live transcript file, resampler, m4a brand and realtime token parsing. */
class RecorderStoresTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val user = "0123456789abcdef01234567"

    @Test
    fun `bookmarks stay until acknowledged`() {
        val store = BookmarkStore(temp.root)
        val first = NativeBookmark(NativeBookmark.newId(), "s1", 12.5, 1_000)
        val second = NativeBookmark(NativeBookmark.newId(), "s1", 30.0, 2_000)
        store.append(first, user)
        store.append(second, user)
        assertEquals(listOf(first, second), BookmarkStore(temp.root).pending(user))

        store.remove(setOf(first.id), user)
        assertEquals(listOf(second), store.pending(user))
        store.remove(setOf(second.id), user)
        assertTrue(store.pending(user).isEmpty())
        assertTrue(!File(temp.root, "$user.json").exists())
    }

    @Test
    fun `bookmark ids are 24 lowercase hex digits`() {
        val id = NativeBookmark.newId()
        assertTrue(StoreFiles.isUserKey(id))
    }

    @Test
    fun `transcript skips a line torn by a kill and duplicate ids`() {
        val dir = temp.newFolder("s1")
        val store = TranscriptStore()
        store.append(TranscriptEntry("b", 5.0, "câu hai"), dir)
        store.append(TranscriptEntry("a", 1.0, "câu một"), dir)
        store.append(TranscriptEntry("a", 1.0, "câu một"), dir)
        File(dir, TranscriptStore.FILE_NAME).appendText("{\"id\":\"c\",\"atS\":9,\"te") // killed mid-write

        val read = TranscriptStore().entries(dir)
        assertEquals(listOf("a", "b"), read.map { it.id })

        // The next append after the torn line still reads back.
        val after = TranscriptStore()
        after.append(TranscriptEntry("gap-1", 7.0, "", gapUntilS = 8.0), dir)
        assertEquals(listOf("a", "b", "gap-1"), TranscriptStore().entries(dir).map { it.id })
        assertEquals(8.0, TranscriptStore().entries(dir).last().gapUntilS ?: 0.0, 0.0)
    }

    @Test
    fun `resampler halves 48 kHz to 24 kHz across chunks and keeps the tone`() {
        val resampler = Resampler(48_000, 24_000)
        val tone = ShortArray(48_000) { (sin(2 * PI * 1_000 * it / 48_000) * 10_000).toInt().toShort() }
        val out = ArrayList<Short>()
        var offset = 0
        while (offset < tone.size) {
            val count = minOf(4_800, tone.size - offset)
            resampler.process(tone, offset, count).forEach { out.add(it) }
            offset += count
        }
        assertTrue("got ${out.size} samples", abs(out.size - 24_000) <= 2)
        // 1 kHz for one second: about 2000 zero crossings.
        val crossings = (1 until out.size).count { (out[it - 1] < 0) != (out[it] < 0) }
        assertTrue("crossings $crossings", abs(crossings - 2_000) <= 4)
    }

    @Test
    fun `resampler handles 44_1 kHz`() {
        val resampler = Resampler(44_100, 24_000)
        val total = (0 until 10).sumOf { resampler.process(ShortArray(4_410)).size }
        assertTrue("got $total", abs(total - 24_000) <= 2)
    }

    @Test
    fun `m4a brand is set in place`() {
        val file = File(temp.root, "seg.m4a")
        val header = byteArrayOf(0, 0, 0, 24) + "ftypmp42".toByteArray() + byteArrayOf(0, 0, 0, 0) + "isommp42".toByteArray()
        file.writeBytes(header + ByteArray(100))
        assertEquals("mp42", M4aBrand.apply(file))
        assertEquals("M4A ", M4aBrand.read(file))
        assertEquals(header.size + 100, file.readBytes().size)
        assertEquals("isommp42", String(file.readBytes(), 16, 8))

        val notMp4 = File(temp.root, "x.bin").apply { writeBytes(ByteArray(20)) }
        assertNull(M4aBrand.apply(notMp4))
    }

    private fun tokenJson() = JSONObject()
        .put("token", "t")
        .put("tokenId", "tok-1")
        .put("expiresAt", "2026-10-23T07:19:00+00:00")
        .put("reservedSeconds", 300)
        .put("provider", "elevenlabs")
        .put("url", "wss://api.elevenlabs.io/v1/speech-to-text/realtime")
        .put("sampleRate", 24_000)

    @Test
    fun `realtime token parses the backend reply`() {
        val token = RealtimeToken.parse(tokenJson())
        assertEquals(RealtimeProvider.ELEVENLABS, token?.provider)
        assertEquals(300.0, token?.reservedS ?: 0.0, 0.0)
        assertEquals(1_792_739_940_000L, token?.expiresAtMs)

        // A backend from before ElevenLabs: OpenAI, default socket address.
        val openAi = RealtimeToken.parse(tokenJson().apply { remove("provider"); remove("url") })
        assertEquals(RealtimeProvider.OPENAI, openAi?.provider)
        assertEquals("wss://api.openai.com/v1/realtime", openAi?.url)

        assertNull(RealtimeToken.parse(tokenJson().put("sampleRate", 16_000)))
        assertNull(RealtimeToken.parse(tokenJson().put("url", "https://api.elevenlabs.io")))
        assertNull(RealtimeToken.parse(tokenJson().put("reservedSeconds", 0)))
    }

    @Test
    fun `token refusals are final only with the backend's own error`() {
        fun reply(status: Int, body: String) = HttpReply(status, body.toByteArray(), emptyMap())
        assertTrue(RealtimeApi.tokenResult(reply(200, tokenJson().toString())) is TokenResult.Ok)
        assertEquals(TokenResult.Final("Không có quyền"), RealtimeApi.tokenResult(reply(403, "{\"error\":\"Không có quyền\"}")))
        assertTrue(RealtimeApi.tokenResult(reply(403, "<html>cloudflare</html>")) is TokenResult.Transient)
        assertTrue(RealtimeApi.tokenResult(reply(503, "")) is TokenResult.Transient)
        assertTrue(RealtimeApi.tokenResult(reply(401, "")) is TokenResult.Final)
    }

    @Test
    fun `audio messages match the providers' formats`() {
        val frame = byteArrayOf(1, 2, 3)
        assertEquals(
            "{\"type\":\"input_audio_buffer.append\",\"audio\":\"AQID\"}",
            RealtimeTranscriber.audioMessage(frame, RealtimeProvider.OPENAI, commit = false),
        )
        val eleven = JSONObject(RealtimeTranscriber.audioMessage(ByteArray(0), RealtimeProvider.ELEVENLABS, commit = true))
        assertEquals("input_audio_chunk", eleven.getString("message_type"))
        assertEquals(true, eleven.getBoolean("commit"))
        assertEquals(24_000, eleven.getInt("sample_rate"))
    }

    @Test
    fun `clock formats like the iOS lock screen`() {
        assertEquals("12:34", RecorderEngine.clock(754.9))
        assertEquals("1:02:34", RecorderEngine.clock(3_754.0))
        assertEquals("00:00", RecorderEngine.clock(-3.0))
    }
}
