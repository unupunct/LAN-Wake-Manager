package com.lanwakemanager.app.data

import android.content.Context
import android.os.Build
import com.lanwakemanager.app.api.*
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.io.IOException

object Roles {
    fun canOperate(role: String?) = role == "admin" || role == "operator"
    fun canConfigure(role: String?) = role == "admin"
}

/**
 * Single source of truth for the active controller: connection state, cached/live data, WebSocket events and
 * all actions. REST is the truth; WebSocket events are hints that patch the in-memory data.
 */
class LwmRepository(
    private val appContext: Context,
    val controllers: ControllerStore,
    val settings: SettingsStore,
    private val cacheStore: CacheStore,
    val net: NetInfo,
    val notifier: Notifier,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val _activeId = MutableStateFlow<String?>(null)
    val activeId: StateFlow<String?> = _activeId.asStateFlow()

    val active: StateFlow<ControllerRecord?> =
        combine(controllers.controllers, _activeId) { list, id -> list.firstOrNull { it.id == id } }
            .stateIn(scope, SharingStarted.Eagerly, null)

    private val _conn = MutableStateFlow(ConnState())
    val conn: StateFlow<ConnState> = _conn.asStateFlow()

    private val _data = MutableStateFlow(ControllerCache())
    val data: StateFlow<ControllerCache> = _data.asStateFlow()

    /** Epoch seconds of the last successful full REST refresh in this process; null = nothing live yet. */
    private val _liveAt = MutableStateFlow<Long?>(null)
    val liveAt: StateFlow<Long?> = _liveAt.asStateFlow()

    private val _refreshing = MutableStateFlow(false)
    val refreshing: StateFlow<Boolean> = _refreshing.asStateFlow()

    /** Operations started from this app (or seen live), by id, with their latest state. */
    private val _tracked = MutableStateFlow<Map<String, Operation>>(emptyMap())
    val tracked: StateFlow<Map<String, Operation>> = _tracked.asStateFlow()

    private val _wsLive = MutableStateFlow(false)
    val wsLive: StateFlow<Boolean> = _wsLive.asStateFlow()

    val messages = MutableSharedFlow<String>(extraBufferCapacity = 16)

    private var api: ControllerApi? = null
    private var foreground = false
    private var loopJob: Job? = null
    private var ws: WebSocket? = null
    private var resolvedNet: NetSnapshot? = null
    private var forceReresolve = false
    private val kick = Channel<Unit>(Channel.CONFLATED)
    private var saveJob: Job? = null
    private val pollJobs = HashMap<String, Job>()

    init {
        val s = settings.settings.value
        val list = controllers.controllers.value
        val start = list.firstOrNull { it.id == s.lastControllerId } ?: list.firstOrNull()
        if (start != null) activate(start.id, persist = false)
        net.start { onNetworkChanged() }
    }

    // ------------------------------------------------------------ lifecycle

    fun setForeground(f: Boolean) {
        foreground = f
        if (f) {
            if (loopJob?.isActive != true) loopJob = scope.launch { runLoop() }
            kick.trySend(Unit)
        } else {
            loopJob?.cancel(); loopJob = null
            closeWs()
        }
    }

    private fun onNetworkChanged() {
        val now = net.read()
        if (now != resolvedNet) {
            forceReresolve = true
            kick.trySend(Unit)
        }
    }

    /** Pull-to-refresh / manual retry. */
    fun refreshNow() {
        if (api == null) forceReresolve = true
        kick.trySend(Unit)
    }

    fun selectController(id: String) { if (id != _activeId.value) activate(id, persist = true) }

    private fun activate(id: String, persist: Boolean) {
        closeWs()
        api = null
        resolvedNet = null
        _activeId.value = id
        _data.value = cacheStore.load(id) ?: ControllerCache()
        _liveAt.value = null
        _tracked.value = emptyMap()
        _conn.value = ConnState(ConnStatus.DISCONNECTED, lastSuccess = _data.value.savedAt.takeIf { it > 0 })
        if (persist) settings.update { it.copy(lastControllerId = id) }
        forceReresolve = true
        kick.trySend(Unit)
    }

    fun removeController(id: String) {
        val wasActive = _activeId.value == id
        controllers.remove(id)
        cacheStore.delete(id)
        settings.update { it.copy(selectedLocation = it.selectedLocation - id, lastNotificationId = it.lastNotificationId - id, reachable = it.reachable - id) }
        if (wasActive) {
            closeWs(); api = null
            val next = controllers.controllers.value.firstOrNull()
            if (next != null) activate(next.id, true) else {
                _activeId.value = null; _data.value = ControllerCache(); _liveAt.value = null
                _conn.value = ConnState(); settings.update { it.copy(lastControllerId = null) }
            }
        }
    }

    // ------------------------------------------------------------ connection loop

    private suspend fun runLoop() {
        var backoff = 2000L
        while (currentCoroutineContext().isActive) {
            val rec = active.value
            if (rec == null) { kick.receive(); continue }
            if (forceReresolve) { api = null; closeWs(); forceReresolve = false }
            if (api == null) {
                if (_conn.value.status == ConnStatus.UNAUTHORIZED) {
                    // Revoked tokens do not come back; wait for the user (re-pair) or a manual retry.
                    withTimeoutOrNull(5 * 60_000L) { kick.receive() }
                    if (_conn.value.status == ConnStatus.UNAUTHORIZED && !forceReresolve) continue
                }
                if (!connectOnce(rec)) {
                    withTimeoutOrNull(backoff) { kick.receive() }
                    backoff = if (forceReresolve) 2000L else (backoff * 2).coerceAtMost(60_000L)
                    continue
                }
                backoff = 2000L
            }
            if (!refreshAll()) continue
            ensureWs()
            val secs = settings.settings.value.refreshSecs.coerceIn(10, 600)
            withTimeoutOrNull(secs * 1000L) { kick.receive() }
        }
    }

    private suspend fun connectOnce(rec: ControllerRecord): Boolean {
        val mode = settings.settings.value.connectionMode
        val snap = net.read()
        _conn.update { it.copy(status = ConnStatus.CONNECTING) }
        val token = controllers.token(rec)
        val r = ConnectionResolver.resolve(rec, token, mode, snap)
        resolvedNet = snap
        val a = r.api
        if (a == null) {
            api = null
            _conn.value = ConnectionResolver.failureState(r, mode, snap, _conn.value.lastSuccess)
            _wsLive.value = false
            return false
        }
        if (token == null) {
            _conn.value = ConnState(ConnStatus.UNAUTHORIZED, r.address, r.latencyMs, _conn.value.lastSuccess, "The stored device token could not be decrypted. Pair again.", r.probes)
            return false
        }
        api = a
        _conn.value = ConnState(ConnStatus.CONNECTED, r.address, r.latencyMs, nowSecs(), null, r.probes)
        return true
    }

    private fun nowSecs() = System.currentTimeMillis() / 1000

    private fun markDisconnected(e: Throwable) {
        api = null
        closeWs()
        _wsLive.value = false
        _conn.update { ConnState(ConnStatus.UNAVAILABLE, null, null, it.lastSuccess, e.userMessage(), it.probes) }
        kick.trySend(Unit)
    }

    private fun markUnauthorized() {
        api = null
        closeWs()
        _conn.update { ConnState(ConnStatus.UNAUTHORIZED, it.active, null, it.lastSuccess, "The controller rejected this device's token (revoked or removed).", it.probes) }
    }

    /** Runs an API call; maps connectivity/auth failures into the connection state. */
    private suspend fun <T> call(block: suspend (ControllerApi) -> T): Result<T> {
        val a = api ?: return Result.failure(IOException("Not connected to the controller. Showing cached data."))
        return try {
            Result.success(block(a))
        } catch (e: CancellationException) {
            throw e
        } catch (e: ApiException) {
            if (e.unauthorized) markUnauthorized()
            Result.failure(e)
        } catch (e: IOException) {
            markDisconnected(e); Result.failure(e)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    // ------------------------------------------------------------ data refresh

    private suspend fun refreshAll(): Boolean {
        val a = api ?: return false
        _refreshing.value = true
        try {
            val res = coroutineScope {
                val dash = async { runCatching { a.dashboard() } }
                val locs = async { runCatching { a.locations() } }
                val devs = async { runCatching { a.devices() } }
                val grps = async { runCatching { a.groups() } }
                val scheds = async { runCatching { a.schedules() } }
                val ops = async { runCatching { a.operations(50) } }
                val me = async { runCatching { a.me() } }
                listOf(dash, locs, devs, grps, scheds, ops, me).map { it.await() }
            }
            val failures = res.mapNotNull { it.exceptionOrNull() }
            failures.filterIsInstance<ApiException>().firstOrNull { it.unauthorized }?.let { markUnauthorized(); return false }
            val critical = res[2].exceptionOrNull() ?: res[0].exceptionOrNull() // devices, dashboard
            if (critical != null) {
                if (critical is ApiException) { _conn.update { it.copy(lastError = critical.userMessage()) } }
                else { markDisconnected(critical); return false }
            }
            val old = _data.value
            @Suppress("UNCHECKED_CAST")
            val new = ControllerCache(
                savedAt = nowSecs(),
                role = (res[6].getOrNull() as? MeResponse)?.role ?: old.role,
                dashboard = (res[0].getOrNull() as? Dashboard) ?: old.dashboard,
                locations = (res[1].getOrNull() as? List<Location>) ?: old.locations,
                devices = (res[2].getOrNull() as? List<Device>) ?: old.devices,
                groups = (res[3].getOrNull() as? List<Group>) ?: old.groups,
                schedules = (res[4].getOrNull() as? List<Schedule>) ?: old.schedules,
                operations = (res[5].getOrNull() as? List<Operation>) ?: old.operations,
            )
            _data.value = new
            _liveAt.value = new.savedAt
            _conn.update { it.copy(lastSuccess = new.savedAt, lastError = null) }
            new.role?.let { r -> active.value?.let { rec -> if (rec.role != r) controllers.update(rec.id) { it.copy(role = r) } } }
            persistSoon()
            return true
        } finally {
            _refreshing.value = false
        }
    }

    private fun persistSoon() {
        val id = _activeId.value ?: return
        saveJob?.cancel()
        saveJob = scope.launch(Dispatchers.IO) {
            delay(800)
            try { cacheStore.save(id, _data.value) } catch (_: Exception) { }
        }
    }

    private inline fun patch(block: (ControllerCache) -> ControllerCache) {
        _data.update { block(it) }
        persistSoon()
    }

    // ------------------------------------------------------------ WebSocket

    private fun ensureWs() {
        val a = api ?: return
        if (ws != null || !foreground) return
        ws = a.openEvents(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                if (webSocket !== ws) return
                handleEvent(text)
            }
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) { wsEnded(webSocket) }
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) { wsEnded(webSocket) }
        })
    }

    private fun wsEnded(w: WebSocket) {
        if (w !== ws) return
        ws = null
        _wsLive.value = false
        if (foreground && api != null) scope.launch { delay(5000); ensureWs() }
    }

    private fun closeWs() {
        try { ws?.close(1000, "bye") } catch (_: Exception) { }
        ws = null
        _wsLive.value = false
    }

    private fun handleEvent(text: String) {
        val ev = try { LwmJson.decodeFromString(WsEvent.serializer(), text) } catch (_: Exception) { return }
        when (ev.type) {
            "hello" -> _wsLive.value = true
            "device_status" -> {
                val id = ev.deviceId ?: return
                patch { c -> c.copy(devices = c.devices.map { d ->
                    if (d.id == id) d.copy(status = ev.status ?: d.status, lastSeen = ev.lastSeen ?: d.lastSeen) else d
                }) }
            }
            "operation" -> ev.operation?.let { applyOperation(it) }
            "relay_status" -> {
                val id = ev.locationId ?: return
                patch { c -> c.copy(locations = c.locations.map { if (it.id == id) it.copy(relayStatus = ev.relayStatus ?: it.relayStatus) else it }) }
            }
            "notification" -> ev.notification?.let { n ->
                val rec = active.value ?: return
                val last = settings.settings.value.lastNotificationId[rec.id] ?: 0
                if (n.id > last) {
                    settings.update { it.copy(lastNotificationId = it.lastNotificationId + (rec.id to n.id)) }
                    notifier.post(rec.id, rec.name, n)
                }
            }
            "schedule_fired" -> scope.launch { call { it.schedules() }.onSuccess { s -> patch { c -> c.copy(schedules = s) } } }
        }
    }

    private fun applyOperation(op: Operation) {
        if (_tracked.value.containsKey(op.id)) _tracked.update { it + (op.id to op) }
        patch { c ->
            val ops = if (c.operations.any { it.id == op.id }) c.operations.map { if (it.id == op.id) op else it } else listOf(op) + c.operations
            c.copy(operations = ops.take(100))
        }
        if (op.terminal) scope.launch { call { it.devices() }.onSuccess { d -> patch { c -> c.copy(devices = d) } } }
    }

    // ------------------------------------------------------------ operations

    private fun track(op: Operation) {
        _tracked.update { it + (op.id to op) }
        applyOperation(op)
        if (op.terminal) return
        pollJobs[op.id]?.cancel()
        pollJobs[op.id] = scope.launch {
            val deadline = System.currentTimeMillis() + 10 * 60_000L
            var misses = 0
            while (isActive && System.currentTimeMillis() < deadline) {
                delay(2000)
                val r = call { it.operation(op.id) }
                val cur = r.getOrNull()
                if (cur != null) {
                    misses = 0
                    applyOperation(cur)
                    _tracked.update { it + (cur.id to cur) }
                    if (cur.terminal) break
                } else if (++misses > 150) break
            }
        }
    }

    fun dismissOperation(id: String) { _tracked.update { it - id } }

    private suspend fun opAction(block: suspend (ControllerApi) -> Operation): Result<Operation> =
        call(block).onSuccess { track(it) }

    suspend fun wake(deviceId: String) = opAction { it.wake(deviceId) }
    suspend fun shutdown(deviceId: String, delaySecs: Int = 0) = opAction { it.shutdown(deviceId, delaySecs) }
    suspend fun restart(deviceId: String, delaySecs: Int = 0) = opAction { it.restart(deviceId, delaySecs) }
    suspend fun cancelShutdown(deviceId: String) = opAction { it.cancelShutdown(deviceId) }

    /** [confirmed] must be true only after the user explicitly confirmed a destructive action. */
    suspend fun bulk(action: String, deviceIds: List<String>, confirmed: Boolean, delaySecs: Int = 0): Result<List<Operation>> {
        val destructive = action != "wake"
        if (destructive && !confirmed) return Result.failure(IllegalStateException("Confirmation required"))
        return call { it.bulk(BulkRequest(action, deviceIds = deviceIds, delaySecs = delaySecs.takeIf { d -> d > 0 }, confirm = if (destructive) true else null)) }
            .onSuccess { ops -> ops.forEach { track(it) } }
    }

    suspend fun groupAction(groupId: String, action: String, confirmed: Boolean): Result<List<Operation>> {
        val destructive = action != "wake"
        if (destructive && !confirmed) return Result.failure(IllegalStateException("Confirmation required"))
        return call { it.groupAction(groupId, GroupActionRequest(action, null, if (destructive) true else null)) }
            .onSuccess { ops -> ops.forEach { track(it) } }
    }

    suspend fun refreshDevice(id: String): Result<Device> = call { it.refreshDevice(id) }.onSuccess { d ->
        patch { c -> c.copy(devices = c.devices.map { x -> if (x.id == d.id) d else x }) }
    }

    suspend fun deviceDetail(id: String): Result<Device> = call { it.device(id) }.onSuccess { d ->
        patch { c -> c.copy(devices = c.devices.map { x -> if (x.id == d.id) d else x }) }
    }

    suspend fun deviceOperations(id: String): Result<List<Operation>> = call { it.operations(50, id) }
    suspend fun troubleshoot(id: String): Result<Troubleshoot> = call { it.troubleshoot(id) }
    suspend fun connectivity(): Result<Connectivity> = call { it.connectivity() }
    suspend fun diagnostics(locationId: String?): Result<DiagnosticsReport> = call { it.diagnostics(locationId) }

    // ------------------------------------------------------------ schedules / groups

    private suspend fun reloadSchedules() { call { it.schedules() }.onSuccess { s -> patch { c -> c.copy(schedules = s) } }; call { it.dashboard() }.onSuccess { d -> patch { c -> c.copy(dashboard = d) } } }

    suspend fun saveSchedule(s: Schedule): Result<Schedule> =
        call { if (s.id == null) it.createSchedule(s) else it.updateSchedule(s.id, s) }.onSuccess { reloadSchedules() }

    suspend fun deleteSchedule(id: String): Result<Unit> = call { it.deleteSchedule(id) }.onSuccess { reloadSchedules() }

    suspend fun setScheduleEnabled(s: Schedule, enabled: Boolean): Result<Schedule> = saveSchedule(s.copy(enabled = enabled))

    suspend fun runScheduleNow(id: String): Result<Unit> = call { it.runScheduleNow(id) }.onSuccess { delay(500); call { it.operations(50) }.onSuccess { o -> patch { c -> c.copy(operations = o) } } }

    private suspend fun reloadGroups() {
        call { it.groups() }.onSuccess { g -> patch { c -> c.copy(groups = g) } }
        call { it.devices() }.onSuccess { d -> patch { c -> c.copy(devices = d) } }
    }

    /** Creates or updates a group and applies the device membership difference. */
    suspend fun saveGroup(id: String?, input: GroupInput, memberIds: Set<String>): Result<Group> {
        val r = call { if (id == null) it.createGroup(input) else it.updateGroup(id, input) }
        val g = r.getOrNull() ?: return r
        val gid = g.id.ifBlank { id ?: return r }
        for (d in _data.value.devices) {
            val isMember = gid in d.groupIds
            val shouldBe = d.id in memberIds
            if (isMember != shouldBe) {
                val ids = if (shouldBe) d.groupIds + gid else d.groupIds - gid
                val u = call { it.updateDevice(d.id, DeviceInput.from(d, ids)) }
                if (u.isFailure) { reloadGroups(); return Result.failure(u.exceptionOrNull()!!) }
            }
        }
        reloadGroups()
        return r
    }

    suspend fun deleteGroup(id: String): Result<Unit> = call { it.deleteGroup(id) }.onSuccess { reloadGroups() }

    // ------------------------------------------------------------ settings helpers

    fun selectedLocationId(): String? = _activeId.value?.let { settings.settings.value.selectedLocation[it] }

    fun selectLocation(locationId: String?) {
        val id = _activeId.value ?: return
        settings.update { s -> s.copy(selectedLocation = if (locationId == null) s.selectedLocation - id else s.selectedLocation + (id to locationId)) }
    }

    fun setConnectionMode(m: ConnMode) {
        settings.update { it.copy(connectionMode = m) }
        forceReresolve = true
        kick.trySend(Unit)
    }

    /** Adds / replaces a user-labelled WireGuard address on a controller (kind = wireguard). */
    fun addWireGuardProfile(controllerId: String, label: String, host: String, port: Int) {
        val url = Addresses.normalizeUrl(if (host.contains("://")) host else "$host:$port")
        val withPort = if (Addresses.hostOf(url) != null && !Regex(":\\d+$").containsMatchIn(url)) "$url:$port" else url
        controllers.update(controllerId) { r ->
            r.copy(addresses = r.addresses.filter { it.url != withPort } + ControllerAddress(withPort, AddrKind.WIREGUARD, label.ifBlank { "WireGuard" }))
        }
        forceReresolve = true; kick.trySend(Unit)
    }

    fun removeAddress(controllerId: String, url: String) {
        controllers.update(controllerId) { r -> r.copy(addresses = r.addresses.filter { it.url != url }) }
        forceReresolve = true; kick.trySend(Unit)
    }

    fun renameController(id: String, name: String) { controllers.update(id) { it.copy(name = name.ifBlank { it.name }) } }

    // ------------------------------------------------------------ pairing

    /** Probes the addresses with the pinned fingerprint (honouring the connection mode). */
    suspend fun probeForPairing(addresses: List<ControllerAddress>, fingerprint: String, expectedId: String?): ResolveResult {
        val tmp = ControllerRecord(expectedId.orEmpty(), "", addresses, fingerprint, tokenEnc = "")
        return ConnectionResolver.resolve(tmp, null, settings.settings.value.connectionMode, net.read())
    }

    /** POST /pair on the probed connection and store the controller. Returns the saved record. */
    suspend fun completePairing(resolved: ResolveResult, name: String, controllerId: String, addresses: List<ControllerAddress>, fingerprint: String, pairingToken: String): Result<ControllerRecord> {
        val a = resolved.api ?: return Result.failure(IOException("No connection"))
        return try {
            val deviceName = (Build.MODEL ?: "Android phone").take(40)
            val pr = a.pair(PairRequest(pairingToken, deviceName, "android"))
            val id = controllerId.ifBlank { pr.controller.id.ifBlank { resolved.info?.controllerId.orEmpty() } }
            if (id.isBlank()) return Result.failure(IOException("The controller did not report its id"))
            val old = controllers.get(id)
            val keptExtra = old?.addresses?.filter { it.kind == AddrKind.WIREGUARD && addresses.none { n -> n.url == it.url } }.orEmpty()
            val rec = ControllerRecord(
                id = id,
                name = pr.controller.name.ifBlank { resolved.info?.name ?: old?.name ?: "Controller" },
                addresses = (addresses + keptExtra).distinctBy { it.url },
                fingerprint = fingerprint,
                role = pr.role,
                deviceId = pr.deviceId,
                tokenEnc = controllers.encryptToken(pr.token),
            )
            controllers.save(rec)
            if (old == null) cacheStore.delete(id)
            activate(id, true)
            Result.success(rec)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    // ------------------------------------------------------------ misc

    fun currentRole(): String? = active.value?.role ?: _data.value.role

    fun pollingGraphIsForeground(): Boolean = foreground
    fun isLiveFor(id: String): Boolean = foreground && _activeId.value == id && _conn.value.connected
}
