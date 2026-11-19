package com.lanwakemanager.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.GroupInput

// ------------------------------------------------------------------ locations

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LocationsScreen(s: AppUiState, actions: AppActions, onAddController: () -> Unit, modifier: Modifier = Modifier) {
    PullToRefreshBox(isRefreshing = s.refreshing, onRefresh = actions::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (s.active == null) { item { NoControllerCard(onAddController) }; return@LazyColumn }
            item { ConnectionBanner(s, actions, onRepair = onAddController) }
            item {
                val sel = s.selectedLocationId == null
                LwmCard(container = if (sel) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surface) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Filled.Public, null, tint = MaterialTheme.colorScheme.primary)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text("All locations", fontWeight = FontWeight.SemiBold)
                            Text("${s.data.devices.size} computers, ${s.data.devices.count { it.status == "online" }} online", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (sel) Pill("Selected", LocalStatusColors.current.online) else OutlinedButton(onClick = { actions.selectLocation(null) }) { Text("Select") }
                    }
                }
            }
            if (s.data.locations.isEmpty()) item { EmptyState("No locations known yet.") }
            items(s.data.locations, key = { it.id }) { l ->
                val sel = s.selectedLocationId == l.id
                // Prefer live counts from the device list when we have it.
                val devs = s.data.devices.filter { it.locationId == l.id }
                val total = if (devs.isNotEmpty()) devs.size else l.deviceCount
                val online = if (devs.isNotEmpty()) devs.count { it.status == "online" } else l.onlineCount
                LwmCard(container = if (sel) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surface) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        androidx.compose.foundation.layout.Box(Modifier.size(14.dp).background(parseColor(l.color, MaterialTheme.colorScheme.primary), CircleShape))
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(l.name, fontWeight = FontWeight.SemiBold, style = MaterialTheme.typography.titleMedium)
                            Text("$online of $total online", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (sel) Pill("Selected", LocalStatusColors.current.online) else OutlinedButton(onClick = { actions.selectLocation(l.id) }) { Text("Select") }
                    }
                    Spacer(Modifier.height(8.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        val relayColor = when (l.relayStatus) { "online", "builtin" -> LocalStatusColors.current.online; "offline" -> LocalStatusColors.current.offline; else -> LocalStatusColors.current.neutral }
                        Pill("Relay ${l.relayStatus}", relayColor, icon = Icons.Filled.Router)
                        l.vpn?.takeIf { it != "none" }?.let { Pill(it.replaceFirstChar { c -> c.uppercase() }, LocalStatusColors.current.info, icon = Icons.Filled.VpnLock) }
                    }
                    Spacer(Modifier.height(6.dp))
                    InfoRow("Relay mode", if (l.relayMode == "remote") "remote relay" + (l.relayName?.let { " ($it)" } ?: "") else "built-in")
                    if (l.relayMode == "remote") {
                        InfoRow("Relay last seen", l.relayLastSeen?.let { Fmt.ago(it, s.nowSecs) })
                        InfoRow("Relay version", l.relayVersion)
                    }
                    InfoRow("Wake settings", l.wakeAttempts?.let { "$it attempts, timeout ${l.wakeTimeoutSecs ?: "?"} s" })
                }
            }
        }
    }
}

