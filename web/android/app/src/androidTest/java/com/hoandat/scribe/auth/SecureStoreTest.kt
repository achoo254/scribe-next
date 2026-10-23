package com.hoandat.scribe.auth

import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** SecureStore against the real AndroidKeyStore (emulator or device: ./gradlew connectedDebugAndroidTest). */
@RunWith(AndroidJUnit4::class)
class SecureStoreTest {
    private val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
    private val store = SecureStore(context)

    @After
    fun tearDown() {
        store.delete("test.token")
        store.delete("test.other")
    }

    @Test
    fun writesReadsAndDeletes() {
        assertTrue(store.write("test.token", "dev-123"))
        assertEquals("dev-123", store.read("test.token"))
        store.delete("test.token")
        assertNull(store.read("test.token"))
    }

    @Test
    fun storesOnlyCiphertext() {
        store.write("test.token", "plain-device-token")
        val raw = context.getSharedPreferences("scribe_secure", Context.MODE_PRIVATE).getString("test.token", null)
        assertFalse(raw.isNullOrEmpty())
        assertFalse(raw.orEmpty().contains("plain-device-token"))
    }

    @Test
    fun aValueMovedToAnotherNameNoLongerDecrypts() {
        store.write("test.token", "dev-123")
        val prefs = context.getSharedPreferences("scribe_secure", Context.MODE_PRIVATE)
        prefs.edit().putString("test.other", prefs.getString("test.token", null)).commit()
        assertNull(store.read("test.other"))
        assertNull(prefs.getString("test.other", null))
    }

    @Test
    fun garbageCountsAsAbsent() {
        context.getSharedPreferences("scribe_secure", Context.MODE_PRIVATE).edit().putString("test.token", "!!not base64!!").commit()
        assertNull(store.read("test.token"))
    }
}
