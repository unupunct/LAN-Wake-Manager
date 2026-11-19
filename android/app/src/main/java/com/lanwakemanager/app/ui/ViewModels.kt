package com.lanwakemanager.app.ui

import android.app.Application
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lanwakemanager.app.LwmApp
import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch

class MainViewModel(app: Application) : AndroidViewModel(app), AppActions {
    private val graph = (app as LwmApp).graph
    private val repo = graph.repository

    private val bulkIds = MutableStateFlow<List<String>>(emptyList())
    private val now = MutableStateFlow(System.currentTimeMillis() / 1000)
    private val notifAllowed = MutableStateFlow(graph.notifier.hasPermission())
    private val biometric = MutableStateFlow(canAuthenticate())

    val snackbar = MutableSharedFlow<String>(extraBufferCapacity = 16)
    val permissionRequests = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    init {
        viewModelScope.launch { while (true) { delay(15_000); now.value = System.currentTimeMillis() / 1000 } }
        viewModelScope.launch { repo.messages.collect { snackbar.emit(it) } }
    }

    @Suppress("UNCHECKED_CAST")
    val ui: StateFlow<AppUiState> = combine(
        listOf(repo.controllers.controllers, repo.active, repo.conn, repo.data, repo.liveAt, repo.refreshing, repo.settings.settings,
            graph.net.snapshot, repo.tracked, bulkIds, repo.wsLive, now, notifAllowed, biometric)
    ) { a ->
        AppUiState(
            controllers = a[0] as List<ControllerRecord>, active = a[1] as ControllerRecord?, conn = a[2] as ConnState,
            data = a[3] as ControllerCache, liveAt = a[4] as Long?, refreshing = a[5] as Boolean, settings = a[6] as AppSettings,
            net = a[7] as NetSnapshot, tracked = a[8] as Map<String, Operation>, bulkIds = a[9] as List<String>, wsLive = a[10] as Boolean,
            nowSecs = a[11] as Long, notificationsAllowed = a[12] as Boolean, biometricAvailable = a[13] as Boolean,
            versionName = com.lanwakemanager.app.BuildConfig.VERSION_NAME,
        )
    }.stateIn(viewModelScope, SharingStarted.Eagerly, AppUiState())

    private fun canAuthenticate(): Boolean = try {
        BiometricManager.from(getApplication()).canAuthenticate(BIOMETRIC_WEAK or DEVICE_CREDENTIAL) == BiometricManager.BIOMETRIC_SUCCESS
    } catch (_: Exception) { false }

    /** Called when the activity resumes: permissions or device lock settings may have changed. */
    fun refreshDeviceState() {
        notifAllowed.value = graph.notifier.hasPermission()
        biometric.value = canAuthenticate()
    }

    private fun toast(t: Throwable) { snackbar.tryEmit(t.userMessage()) }

    override fun refresh() = repo.refreshNow()

    override fun wake(deviceId: String) { viewModelScope.launch { repo.wake(deviceId).onFailure(::toast) } }

    override fun power(deviceId: String, kind: String, delaySecs: Int) {
        viewModelScope.launch {
            (if (kind == "shutdown") repo.shutdown(deviceId, delaySecs) else repo.restart(deviceId, delaySecs)).onFailure(::toast)
        }
    }

    override fun cancelShutdown(deviceId: String) { viewModelScope.launch { repo.cancelShutdown(deviceId).onFailure(::toast) } }

    override fun bulk(action: String, deviceIds: List<String>, confirmed: Boolean) {
        viewModelScope.launch {
            repo.bulk(action, deviceIds, confirmed).onSuccess { ops -> bulkIds.value = ops.map { it.id } }.onFailure(::toast)
        }
    }

    override fun groupAction(groupId: String, action: String, confirmed: Boolean) {
        viewModelScope.launch {
            repo.groupAction(groupId, action, confirmed).onSuccess { ops -> bulkIds.value = ops.map { it.id } }.onFailure(::toast)
        }
    }

    override fun dismissBulk() { bulkIds.value = emptyList() }
    override fun dismissOperation(id: String) = repo.dismissOperation(id)

    override fun selectController(id: String) = repo.selectController(id)
    override fun removeController(id: String) = repo.removeController(id)
    override fun renameController(id: String, name: String) = repo.renameController(id, name)
    override fun selectLocation(id: String?) = repo.selectLocation(id)
    override fun setConnMode(m: ConnMode) = repo.setConnectionMode(m)
    override fun setTheme(t: String) = graph.settings.update { it.copy(theme = t) }
    override fun setRefreshSecs(s: Int) = graph.settings.update { it.copy(refreshSecs = s) }
    override fun setNotify(type: String, on: Boolean) {
        graph.settings.update { it.copy(notify = it.notify + (type to on)) }
        if (on && !graph.notifier.hasPermission()) permissionRequests.tryEmit(Unit)
    }
    override fun setAppLock(on: Boolean) {
        if (on && !canAuthenticate()) { snackbar.tryEmit("Set up a screen lock or biometrics on the phone first."); return }
        graph.settings.update { it.copy(appLock = on) }
    }
    override fun requestNotificationPermission() { permissionRequests.tryEmit(Unit) }
    override fun addWireGuard(controllerId: String, label: String, host: String, port: Int) = repo.addWireGuardProfile(controllerId, label, host, port)
    override fun removeAddress(controllerId: String, url: String) = repo.removeAddress(controllerId, url)

