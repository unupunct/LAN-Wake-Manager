package com.lanwakemanager.app.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import java.net.URI
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.HostnameVerifier
import javax.net.ssl.SSLContext
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

// ---------------------------------------------------------------- QR payload

/** The pairing QR code content (docs/API.md "Mobile access"). */
@Serializable
data class QrPayload(
    val v: Int = 1,
    val id: String = "",
    val name: String = "",
    val addrs: List<String> = emptyList(),
    val fp: String = "",
    val token: String = "",
    val exp: Long = 0,
)

class QrParseException(message: String) : Exception(message)

object QrParser {
    /** Parses and validates a QR payload. [nowSecs] is injectable for tests. */
    fun parse(raw: String, nowSecs: Long = System.currentTimeMillis() / 1000): QrPayload {
        val p = try {
            LwmJson.decodeFromString(QrPayload.serializer(), raw.trim())
        } catch (e: Exception) {
            throw QrParseException("This is not a LAN Wake Manager pairing code.")
        }
        if (p.v != 1) throw QrParseException("Unsupported pairing code version ${p.v}.")
        if (p.id.isBlank() || p.token.isBlank()) throw QrParseException("The pairing code is incomplete.")
        val fp = Fingerprint.normalize(p.fp) ?: throw QrParseException("The pairing code has an invalid certificate fingerprint.")
        val addrs = p.addrs.map { Addresses.normalizeUrl(it) }.filter { it.isNotEmpty() }
        if (addrs.isEmpty()) throw QrParseException("The pairing code contains no controller address.")
        if (p.exp in 1 until nowSecs) throw QrParseException("This pairing code has expired. Create a new one on the controller.")
        return p.copy(fp = fp, addrs = addrs)
    }
}

object Fingerprint {
    private val HEX = Regex("^[0-9a-f]{64}$")

    /** Accepts upper/lower case, optional colons or spaces; returns 64 lowercase hex or null. */
    fun normalize(input: String): String? {
        val s = input.lowercase().filter { it != ':' && !it.isWhitespace() }
        return if (HEX.matches(s)) s else null
    }

    fun sha256Hex(der: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(der).joinToString("") { "%02x".format(it) }

    /** Short form for display, e.g. "3F:A9:1C:..:7B" (first and last 4 bytes). */
    fun short(fp: String): String {
        val f = fp.uppercase()
        if (f.length < 64) return f
        val pairs = f.chunked(2)
        return pairs.take(4).joinToString(":") + ":..:" + pairs.takeLast(4).joinToString(":")
    }
}

// ---------------------------------------------------------------- address model

@Serializable
enum class AddrKind {
    @SerialName("lan") LAN,
    @SerialName("tailscale") TAILSCALE,
    @SerialName("wireguard") WIREGUARD,
    @SerialName("other") OTHER;

    val isVpn: Boolean get() = this == TAILSCALE || this == WIREGUARD
}

@Serializable
enum class ConnMode {
    @SerialName("automatic") AUTOMATIC,
    @SerialName("vpn_only") VPN_ONLY,
    @SerialName("lan_only") LAN_ONLY,
}

@Serializable
data class ControllerAddress(
    val url: String,
    val kind: AddrKind = AddrKind.OTHER,
    /** User label (WireGuard profiles). */
    val label: String? = null,
)

object Addresses {
    /** Ensures a https:// scheme and no trailing slash. */
    fun normalizeUrl(input: String): String {
        var s = input.trim().trimEnd('/')
        if (s.isEmpty()) return ""
        if (!s.contains("://")) s = "https://$s"
        if (s.startsWith("http://", ignoreCase = true)) s = "https://" + s.substring(7)
        return s
    }

    fun hostOf(url: String): String? = try {
        URI(normalizeUrl(url)).host?.removePrefix("[")?.removeSuffix("]")?.lowercase()
    } catch (e: Exception) { null }

    private fun ipv4(host: String): IntArray? {
        val parts = host.split('.')
        if (parts.size != 4) return null
        val n = parts.map { it.toIntOrNull() ?: return null }
        return if (n.all { it in 0..255 }) n.toIntArray() else null
    }

