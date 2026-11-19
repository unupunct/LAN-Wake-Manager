package com.lanwakemanager.app.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanwakemanager.app.api.Device
import com.lanwakemanager.app.api.Operation

private data class PendingConfirm(val title: String, val text: String, val label: String, val run: () -> Unit)

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun ComputersScreen(
    s: AppUiState, actions: AppActions, onDetails: (String) -> Unit, onAddController: () -> Unit,
    modifier: Modifier = Modifier, initialSelection: Set<String> = emptySet(),
) {
    var selected by remember { mutableStateOf(initialSelection) }
    var confirm by remember { mutableStateOf<PendingConfirm?>(null) }
    val canAct = s.canOperate && s.conn.connected
    val devices = s.visibleDevices

    Column(modifier.fillMaxSize()) {
        if (selected.isNotEmpty()) {
            Surface(color = MaterialTheme.colorScheme.secondaryContainer) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("${selected.size} selected", fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onSecondaryContainer)
                        TextButton(onClick = { selected = devices.map { it.id }.toSet() }) { Text("All") }
                        TextButton(onClick = { selected = emptySet() }) { Text("Clear") }
                    }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val names = devices.filter { it.id in selected }.joinToString { it.name }
                        Button(enabled = canAct, onClick = { actions.bulk("wake", selected.toList(), false); selected = emptySet() }) { Text("Wake selected") }
                        OutlinedButton(enabled = canAct, onClick = {
                            val ids = selected.toList()
                            confirm = PendingConfirm("Shut down ${ids.size} computer(s)?", "These computers will be shut down:\n$names", "Shut down") { actions.bulk("shutdown", ids, true); selected = emptySet() }
                        }) { Text("Shutdown") }
                        OutlinedButton(enabled = canAct, onClick = {
                            val ids = selected.toList()
                            confirm = PendingConfirm("Restart ${ids.size} computer(s)?", "These computers will be restarted:\n$names", "Restart") { actions.bulk("restart", ids, true); selected = emptySet() }
                        }) { Text("Restart") }
                    }
                }
            }
        }
        PullToRefreshBox(isRefreshing = s.refreshing, onRefresh = actions::refresh, modifier = Modifier.weight(1f)) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (s.active == null) { item { NoControllerCard(onAddController) }; return@LazyColumn }
                item {
                    Column {
                        ConnectionBanner(s, actions, onRepair = onAddController)
                        if (!s.canOperate) Text("Your role (${s.role}) is read-only: power actions are hidden.", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                        else if (selected.isEmpty()) Text("Long-press a computer to select several.", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                        s.selectedLocation?.let { Text("Location: ${it.name}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.secondary) }
                    }
                }
                if (devices.isEmpty()) item { EmptyState("No computers in this view.") }
                items(devices, key = { it.id }) { d ->
                    DeviceCard(
                        d, s, canAct, d.id in selected, selectionMode = selected.isNotEmpty(),
                        onToggle = { selected = if (d.id in selected) selected - d.id else selected + d.id },
                        onDetails = { onDetails(d.id) },
                        onWake = { actions.wake(d.id) },
                        onPower = { kind ->
                            val verb = if (kind == "shutdown") "Shut down" else "Restart"
                            confirm = PendingConfirm("$verb ${d.name}?", "${d.name} will be ${if (kind == "shutdown") "shut down" else "restarted"} now. Unsaved work on that computer may be lost.", verb) { actions.power(d.id, kind) }
                        },
                        onDismissOp = actions::dismissOperation,
                    )
                }
            }
        }
    }
    confirm?.let { c -> ConfirmDialog(c.title, c.text, c.label, onConfirm = { c.run(); confirm = null }, onDismiss = { confirm = null }) }
    if (s.bulkIds.isNotEmpty()) BulkResultDialog(s, actions)
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun DeviceCard(
    d: Device, s: AppUiState, canAct: Boolean, selected: Boolean, selectionMode: Boolean,
    onToggle: () -> Unit, onDetails: () -> Unit, onWake: () -> Unit, onPower: (String) -> Unit, onDismissOp: (String) -> Unit,
) {
    val op = s.operationFor(d.id)
    val container = if (selected) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surface
    Surface(
        Modifier.fillMaxWidth().combinedClickable(onClick = { if (selectionMode) onToggle() else onDetails() }, onLongClick = { if (s.canOperate) onToggle() }),
        shape = MaterialTheme.shapes.large, color = container,
        border = androidx.compose.foundation.BorderStroke(1.dp, if (selected) MaterialTheme.colorScheme.secondary else MaterialTheme.colorScheme.outlineVariant),
    ) {
        Column(Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (selectionMode) { Checkbox(checked = selected, onCheckedChange = { onToggle() }); Spacer(Modifier.width(2.dp)) }
                Icon(deviceIcon(d.deviceType), null, tint = statusColor(d.status))
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(d.name, fontWeight = FontWeight.SemiBold, style = MaterialTheme.typography.titleMedium)
                    Text("${d.ip ?: "no IP"} · ${Fmt.osLabel(d.os)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Pill(Fmt.statusLabel(d.status), statusColor(d.status))
            }
            Spacer(Modifier.height(6.dp))
            Text("Last seen ${Fmt.ago(d.lastSeen, s.nowSecs)}" + (if (!s.isLive) " (cached)" else ""), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Pill(Fmt.wolLabel(d), when (d.wolCapable) { "yes" -> LocalStatusColors.current.online; "no" -> LocalStatusColors.current.offline; else -> LocalStatusColors.current.neutral }, icon = Icons.Filled.Bolt)
                Pill(Fmt.agentLabel(d), when (d.agent?.state) { "online" -> LocalStatusColors.current.online; "offline" -> LocalStatusColors.current.warn; else -> LocalStatusColors.current.neutral }, icon = Icons.Filled.SettingsEthernet)
            }
            if (op != null) OpProgress(op, s.nowSecs, onDismissOp)
            if (!selectionMode) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(2.dp), modifier = Modifier.padding(top = 6.dp)) {
                    if (s.canOperate) {
                        FilledTonalButton(onClick = onWake, enabled = canAct && d.status != "online" && d.status != "waking", contentPadding = PaddingValues(horizontal = 12.dp)) {
                            Icon(Icons.Filled.PowerSettingsNew, null, Modifier.size(18.dp)); Spacer(Modifier.width(4.dp)); Text("Wake", fontSize = 13.sp)
                        }
                        TextButton(onClick = { onPower("shutdown") }, enabled = canAct && d.status == "online") { Text("Shutdown", fontSize = 13.sp) }
                        TextButton(onClick = { onPower("restart") }, enabled = canAct && d.status == "online") { Text("Restart", fontSize = 13.sp) }
                    }
                    TextButton(onClick = onDetails) { Text("Details", fontSize = 13.sp) }
                }
            }
        }
    }
}

