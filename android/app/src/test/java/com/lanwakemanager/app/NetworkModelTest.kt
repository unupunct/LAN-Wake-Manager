package com.lanwakemanager.app

import com.lanwakemanager.app.api.*
import org.junit.Assert.*
import org.junit.Test

class QrParserTest {
    private val fp = "a".repeat(64)
    private fun json(exp: Long = 2_000_000_000, fp: String = this.fp, addrs: String = """["https://100.64.1.2:8443","https://192.168.1.5:8443"]""", extra: String = "") =
        """{"v":1,"id":"ctl-1","name":"Home controller","addrs":$addrs,"fp":"$fp","token":"tok123","exp":$exp$extra}"""

    @Test fun parsesValidPayload() {
        val p = QrParser.parse(json(), nowSecs = 1_000)
        assertEquals("ctl-1", p.id); assertEquals("Home controller", p.name); assertEquals("tok123", p.token)
        assertEquals(2, p.addrs.size); assertEquals(fp, p.fp)
    }

    @Test fun ignoresUnknownFields() { assertEquals("ctl-1", QrParser.parse(json(extra = ""","future":{"x":1}"""), 1_000).id) }

    @Test fun normalizesFingerprintCase() {
        val p = QrParser.parse(json(fp = "AB:CD:" + "ef".repeat(30)), 1_000)
        assertEquals(64, p.fp.length); assertEquals(p.fp, p.fp.lowercase())
    }

    @Test fun rejectsExpired() {
        val e = assertThrows(QrParseException::class.java) { QrParser.parse(json(exp = 1_000), nowSecs = 2_000) }
        assertTrue(e.message!!.contains("expired"))
    }

    @Test fun rejectsGarbage() { assertThrows(QrParseException::class.java) { QrParser.parse("hello world", 0) } }
    @Test fun rejectsBadFingerprint() { assertThrows(QrParseException::class.java) { QrParser.parse(json(fp = "xyz"), 1_000) } }
    @Test fun rejectsNoAddresses() { assertThrows(QrParseException::class.java) { QrParser.parse(json(addrs = "[]"), 1_000) } }
    @Test fun rejectsWrongVersion() { assertThrows(QrParseException::class.java) { QrParser.parse(json().replace("\"v\":1", "\"v\":9"), 1_000) } }

    @Test fun upgradesHttpAndAddsScheme() {
        val p = QrParser.parse(json(addrs = """["http://10.0.0.5:8443/","100.64.1.2:8443"]"""), 1_000)
        assertEquals(listOf("https://10.0.0.5:8443", "https://100.64.1.2:8443"), p.addrs)
    }

    @Test fun shortFingerprint() { assertEquals("AA:AA:AA:AA:..:AA:AA:AA:AA", Fingerprint.short(fp)) }
}

class AddressClassificationTest {
    private fun k(u: String, wg: Set<String> = emptySet()) = Addresses.classify(u, wg)

    @Test fun tailscaleCgnat() {
        assertEquals(AddrKind.TAILSCALE, k("https://100.64.0.1:8443"))
        assertEquals(AddrKind.TAILSCALE, k("https://100.127.255.254:8443"))
        assertEquals(AddrKind.OTHER, k("https://100.63.0.1:8443"))
        assertEquals(AddrKind.OTHER, k("https://100.128.0.1:8443"))
    }
    @Test fun tailscaleMagicDns() { assertEquals(AddrKind.TAILSCALE, k("https://pc.tail1234.ts.net:8443")) }
    @Test fun rfc1918() {
        assertEquals(AddrKind.LAN, k("https://10.1.2.3:8443"))
        assertEquals(AddrKind.LAN, k("https://172.16.0.1:8443")); assertEquals(AddrKind.LAN, k("https://172.31.255.1:8443"))
        assertEquals(AddrKind.OTHER, k("https://172.32.0.1:8443")); assertEquals(AddrKind.OTHER, k("https://172.15.0.1:8443"))
        assertEquals(AddrKind.LAN, k("https://192.168.1.5:8443"))
        assertEquals(AddrKind.LAN, k("https://169.254.10.10:8443"))
    }
    @Test fun publicIsOther() { assertEquals(AddrKind.OTHER, k("https://8.8.8.8:8443")); assertEquals(AddrKind.OTHER, k("https://example.com:8443")) }
    @Test fun userLabelledWireguardWins() {
        assertEquals(AddrKind.WIREGUARD, k("https://10.8.0.1:8443", setOf("10.8.0.1")))
        assertEquals(AddrKind.LAN, k("https://10.8.0.1:8443"))
    }
    @Test fun ipv6UlaAndLinkLocalAreLan() { assertEquals(AddrKind.LAN, k("https://[fd00::1]:8443")); assertEquals(AddrKind.LAN, k("https://[fe80::1]:8443")) }
    @Test fun hostParsing() { assertEquals("100.64.1.2", Addresses.hostOf("100.64.1.2:8443")) }
}

class ConnectionPlannerTest {
    private val lan = ControllerAddress("https://192.168.1.5:8443", AddrKind.LAN)
    private val ts = ControllerAddress("https://100.64.1.2:8443", AddrKind.TAILSCALE)
    private val wg = ControllerAddress("https://10.8.0.1:8443", AddrKind.WIREGUARD)
    private val other = ControllerAddress("https://203.0.113.9:8443", AddrKind.OTHER)
    private val all = listOf(ts, other, wg, lan)

    @Test fun automaticOrderLanFirst() {
        assertEquals(listOf(lan, ts, wg, other), ConnectionPlanner.candidates(all, ConnMode.AUTOMATIC, NetSnapshot(wifi = true)))
    }
    @Test fun automaticOnMobileDataTriesVpnBeforeLan() {
        val c = ConnectionPlanner.candidates(all, ConnMode.AUTOMATIC, NetSnapshot(wifi = false, cellular = true, vpn = true))
        assertEquals(listOf(ts, wg, lan, other), c)
    }
    @Test fun vpnOnlyNeverIncludesLanOrOther() {
        val c = ConnectionPlanner.candidates(all, ConnMode.VPN_ONLY)
        assertEquals(listOf(ts, wg), c); assertFalse(c.contains(lan))
    }
    @Test fun lanOnlyOnlyLan() { assertEquals(listOf(lan), ConnectionPlanner.candidates(all, ConnMode.LAN_ONLY)) }
    @Test fun modesWithoutMatchingAddressAreEmpty() {
        assertTrue(ConnectionPlanner.candidates(listOf(lan), ConnMode.VPN_ONLY).isEmpty())
        assertTrue(ConnectionPlanner.candidates(listOf(ts), ConnMode.LAN_ONLY).isEmpty())
    }
    @Test fun lanProbeIsShort() { assertTrue(ConnectionPlanner.connectTimeoutMs(AddrKind.LAN) < ConnectionPlanner.connectTimeoutMs(AddrKind.TAILSCALE)) }
}
