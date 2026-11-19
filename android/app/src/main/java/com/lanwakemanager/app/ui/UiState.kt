package com.lanwakemanager.app.ui

import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.*

/** Everything the screens render. Pure data so screens can be previewed and snapshot-tested with sample data. */
data class AppUiState(
    val controllers: List<ControllerRecord> = emptyList(),
    val active: ControllerRecord? = null,
    val conn: ConnState = ConnState(),
    val data: ControllerCache = ControllerCache(),
    val liveAt: Long? = null,
    val refreshing: Boolean = false,
    val settings: AppSettings = AppSettings(),
    val net: NetSnapshot = NetSnapshot(),
    val tracked: Map<String, Operation> = emptyMap(),
    val bulkIds: List<String> = emptyList(),
    val wsLive: Boolean = false,
    val nowSecs: Long = System.currentTimeMillis() / 1000,
    val notificationsAllowed: Boolean = true,
    val biometricAvailable: Boolean = true,
    val versionName: String = "1.0.0",
) {
    val role: String get() = active?.role ?: data.role ?: "viewer"
    val canOperate: Boolean get() = Roles.canOperate(role)
    val canConfigure: Boolean get() = Roles.canConfigure(role)

    /** Live = connected and a REST refresh succeeded in this session. Otherwise whatever is shown is cached. */
    val isLive: Boolean get() = conn.connected && liveAt != null
    val dataTime: Long? get() = liveAt ?: data.savedAt.takeIf { it > 0 }

    val selectedLocationId: String? get() = active?.id?.let { settings.selectedLocation[it] }
    val selectedLocation: Location? get() = data.locations.firstOrNull { it.id == selectedLocationId }

    val visibleDevices: List<Device>
        get() = selectedLocationId?.let { id -> data.devices.filter { it.locationId == id } } ?: data.devices

    val onlineCount get() = visibleDevices.count { it.status == "online" }
    val offlineCount get() = visibleDevices.count { it.status == "offline" }
    val otherCount get() = visibleDevices.size - onlineCount - offlineCount

    fun operationFor(deviceId: String): Operation? =
        tracked.values.filter { it.deviceId == deviceId }.maxByOrNull { it.startedAt }

    fun location(id: String?) = data.locations.firstOrNull { it.id == id }
    fun group(id: String?) = data.groups.firstOrNull { it.id == id }
}

data class DeviceExtras(
    val device: Device? = null,
    val operations: List<Operation> = emptyList(),
    val troubleshoot: Troubleshoot? = null,
    val error: String? = null,
)

/** Everything screens can ask for. Defaults do nothing, which is what previews and snapshot tests use. */
interface AppActions {
    fun refresh() {}
    fun wake(deviceId: String) {}
    /** Call only after the user confirmed. [kind] is "shutdown" or "restart". */
    fun power(deviceId: String, kind: String, delaySecs: Int = 0) {}
    fun cancelShutdown(deviceId: String) {}
    /** [confirmed] is true only after an explicit confirmation dialog for shutdown/restart. */
    fun bulk(action: String, deviceIds: List<String>, confirmed: Boolean) {}
    fun groupAction(groupId: String, action: String, confirmed: Boolean) {}
    fun dismissBulk() {}
    fun dismissOperation(id: String) {}

    fun selectController(id: String) {}
    fun removeController(id: String) {}
    fun renameController(id: String, name: String) {}
    fun selectLocation(id: String?) {}
    fun setConnMode(m: ConnMode) {}
    fun setTheme(t: String) {}
    fun setRefreshSecs(s: Int) {}
    fun setNotify(type: String, on: Boolean) {}
    fun setAppLock(on: Boolean) {}
    fun requestNotificationPermission() {}
    fun addWireGuard(controllerId: String, label: String, host: String, port: Int) {}
    fun removeAddress(controllerId: String, url: String) {}

    fun saveSchedule(s: Schedule, done: (String?) -> Unit) { done(null) }
    fun deleteSchedule(id: String, done: (String?) -> Unit) { done(null) }
    fun runScheduleNow(id: String) {}
    fun setScheduleEnabled(s: Schedule, on: Boolean) {}
    fun saveGroup(id: String?, input: GroupInput, members: Set<String>, done: (String?) -> Unit) { done(null) }
    fun deleteGroup(id: String, done: (String?) -> Unit) { done(null) }

    suspend fun deviceExtras(id: String): DeviceExtras = DeviceExtras()
    suspend fun connectivity(): Result<Connectivity> = Result.failure(Exception("Not available"))
    suspend fun runDiagnostics(locationId: String?): Result<DiagnosticsReport> = Result.failure(Exception("Not available"))
}

object NoActions : AppActions
