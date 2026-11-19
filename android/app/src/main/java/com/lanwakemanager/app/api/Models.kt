package com.lanwakemanager.app.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Shared JSON configuration: unknown fields are ignored, missing optional fields become null/defaults. */
val LwmJson: Json = Json {
    ignoreUnknownKeys = true
    coerceInputValues = true
    explicitNulls = false
    encodeDefaults = true
    isLenient = true
}

@Serializable
data class ApiErrorBody(val error: ApiErrorDetail? = null)

@Serializable
data class ApiErrorDetail(val code: String = "", val message: String = "")

@Serializable
data class InfoResponse(
    @SerialName("controller_id") val controllerId: String = "",
    val name: String = "",
    val version: String? = null,
    @SerialName("api_version") val apiVersion: Int = 1,
    @SerialName("setup_required") val setupRequired: Boolean = false,
    @SerialName("tls_fingerprint") val tlsFingerprint: String? = null,
    val time: Long? = null,
)

@Serializable
data class PairRequest(
    @SerialName("pairing_token") val pairingToken: String,
    @SerialName("device_name") val deviceName: String,
    val platform: String = "android",
)

@Serializable
data class PairedController(val id: String = "", val name: String = "")

@Serializable
data class PairResponse(
    @SerialName("device_id") val deviceId: String = "",
    val token: String = "",
    val role: String = "viewer",
    val controller: PairedController = PairedController(),
)

@Serializable
data class MeResponse(
    val kind: String = "mobile",
    val role: String = "viewer",
    @SerialName("device_id") val deviceId: String? = null,
    val name: String = "",
)

@Serializable
data class Totals(val devices: Int = 0, val online: Int = 0, val offline: Int = 0, val unknown: Int = 0)

@Serializable
data class RelaySummary(val total: Int = 0, val online: Int = 0)

@Serializable
data class TailscaleSummary(
    val installed: Boolean = false,
    val running: Boolean = false,
    val ip: String? = null,
    val hostname: String? = null,
)

@Serializable
data class VpnSummary(
    val tailscale: TailscaleSummary? = null,
    @SerialName("wireguard_profiles") val wireguardProfiles: Int = 0,
    @SerialName("active_connections") val activeConnections: Int = 0,
)

@Serializable
data class UpcomingSchedule(
    @SerialName("schedule_id") val scheduleId: String = "",
    val name: String = "",
    val action: String = "wake",
    @SerialName("next_run") val nextRun: Long? = null,
)

@Serializable
data class Dashboard(
    val totals: Totals = Totals(),
    val relays: RelaySummary = RelaySummary(),
    val vpn: VpnSummary = VpnSummary(),
    @SerialName("pending_schedules") val pendingSchedules: Int = 0,
    @SerialName("failed_operations_24h") val failedOperations24h: Int = 0,
    @SerialName("recent_operations") val recentOperations: List<Operation> = emptyList(),
    @SerialName("upcoming_schedules") val upcomingSchedules: List<UpcomingSchedule> = emptyList(),
    val locations: List<Location> = emptyList(),
)

@Serializable
data class Location(
    val id: String = "",
    val name: String = "",
    val color: String? = null,
    val notes: String? = null,
    @SerialName("broadcast_addr") val broadcastAddr: String? = null,
    @SerialName("wol_ports") val wolPorts: List<Int> = emptyList(),
    @SerialName("wake_attempts") val wakeAttempts: Int? = null,
    @SerialName("wake_interval_secs") val wakeIntervalSecs: Int? = null,
    @SerialName("wake_timeout_secs") val wakeTimeoutSecs: Int? = null,
    @SerialName("relay_mode") val relayMode: String = "builtin",
    @SerialName("relay_url") val relayUrl: String? = null,
    @SerialName("relay_status") val relayStatus: String = "builtin",
    @SerialName("relay_last_seen") val relayLastSeen: Long? = null,
    @SerialName("relay_version") val relayVersion: String? = null,
    @SerialName("relay_name") val relayName: String? = null,
    @SerialName("device_count") val deviceCount: Int = 0,
    @SerialName("online_count") val onlineCount: Int = 0,
    @SerialName("controller_status") val controllerStatus: String? = null,
    val vpn: String? = null,
)

@Serializable
data class SshConfig(val user: String? = null, val port: Int? = null, @SerialName("key_id") val keyId: String? = null)

@Serializable
data class AgentInfo(
    val state: String = "none",
    val version: String? = null,
    @SerialName("last_heartbeat") val lastHeartbeat: Long? = null,
    val enrolled: Boolean = false,
)

