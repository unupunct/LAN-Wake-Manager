package com.lanwakemanager.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.Schedule

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SchedulesScreen(s: AppUiState, actions: AppActions, onEdit: (String?) -> Unit, onAddController: () -> Unit, modifier: Modifier = Modifier) {
    var deleteTarget by remember { mutableStateOf<Schedule?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    val canAct = s.canOperate && s.conn.connected
    Box(modifier.fillMaxSize()) {
        PullToRefreshBox(isRefreshing = s.refreshing, onRefresh = actions::refresh, modifier = Modifier.fillMaxSize()) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp, 16.dp, 16.dp, 90.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (s.active == null) { item { NoControllerCard(onAddController) }; return@LazyColumn }
                item {
                    Column {
                        ConnectionBanner(s, actions, onRepair = onAddController)
                        Text("Wake schedules run on the controller or relay, not on the sleeping computer.", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                    }
                }
                if (s.data.schedules.isEmpty()) item { EmptyState("No schedules yet.") }
                items(s.data.schedules, key = { it.id ?: it.name }) { sc ->
                    LwmCard {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(sc.name, fontWeight = FontWeight.SemiBold, style = MaterialTheme.typography.titleMedium)
                                Text(ScheduleText.recurrence(sc.recurrence), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.secondary)
                            }
                            Switch(checked = sc.enabled, enabled = canAct, onCheckedChange = { actions.setScheduleEnabled(sc, it) })
                        }
                        Spacer(Modifier.height(6.dp))
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Pill(Fmt.kindLabel(sc.action), if (sc.action == "wake") LocalStatusColors.current.online else LocalStatusColors.current.warn)
                            Pill(if (sc.executor == "relay") "Runs on relay" else "Runs on controller", LocalStatusColors.current.neutral)
                        }
                        Text("Targets: ${ScheduleText.target(sc.target, s)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                        Text(
                            if (sc.enabled && sc.nextRun != null) "Next run ${Fmt.until(sc.nextRun, s.nowSecs)} (${Fmt.dateTime(sc.nextRun)})" else if (!sc.enabled) "Disabled" else "No upcoming run",
                            style = MaterialTheme.typography.bodySmall,
                        )
                        if (sc.lastRun != null) Text("Last run ${Fmt.ago(sc.lastRun, s.nowSecs)}: ${sc.lastState ?: "unknown"}", style = MaterialTheme.typography.bodySmall,
                            color = statusColor(sc.lastState ?: "info"))
                        if (s.canOperate) FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.padding(top = 6.dp)) {
                            TextButton(enabled = canAct, onClick = { onEdit(sc.id) }) { Icon(Icons.Filled.Edit, null, Modifier.size(18.dp)); Spacer(Modifier.width(4.dp)); Text("Edit") }
                            TextButton(enabled = canAct, onClick = { sc.id?.let(actions::runScheduleNow) }) { Icon(Icons.Filled.PlayArrow, null, Modifier.size(18.dp)); Spacer(Modifier.width(4.dp)); Text("Run now") }
                            TextButton(enabled = canAct, onClick = { deleteTarget = sc }) { Icon(Icons.Filled.Delete, null, Modifier.size(18.dp)); Spacer(Modifier.width(4.dp)); Text("Delete") }
                        }
                    }
                }
            }
        }
        if (s.canOperate && s.conn.connected) ExtendedFloatingActionButton(
            onClick = { onEdit(null) }, modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
            icon = { Icon(Icons.Filled.Add, null) }, text = { Text("New schedule") },
        )
    }
    deleteTarget?.let { t ->
        ConfirmDialog("Delete schedule?", "\"${t.name}\" will be deleted and will not run again.", "Delete",
            onConfirm = { deleteTarget = null; t.id?.let { id -> actions.deleteSchedule(id) { err -> error = err } } }, onDismiss = { deleteTarget = null })
    }
    if (s.bulkIds.isNotEmpty()) BulkResultDialog(s, actions)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChipRow(options: List<Pair<String, String>>, selected: String, onSelect: (String) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { (k, label) -> FilterChip(selected = selected == k, onClick = { onSelect(k) }, label = { Text(label) }) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ScheduleEditScreen(s: AppUiState, scheduleId: String?, actions: AppActions, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val existing = s.data.schedules.firstOrNull { it.id == scheduleId }
    var d by remember { mutableStateOf(ScheduleDraft.from(existing)) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var newException by remember { mutableStateOf("") }
    val num = KeyboardOptions(keyboardType = KeyboardType.Number)
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { OutlinedTextField(d.name, { d = d.copy(name = it) }, label = { Text("Name") }, singleLine = true, modifier = Modifier.fillMaxWidth()) }
        item {
            Row(verticalAlignment = Alignment.CenterVertically) { Text("Enabled", Modifier.weight(1f)); Switch(d.enabled, { d = d.copy(enabled = it) }) }
        }
        item {
            Text("Action", style = MaterialTheme.typography.labelLarge)
            ChipRow(listOf("wake" to "Wake", "shutdown" to "Shutdown", "restart" to "Restart"), d.action) { d = d.copy(action = it, executor = if (it == "wake") d.executor else "controller") }
            if (d.action != "wake") Text("Shutdown and restart need the agent on the computer and run on the controller.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        item {
            Text("Targets", style = MaterialTheme.typography.labelLarge)
            Text("Computers", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                s.data.devices.forEach { dev -> FilterChip(dev.id in d.deviceIds, { d = d.copy(deviceIds = if (dev.id in d.deviceIds) d.deviceIds - dev.id else d.deviceIds + dev.id) }, label = { Text(dev.name) }) }
            }
            Text("Groups", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                s.data.groups.forEach { g -> FilterChip(g.id in d.groupIds, { d = d.copy(groupIds = if (g.id in d.groupIds) d.groupIds - g.id else d.groupIds + g.id) }, label = { Text(g.name) }) }
            }
            Text("Locations", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                s.data.locations.forEach { l -> FilterChip(l.id in d.locationIds, { d = d.copy(locationIds = if (l.id in d.locationIds) d.locationIds - l.id else d.locationIds + l.id) }, label = { Text(l.name) }) }
            }
        }
        item {
            Text("Repeat", style = MaterialTheme.typography.labelLarge)
            ChipRow(listOf("once" to "Once", "daily" to "Daily", "weekly" to "Weekly", "monthly" to "Monthly", "custom" to "Custom"), d.kind) { d = d.copy(kind = it) }
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(d.at, { d = d.copy(at = it.take(5)) }, label = { Text("Time (HH:MM, controller time zone)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            when (d.kind) {
                "once" -> OutlinedTextField(d.date, { d = d.copy(date = it.take(10)) }, label = { Text("Date (YYYY-MM-DD)") }, singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
                "weekly" -> FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(top = 8.dp)) {
                    listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun").forEachIndexed { i, n ->
                        val k = i + 1
                        FilterChip(k in d.weekdays, { d = d.copy(weekdays = if (k in d.weekdays) d.weekdays - k else d.weekdays + k) }, label = { Text(n) })
                    }
                }
                "monthly" -> OutlinedTextField(d.dayOfMonth, { d = d.copy(dayOfMonth = it.filter(Char::isDigit).take(2)) }, label = { Text("Day of month (clamped to month length)") }, singleLine = true, keyboardOptions = num, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
                "custom" -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 8.dp)) {
                    OutlinedTextField(d.every, { d = d.copy(every = it.filter(Char::isDigit).take(4)) }, label = { Text("Every") }, singleLine = true, keyboardOptions = num, modifier = Modifier.width(100.dp))
                    ChipRow(listOf("minutes" to "min", "hours" to "hours", "days" to "days"), d.unit) { d = d.copy(unit = it) }
                }
            }
        }
        item {
            Text("Validity", style = MaterialTheme.typography.labelLarge)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(d.startDate, { d = d.copy(startDate = it.take(10)) }, label = { Text("Start date") }, placeholder = { Text("optional") }, singleLine = true, modifier = Modifier.weight(1f))
                OutlinedTextField(d.endDate, { d = d.copy(endDate = it.take(10)) }, label = { Text("End date") }, placeholder = { Text("optional") }, singleLine = true, modifier = Modifier.weight(1f))
            }
            Text("Skip these dates", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                d.exceptions.forEach { ex -> InputChip(false, { d = d.copy(exceptions = d.exceptions - ex) }, label = { Text(ex) }, trailingIcon = { Icon(Icons.Filled.Close, "Remove", Modifier.size(16.dp)) }) }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(newException, { newException = it.take(10) }, label = { Text("Exception date (YYYY-MM-DD)") }, singleLine = true, modifier = Modifier.weight(1f))
                OutlinedButton(enabled = ScheduleDraft.isDate(newException), onClick = { d = d.copy(exceptions = (d.exceptions + newException).distinct()); newException = "" }) { Text("Add") }
            }
        }
        item {
            Text("If the controller was offline at run time", style = MaterialTheme.typography.labelLarge)
            ChipRow(listOf("skip" to "Skip the run", "run_once" to "Run once when back"), d.missedPolicy) { d = d.copy(missedPolicy = it) }
            OutlinedTextField(d.graceMinutes, { d = d.copy(graceMinutes = it.filter(Char::isDigit).take(5)) }, label = { Text("Grace period (minutes)") }, singleLine = true, keyboardOptions = num, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
        }
        item {
            Text("Retry", style = MaterialTheme.typography.labelLarge)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(d.retryAttempts, { d = d.copy(retryAttempts = it.filter(Char::isDigit).take(2)) }, label = { Text("Max attempts") }, singleLine = true, keyboardOptions = num, modifier = Modifier.weight(1f))
                OutlinedTextField(d.retryIntervalSecs, { d = d.copy(retryIntervalSecs = it.filter(Char::isDigit).take(5)) }, label = { Text("Interval (s)") }, singleLine = true, keyboardOptions = num, modifier = Modifier.weight(1f))
            }
            if (d.action != "wake") OutlinedTextField(d.delaySecs, { d = d.copy(delaySecs = it.filter(Char::isDigit).take(5)) }, label = { Text("Delay before action (s)") }, singleLine = true, keyboardOptions = num, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
        }
        if (d.action == "wake") item {
            Text("Run on", style = MaterialTheme.typography.labelLarge)
            ChipRow(listOf("controller" to "Controller", "relay" to "Location relay"), d.executor) { d = d.copy(executor = it) }
            Text("A relay can wake computers even while the controller is down. It needs exactly one location as target.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        item {
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            if (!s.conn.connected) Text("Not connected: the schedule cannot be saved right now.", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp), modifier = Modifier.padding(top = 8.dp)) {
                Button(enabled = !busy && s.conn.connected && s.canOperate, onClick = {
                    val v = d.validate()
                    if (v != null) { error = v } else {
                        busy = true; error = null
                        actions.saveSchedule(d.toSchedule()) { err -> busy = false; if (err == null) onDone() else error = err }
                    }
                }) { Text(if (busy) "Saving..." else "Save") }
                TextButton(onClick = onDone) { Text("Cancel") }
            }
        }
    }
}