    /**
     * Classifies a controller address.
     * 100.64.0.0/10 or *.ts.net -> Tailscale; RFC1918 / link-local / ULA -> LAN; hosts listed in
     * [wireguardHosts] -> WireGuard; everything else -> other.
     */
    fun classify(url: String, wireguardHosts: Set<String> = emptySet()): AddrKind {
        val host = hostOf(url) ?: return AddrKind.OTHER
        if (host in wireguardHosts) return AddrKind.WIREGUARD
        if (host.endsWith(".ts.net")) return AddrKind.TAILSCALE
        val v4 = ipv4(host)
        if (v4 != null) {
            val (a, b) = v4[0] to v4[1]
            return when {
                a == 100 && b in 64..127 -> AddrKind.TAILSCALE
                a == 10 -> AddrKind.LAN
                a == 172 && b in 16..31 -> AddrKind.LAN
                a == 192 && b == 168 -> AddrKind.LAN
                a == 169 && b == 254 -> AddrKind.LAN
                a == 127 -> AddrKind.LAN
                else -> AddrKind.OTHER
            }
        }
        if (host.contains(':')) {
            if (host.startsWith("fe80") || host.startsWith("fc") || host.startsWith("fd") || host == "::1") return AddrKind.LAN
        }
        return AddrKind.OTHER
    }

    fun withKind(url: String, wireguardHosts: Set<String> = emptySet()) =
        ControllerAddress(normalizeUrl(url), classify(url, wireguardHosts))
}

/** What the phone currently knows about its own connectivity. */
data class NetSnapshot(
    val connected: Boolean = true,
    val wifi: Boolean = true,
    val ethernet: Boolean = false,
    val vpn: Boolean = false,
    val cellular: Boolean = false,
) {
    val onLocalLink: Boolean get() = wifi || ethernet
}

object ConnectionPlanner {
    /**
     * Candidate addresses, in probing order, for the given mode.
     *  - LAN_ONLY: only LAN addresses.
     *  - VPN_ONLY: only Tailscale/WireGuard addresses, never LAN.
     *  - AUTOMATIC: LAN first (short timeout; last if the phone is not on a local link), then Tailscale,
     *    then WireGuard, then any other address.
     */
    fun candidates(addrs: List<ControllerAddress>, mode: ConnMode, net: NetSnapshot = NetSnapshot()): List<ControllerAddress> {
        val lan = addrs.filter { it.kind == AddrKind.LAN }
        val ts = addrs.filter { it.kind == AddrKind.TAILSCALE }
        val wg = addrs.filter { it.kind == AddrKind.WIREGUARD }
        val other = addrs.filter { it.kind == AddrKind.OTHER }
        val out = when (mode) {
            ConnMode.LAN_ONLY -> lan
            ConnMode.VPN_ONLY -> ts + wg
            ConnMode.AUTOMATIC -> if (net.onLocalLink || !net.connected) lan + ts + wg + other else ts + wg + lan + other
        }
        return out.distinctBy { it.url }
    }

    fun connectTimeoutMs(kind: AddrKind): Long = if (kind == AddrKind.LAN) 1500 else 4000
}

// ---------------------------------------------------------------- pinned TLS

/**
 * Accepts only a server certificate whose SHA-256 (DER) equals the pinned fingerprint. No CA validation.
 * Only the leaf certificate (chain[0]) is considered.
 */
class PinnedTrustManager(pinnedHex: String) : X509TrustManager {
    private val pinned = Fingerprint.normalize(pinnedHex)
        ?: throw IllegalArgumentException("Invalid fingerprint")

    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        throw CertificateException("Client certificates are not accepted")
    }

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        if (chain.isNullOrEmpty()) throw CertificateException("Empty certificate chain")
        val actual = Fingerprint.sha256Hex(chain[0].encoded)
        if (!MessageDigest.isEqual(actual.toByteArray(), pinned.toByteArray())) {
            throw CertificateException("Certificate fingerprint mismatch (pinned ${pinned.take(8)}.., got ${actual.take(8)}..)")
        }
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
}

object PinnedClient {
    private val acceptAnyHost = HostnameVerifier { _, _ -> true }

    /** OkHttp client that trusts exactly one leaf certificate. */
    fun create(fingerprintHex: String, connectTimeoutMs: Long = 4000, readTimeoutMs: Long = 15000): OkHttpClient {
        val tm = PinnedTrustManager(fingerprintHex)
        val ctx = SSLContext.getInstance("TLS")
        ctx.init(null, arrayOf(tm), null)
        return OkHttpClient.Builder()
            .sslSocketFactory(ctx.socketFactory, tm)
            .hostnameVerifier(acceptAnyHost)
            .connectTimeout(connectTimeoutMs, TimeUnit.MILLISECONDS)
            .readTimeout(readTimeoutMs, TimeUnit.MILLISECONDS)
            .writeTimeout(10, TimeUnit.SECONDS)
            .pingInterval(25, TimeUnit.SECONDS)
            .retryOnConnectionFailure(false)
            .build()
    }
}
