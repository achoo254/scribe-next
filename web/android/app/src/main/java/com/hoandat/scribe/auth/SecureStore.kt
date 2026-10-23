package com.hoandat.scribe.auth

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.ProviderException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Small secrets of the app (the backend device token, the page's allowed Secrets), encrypted with an
 * AES-256-GCM key that never leaves the AndroidKeyStore, then kept as `base64(iv || ciphertext)` in the
 * app's private SharedPreferences `scribe_secure`. No EncryptedSharedPreferences: security-crypto is
 * no longer developed (plan decision, same pattern as Localman's SecureToken plugin).
 *
 * The key needs no user authentication, so uploads can renew the session while the phone is locked
 * (iOS: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly). The value name is bound as associated data,
 * so one entry's ciphertext cannot be swapped into another. A value that no longer decrypts (key lost
 * after a screen-lock reset on some devices, restored data) counts as absent and is removed: the user
 * signs in again; recordings not uploaded stay on the phone.
 */
class SecureStore(context: Context) : TokenStore {
    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    @Synchronized
    override fun read(name: String): String? {
        val stored = prefs.getString(name, null) ?: return null
        return try {
            decrypt(name, stored)
        } catch (e: GeneralSecurityException) {
            dropUnreadable(name, e)
        } catch (e: ProviderException) {
            dropUnreadable(name, e)
        } catch (e: IllegalArgumentException) {
            dropUnreadable(name, e)
        }
    }

    @Synchronized
    override fun write(name: String, value: String): Boolean = try {
        prefs.edit().putString(name, encrypt(name, value)).commit()
    } catch (e: GeneralSecurityException) {
        Log.e(TAG, "encrypt failed", e)
        false
    } catch (e: ProviderException) {
        Log.e(TAG, "encrypt failed", e)
        false
    }

    @Synchronized
    override fun delete(name: String) {
        prefs.edit().remove(name).commit()
    }

    private fun dropUnreadable(name: String, error: Exception): String? {
        Log.w(TAG, "stored value no longer decrypts; removed", error)
        prefs.edit().remove(name).commit()
        return null
    }

    private fun encrypt(name: String, value: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        // The keystore picks a fresh random IV for every encryption.
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(name.toByteArray(Charsets.UTF_8))
        val iv = cipher.iv
        if (iv.size != IV_BYTES) throw GeneralSecurityException("unexpected IV length ${iv.size}")
        val sealed = cipher.doFinal(value.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(iv + sealed, Base64.NO_WRAP)
    }

    private fun decrypt(name: String, stored: String): String {
        val bytes = Base64.decode(stored, Base64.NO_WRAP)
        if (bytes.size <= IV_BYTES) throw GeneralSecurityException("stored value too short")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, bytes, 0, IV_BYTES))
        cipher.updateAAD(name.toByteArray(Charsets.UTF_8))
        return String(cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES), Charsets.UTF_8)
    }

    private fun key(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keyStore.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    companion object {
        private const val TAG = "ScribeSecureStore"
        private const val PREFS = "scribe_secure"
        private const val KEYSTORE = "AndroidKeyStore"
        private const val ALIAS = "scribe.session"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val IV_BYTES = 12
        private const val TAG_BITS = 128

        @Volatile
        private var shared: SecureStore? = null

        fun shared(context: Context): SecureStore = shared ?: synchronized(this) {
            shared ?: SecureStore(context).also { shared = it }
        }
    }
}