@Serializable
data class DiskInfo(
    val mount: String = "",
    @SerialName("total_bytes") val totalBytes: Long? = null,
    @SerialName("free_bytes") val freeBytes: Long? = null,
)

@Serializable
data class NetIf(val name: String = "", val ips: List<String> = emptyList(), val mac: String? = null)

@Serializable
data class TempInfo(val label: String = "", val celsius: Double? = null)

@Serializable
data class SysInfo(
    val hostname: String? = null,
    @SerialName("os_name") val osName: String? = null,
    @SerialName("os_version") val osVersion: String? = null,
    val kernel: String? = null,
    val arch: String? = null,
    @SerialName("uptime_secs") val uptimeSecs: Long? = null,
    @SerialName("cpu_model") val cpuModel: String? = null,
    @SerialName("cpu_cores") val cpuCores: Int? = null,
    @SerialName("cpu_usage_percent") val cpuUsagePercent: Double? = null,
    @SerialName("mem_total_bytes") val memTotalBytes: Long? = null,
    @SerialName("mem_used_bytes") val memUsedBytes: Long? = null,
    val disks: List<DiskInfo> = emptyList(),
    val networks: List<NetIf> = emptyList(),
    val temperatures: List<TempInfo> = emptyList(),
    @SerialName("reported_at") val reportedAt: Long? = null,
)

@Serializable
data class Device(
    val id: String = "",
    @SerialName("location_id") val locationId: String = "",
    val name: String = "",
    val hostname: String? = null,
    val ip: String? = null,
    val mac: String? = null,
    val os: String? = null,
    @SerialName("device_type") val deviceType: String? = null,
    @SerialName("wol_capable") val wolCapable: String = "unknown",
    @SerialName("group_ids") val groupIds: List<String> = emptyList(),
    @SerialName("tcp_check_ports") val tcpCheckPorts: List<Int> = emptyList(),
    @SerialName("power_method") val powerMethod: String? = null,
    val ssh: SshConfig? = null,
    val notes: String? = null,
    val status: String = "unknown",
    @SerialName("status_checked_at") val statusCheckedAt: Long? = null,
    @SerialName("last_seen") val lastSeen: Long? = null,
    @SerialName("status_changed_at") val statusChangedAt: Long? = null,
    val agent: AgentInfo? = null,
    val sysinfo: SysInfo? = null,
)

/** Body for PUT /devices/{id}; used only to change group membership. */
@Serializable
data class DeviceInput(
    @SerialName("location_id") val locationId: String,
    val name: String,
    val hostname: String? = null,
    val ip: String? = null,
    val mac: String? = null,
    val os: String? = null,
    @SerialName("device_type") val deviceType: String? = null,
    @SerialName("group_ids") val groupIds: List<String> = emptyList(),
    @SerialName("tcp_check_ports") val tcpCheckPorts: List<Int> = emptyList(),
    @SerialName("power_method") val powerMethod: String? = null,
    val ssh: SshConfig? = null,
    val notes: String? = null,
) {
    companion object {
        fun from(d: Device, groupIds: List<String>) = DeviceInput(
            d.locationId, d.name, d.hostname, d.ip, d.mac, d.os, d.deviceType, groupIds,
            d.tcpCheckPorts, d.powerMethod, d.ssh, d.notes,
        )
    }
}

@Serializable
data class OpStep(val t: Long = 0, val text: String = "")

@Serializable
data class Operation(
    val id: String = "",
    val kind: String = "wake",
    @SerialName("device_id") val deviceId: String? = null,
    @SerialName("device_name") val deviceName: String? = null,
    @SerialName("location_id") val locationId: String? = null,
    val state: String = "pending",
    val message: String? = null,
    val steps: List<OpStep> = emptyList(),
    @SerialName("requested_by") val requestedBy: String? = null,
    val source: String? = null,
    @SerialName("schedule_id") val scheduleId: String? = null,
    @SerialName("delay_secs") val delaySecs: Int = 0,
    @SerialName("started_at") val startedAt: Long = 0,
    @SerialName("finished_at") val finishedAt: Long? = null,
) {
    val terminal: Boolean get() = state == "succeeded" || state == "failed" || state == "timeout"
}

@Serializable
data class Group(
    val id: String = "",
    val name: String = "",
    val icon: String? = null,
    val color: String? = null,
    @SerialName("device_count") val deviceCount: Int = 0,
    @SerialName("online_count") val onlineCount: Int = 0,
)