@Composable
fun OpProgress(op: Operation, now: Long, onDismiss: (String) -> Unit) {
    val color = statusColor(op.state)
    Surface(Modifier.fillMaxWidth().padding(top = 8.dp), shape = MaterialTheme.shapes.medium, color = color.copy(alpha = 0.12f)) {
        Row(Modifier.padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
            if (!op.terminal) { CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = color); Spacer(Modifier.width(10.dp)) }
            Column(Modifier.weight(1f)) {
                Text("${Fmt.kindLabel(op.kind)}: ${Fmt.opOutcome(op)}", color = color, fontWeight = FontWeight.Medium, style = MaterialTheme.typography.bodyMedium)
                if (!op.message.isNullOrBlank()) Text(op.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (op.terminal) IconButton(onClick = { onDismiss(op.id) }, Modifier.size(28.dp)) { Icon(Icons.Filled.Close, "Dismiss", Modifier.size(16.dp)) }
        }
    }
}

@Composable
fun BulkResultDialog(s: AppUiState, actions: AppActions) {
    val ops = s.bulkIds.map { id -> s.tracked[id] ?: s.data.operations.firstOrNull { it.id == id } }.filterNotNull()
    val done = ops.isNotEmpty() && ops.all { it.terminal }
    AlertDialog(
        onDismissRequest = actions::dismissBulk,
        title = { Text(if (done) "Results" else "Working...") },
        text = {
            LazyColumn(Modifier.heightIn(max = 380.dp)) {
                if (ops.isEmpty()) item { Text("The controller returned no operations.") }
                items(ops, key = { it.id }) { op ->
                    OperationRow(op, s.nowSecs)
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                }
            }
        },
        confirmButton = { TextButton(onClick = actions::dismissBulk) { Text(if (done) "Close" else "Hide") } },
    )
}

// ------------------------------------------------------------------ details

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DeviceDetailScreen(s: AppUiState, deviceId: String, actions: AppActions, modifier: Modifier = Modifier, preloaded: DeviceExtras? = null) {
    val base = s.data.devices.firstOrNull { it.id == deviceId }
    var extras by remember { mutableStateOf(preloaded ?: DeviceExtras()) }
    var loading by remember { mutableStateOf(preloaded == null) }
    LaunchedEffect(deviceId, s.conn.connected) {
        if (preloaded == null) { loading = true; extras = actions.deviceExtras(deviceId); loading = false }
    }
    val d = extras.device ?: base
    var confirm by remember { mutableStateOf<PendingConfirm?>(null) }
    if (d == null) { EmptyState("This computer is not in the cached data."); return }
    val canAct = s.canOperate && s.conn.connected
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            LwmCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(deviceIcon(d.deviceType), null, tint = statusColor(d.status), modifier = Modifier.size(32.dp))
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(d.name, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
                        Text("${d.ip ?: "no IP"} · ${Fmt.osLabel(d.os)}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Pill(Fmt.statusLabel(d.status), statusColor(d.status))
                }
                Spacer(Modifier.height(6.dp))
                FreshnessChip(s)
                s.operationFor(d.id)?.let { OpProgress(it, s.nowSecs, actions::dismissOperation) }
                if (s.canOperate) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 8.dp)) {
                    Button(onClick = { actions.wake(d.id) }, enabled = canAct && d.status != "online" && d.status != "waking") { Text("Wake") }
                    OutlinedButton(onClick = { confirm = PendingConfirm("Shut down ${d.name}?", "${d.name} will be shut down now.", "Shut down") { actions.power(d.id, "shutdown") } }, enabled = canAct && d.status == "online") { Text("Shutdown") }
                    OutlinedButton(onClick = { confirm = PendingConfirm("Restart ${d.name}?", "${d.name} will be restarted now.", "Restart") { actions.power(d.id, "restart") } }, enabled = canAct && d.status == "online") { Text("Restart") }
                }
            }
        }
        item {
            LwmCard {
                Text("Overview", fontWeight = FontWeight.SemiBold)
                InfoRow("Hostname", d.hostname); InfoRow("IP address", d.ip); InfoRow("MAC address", d.mac)
                InfoRow("Device type", d.deviceType); InfoRow("Location", s.location(d.locationId)?.name)
                InfoRow("Groups", d.groupIds.mapNotNull { s.group(it)?.name }.joinToString().ifBlank { null })
                InfoRow("Power method", d.powerMethod)
                InfoRow("Wake-on-LAN", Fmt.wolLabel(d))
                InfoRow("Agent", Fmt.agentLabel(d) + (d.agent?.version?.let { " v$it" } ?: ""))
                InfoRow("Agent heartbeat", d.agent?.lastHeartbeat?.let { Fmt.ago(it, s.nowSecs) })
                InfoRow("Last seen", Fmt.ago(d.lastSeen, s.nowSecs))
                InfoRow("Status checked", d.statusCheckedAt?.let { Fmt.ago(it, s.nowSecs) })
                if (!d.notes.isNullOrBlank()) InfoRow("Notes", d.notes)
            }
        }
        item { SysInfoCard(d, s.nowSecs) }
        item {
            LwmCard {
                Text("Wake-on-LAN troubleshooting", fontWeight = FontWeight.SemiBold)
                val t = extras.troubleshoot
                when {
                    loading -> LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 8.dp))
                    t == null -> Text(extras.error ?: "Not available while disconnected.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    else -> {
                        t.checks.forEach { c ->
                            Row(Modifier.padding(vertical = 3.dp), verticalAlignment = Alignment.Top) {
                                Pill(c.status.uppercase(), statusColor(c.status)); Spacer(Modifier.width(8.dp))
                                Column { Text(c.name, fontWeight = FontWeight.Medium); if (!c.detail.isNullOrBlank()) Text(c.detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                            }
                        }
                        if (t.advice.isNotEmpty()) {
                            Spacer(Modifier.height(6.dp)); Text("Advice", fontWeight = FontWeight.Medium)
                            t.advice.forEach { Text("• $it", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        }
                    }
                }
            }
        }
        item {
            LwmCard {
                Text("History", fontWeight = FontWeight.SemiBold)
                when {
                    loading -> LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 8.dp))
                    extras.operations.isEmpty() -> Text("No operations loaded (offline or none yet).", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    else -> extras.operations.forEachIndexed { i, op -> if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant); OperationRow(op, s.nowSecs, showSteps = true) }
                }
            }
        }
    }
    confirm?.let { c -> ConfirmDialog(c.title, c.text, c.label, onConfirm = { c.run(); confirm = null }, onDismiss = { confirm = null }) }
}

