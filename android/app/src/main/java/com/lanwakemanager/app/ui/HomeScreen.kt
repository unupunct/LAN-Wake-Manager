package com.lanwakemanager.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.Operation

@Composable
fun OperationRow(op: Operation, now: Long, modifier: Modifier = Modifier, showSteps: Boolean = false) {
    val color = statusColor(op.state)
    Column(modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("${Fmt.kindLabel(op.kind)} – ${op.deviceName ?: "computer"}", fontWeight = FontWeight.Medium)
                Text(
                    "${Fmt.ago(op.startedAt, now)} · ${op.requestedBy ?: "unknown"}" + (if (op.source == "schedule") " (schedule)" else ""),
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Pill(op.state.replaceFirstChar { it.uppercase() }, color)
        }
        Text(Fmt.opOutcome(op), style = MaterialTheme.typography.bodySmall, color = color, modifier = Modifier.padding(top = 2.dp))
        if (!op.message.isNullOrBlank()) Text(op.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (showSteps && op.steps.isNotEmpty()) {
            Column(Modifier.padding(top = 4.dp, start = 6.dp)) {
                op.steps.forEach { st ->
                    Text("${Fmt.time(st.t)}  ${st.text}", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
fun NoControllerCard(onAdd: () -> Unit) {
    LwmCard {
        Text("No controller paired yet", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(6.dp))
        Text(
            "Open the LAN Wake Manager dashboard on your always-on PC, go to Mobile access, create a pairing code and scan it here. " +
                "Your phone must reach the controller through your VPN app (Tailscale or WireGuard) or the home network.",
            style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(10.dp))
        Button(onClick = onAdd) { Icon(Icons.Filled.QrCodeScanner, null); Spacer(Modifier.width(8.dp)); Text("Add controller") }
    }
}

@Composable
fun HomeScreen(s: AppUiState, actions: AppActions, onNavigate: (String) -> Unit, onAddController: () -> Unit, modifier: Modifier = Modifier) {
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (s.active == null) {
            item { NoControllerCard(onAddController) }
            return@LazyColumn
        }
        item {
            Column {
                Text(s.active.name, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
                Text("Role: ${s.role}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(8.dp))
                ConnectionBanner(s, actions, onRepair = onAddController)
            }
        }
        item {
            LwmCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Filled.Place, null, tint = MaterialTheme.colorScheme.secondary)
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text("Network location", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(s.selectedLocation?.name ?: "All locations", fontWeight = FontWeight.SemiBold)
                        s.selectedLocation?.let {
                            Text("Relay: ${relayLabel(it.relayStatus)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    TextButton(onClick = { onNavigate("locations") }) { Text("Change") }
                }
            }
        }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                StatTile("Computers", s.visibleDevices.size.toString(), MaterialTheme.colorScheme.onSurface, Modifier.weight(1f))
                StatTile("Online", s.onlineCount.toString(), LocalStatusColors.current.online, Modifier.weight(1f))
                StatTile("Offline", s.offlineCount.toString(), LocalStatusColors.current.offline, Modifier.weight(1f))
            }
            if (s.otherCount > 0) Text("${s.otherCount} waking, shutting down or unknown", style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp, start = 4.dp))
        }
        val running = s.tracked.values.filter { !it.terminal }
        if (running.isNotEmpty()) item {
            LwmCard(container = MaterialTheme.colorScheme.secondaryContainer) {
                Text("In progress", fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSecondaryContainer)
                running.forEach { OperationRow(it, s.nowSecs) }
            }
        }
        item {
            LwmCard {
                Text("Upcoming schedules", fontWeight = FontWeight.SemiBold)
                val up = s.data.dashboard?.upcomingSchedules.orEmpty()
                if (up.isEmpty()) Text("Nothing scheduled.", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                up.take(4).forEach { u ->
                    Row(Modifier.fillMaxWidth().clickable { onNavigate("schedules") }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Filled.Schedule, null, tint = MaterialTheme.colorScheme.primary)
                        Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            Text(u.name, fontWeight = FontWeight.Medium)
                            Text("${Fmt.kindLabel(u.action)} · ${Fmt.until(u.nextRun, s.nowSecs)} (${Fmt.dateTime(u.nextRun)})", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
        item {
            LwmCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Recent activity", fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                    TextButton(onClick = { onNavigate("history") }) { Text("All") }
                }
                val ops = s.data.dashboard?.recentOperations?.takeIf { it.isNotEmpty() } ?: s.data.operations
                if (ops.isEmpty()) Text("No operations yet.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                ops.take(5).forEachIndexed { i, op -> if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant); OperationRow(op, s.nowSecs) }
            }
        }
        val failed = s.data.dashboard?.failedOperations24h ?: 0
        if (failed > 0) item {
            Text("$failed failed operation(s) in the last 24 hours.", style = MaterialTheme.typography.bodySmall, color = LocalStatusColors.current.warn)
        }
    }
}

fun relayLabel(s: String) = when (s) {
    "builtin" -> "built-in (controller sends packets)"
    "online" -> "online"
    "offline" -> "offline"
    "unpaired" -> "not paired"
    else -> s
}
