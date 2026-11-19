package com.lanwakemanager.app.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lanwakemanager.app.data.ConnStatus

fun parseColor(hex: String?, fallback: Color): Color = try {
    if (hex.isNullOrBlank()) fallback else Color(android.graphics.Color.parseColor(hex))
} catch (_: Exception) { fallback }

@Composable
fun LwmCard(modifier: Modifier = Modifier, container: Color = MaterialTheme.colorScheme.surface, content: @Composable ColumnScope.() -> Unit) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        color = container,
        border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
    ) { Column(Modifier.padding(14.dp), content = content) }
}

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier = modifier.padding(top = 8.dp, bottom = 2.dp), style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant, fontWeight = FontWeight.SemiBold)
}

@Composable
fun Pill(text: String, color: Color, modifier: Modifier = Modifier, icon: ImageVector? = null) {
    Row(
        modifier.background(color.copy(alpha = 0.14f), RoundedCornerShape(50)).border(1.dp, color.copy(alpha = 0.45f), RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) { Icon(icon, null, tint = color, modifier = Modifier.size(14.dp)); Spacer(Modifier.width(5.dp)) }
        else { Box(Modifier.size(8.dp).background(color, CircleShape)); Spacer(Modifier.width(6.dp)) }
        Text(text, color = color, fontSize = 12.sp, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
fun statusColor(status: String): Color = when (status) {
    "online", "succeeded", "ok" -> sc().online
    "offline", "failed", "fail", "error" -> sc().offline
    "waking", "shutting_down", "running", "pending", "warn", "timeout" -> sc().warn
    "info" -> sc().info
    else -> sc().neutral
}

@Composable
private fun sc(): StatusColors = LocalStatusColors.current

@Composable
fun ConnChip(conn: com.lanwakemanager.app.data.ConnState, modifier: Modifier = Modifier) {
    val c = when (conn.status) {
        ConnStatus.CONNECTED -> sc().online
        ConnStatus.CONNECTING -> sc().info
        ConnStatus.UNAVAILABLE, ConnStatus.CERT_MISMATCH, ConnStatus.UNAUTHORIZED -> sc().offline
        else -> sc().neutral
    }
    Pill(conn.chipText(), c, modifier)
}

/** "Live" or "Cached - 5 min ago". Never says Live while disconnected. */
@Composable
fun FreshnessChip(s: AppUiState, modifier: Modifier = Modifier) {
    if (s.isLive) Pill("Live", sc().online, modifier, Icons.Filled.Bolt)
    else Pill("Cached – " + Fmt.ago(s.dataTime, s.nowSecs), sc().warn, modifier, Icons.Filled.History)
}

@Composable
fun StatTile(label: String, value: String, color: Color, modifier: Modifier = Modifier) {
    Surface(modifier, shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.surfaceVariant) {
        Column(Modifier.padding(12.dp), horizontalAlignment = Alignment.Start) {
            Text(value, fontSize = 26.sp, fontWeight = FontWeight.Bold, color = color)
            Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
fun EmptyState(text: String, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().padding(28.dp), contentAlignment = Alignment.Center) {
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
fun ConfirmDialog(
    title: String, text: String, confirmLabel: String,
    onConfirm: () -> Unit, onDismiss: () -> Unit, destructive: Boolean = true,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Text(text) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(confirmLabel, color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary)
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
fun InfoRow(label: String, value: String?, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth().padding(vertical = 3.dp), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(0.42f))
        Text(value ?: "not reported", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(0.58f),
            color = if (value == null) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface)
    }
}

@Composable
fun deviceIcon(type: String?): ImageVector = when (type) {
    "laptop" -> Icons.Filled.Laptop
    "server" -> Icons.Filled.Dns
    "nas" -> Icons.Filled.Storage
    else -> Icons.Filled.DesktopWindows
}

fun groupIcon(icon: String?): ImageVector = when (icon) {
    "gamepad" -> Icons.Filled.SportsEsports
    "server" -> Icons.Filled.Dns
    "nas" -> Icons.Filled.Storage
    "building" -> Icons.Filled.Business
    "laptop" -> Icons.Filled.Laptop
    "star" -> Icons.Filled.Star
    "all" -> Icons.Filled.Apps
    else -> Icons.Filled.DesktopWindows
}

val GroupIcons = listOf("desktop", "gamepad", "server", "nas", "building", "laptop", "star", "all")
val GroupColors = listOf("#2ee59d", "#22d3ee", "#f5b942", "#ff6b6b", "#a78bfa", "#f472b6", "#60a5fa", "#94a3b8")

/** Banner shown on data screens: honest about live vs cached data and why. */
@Composable
fun ConnectionBanner(s: AppUiState, actions: AppActions, onRepair: () -> Unit = {}, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ConnChip(s.conn); FreshnessChip(s)
        }
        if (!s.conn.connected && s.active != null) {
            val msg = when (s.conn.status) {
                ConnStatus.UNAUTHORIZED -> "This phone is no longer authorised on the controller. Showing cached data only."
                ConnStatus.CERT_MISMATCH -> "The certificate does not match the pinned fingerprint. No data is exchanged. Showing cached data only."
                ConnStatus.CONNECTING -> "Looking for the controller..."
                else -> "No live access to the controller. Showing cached data from ${Fmt.ago(s.dataTime, s.nowSecs)}; actions are unavailable."
            }
            Text(msg, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (s.conn.status == ConnStatus.UNAUTHORIZED) Button(onClick = onRepair) { Text("Pair again") }
                else if (s.conn.status != ConnStatus.CONNECTING) OutlinedButton(onClick = actions::refresh) { Text("Retry now") }
            }
        }
    }
}