@Serializable
data class GroupInput(val name: String, val icon: String? = null, val color: String? = null)

@Serializable
data class ScheduleTarget(
    @SerialName("device_ids") val deviceIds: List<String> = emptyList(),
    @SerialName("group_ids") val groupIds: List<String> = emptyList(),
    @SerialName("location_ids") val locationIds: List<String> = emptyList(),
)

@Serializable
data class IntervalSpec(val every: Int = 1, val unit: String = "hours")

@Serializable
data class Recurrence(
    val kind: String = "daily",
    val at: String? = null,
    val date: String? = null,
    val weekdays: List<Int> = emptyList(),
    @SerialName("day_of_month") val dayOfMonth: Int? = null,
    val interval: IntervalSpec? = null,
)

@Serializable
data class RetryPolicy(
    @SerialName("max_attempts") val maxAttempts: Int = 1,
    @SerialName("interval_secs") val intervalSecs: Int = 60,
)

@Serializable
data class Schedule(
    val id: String? = null,
    val name: String = "",
    val enabled: Boolean = true,
    val action: String = "wake",
    val target: ScheduleTarget = ScheduleTarget(),
    val recurrence: Recurrence = Recurrence(),
    @SerialName("start_date") val startDate: String? = null,
    @SerialName("end_date") val endDate: String? = null,
    val exceptions: List<String> = emptyList(),
    @SerialName("missed_policy") val missedPolicy: String = "skip",
    @SerialName("missed_grace_minutes") val missedGraceMinutes: Int = 60,
    val retry: RetryPolicy = RetryPolicy(),
    val executor: String = "controller",
    @SerialName("delay_secs") val delaySecs: Int = 0,
    @SerialName("next_run") val nextRun: Long? = null,
    @SerialName("last_run") val lastRun: Long? = null,
    @SerialName("last_state") val lastState: String? = null,
)

@Serializable
data class TailscaleInfo(
    val installed: Boolean = false,
    val running: Boolean = false,
    @SerialName("backend_state") val backendState: String? = null,
    val ip: String? = null,
    val hostname: String? = null,
    @SerialName("magicdns_name") val magicdnsName: String? = null,
    val tailnet: String? = null,
    val error: String? = null,
)

@Serializable
data class ConnAddress(val address: String = "", val kind: String = "other", @SerialName("interface") val iface: String? = null)

@Serializable
data class Connectivity(
    val tailscale: TailscaleInfo? = null,
    val addresses: List<ConnAddress> = emptyList(),
    val port: Int? = null,
    @SerialName("allow_public") val allowPublic: Boolean = false,
)

@Serializable
data class Check(
    val name: String = "",
    val status: String = "info",
    val detail: String? = null,
    @SerialName("duration_ms") val durationMs: Long? = null,
)

@Serializable
data class DiagnosticsReport(
    val checks: List<Check> = emptyList(),
    @SerialName("generated_at") val generatedAt: Long? = null,
)

@Serializable
data class Troubleshoot(val checks: List<Check> = emptyList(), val advice: List<String> = emptyList())

@Serializable
data class NotificationItem(
    val id: Long = 0,
    val ts: Long = 0,
    val type: String = "controller_notice",
    val severity: String = "info",
    val title: String = "",
    val body: String? = null,
    @SerialName("device_id") val deviceId: String? = null,
)

@Serializable
data class BulkRequest(
    val action: String,
    @SerialName("device_ids") val deviceIds: List<String>? = null,
    @SerialName("group_id") val groupId: String? = null,
    @SerialName("location_id") val locationId: String? = null,
    @SerialName("delay_secs") val delaySecs: Int? = null,
    val confirm: Boolean? = null,
)

@Serializable
data class GroupActionRequest(
    val action: String,
    @SerialName("delay_secs") val delaySecs: Int? = null,
    val confirm: Boolean? = null,
)

@Serializable
data class BulkResponse(val operations: List<Operation> = emptyList())

@Serializable
data class DelayBody(@SerialName("delay_secs") val delaySecs: Int = 0)

/** WebSocket event envelope; only the fields we use. */
@Serializable
data class WsEvent(
    val type: String = "",
    @SerialName("device_id") val deviceId: String? = null,
    val status: String? = null,
    @SerialName("last_seen") val lastSeen: Long? = null,
    val operation: Operation? = null,
    @SerialName("location_id") val locationId: String? = null,
    @SerialName("relay_status") val relayStatus: String? = null,
    val notification: NotificationItem? = null,
    @SerialName("schedule_id") val scheduleId: String? = null,
)
