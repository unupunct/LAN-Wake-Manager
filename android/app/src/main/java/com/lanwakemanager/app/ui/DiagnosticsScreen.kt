package com.lanwakemanager.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.ConnStatus
import kotlinx.coroutines.launch

data class Finding(val severity: String, val title: String, val text: String, val offerRepair: Boolean = false)

/** Plain-language explanation of why the controller is (not) reachable. Pure function of the UI state. */
object Explanations {
    fun forState(s: AppUiState): List<Finding> {
        val out = mutableListOf<Finding>()
        val mode = s.settings.connectionMode
        val c = s.conn
        if (s.active == null) return listOf(Finding("info", "No controller", "Add a controller by scanning the pairing QR code."))
        if (!s.net.connected) out += Finding("fail", "Phone has no network", "Turn on Wi-Fi or mobile data. Without a network the controller cannot be reached.")
        if (c.status == ConnStatus.UNAUTHORIZED) out += Finding("fail", "Pairing was revoked (401)",
            "The controller no longer accepts this phone's token. An administrator probably removed this device. Pair again with a new QR code.", offerRepair = true)
        if (c.status == ConnStatus.CERT_MISMATCH) out += Finding("fail", "Certificate fingerprint mismatch",
            "The controller presented a certificate that is not the one you paired with. This happens after the controller's certificate was regenerated, " +
                "but it could also mean another device is answering at that address. The app refuses to talk to it. If you trust the change, pair again and compare the fingerprint.", offerRepair = true)
        if (!c.connected && c.status != ConnStatus.UNAUTHORIZED && c.status != ConnStatus.CERT_MISMATCH) {
            if (mode == ConnMode.VPN_ONLY && !s.net.vpn) out += Finding("fail", "VPN app not connected",
                "Connection mode is \"VPN only\" but no VPN is active on this phone. Open the Tailscale or WireGuard app and connect, then retry.")
            if (mode == ConnMode.LAN_ONLY && !s.net.onLocalLink) out += Finding("fail", "Phone not on the LAN in LAN-only mode",
                "Connection mode is \"LAN only\" but the phone is not on Wi-Fi/Ethernet. Join your home network or switch the mode to Automatic.")
            if (mode == ConnMode.AUTOMATIC && !s.net.vpn && !s.net.wifi) out += Finding("warn", "No VPN and not on Wi-Fi",
                "Away from home you need a VPN (Tailscale or WireGuard) to reach the controller. Connect your VPN app.")
            val candidates = ConnectionPlanner.candidates(s.active.addresses, mode, s.net)
            if (candidates.isEmpty()) out += Finding("fail", "No usable address for this mode",
                when (mode) {
                    ConnMode.VPN_ONLY -> "This controller has no Tailscale or WireGuard address saved. Add a WireGuard profile in Settings or re-pair while connected to the VPN."
                    ConnMode.LAN_ONLY -> "This controller has no LAN address saved."
                    else -> "This controller has no address saved."
                })
            else out += Finding("fail", "Controller offline or unreachable",
                "None of the ${candidates.size} address(es) answered. Check that the controller PC is on and the LAN Wake Manager service is running, " +
                    "and that its firewall allows port 8443. Last error: ${c.lastError?.trimEnd('.') ?: "none"}.")
        }
        if (c.connected) {
            val offlineRelays = s.data.locations.filter { it.relayStatus == "offline" }
            offlineRelays.forEach { out += Finding("warn", "Relay offline: ${it.name}", "The relay at ${it.name} is not answering, so computers there cannot be woken until it is back.") }
            if (out.isEmpty()) out += Finding("ok", "Everything reachable", "The controller answers over ${c.chipText().removePrefix("Connected through ")}.")
        }
        return out
    }
}