// ------------------------------------------------------------------ groups

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun GroupsScreen(s: AppUiState, actions: AppActions, onEdit: (String?) -> Unit, onAddController: () -> Unit, modifier: Modifier = Modifier) {
    var confirm by remember { mutableStateOf<Triple<String, String, () -> Unit>?>(null) }
    val canAct = s.canOperate && s.conn.connected
    Box(modifier.fillMaxSize()) {
        PullToRefreshBox(isRefreshing = s.refreshing, onRefresh = actions::refresh, modifier = Modifier.fillMaxSize()) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp, 16.dp, 16.dp, 90.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (s.active == null) { item { NoControllerCard(onAddController) }; return@LazyColumn }
                item { ConnectionBanner(s, actions, onRepair = onAddController) }
                if (s.data.groups.isEmpty()) item { EmptyState("No groups yet.") }
                items(s.data.groups, key = { it.id }) { g ->
                    val members = s.data.devices.filter { g.id in it.groupIds }
                    val total = if (s.data.devices.isNotEmpty()) members.size else g.deviceCount
                    val online = if (s.data.devices.isNotEmpty()) members.count { it.status == "online" } else g.onlineCount
                    val color = parseColor(g.color, MaterialTheme.colorScheme.primary)
                    LwmCard {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.size(40.dp).background(color.copy(alpha = 0.18f), CircleShape), contentAlignment = Alignment.Center) { Icon(groupIcon(g.icon), null, tint = color) }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(g.name, fontWeight = FontWeight.SemiBold, style = MaterialTheme.typography.titleMedium)
                                Text("$online of $total online", style = MaterialTheme.typography.bodySmall, color = if (online == total && total > 0) LocalStatusColors.current.online else MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            if (s.canConfigure) IconButton(onClick = { onEdit(g.id) }) { Icon(Icons.Filled.Edit, "Edit group") }
                        }
                        if (members.isNotEmpty()) Text(members.joinToString { it.name }, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                        if (s.canOperate) FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 8.dp)) {
                            FilledTonalButton(enabled = canAct && total > 0, onClick = { actions.groupAction(g.id, "wake", false) }) { Text("Wake all") }
                            OutlinedButton(enabled = canAct && total > 0, onClick = { confirm = Triple("Shut down group ${g.name}?", "All ${total} computers in this group that can be shut down will be shut down.") { actions.groupAction(g.id, "shutdown", true) } }) { Text("Shutdown") }
                            OutlinedButton(enabled = canAct && total > 0, onClick = { confirm = Triple("Restart group ${g.name}?", "All $total computers in this group will be restarted.") { actions.groupAction(g.id, "restart", true) } }) { Text("Restart") }
                        }
                    }
                }
            }
        }
        if (s.canConfigure && s.conn.connected) ExtendedFloatingActionButton(
            onClick = { onEdit(null) }, modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
            icon = { Icon(Icons.Filled.Add, null) }, text = { Text("New group") },
        )
    }
    confirm?.let { (t, m, run) -> ConfirmDialog(t, m, if (t.startsWith("Shut")) "Shut down" else "Restart", onConfirm = { run(); confirm = null }, onDismiss = { confirm = null }) }
    if (s.bulkIds.isNotEmpty()) BulkResultDialog(s, actions)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun GroupEditScreen(s: AppUiState, groupId: String?, actions: AppActions, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val existing = s.group(groupId)
    var name by remember { mutableStateOf(existing?.name ?: "") }
    var icon by remember { mutableStateOf(existing?.icon ?: "desktop") }
    var color by remember { mutableStateOf(existing?.color ?: GroupColors[0]) }
    var members by remember { mutableStateOf(s.data.devices.filter { groupId != null && groupId in it.groupIds }.map { it.id }.toSet()) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item {
            OutlinedTextField(name, { name = it }, label = { Text("Group name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        }
        item {
            Text("Icon", style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                GroupIcons.forEach { ic ->
                    FilterChip(selected = icon == ic, onClick = { icon = ic }, label = { Text(ic) }, leadingIcon = { Icon(groupIcon(ic), null, Modifier.size(18.dp)) })
                }
            }
        }
        item {
            Text("Color", style = MaterialTheme.typography.labelLarge)
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(top = 6.dp)) {
                GroupColors.forEach { c ->
                    val col = parseColor(c, MaterialTheme.colorScheme.primary)
                    Box(Modifier.size(32.dp).background(col, CircleShape).border(if (color == c) 3.dp else 0.dp, MaterialTheme.colorScheme.onSurface, CircleShape).clickable { color = c })
                }
            }
        }
        item { Text("Computers in this group", style = MaterialTheme.typography.labelLarge) }
        items(s.data.devices, key = { it.id }) { d ->
            Row(Modifier.fillMaxWidth().clickable { members = if (d.id in members) members - d.id else members + d.id }, verticalAlignment = Alignment.CenterVertically) {
                Checkbox(d.id in members, { members = if (it) members + d.id else members - d.id })
                Column { Text(d.name); Text(s.location(d.locationId)?.name ?: "", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
        }
        item {
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(top = 8.dp)) {
                Button(enabled = name.isNotBlank() && !busy && s.conn.connected, onClick = {
                    busy = true; error = null
                    actions.saveGroup(groupId, GroupInput(name.trim(), icon, color), members) { err -> busy = false; if (err == null) onDone() else error = err }
                }) { Text(if (busy) "Saving..." else "Save") }
                if (groupId != null) OutlinedButton(enabled = !busy && s.conn.connected, onClick = { confirmDelete = true }) { Text("Delete") }
                TextButton(onClick = onDone) { Text("Cancel") }
            }
            if (!s.conn.connected) Text("Not connected: changes cannot be saved.", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
    }
    if (confirmDelete && groupId != null) ConfirmDialog("Delete group?", "The group \"${existing?.name}\" will be deleted. The computers in it are not deleted.", "Delete",
        onConfirm = { confirmDelete = false; actions.deleteGroup(groupId) { err -> if (err == null) onDone() else error = err } }, onDismiss = { confirmDelete = false })
}

// ------------------------------------------------------------------ history

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(s: AppUiState, actions: AppActions, onAddController: () -> Unit, modifier: Modifier = Modifier) {
    PullToRefreshBox(isRefreshing = s.refreshing, onRefresh = actions::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (s.active == null) { item { NoControllerCard(onAddController) }; return@LazyColumn }
            item { ConnectionBanner(s, actions, onRepair = onAddController); Spacer(Modifier.height(8.dp)) }
            val ops = s.data.operations.sortedByDescending { it.startedAt }
            if (ops.isEmpty()) item { EmptyState("No operations recorded.") }
            items(ops, key = { it.id }) { op ->
                LwmCard(Modifier.padding(bottom = 6.dp)) { OperationRow(op, s.nowSecs, showSteps = true) }
            }
        }
    }
}

// ------------------------------------------------------------------ more

@Composable
fun MoreScreen(onNavigate: (String) -> Unit, s: AppUiState, modifier: Modifier = Modifier) {
    val items = listOf(
        Triple("groups", "Groups" to "Group status and actions", Icons.Filled.Workspaces),
        Triple("history", "History" to "Wake, shutdown and restart results", Icons.Filled.History),
        Triple("diagnostics", "Diagnostics" to "Why can't I connect?", Icons.Filled.MonitorHeart),
        Triple("settings", "Settings" to "Controllers, connection, notifications, security", Icons.Filled.Settings),
    )
    Column(modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        items.forEach { (route, text, icon) ->
            LwmCard(Modifier.clickable { onNavigate(route) }) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(14.dp))
                    Column(Modifier.weight(1f)) { Text(text.first, fontWeight = FontWeight.SemiBold); Text(text.second, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                    Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, null)
                }
            }
        }
    }
}
