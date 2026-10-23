package com.hoandat.scribe.auth

import com.hoandat.scribe.net.Base64Text
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.net.URI

class AuthRequestTest {
    private val config = NativeAuthConfig(
        authorizationEndpoint = "https://auth.hoandat.com/auth",
        clientId = "scribe-mobile",
        redirectUri = "com.hoandat.scribe:/auth/callback",
        scope = "openid email offline_access",
    )
    private val request = AuthRequest(config, verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", state = "st4te", nonce = "n0nce")

    @Test
    fun `challenge is S256 of the verifier (RFC 7636 appendix B)`() {
        assertEquals("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", request.challenge)
    }

    @Test
    fun `base64 encoders match RFC 4648 vectors`() {
        val cases = mapOf("" to "", "f" to "Zg==", "fo" to "Zm8=", "foo" to "Zm9v", "foob" to "Zm9vYg==", "fooba" to "Zm9vYmE=", "foobar" to "Zm9vYmFy")
        for ((input, expected) in cases) {
            assertEquals(expected, Base64Text.standard(input.toByteArray()))
            assertEquals(expected.trimEnd('='), Base64Text.urlSafe(input.toByteArray()))
        }
        assertEquals("-_8", Base64Text.urlSafe(byteArrayOf(0xfb.toByte(), 0xff.toByte())))
    }

    @Test
    fun `random values are url-safe and of the requested strength`() {
        val verifier = Pkce.randomUrlSafe(32)
        assertEquals(43, verifier.length)
        assertTrue(verifier.all { it.isLetterOrDigit() || it == '-' || it == '_' })
    }

    @Test
    fun `authorization URL carries every parameter, spaces as %20`() {
        val url = URI(request.authorizationUrl())
        assertEquals("auth.hoandat.com", url.host)
        val query = AuthRequest.parseQuery(url.rawQuery)
        assertEquals("scribe-mobile", query["client_id"])
        assertEquals("com.hoandat.scribe:/auth/callback", query["redirect_uri"])
        assertEquals("code", query["response_type"])
        assertEquals("openid email offline_access", query["scope"])
        assertEquals(request.challenge, query["code_challenge"])
        assertEquals("S256", query["code_challenge_method"])
        assertEquals("st4te", query["state"])
        assertEquals("n0nce", query["nonce"])
        assertTrue(url.rawQuery.contains("scope=openid%20email%20offline_access"))
    }

    @Test
    fun `authorization URL keeps an endpoint's own query`() {
        val withQuery = AuthRequest(config.copy(authorizationEndpoint = "https://auth.hoandat.com/auth?prompt=login"), "v", "s", "n")
        assertTrue(withQuery.authorizationUrl().startsWith("https://auth.hoandat.com/auth?prompt=login&client_id="))
    }

    @Test
    fun `callback with the right state gives the code`() {
        assertEquals(CallbackResult.Code("abc"), request.parseCallback("com.hoandat.scribe:/auth/callback?code=abc&state=st4te"))
    }

    @Test
    fun `callback with another state, no code or another redirect is refused`() {
        val mismatch = CallbackResult.Failure("Phản hồi đăng nhập không khớp, thử lại.")
        assertEquals(mismatch, request.parseCallback("com.hoandat.scribe:/auth/callback?code=abc&state=other"))
        assertEquals(mismatch, request.parseCallback("com.hoandat.scribe:/auth/callback?state=st4te"))
        assertEquals(mismatch, request.parseCallback("com.evil.app:/auth/callback?code=abc&state=st4te"))
    }

    @Test
    fun `callback errors are explained`() {
        assertEquals(
            CallbackResult.Failure("Bạn đã từ chối đăng nhập."),
            request.parseCallback("com.hoandat.scribe:/auth/callback?error=access_denied&state=st4te"),
        )
        assertEquals(
            CallbackResult.Failure("Đăng nhập thất bại (server_error)."),
            request.parseCallback("com.hoandat.scribe:/auth/callback?error=server_error"),
        )
    }

    @Test
    fun `native config needs every field`() {
        val json = JSONObject()
            .put("authorizationEndpoint", "https://auth.hoandat.com/auth")
            .put("clientId", "scribe-mobile")
            .put("redirectUri", "com.hoandat.scribe:/auth/callback")
            .put("scope", "openid")
        assertEquals("com.hoandat.scribe", NativeAuthConfig.parse(json)?.redirectScheme)
        json.remove("scope")
        assertNull(NativeAuthConfig.parse(json))
    }
}
