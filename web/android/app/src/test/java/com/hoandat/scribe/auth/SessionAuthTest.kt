package com.hoandat.scribe.auth

import com.hoandat.scribe.AppText
import com.hoandat.scribe.R
import com.hoandat.scribe.net.BackendHttp
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.util.concurrent.TimeUnit

/** In-memory stand-in for SecureStore (AndroidKeyStore is not in JVM tests). */
class MemoryTokenStore : TokenStore {
    val values = HashMap<String, String>()
    override fun read(name: String): String? = values[name]
    override fun write(name: String, value: String): Boolean {
        values[name] = value
        return true
    }
    override fun delete(name: String) {
        values.remove(name)
    }
}

class SessionAuthTest {
    private lateinit var server: MockWebServer
    private lateinit var store: MemoryTokenStore
    private lateinit var auth: SessionAuth

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
        store = MemoryTokenStore()
        auth = SessionAuth(BackendHttp(server.url("/").toString().trimEnd('/')), store, conflictWaitMs = 0)
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    private fun json(status: Int, body: JSONObject) = MockResponse().setResponseCode(status).setBody(body.toString())

    @Test
    fun `exchange keeps the device token and returns the email`() {
        server.enqueue(json(200, JSONObject().put("deviceToken", "dev-1").put("email", "a@hoandat.com")))
        assertEquals("a@hoandat.com", auth.exchange("code", "verifier", "nonce"))
        assertEquals("dev-1", auth.deviceToken())
        val sent = JSONObject(server.takeRequest().body.readUtf8())
        assertEquals("verifier", sent.getString("codeVerifier"))
        assertEquals("nonce", sent.getString("nonce"))
    }

    @Test
    fun `exchange refused by the backend shows its message`() {
        server.enqueue(json(400, JSONObject().put("error", "mã đã dùng")))
        try {
            auth.exchange("code", "verifier", "nonce")
            fail("expected AuthException")
        } catch (e: AuthException) {
            assertEquals("${AppText.get(R.string.session_exchange_failed)}: mã đã dùng", e.message)
        }
        assertNull(auth.deviceToken())
    }

    @Test
    fun `refresh sends the device token`() = runBlocking {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(200, JSONObject().put("ok", true)))
        auth.refreshSession()
        val request = server.takeRequest()
        assertEquals("/api/auth/native/refresh", request.path)
        assertEquals("dev-1", JSONObject(request.body.readUtf8()).getString("deviceToken"))
    }

    @Test
    fun `refresh 409 waits for the other renewal instead of failing`() = runBlocking {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(409, JSONObject().put("error", "đang làm mới")))
        auth.refreshSession()
        assertEquals("dev-1", auth.deviceToken())
    }

    @Test
    fun `refresh 401 forgets the device`() = runBlocking {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(401, JSONObject().put("error", "thiết bị đã bị thu hồi")))
        try {
            auth.refreshSession()
            fail("expected AuthException")
        } catch (e: AuthException) {
            assertTrue(e !is AccountDeletedException)
        }
        assertNull(auth.deviceToken())
    }

    @Test
    fun `refresh 410 means the account was deleted`() = runBlocking {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(410, JSONObject().put("error", "ACCOUNT_DELETED")))
        try {
            auth.refreshSession()
            fail("expected AccountDeletedException")
        } catch (e: AccountDeletedException) {
            // expected
        }
        assertNull(auth.deviceToken())
    }

    @Test
    fun `refresh without a device token fails without a request`() = runBlocking {
        try {
            auth.refreshSession()
            fail("expected AuthException")
        } catch (e: AuthException) {
            // expected
        }
        assertEquals(0, server.requestCount)
    }

    @Test
    fun `concurrent refreshes share one request`() = runBlocking {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(200, JSONObject()).setBodyDelay(300, TimeUnit.MILLISECONDS))
        server.enqueue(json(200, JSONObject()))
        (1..3).map { async { auth.refreshSession() } }.awaitAll()
        assertEquals(1, server.requestCount)
    }

    @Test
    fun `sign-out sends the device token and forgets it even offline`() {
        store.values["deviceToken"] = "dev-1"
        server.enqueue(json(200, JSONObject()))
        auth.signOut()
        assertEquals("dev-1", JSONObject(server.takeRequest().body.readUtf8()).getString("deviceToken"))
        assertNull(auth.deviceToken())

        store.values["deviceToken"] = "dev-2"
        server.shutdown()
        auth.signOut()
        assertNull(auth.deviceToken())
    }
}
