package com.lanwakemanager.app.ui

import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.*

/** Realistic sample data for Compose previews and JVM snapshot tests. Never used at runtime. */
object Sample {
    const val NOW = 1_767_000_000L
    private const val FP = "3fa91c0b7d2e4a5c8d6f10b9e3a47c52d81f6e0a9b3c7d4e5f60718293a4b5c6"

    val controller = ControllerRecord(
        id = "ctl-7f3a", name = "Home controller",
        addresses = listOf(
            ControllerAddress("https://192.168.1.5:8443", AddrKind.LAN),
            ControllerAddress("https://100.64.1.2:8443", AddrKind.TAILSCALE),
            ControllerAddress("https://10.8.0.1:8443", AddrKind.WIREGUARD, "Office WireGuard"),
        ),
        fingerprint = FP, role = "admin", tokenEnc = "x", pairedAt = NOW - 86400 * 3,
    )

    val locations = listOf(
        Location("loc-home", "Home Network", "#2ee59d", relayMode = "builtin", relayStatus = "builtin", deviceCount = 4, onlineCount = 2, controllerStatus = "online", vpn = "tailscale"),
        Location("loc-office", "Office", "#22d3ee", relayMode = "remote", relayUrl = "https://100.64.1.9:8444", relayStatus = "online", relayLastSeen = NOW - 20, relayName = "office-nuc", relayVersion = "1.0.0", deviceCount = 3, onlineCount = 1, controllerStatus = "online", vpn = "tailscale"),
    )

    val groups = listOf(
        Group("g1", "Workstations", "desktop", "#2ee59d", 3, 2),
        Group("g2", "Gaming PCs", "gamepad", "#22d3ee", 1, 0),
        Group("g3", "Servers", "server", "#f5b942", 2, 1),
    )

    val devices = listOf(
        Device("d1", "loc-home", "Gaming PC", "DESKTOP-GAME", "192.168.1.20", "AA:BB:CC:DD:EE:01", "windows", "pc", "yes", listOf("g2"),
            status = "online", lastSeen = NOW - 12, agent = AgentInfo("online", "1.0.0", NOW - 12, true),
            sysinfo = SysInfo("DESKTOP-GAME", "Windows 11", "10.0.26200", null, "x86_64", 93_600, "AMD Ryzen 9 7950X", 16, 12.5, 34_359_738_368, 14_000_000_000,
                listOf(DiskInfo("C:\\", 1_000_000_000_000, 420_000_000_000), DiskInfo("D:\\", 2_000_000_000_000, 1_500_000_000_000)),
                listOf(NetIf("Ethernet", listOf("192.168.1.20"), "AA:BB:CC:DD:EE:01")), listOf(TempInfo("CPU", 55.0)), NOW - 12)),
        Device("d2", "loc-home", "Media Server", "nas-01", "192.168.1.30", "AA:BB:CC:DD:EE:02", "linux", "server", "yes", listOf("g3"),
            status = "online", lastSeen = NOW - 30, agent = AgentInfo("online", "1.0.0", NOW - 30, true)),
        Device("d3", "loc-home", "Work Laptop", "LAPTOP-WORK", "192.168.1.41", "AA:BB:CC:DD:EE:03", "windows", "laptop", "unknown", listOf("g1"),
            status = "offline", lastSeen = NOW - 7200, agent = AgentInfo("offline", "1.0.0", NOW - 7200, true)),
        Device("d4", "loc-home", "Backup NAS", "nas-02", "192.168.1.31", "AA:BB:CC:DD:EE:04", "linux", "nas", "no", listOf("g3"),
            status = "offline", lastSeen = NOW - 86400, agent = AgentInfo("none", null, null, false)),
        Device("d5", "loc-office", "Office PC 1", "OFFICE-01", "10.20.0.11", "AA:BB:CC:DD:EE:05", "windows", "pc", "yes", listOf("g1"),
            status = "online", lastSeen = NOW - 45, agent = AgentInfo("online", "1.0.0", NOW - 45, true)),
        Device("d6", "loc-office", "Office PC 2", "OFFICE-02", "10.20.0.12", "AA:BB:CC:DD:EE:06", "windows", "pc", "unknown", listOf("g1"),
            status = "waking", lastSeen = NOW - 3600, agent = AgentInfo("offline", "1.0.0", NOW - 3600, true)),
        Device("d7", "loc-office", "Print Server", "print", "10.20.0.50", "AA:BB:CC:DD:EE:07", "linux", "server", "unknown", emptyList(),
            status = "unknown"),
    )

