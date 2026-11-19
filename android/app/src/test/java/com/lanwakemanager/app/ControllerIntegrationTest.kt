package com.lanwakemanager.app

import com.lanwakemanager.app.api.*
import kotlinx.coroutines.runBlocking
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.Assert.*

/**
 * Optional integration test against a REAL controller. Skipped unless these environment variables are set:
 *   LWM_TEST_ADDR           e.g. https://127.0.0.1:8443
 *   LWM_TEST_FP             controller certificate SHA-256 (hex)
 *   LWM_TEST_PAIRING_TOKEN  fresh single-use pairing token (role chosen when creating it)
 * Run: gradlew testDebugUnitTest --tests "*ControllerIntegrationTest*" (set the variables in the same shell).
 */
class ControllerIntegrationTest {
    private val addr = System.getenv("LWM_TEST_ADDR")
    private val fp = System.getenv("LWM_TEST_FP")
    private val pairing = System.getenv("LWM_TEST_PAIRING_TOKEN")

    @Test fun pairAndReadEverything() = runBlocking {
        assumeTrue("LWM_TEST_ADDR / LWM_TEST_FP / LWM_TEST_PAIRING_TOKEN not set", !addr.isNullOrBlank() && !fp.isNullOrBlank() && !pairing.isNullOrBlank())
        val nfp = Fingerprint.normalize(fp!!)!!
        val api = ControllerApi(addr!!, nfp)
        val info = api.info()
        assertTrue(info.controllerId.isNotBlank())
        println("controller: ${info.name} (${info.controllerId}) api v${info.apiVersion}")

        val paired = api.pair(PairRequest(pairing!!, "JVM integration test", "android"))
        assertTrue(paired.token.isNotBlank())
        api.token = paired.token
        println("paired as role=${paired.role}")

        assertEquals(paired.role, api.me().role)
        val dash = api.dashboard(); println("dashboard totals: ${dash.totals}")
        val locations = api.locations(); println("locations: ${locations.size}")
        val devices = api.devices(); println("devices: ${devices.size}")
        api.groups(); api.schedules(); api.operations(10)
        api.connectivity(); api.diagnostics(null)
        api.notifications(0, 5)
        devices.firstOrNull()?.let { d -> api.device(d.id); api.troubleshoot(d.id); println("first device: ${d.name} ${d.status}") }

        // The token must stop working after revocation (done by the caller on the controller side); here only check 401 for a bad token.
        val bad = ControllerApi(addr, nfp, "definitely-not-a-token")
        val e = try { bad.me(); null } catch (e: ApiException) { e }
        assertEquals(401, e?.status)
    }
}
