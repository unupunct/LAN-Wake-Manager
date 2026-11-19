package com.lanwakemanager.app

import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.*
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import javax.net.ssl.SSLHandshakeException

/** The pinned-TLS client against an in-process HTTPS server with a freshly generated self-signed certificate. */
class PinnedTlsTest {
    private lateinit var server: MockWebServer
    private lateinit var fp: String
    private val infoJson = """{"controller_id":"ctl-1","name":"Test controller","version":"1.0.0","api_version":1,"setup_required":false,"tls_fingerprint":"x","time":1700000000}"""

    @Before fun setUp() {
        // Self-signed, CN does not match the host we connect to: proves hostname checks are skipped and the pin is what counts.
        val held = HeldCertificate.Builder().commonName("some-other-name.invalid").build()
        fp = Fingerprint.sha256Hex(held.certificate.encoded)
        val hc = HandshakeCertificates.Builder().heldCertificate(held).build()
        server = MockWebServer()
        server.useHttps(hc.sslSocketFactory(), false)
        server.start()
    }

    @After fun tearDown() { server.shutdown() }

    private fun base() = "https://127.0.0.1:${server.port}"

    @Test fun acceptsCorrectFingerprintAndIgnoresHostname() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson))
        val info = ControllerApi(base(), fp).info()
        assertEquals("ctl-1", info.controllerId)
        val rec = server.takeRequest()
        assertEquals("/api/v1/info", rec.path)
        assertNull("public endpoint must not carry the token", rec.getHeader("Authorization"))
    }

    @Test fun acceptsUppercaseColonFingerprint() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson))
        val colon = fp.uppercase().chunked(2).joinToString(":")
        assertEquals("ctl-1", ControllerApi(base(), Fingerprint.normalize(colon)!!).info().controllerId)
    }

    @Test fun rejectsWrongFingerprint() {
        server.enqueue(MockResponse().setBody(infoJson))
        val wrong = "0".repeat(64)
        val e = try { runBlocking { ControllerApi(base(), wrong).info() }; null } catch (e: Exception) { e }
        assertNotNull("a wrong pin must fail", e)
        assertTrue("expected TLS failure, got $e", e is SSLHandshakeException || e!!.isCertificateProblem())
        assertTrue(e!!.isCertificateProblem())
        assertEquals(0, server.requestCount) // no HTTP request was ever sent
    }

    @Test fun normalCaValidationWouldRejectButPinWorks() {
        // The same server fails with the default trust store, proving the pin is the only reason it works above.
        val plain = okhttp3.OkHttpClient()
        val req = okhttp3.Request.Builder().url("${base()}/api/v1/info").build()
        assertThrows(javax.net.ssl.SSLException::class.java) { plain.newCall(req).execute() }
    }

    @Test fun sendsBearerTokenAndParsesErrors() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(401).setBody("""{"error":{"code":"unauthorized","message":"bad token"}}"""))
        val api = ControllerApi(base(), fp, token = "secret")
        val e = try { api.me(); null } catch (e: ApiException) { e }
        assertNotNull(e); assertEquals(401, e!!.status); assertEquals("unauthorized", e.code); assertTrue(e.unauthorized)
        assertEquals("Bearer secret", server.takeRequest().getHeader("Authorization"))
    }

    @Test fun pairPostsExpectedBody() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"device_id":"dev1","token":"T","role":"viewer","controller":{"id":"ctl-1","name":"Test controller"}}"""))
        val r = ControllerApi(base(), fp).pair(PairRequest("pt", "Pixel", "android"))
        assertEquals("T", r.token)
        val req = server.takeRequest()
        assertEquals("POST", req.method); assertEquals("/api/v1/pair", req.path)
        assertTrue(req.body.readUtf8().contains("\"pairing_token\":\"pt\""))
        assertNull(req.getHeader("Authorization"))
    }

    @Test fun shutdownBulkNeedsExplicitConfirmFlag() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(202).setBody("""{"operations":[{"id":"o1","kind":"shutdown","state":"pending","started_at":1}]}"""))
        val ops = ControllerApi(base(), fp, "t").bulk(BulkRequest("shutdown", deviceIds = listOf("d1"), confirm = true))
        assertEquals(1, ops.size)
        assertTrue(server.takeRequest().body.readUtf8().contains("\"confirm\":true"))
    }

    @Test fun resolverLanOnlyUsesLanAddressAndChecksControllerId() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson))
        val rec = ControllerRecord("ctl-1", "Test", listOf(ControllerAddress(base(), AddrKind.LAN)), fp, tokenEnc = "x")
        val r = ConnectionResolver.resolve(rec, "t", ConnMode.LAN_ONLY, NetSnapshot(wifi = true))
        assertNotNull(r.api); assertEquals(AddrKind.LAN, r.address!!.kind); assertTrue(r.probes.single().ok)
    }

    @Test fun resolverRejectsDifferentController() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson.replace("ctl-1", "someone-else")))
        val rec = ControllerRecord("ctl-1", "Test", listOf(ControllerAddress(base(), AddrKind.LAN)), fp, tokenEnc = "x")
        val r = ConnectionResolver.resolve(rec, "t", ConnMode.AUTOMATIC, NetSnapshot())
        assertNull(r.api); assertFalse(r.probes.single().ok)
    }

    @Test fun resolverVpnOnlyNeverProbesLan() = runBlocking {
        val rec = ControllerRecord("ctl-1", "Test", listOf(ControllerAddress(base(), AddrKind.LAN)), fp, tokenEnc = "x")
        val r = ConnectionResolver.resolve(rec, "t", ConnMode.VPN_ONLY, NetSnapshot())
        assertNull(r.api); assertTrue(r.probes.isEmpty()); assertEquals(0, server.requestCount)
        assertEquals(ConnStatus.NO_NETWORK_PATH, ConnectionResolver.failureState(r, ConnMode.VPN_ONLY, NetSnapshot(), null).status)
    }

    @Test fun resolverWithWrongPinReportsCertMismatchAndSendsNothing() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson))
        val rec = ControllerRecord("ctl-1", "Test", listOf(ControllerAddress(base(), AddrKind.LAN)), "f".repeat(64), tokenEnc = "x")
        val r = ConnectionResolver.resolve(rec, "t", ConnMode.AUTOMATIC, NetSnapshot())
        assertNull(r.api)
        assertEquals(ConnStatus.CERT_MISMATCH, ConnectionResolver.failureState(r, ConnMode.AUTOMATIC, NetSnapshot(), null).status)
        assertEquals(0, server.requestCount)
    }

    @Test fun resolverFallsThroughUnreachableAddressToWorkingOne() = runBlocking {
        server.enqueue(MockResponse().setBody(infoJson))
        // 127.0.0.1:1 refuses connections immediately; the second address works.
        val rec = ControllerRecord("ctl-1", "Test", listOf(ControllerAddress("https://127.0.0.1:1", AddrKind.LAN), ControllerAddress(base(), AddrKind.OTHER)), fp, tokenEnc = "x")
        val r = ConnectionResolver.resolve(rec, "t", ConnMode.AUTOMATIC, NetSnapshot(wifi = true))
        assertNotNull(r.api); assertEquals(2, r.probes.size); assertFalse(r.probes[0].ok); assertTrue(r.probes[1].ok)
    }

    @Test fun trustManagerOnlyChecksLeaf() {
        val held = HeldCertificate.Builder().build()
        val tm = PinnedTrustManager(Fingerprint.sha256Hex(held.certificate.encoded))
        tm.checkServerTrusted(arrayOf(held.certificate), "RSA")
        val other = HeldCertificate.Builder().build()
        assertThrows(java.security.cert.CertificateException::class.java) { tm.checkServerTrusted(arrayOf(other.certificate), "RSA") }
        assertThrows(java.security.cert.CertificateException::class.java) { tm.checkServerTrusted(emptyArray(), "RSA") }
        assertThrows(java.security.cert.CertificateException::class.java) { tm.checkClientTrusted(arrayOf(held.certificate), "RSA") }
        assertThrows(IllegalArgumentException::class.java) { PinnedTrustManager("zz") }
    }
}
