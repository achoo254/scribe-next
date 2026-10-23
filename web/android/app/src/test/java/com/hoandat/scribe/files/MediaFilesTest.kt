package com.hoandat.scribe.files

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import java.net.URI

class MediaFilesTest {
    private val origin = "https://scribe.hoandat.com"

    @Test
    fun `accepts only https api URLs of the backend host`() {
        assertNotNull(MediaFiles.backendUrl("https://scribe.hoandat.com/api/files/abc/media", origin))
        assertNotNull(MediaFiles.backendUrl("https://SCRIBE.hoandat.com/api/files/abc/media?download=1", origin))
        assertNull(MediaFiles.backendUrl("http://scribe.hoandat.com/api/files/abc/media", origin))
        assertNull(MediaFiles.backendUrl("https://evil.example/api/files/abc", origin))
        assertNull(MediaFiles.backendUrl("https://scribe.hoandat.com.evil.example/api/files", origin))
        assertNull(MediaFiles.backendUrl("https://user@scribe.hoandat.com/api/files", origin))
        assertNull(MediaFiles.backendUrl("https://scribe.hoandat.com:8443/api/files", origin))
        assertNull(MediaFiles.backendUrl("https://scribe.hoandat.com/files/abc", origin))
        assertNull(MediaFiles.backendUrl("https://scribe.hoandat.com/api/../admin", origin))
        assertNull(MediaFiles.backendUrl("not a url", origin))
        assertNull(MediaFiles.backendUrl(null, origin))
    }

    @Test
    fun `cache key is the path with slashes replaced`() {
        assertEquals("_api_files_abc_media", MediaFiles.cacheKey(URI("https://scribe.hoandat.com/api/files/abc/media")))
    }

    @Test
    fun `file name comes from Content-Disposition, UTF-8 form first`() {
        assertEquals("Họp.m4a", MediaFiles.fileName("attachment; filename=\"Hop.m4a\"; filename*=UTF-8''H%E1%BB%8Dp.m4a"))
        assertEquals("Hop.m4a", MediaFiles.fileName("attachment; filename=\"Hop.m4a\""))
        assertEquals("passwd", MediaFiles.fileName("attachment; filename=\"../../etc/passwd\""))
        assertNull(MediaFiles.fileName("attachment"))
        assertNull(MediaFiles.fileName(null))
    }

    @Test
    fun `extension follows the name, else the content type`() {
        assertEquals("m4a", MediaFiles.extension("Họp.m4a", "video/mp4"))
        assertEquals("mp4", MediaFiles.extension("/api/files/abc/media", "video/mp4; codecs=avc1"))
        assertEquals("webm", MediaFiles.extension(null, "audio/webm"))
        assertEquals("", MediaFiles.extension(null, "application/octet-stream"))
    }
}
