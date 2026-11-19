package com.lanwakemanager.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.ui.*
import org.junit.Rule
import org.junit.Test

/**
 * Renders the main screens with sample data on the JVM (Paparazzi / layoutlib). Record with
 * `gradlew recordPaparazziDebug`; PNGs land in app/src/test/snapshots/images.
 */
class ScreenshotTest {
    @get:Rule
    val paparazzi = Paparazzi(deviceConfig = DeviceConfig.PIXEL_5, theme = "android:Theme.Material.NoActionBar")

    private val s = Sample.state

    private fun shot(name: String, route: String, theme: String = "dark", content: @Composable () -> Unit) {
        val tab = route in setOf("home", "computers", "locations", "schedules", "more", "settings", "diagnostics", "groups", "history")
        paparazzi.snapshot(name) {
            LwmTheme(theme) {
                LwmFrame(route, isTab = route in setOf("home", "computers", "locations", "schedules", "more"), showBottom = tab, canRefresh = true,
                    onTab = {}, onBack = {}, onRefresh = {}, snackbarHost = SnackbarHostState()) { pad ->
                    Box(Modifier.fillMaxSize().padding(pad)) { content() }
                }
            }
        }
    }

    @Test fun home() = shot("home", "home") { HomeScreen(s, NoActions, {}, {}) }
    @Test fun homeLight() = shot("home_light", "home", "light") { HomeScreen(s, NoActions, {}, {}) }
    @Test fun homeCached() = shot("home_cached", "home") { HomeScreen(Sample.disconnected, NoActions, {}, {}) }
    @Test fun computers() = shot("computers", "computers") { ComputersScreen(s, NoActions, {}, {}) }
    @Test fun computersSelection() = shot("computers_selection", "computers") { ComputersScreen(s, NoActions, {}, {}, initialSelection = setOf("d1", "d2")) }
    @Test fun deviceDetail() = shot("device", "device/d1") {
        DeviceDetailScreen(s, "d1", NoActions, preloaded = DeviceExtras(Sample.devices[0], Sample.operations.filter { it.deviceId == "d1" },
            Troubleshoot(listOf(Check("Wake-on-LAN confirmed", "ok", "A previous wake was confirmed."), Check("MAC address", "ok", "AA:BB:CC:DD:EE:01")), listOf("Keep the network adapter's Wake on Magic Packet enabled."))))
    }
    @Test fun locations() = shot("locations", "locations") { LocationsScreen(s, NoActions, {}) }
    @Test fun schedules() = shot("schedules", "schedules") { SchedulesScreen(s, NoActions, {}, {}) }
    @Test fun scheduleEdit() = shot("schedule_edit", "schedule_edit/s1") { ScheduleEditScreen(s, "s1", NoActions, {}) }
    @Test fun groups() = shot("groups", "groups") { GroupsScreen(s, NoActions, {}, {}) }
    @Test fun history() = shot("history", "history") { HistoryScreen(s, NoActions, {}) }
    @Test fun diagnostics() = shot("diagnostics", "diagnostics") {
        DiagnosticsScreen(s, NoActions, {}, preloaded = DiagnosticsReport(listOf(Check("Controller", "ok", "Running 1.0.0", 3), Check("Relay: office-nuc", "ok", "Reachable", 41), Check("Scheduler", "ok", "3 schedules active", 1))))
    }
    @Test fun diagnosticsDisconnected() = shot("diagnostics_offline", "diagnostics") {
        DiagnosticsScreen(Sample.disconnected.copy(settings = Sample.disconnected.settings.copy(connectionMode = ConnMode.VPN_ONLY), net = NetSnapshot(wifi = true, vpn = false)), NoActions, {})
    }
    @Test fun settings() = shot("settings", "settings") {
        SettingsScreen(s, NoActions, {}, preloadedConn = Connectivity(TailscaleInfo(true, true, "Running", "100.64.1.2", "pc", "pc.tail1234.ts.net", "tail1234.ts.net"),
            listOf(ConnAddress("192.168.1.5", "lan", "Ethernet"), ConnAddress("100.64.1.2", "tailscale", "tailscale0")), 8443))
    }
    @Test fun addController() = shot("add_controller", "add_controller") {
        AddControllerScreen(PairingUiState(), NoPairingActions, ConnMode.AUTOMATIC, {}, {})
    }
    @Test fun addControllerConfirm() = shot("add_controller_confirm", "add_controller") {
        AddControllerScreen(PairingUiState(PairStep.CONFIRM, controllerId = "ctl-7f3a", controllerName = "Home controller",
            fingerprint = Sample.controller.fingerprint, chosen = Sample.controller.addresses[1], latencyMs = 38), NoPairingActions, ConnMode.AUTOMATIC, {}, {})
    }
}