    override fun saveSchedule(s: Schedule, done: (String?) -> Unit) {
        viewModelScope.launch { repo.saveSchedule(s).fold({ done(null) }, { done(it.userMessage()) }) }
    }
    override fun deleteSchedule(id: String, done: (String?) -> Unit) {
        viewModelScope.launch { repo.deleteSchedule(id).fold({ done(null) }, { done(it.userMessage()) }) }
    }
    override fun runScheduleNow(id: String) {
        viewModelScope.launch { repo.runScheduleNow(id).fold({ snackbar.emit("Schedule started") }, ::toast) }
    }
    override fun setScheduleEnabled(s: Schedule, on: Boolean) {
        viewModelScope.launch { repo.setScheduleEnabled(s, on).onFailure(::toast) }
    }
    override fun saveGroup(id: String?, input: GroupInput, members: Set<String>, done: (String?) -> Unit) {
        viewModelScope.launch { repo.saveGroup(id, input, members).fold({ done(null) }, { done(it.userMessage()) }) }
    }
    override fun deleteGroup(id: String, done: (String?) -> Unit) {
        viewModelScope.launch { repo.deleteGroup(id).fold({ done(null) }, { done(it.userMessage()) }) }
    }

    override suspend fun deviceExtras(id: String): DeviceExtras {
        val dev = repo.deviceDetail(id)
        val ops = repo.deviceOperations(id)
        val ts = repo.troubleshoot(id)
        return DeviceExtras(dev.getOrNull(), ops.getOrNull().orEmpty(), ts.getOrNull(), listOf(dev, ts).firstNotNullOfOrNull { it.exceptionOrNull() }?.userMessage())
    }

    override suspend fun connectivity(): Result<Connectivity> = repo.connectivity()
    override suspend fun runDiagnostics(locationId: String?): Result<DiagnosticsReport> = repo.diagnostics(locationId)
}

class PairingViewModel(app: Application) : AndroidViewModel(app), PairingActions {
    private val repo = (app as LwmApp).graph.repository
    private val _state = MutableStateFlow(PairingUiState())
    val state: StateFlow<PairingUiState> = _state.asStateFlow()

    private var resolved: ResolveResult? = null
    private var pairingToken: String = ""
    private var expiresAt: Long = 0

    override fun reset() { resolved = null; pairingToken = ""; _state.value = PairingUiState() }
    fun cameraDenied() { _state.update { it.copy(cameraDenied = true) } }
    override fun setManual(on: Boolean) = _state.update { it.copy(manual = on, error = null) }

    override fun editManual(addr: String?, fp: String?, token: String?) = _state.update {
        it.copy(manualAddr = addr ?: it.manualAddr, manualFp = fp ?: it.manualFp, manualToken = token ?: it.manualToken, error = null)
    }

    /** Raw text of a scanned QR code. */
    fun onScanned(raw: String) {
        val p = try { QrParser.parse(raw) } catch (e: QrParseException) { _state.update { it.copy(error = e.message) }; return }
        pairingToken = p.token; expiresAt = p.exp
        val addrs = p.addrs.map { Addresses.withKind(it) }
        probe(addrs, p.fp, p.id, p.name)
    }

    override fun submitManual() {
        val s = _state.value
        val fp = Fingerprint.normalize(s.manualFp) ?: return _state.update { it.copy(error = "The fingerprint must be 64 hexadecimal characters (colons are allowed).") }
        val addr = Addresses.normalizeUrl(s.manualAddr)
        if (Addresses.hostOf(addr) == null) return _state.update { it.copy(error = "Enter a valid address, e.g. 100.64.1.2:8443.") }
        if (s.manualToken.isBlank()) return _state.update { it.copy(error = "Enter the pairing token.") }
        pairingToken = s.manualToken.trim(); expiresAt = 0
        probe(listOf(Addresses.withKind(addr)), fp, "", "")
    }

    private fun probe(addrs: List<ControllerAddress>, fp: String, id: String, name: String) {
        _state.update { it.copy(step = PairStep.PROBING, error = null, controllerId = id, controllerName = name, fingerprint = fp, addresses = addrs) }
        viewModelScope.launch {
            val r = repo.probeForPairing(addrs, fp, id.ifBlank { null })
            resolved = r
            if (r.api == null) {
                val why = when {
                    r.probes.isEmpty() -> "No address of this controller can be used in the current connection mode. Change the mode in Settings, or connect your VPN."
                    r.probes.any { it.certProblem } -> "The controller's certificate does not match the fingerprint in the pairing code. Pairing was cancelled."
                    else -> "The controller could not be reached. " + (r.probes.lastOrNull()?.error ?: "") + " Is your VPN connected, and is the controller running?"
                }
                _state.update { it.copy(step = PairStep.INPUT, error = why) }
            } else {
                _state.update { it.copy(step = PairStep.CONFIRM, chosen = r.address, latencyMs = r.latencyMs,
                    controllerId = r.info?.controllerId ?: id, controllerName = r.info?.name?.ifBlank { null } ?: name) }
            }
        }
    }

    override fun confirm() {
        val s = _state.value
        val r = resolved ?: return
        if (expiresAt in 1 until System.currentTimeMillis() / 1000) {
            _state.update { it.copy(step = PairStep.INPUT, error = "The pairing code expired while waiting. Create a new one.") }; return
        }
        _state.update { it.copy(step = PairStep.PAIRING, error = null) }
        viewModelScope.launch {
            repo.completePairing(r, s.controllerName, s.controllerId, s.addresses, s.fingerprint, pairingToken).fold({ rec ->
                pairingToken = ""
                _state.update { it.copy(step = PairStep.DONE, controllerName = rec.name) }
            }, { e ->
                val msg = if (e is ApiException) when (e.status) {
                    400, 401, 403, 404 -> "The pairing token was rejected: ${e.message}. It may be expired or already used. Create a new code."
                    429 -> "Too many attempts. Wait a moment and try again."
                    else -> e.message
                } else e.userMessage()
                _state.update { it.copy(step = PairStep.CONFIRM, error = msg) }
            })
        }
    }
}
