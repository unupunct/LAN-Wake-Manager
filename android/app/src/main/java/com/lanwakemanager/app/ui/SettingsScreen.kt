package com.lanwakemanager.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.ControllerRecord
import com.lanwakemanager.app.data.NotifyType

@Composable
private fun RadioRow(label: String, desc: String? = null, selected: Boolean, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected, onClick = onClick)
        Column(Modifier.padding(start = 4.dp)) {
            Text(label)
            if (desc != null) Text(desc, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SettingsScreen(s: AppUiState, actions: AppActions, onAddController: () -> Unit, modifier: Modifier = Modifier, preloadedConn: Connectivity? = null) {
    var removeTarget by remember { mutableStateOf<ControllerRecord?>(null) }
    var renameTarget by remember { mutableStateOf<ControllerRecord?>(null) }
    var conn by remember { mutableStateOf(preloadedConn) }
    var connError by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(s.active?.id, s.conn.connected) {
        if (preloadedConn == null && s.conn.connected) actions.connectivity().fold({ conn = it; connError = null }, { connError = it.userMessage() })
    }
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        // ---- controllers
        item {
            LwmCard {
                Text("Controllers", fontWeight = FontWeight.SemiBold)
                if (s.controllers.isEmpty()) Text("None paired.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                s.controllers.forEach { c ->
                    val isActive = c.id == s.active?.id
                    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(c.name, fontWeight = FontWeight.Medium)
                                Text("Role ${c.role} · fingerprint ${Fingerprint.short(c.fingerprint)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            if (isActive) Pill("Active", LocalStatusColors.current.online)
                        }
                        c.addresses.forEach { a ->
                            Text("${kindLabel(a.kind)}: ${a.url}" + (a.label?.let { " ($it)" } ?: ""), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            if (!isActive) TextButton(onClick = { actions.selectController(c.id) }) { Text("Use") }
                            TextButton(onClick = { renameTarget = c }) { Text("Rename") }
                            TextButton(onClick = onAddController) { Text("Pair again") }
                            TextButton(onClick = { removeTarget = c }) { Text("Remove", color = MaterialTheme.colorScheme.error) }
                        }
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                }
                Button(onClick = onAddController, modifier = Modifier.padding(top = 8.dp)) { Icon(Icons.Filled.Add, null); Spacer(Modifier.width(6.dp)); Text("Add controller") }
            }
        }
        // ---- connection mode
        item {
            LwmCard {
                Text("Connection mode", fontWeight = FontWeight.SemiBold)
                RadioRow("Automatic", "Try the local network first, then Tailscale, then WireGuard.", s.settings.connectionMode == ConnMode.AUTOMATIC) { actions.setConnMode(ConnMode.AUTOMATIC) }
                RadioRow("VPN only", "Only Tailscale or WireGuard addresses. The local network is never used.", s.settings.connectionMode == ConnMode.VPN_ONLY) { actions.setConnMode(ConnMode.VPN_ONLY) }
                RadioRow("LAN only", "Only addresses on the local network.", s.settings.connectionMode == ConnMode.LAN_ONLY) { actions.setConnMode(ConnMode.LAN_ONLY) }
                Text("Every connection is checked against the controller's pinned certificate. The app never falls back to an unverified connection.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
            }
        }
        // ---- tailscale
        item {
            LwmCard {
                Text("Tailscale", fontWeight = FontWeight.SemiBold)
                val t = conn?.tailscale
                if (s.active == null) Text("Pair a controller first.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                else if (t == null) Text(connError ?: if (s.conn.connected) "Loading what the controller reports..." else "The controller reports its Tailscale status only while connected.", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                else {
                    InfoRow("On the controller PC", if (!t.installed) "not installed" else if (t.running) "running" else "installed, not running (${t.backendState ?: "stopped"})")
                    InfoRow("Tailscale IP", t.ip); InfoRow("Name", t.magicdnsName ?: t.hostname); InfoRow("Tailnet", t.tailnet)
                    t.error?.let { InfoRow("Error", it) }
                    conn?.addresses?.takeIf { it.isNotEmpty() }?.let { a -> InfoRow("Controller addresses", a.joinToString { it.address }) }
                }
                Text("How to connect away from home:", fontWeight = FontWeight.Medium, modifier = Modifier.padding(top = 8.dp))
                Text("1. Install Tailscale on the controller PC and on this phone and sign in to the same tailnet.\n2. Open the Tailscale app on the phone and connect.\n3. Add or re-scan the controller's pairing code while Tailscale is connected so its 100.x address is saved.\nThis app never sees your Tailscale credentials; the VPN app handles them.",
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        // ---- wireguard
        item { WireGuardCard(s, actions) }
        // ---- notifications
        item {
            LwmCard {
                Text("Notifications", fontWeight = FontWeight.SemiBold)
                Text("Notifications are created on this phone. They are delivered only while the phone can reach the controller (on the LAN or through your VPN), by the live connection while the app is open and by a check about every 15 minutes in the background. There is no cloud push, so nothing arrives while the phone cannot reach the controller.",
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (!s.notificationsAllowed) {
                    Text("Android notification permission is not granted.", color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 6.dp))
                    Button(onClick = actions::requestNotificationPermission) { Text("Allow notifications") }
                }
                listOf(
                    NotifyType.OPERATIONS to "Operation results (wake, shutdown, restart)",
                    NotifyType.DEVICES to "Computer unavailable",
                    NotifyType.RELAY to "Relay offline / online",
                    NotifyType.CONTROLLER to "Controller unavailable",
                    NotifyType.SCHEDULE to "Schedule approaching",
                ).forEach { (k, label) ->
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp)) {
                        Text(label, Modifier.weight(1f))
                        Switch(s.settings.notifyEnabled(k), { actions.setNotify(k, it) })
                    }
                }
            }
        }
        // ---- security
        item {
            LwmCard {
                Text("Security", fontWeight = FontWeight.SemiBold)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Require unlock")
                        Text(if (s.biometricAvailable) "Fingerprint, face or device PIN when opening the app." else "Set up a screen lock or biometrics on the phone to use this.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Switch(s.settings.appLock, { actions.setAppLock(it) }, enabled = s.biometricAvailable || s.settings.appLock)
                }
                Text("Device tokens are stored encrypted with a key in the Android Keystore.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
            }
        }
        // ---- appearance / refresh
        item {
            LwmCard {
                Text("Appearance", fontWeight = FontWeight.SemiBold)
                RadioRow("Dark (graphite)", null, s.settings.theme == "dark") { actions.setTheme("dark") }
                RadioRow("Light", null, s.settings.theme == "light") { actions.setTheme("light") }
                RadioRow("Follow system", null, s.settings.theme == "system") { actions.setTheme("system") }
                Text("Refresh interval", fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 10.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(15, 30, 60, 120, 300).forEach { v ->
                        FilterChip(s.settings.refreshSecs == v, { actions.setRefreshSecs(v) }, label = { Text(if (v < 60) "$v s" else "${v / 60} min") })
                    }
                }
            }
        }
        item {
            LwmCard {
                Text("About", fontWeight = FontWeight.SemiBold)
                InfoRow("Version", s.versionName)
                InfoRow("Package", "com.lanwakemanager.app")
                Text("LAN Wake Manager wakes, shuts down and restarts your computers through your own controller. No cloud service and no Google services are used.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
            }
        }
    }
    removeTarget?.let { c ->
        ConfirmDialog("Remove ${c.name}?", "The stored token and cached data for this controller are deleted from this phone. The device stays listed on the controller until an administrator revokes it.", "Remove",
            onConfirm = { actions.removeController(c.id); removeTarget = null }, onDismiss = { removeTarget = null })
    }
    renameTarget?.let { c ->
        var name by remember(c.id) { mutableStateOf(c.name) }
        AlertDialog(onDismissRequest = { renameTarget = null }, title = { Text("Rename controller") },
            text = { OutlinedTextField(name, { name = it }, singleLine = true) },
            confirmButton = { TextButton(onClick = { actions.renameController(c.id, name.trim()); renameTarget = null }) { Text("Save") } },
            dismissButton = { TextButton(onClick = { renameTarget = null }) { Text("Cancel") } })
    }
}

@Composable
private fun WireGuardCard(s: AppUiState, actions: AppActions) {
    var label by remember { mutableStateOf("") }
    var host by remember { mutableStateOf("") }
    var port by remember { mutableStateOf("8443") }
    var err by remember { mutableStateOf<String?>(null) }
    val c = s.active
    LwmCard {
        Text("WireGuard profiles", fontWeight = FontWeight.SemiBold)
        Text("Tell the app which controller address belongs to your WireGuard tunnel. The tunnel itself is managed by the WireGuard app; no keys are entered or stored here.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (c == null) { Text("Pair a controller first.", modifier = Modifier.padding(top = 6.dp)); return@LwmCard }
        val profiles = c.addresses.filter { it.kind == AddrKind.WIREGUARD }
        if (profiles.isEmpty()) Text("No WireGuard profile for ${c.name}.", modifier = Modifier.padding(top = 6.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
        profiles.forEach { a ->
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp)) {
                Column(Modifier.weight(1f)) { Text(a.label ?: "WireGuard", fontWeight = FontWeight.Medium); Text(a.url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                IconButton(onClick = { actions.removeAddress(c.id, a.url) }) { Icon(Icons.Filled.Delete, "Remove profile") }
            }
        }
        OutlinedTextField(label, { label = it }, label = { Text("Label (e.g. Office tunnel)") }, singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 6.dp)) {
            OutlinedTextField(host, { host = it }, label = { Text("Controller VPN address") }, singleLine = true, modifier = Modifier.weight(1f))
            OutlinedTextField(port, { port = it.filter(Char::isDigit).take(5) }, label = { Text("Port") }, singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), modifier = Modifier.width(90.dp))
        }
        err?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Button(modifier = Modifier.padding(top = 8.dp), onClick = {
            val p = port.toIntOrNull()
            if (host.isBlank() || Addresses.hostOf(host.trim().let { if (it.contains("://")) it else "https://$it" }) == null) err = "Enter the controller's address inside the tunnel, e.g. 10.8.0.1."
            else if (p == null || p !in 1..65535) err = "Port must be 1-65535."
            else { err = null; actions.addWireGuard(c.id, label.trim(), host.trim(), p); label = ""; host = "" }
        }) { Text("Add WireGuard profile") }
    }
}
