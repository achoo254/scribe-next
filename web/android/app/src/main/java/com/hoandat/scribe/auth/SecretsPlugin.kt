package com.hoandat.scribe.auth

import com.getcapacitor.JSObject
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin

/**
 * Values the WebView may touch in [SecureStore], same contract as SecretsPlugin.swift: `get`, `set`,
 * `remove` of a name in [ALLOWED] only, so the page can never read the sign-in device token.
 * Transcription uses the server's key now; the page only removes the key older iOS builds stored
 * (no Android build ever stored one), but the methods behave as on iOS.
 */
@CapacitorPlugin(name = "Secrets")
class SecretsPlugin : Plugin() {

    private val store: SecureStore get() = SecureStore.shared(context)

    private fun name(call: PluginCall): String? {
        val name = call.getString("name")
        if (name == null || name !in ALLOWED) {
            call.reject("Tên khoá không hợp lệ.", "BAD_NAME")
            return null
        }
        return name
    }

    @PluginMethod
    fun get(call: PluginCall) {
        val name = name(call) ?: return
        execute {
            val value = store.read(key(name))
            call.resolve(if (value != null) JSObject().put("value", value) else JSObject())
        }
    }

    @PluginMethod
    fun set(call: PluginCall) {
        val name = name(call) ?: return
        val value = call.getString("value")
        if (value.isNullOrEmpty()) {
            call.reject("Thiếu giá trị.", "NO_VALUE")
            return
        }
        execute {
            if (store.write(key(name), value)) {
                call.resolve()
            } else {
                call.reject("Không lưu được vào kho khoá của máy.", "KEYSTORE")
            }
        }
    }

    @PluginMethod
    fun remove(call: PluginCall) {
        val name = name(call) ?: return
        execute {
            store.delete(key(name))
            call.resolve()
        }
    }

    companion object {
        private val ALLOWED = setOf("openaiApiKey")

        /** Kept apart from the device token's entry in the same store. */
        private fun key(name: String) = "secret.$name"
    }
}