@Composable
fun DiagnosticsScreen(s: AppUiState, actions: AppActions, onRepair: () -> Unit, modifier: Modifier = Modifier, preloaded: DiagnosticsReport? = null) {
    val scope = rememberCoroutineScope()
    var report by remember { mutableStateOf(preloaded) }
    var reportError by remember { mutableStateOf<String?>(null) }
    var running by remember { mutableStateOf(false) }
    val findings = remember(s) { Explanations.forState(s) }
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { ConnectionBanner(s, actions, onRepair) }
        item {
            LwmCard {
                Text("What is going on", fontWeight = FontWeight.SemiBold)
                findings.forEach { f ->
                    Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.Top) {
                        Pill(if (f.severity == "ok") "OK" else f.severity.uppercase(), statusColor(f.severity)); Spacer(Modifier.width(8.dp))
                        Column {
                            Text(f.title, fontWeight = FontWeight.Medium)
                            Text(f.text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            if (f.offerRepair) TextButton(onClick = onRepair) { Text("Pair again") }
                        }
                    }
                }
            }
        }
        item {
            val c = s.conn
            val ts = s.data.dashboard?.vpn?.tailscale
            LwmCard {
                Text("Connection", fontWeight = FontWeight.SemiBold)
                InfoRow("Controller", s.active?.name)
                InfoRow("Connection mode", when (s.settings.connectionMode) { ConnMode.AUTOMATIC -> "Automatic"; ConnMode.VPN_ONLY -> "VPN only"; ConnMode.LAN_ONLY -> "LAN only" })
                InfoRow("Active method", if (c.connected) c.chipText().removePrefix("Connected through ") else c.chipText())
                InfoRow("Active address", c.active?.url ?: "none")
                InfoRow("Controller reachable", if (c.connected) "yes" else "no")
                InfoRow("API response time", c.latencyMs?.let { "$it ms" } ?: "not measured")
                InfoRow("Last successful connection", c.lastSuccess?.let { Fmt.dateTime(it) + " (" + Fmt.ago(it, s.nowSecs) + ")" } ?: "never")
                InfoRow("VPN active on this phone", if (s.net.vpn) "yes" else "no")
                InfoRow("Phone network", listOfNotNull("Wi-Fi".takeIf { s.net.wifi }, "Ethernet".takeIf { s.net.ethernet }, "Mobile data".takeIf { s.net.cellular }).joinToString().ifBlank { if (s.net.connected) "other" else "offline" })
                InfoRow("Network location", s.selectedLocation?.name ?: "All locations")
                InfoRow("Controller VPN IP", ts?.ip?.let { it + (if (ts.running) "" else " (Tailscale not running)") } ?: "not reported")
                InfoRow("WebSocket (live events)", if (s.wsLive) "connected" else "not connected")
                InfoRow("Data", if (s.isLive) "Live" else "Cached – ${Fmt.ago(s.dataTime, s.nowSecs)}")
                c.lastError?.let { InfoRow("Last error", it) }
            }
        }
        item {
            LwmCard {
                Text("Relays", fontWeight = FontWeight.SemiBold)
                if (s.data.locations.isEmpty()) Text("No locations known.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                s.data.locations.forEach { l ->
                    Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(l.name, Modifier.weight(1f))
                        Pill(relayLabel(l.relayStatus).substringBefore(" ("), when (l.relayStatus) { "online", "builtin" -> LocalStatusColors.current.online; "offline" -> LocalStatusColors.current.offline; else -> LocalStatusColors.current.neutral })
                    }
                }
            }
        }
        item {
            LwmCard {
                Text("Addresses tried", fontWeight = FontWeight.SemiBold)
                val probes = s.conn.probes
                if (probes.isEmpty()) Text("No probe has run yet.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                probes.forEach { p ->
                    Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.Top) {
                        Pill(if (p.ok) "OK" else "FAIL", if (p.ok) LocalStatusColors.current.online else LocalStatusColors.current.offline); Spacer(Modifier.width(8.dp))
                        Column {
                            Text("${p.address.url}  (${kindLabel(p.address.kind)})", style = MaterialTheme.typography.bodyMedium)
                            Text(if (p.ok) "${p.ms} ms" else p.error ?: "failed", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
        item {
            LwmCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Controller self-check", fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                    Button(enabled = s.conn.connected && !running, onClick = {
                        running = true; reportError = null
                        scope.launch { actions.runDiagnostics(s.selectedLocationId).fold({ report = it }, { reportError = it.userMessage() }); running = false }
                    }) { Text(if (running) "Running..." else "Run") }
                }
                if (running) LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 8.dp))
                reportError?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (!s.conn.connected) Text("Requires a live connection.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                report?.checks?.forEach { c ->
                    Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.Top) {
                        Pill(c.status.uppercase(), statusColor(c.status)); Spacer(Modifier.width(8.dp))
                        Column { Text(c.name + (c.durationMs?.let { "  ($it ms)" } ?: ""), fontWeight = FontWeight.Medium); c.detail?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) } }
                    }
                }
            }
        }
        item {
            LwmCard {
                Text("Common problems", fontWeight = FontWeight.SemiBold)
                listOf(
                    "VPN app not connected" to "Tailscale/WireGuard must be connected on the phone to reach the controller away from home.",
                    "Phone not on the LAN" to "LAN-only mode needs Wi-Fi on the same network as the controller.",
                    "Controller offline" to "The controller PC is off, asleep or its service is stopped. Nothing can be woken until it is back (relay schedules still run).",
                    "Certificate fingerprint mismatch" to "The controller's certificate changed. Pair again after checking the fingerprint on the controller.",
                    "Token revoked (401)" to "An administrator removed this phone. Pair again with a new QR code.",
                    "Computer did not wake" to "Magic packet sent but no answer: enable Wake-on-LAN in BIOS and the network adapter, use a wired connection, and see the computer's troubleshooting report.",
                ).forEach { (t, d) -> Text(t, fontWeight = FontWeight.Medium, modifier = Modifier.padding(top = 8.dp)); Text(d, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
        }
    }
}

fun kindLabel(k: AddrKind) = when (k) { AddrKind.LAN -> "LAN"; AddrKind.TAILSCALE -> "Tailscale"; AddrKind.WIREGUARD -> "WireGuard"; AddrKind.OTHER -> "other" }