@Composable
private fun SysInfoCard(d: Device, now: Long) {
    val si = d.sysinfo
    LwmCard {
        Text("System information", fontWeight = FontWeight.SemiBold)
        if (si == null) {
            Text("not reported – this computer has no agent reporting system data.", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
            return@LwmCard
        }
        InfoRow("Operating system", listOfNotNull(si.osName, si.osVersion).joinToString(" ").ifBlank { null })
        InfoRow("Kernel / arch", listOfNotNull(si.kernel, si.arch).joinToString(" / ").ifBlank { null })
        InfoRow("CPU", si.cpuModel?.let { it + (si.cpuCores?.let { c -> " ($c cores)" } ?: "") })
        InfoRow("CPU usage", si.cpuUsagePercent?.let { String.format(java.util.Locale.US, "%.1f %%", it) })
        InfoRow("Memory", if (si.memTotalBytes != null) "${Fmt.bytes(si.memUsedBytes)} of ${Fmt.bytes(si.memTotalBytes)}" else null)
        InfoRow("Uptime", si.uptimeSecs?.let { Fmt.duration(it) })
        if (si.disks.isEmpty()) InfoRow("Disks", null)
        si.disks.forEach { InfoRow("Disk ${it.mount}", if (it.totalBytes != null) "${Fmt.bytes(it.freeBytes)} free of ${Fmt.bytes(it.totalBytes)}" else null) }
        if (si.temperatures.isEmpty()) InfoRow("Temperatures", null)
        si.temperatures.forEach { InfoRow(it.label.ifBlank { "Temperature" }, it.celsius?.let { c -> String.format(java.util.Locale.US, "%.0f °C", c) }) }
        si.networks.forEach { InfoRow("Network ${it.name}", it.ips.joinToString().ifBlank { null }) }
        Text("Reported ${Fmt.ago(si.reportedAt, now)}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
    }
}
