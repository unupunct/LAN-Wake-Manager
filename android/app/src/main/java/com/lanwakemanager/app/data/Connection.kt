package com.lanwakemanager.app.data

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import com.lanwakemanager.app.api.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Reads and observes the phone's own connectivity (Wi-Fi / VPN / cellular). */
class NetInfo(context: Context) {
    private val cm = context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private val _snap = MutableStateFlow(read())
    val snapshot: StateFlow<NetSnapshot> = _snap.asStateFlow()
    private var listener: (() -> Unit)? = null

    fun read(): NetSnapshot {
        return try {
            val net = cm.activeNetwork
            val caps = net?.let { cm.getNetworkCapabilities(it) }
            // The active network may be the VPN itself; look at all networks to see Wi-Fi underneath.
            var wifi = caps?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
            var eth = caps?.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) == true
            var cell = caps?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true
            var vpn = caps?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
            @Suppress("DEPRECATION")
            for (n in cm.allNetworks) {
                val c = cm.getNetworkCapabilities(n) ?: continue
                if (c.hasTransport(NetworkCapabilities.TRANSPORT_VPN)) vpn = true
                if (c.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) || c.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)) {
                    if (c.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) wifi = true
                    if (c.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)) eth = true
                    if (c.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)) cell = true
                }
            }
            NetSnapshot(connected = net != null, wifi = wifi, ethernet = eth, vpn = vpn, cellular = cell)
        } catch (e: Exception) {
            NetSnapshot()
        }
    }

    /** Calls [onChange] whenever connectivity changes. Call once. */
    fun start(onChange: () -> Unit) {
        listener = onChange
        val cb = object : ConnectivityManager.NetworkCallback() {
            private fun changed() { _snap.value = read(); listener?.invoke() }
            override fun onAvailable(network: Network) = changed()
            override fun onLost(network: Network) = changed()
            override fun onCapabilitiesChanged(network: Network, networkCapabilities: NetworkCapabilities) = changed()
        }
        try {
            cm.registerNetworkCallback(NetworkRequest.Builder().build(), cb)
        } catch (_: Exception) { }
    }
}

enum class ConnStatus { CONNECTING, CONNECTED, DISCONNECTED, UNAVAILABLE, CERT_MISMATCH, UNAUTHORIZED, NO_NETWORK_PATH }

data class ProbeResult(val address: ControllerAddress, val ok: Boolean, val ms: Long?, val error: String?, val certProblem: Boolean = false)

data class ConnState(
    val status: ConnStatus = ConnStatus.DISCONNECTED,
    val active: ControllerAddress? = null,
    val latencyMs: Long? = null,
    val lastSuccess: Long? = null,
    val lastError: String? = null,
    val probes: List<ProbeResult> = emptyList(),
) {
    val connected: Boolean get() = status == ConnStatus.CONNECTED

    /** Chip text per the product spec. */
    fun chipText(): String = when (status) {
        ConnStatus.CONNECTED -> when (active?.kind) {
            AddrKind.TAILSCALE -> "Connected through Tailscale"
            AddrKind.WIREGUARD -> "Connected through WireGuard"
            AddrKind.LAN -> "Connected through Local LAN"
            else -> "Connected (direct address)"
        }
        ConnStatus.CONNECTING -> "Connecting..."
        ConnStatus.UNAVAILABLE -> "Controller unavailable"
        ConnStatus.CERT_MISMATCH -> "Certificate mismatch"
        ConnStatus.UNAUTHORIZED -> "Pairing revoked"
        ConnStatus.NO_NETWORK_PATH, ConnStatus.DISCONNECTED -> "Disconnected"
    }
}

data class ResolveResult(val api: ControllerApi?, val address: ControllerAddress?, val latencyMs: Long?, val probes: List<ProbeResult>, val info: InfoResponse?)

/**
 * Chooses the controller address according to the connection mode. Probes GET /info (pinned TLS) on the
 * candidates in order and takes the first that answers with the matching controller id. Never falls back to
 * an unpinned connection.
 */
object ConnectionResolver {
    suspend fun resolve(
        record: ControllerRecord,
        token: String?,
        mode: ConnMode,
        net: NetSnapshot,
        clientFactory: (String, Long, Long) -> okhttp3.OkHttpClient = { fp, c, r -> PinnedClient.create(fp, c, r) },
        nowMs: () -> Long = { System.currentTimeMillis() },
    ): ResolveResult {
        val candidates = ConnectionPlanner.candidates(record.addresses, mode, net)
        val probes = mutableListOf<ProbeResult>()
        for (addr in candidates) {
            val connect = ConnectionPlanner.connectTimeoutMs(addr.kind)
            val api = ControllerApi(addr.url, record.fingerprint, token, clientFactory(record.fingerprint, connect, 15000))
            val t0 = nowMs()
            try {
                val info = api.withTimeouts(connect, connect + 2500).info()
                val ms = nowMs() - t0
                if (record.id.isNotEmpty() && info.controllerId != record.id) {
                    probes += ProbeResult(addr, false, ms, "Answered, but it is a different controller (${info.controllerId.take(8)}).")
                    continue
                }
                probes += ProbeResult(addr, true, ms, null)
                return ResolveResult(api, addr, ms, probes, info)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                probes += ProbeResult(addr, false, null, e.userMessage(), e.isCertificateProblem())
            }
        }
        return ResolveResult(null, null, null, probes, null)
    }

    /** Maps a failed resolve to a state. */
    fun failureState(r: ResolveResult, mode: ConnMode, net: NetSnapshot, lastSuccess: Long?): ConnState {
        val cert = r.probes.isNotEmpty() && r.probes.all { !it.ok && it.certProblem }
        val anyCert = r.probes.any { it.certProblem }
        val status = when {
            r.probes.isEmpty() -> ConnStatus.NO_NETWORK_PATH
            cert || (anyCert && r.probes.none { it.ok }) -> ConnStatus.CERT_MISMATCH
            !net.connected -> ConnStatus.DISCONNECTED
            else -> ConnStatus.UNAVAILABLE
        }
        val msg = when (status) {
            ConnStatus.NO_NETWORK_PATH -> when (mode) {
                ConnMode.VPN_ONLY -> "No VPN address is known for this controller. Add a Tailscale or WireGuard address."
                ConnMode.LAN_ONLY -> "No LAN address is known for this controller."
                else -> "No address is known for this controller."
            }
            else -> r.probes.lastOrNull { it.error != null }?.error
        }
        return ConnState(status, null, null, lastSuccess, msg, r.probes)
    }
}