    val operations = listOf(
        Operation("op1", "wake", "d6", "Office PC 2", "loc-office", "running", "Magic packet sent, waiting", listOf(OpStep(NOW - 20, "magic packet sent via relay office-nuc -> 10.20.0.255:9 (attempt 1/3)")), "Pixel 8", "manual", startedAt = NOW - 20),
        Operation("op2", "wake", "d1", "Gaming PC", "loc-home", "succeeded", "Online after 34 s (confirmed by ping and agent heartbeat)",
            listOf(OpStep(NOW - 900, "magic packet sent via 192.168.1.2 -> 192.168.1.255:9 (attempt 1/3)"), OpStep(NOW - 866, "host answered ping")), "admin", "manual", startedAt = NOW - 900, finishedAt = NOW - 866),
        Operation("op3", "wake", "d4", "Backup NAS", "loc-home", "timeout", "Packets sent but the machine did not come up in 120 s", emptyList(), "schedule:Nightly backup", "schedule", startedAt = NOW - 5400, finishedAt = NOW - 5280),
        Operation("op4", "shutdown", "d3", "Work Laptop", "loc-home", "succeeded", "Shutdown confirmed", emptyList(), "Pixel 8", "manual", startedAt = NOW - 7300, finishedAt = NOW - 7210),
        Operation("op5", "restart", "d2", "Media Server", "loc-home", "failed", "Agent offline: cannot restart", emptyList(), "admin", "bulk", startedAt = NOW - 20000, finishedAt = NOW - 19990),
    )

    val schedules = listOf(
        Schedule("s1", "Wake office PCs", true, "wake", ScheduleTarget(groupIds = listOf("g1")), Recurrence("weekly", "07:30", weekdays = listOf(1, 2, 3, 4, 5)),
            retry = RetryPolicy(3, 60), executor = "relay", nextRun = NOW + 5400, lastRun = NOW - 80000, lastState = "succeeded"),
        Schedule("s2", "Shutdown gaming PC", true, "shutdown", ScheduleTarget(deviceIds = listOf("d1")), Recurrence("daily", "23:30"),
            missedPolicy = "run_once", nextRun = NOW + 40000, lastRun = NOW - 46000, lastState = "succeeded"),
        Schedule("s3", "Monthly maintenance wake", false, "wake", ScheduleTarget(locationIds = listOf("loc-home")), Recurrence("monthly", "03:00", dayOfMonth = 1),
            nextRun = null, lastState = null),
    )

    val dashboard = Dashboard(
        totals = Totals(7, 3, 3, 1), relays = RelaySummary(2, 2),
        vpn = VpnSummary(TailscaleSummary(true, true, "100.64.1.2", "pc.tail1234.ts.net"), 1, 1),
        pendingSchedules = 2, failedOperations24h = 1, recentOperations = operations,
        upcomingSchedules = schedules.filter { it.enabled }.map { UpcomingSchedule(it.id!!, it.name, it.action, it.nextRun) },
        locations = locations,
    )

    val cache = ControllerCache(NOW - 5, "admin", dashboard, devices, locations, groups, schedules, operations)

    val conn = ConnState(ConnStatus.CONNECTED, controller.addresses[1], 38, NOW - 5, null,
        listOf(ProbeResult(controller.addresses[0], false, null, "The controller did not answer in time."), ProbeResult(controller.addresses[1], true, 38, null)))

    val state = AppUiState(
        controllers = listOf(controller), active = controller, conn = conn, data = cache, liveAt = NOW - 5,
        settings = AppSettings(connectionMode = ConnMode.AUTOMATIC, theme = "dark", selectedLocation = mapOf(controller.id to "loc-home")),
        net = NetSnapshot(connected = true, wifi = false, vpn = true, cellular = true),
        wsLive = true, nowSecs = NOW,
    )

    val disconnected = state.copy(
        conn = ConnState(ConnStatus.UNAVAILABLE, null, null, NOW - 4000, "The controller did not answer in time."),
        liveAt = null, wsLive = false,
        data = cache.copy(savedAt = NOW - 4000),
    )
}
