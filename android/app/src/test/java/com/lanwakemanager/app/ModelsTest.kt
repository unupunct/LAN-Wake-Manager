package com.lanwakemanager.app

import com.lanwakemanager.app.api.*
import com.lanwakemanager.app.data.AppSettings
import com.lanwakemanager.app.data.ControllerCache
import com.lanwakemanager.app.ui.*
import org.junit.Assert.*
import org.junit.Test

/** JSON models against sample bodies copied from docs/API.md. */
class ModelsTest {
    private fun <T> parse(ser: kotlinx.serialization.KSerializer<T>, s: String) = LwmJson.decodeFromString(ser, s)

    @Test fun info() {
        val i = parse(InfoResponse.serializer(), """{"controller_id":"c1","name":"Home","version":"1.0.0","api_version":1,"setup_required":false,"tls_fingerprint":"ab","time":1700000000,"new_field":1}""")
        assertEquals("c1", i.controllerId); assertEquals(1700000000L, i.time)
    }

    @Test fun pairResponse() {
        val p = parse(PairResponse.serializer(), """{"device_id":"d","token":"t","role":"operator","controller":{"id":"c1","name":"Home"}}""")
        assertEquals("operator", p.role); assertEquals("c1", p.controller.id)
    }

    @Test fun pairRequestFieldNames() {
        val j = LwmJson.encodeToString(PairRequest.serializer(), PairRequest("tok", "Pixel", "android"))
        assertTrue(j.contains("\"pairing_token\":\"tok\"")); assertTrue(j.contains("\"device_name\":\"Pixel\"")); assertTrue(j.contains("\"platform\":\"android\""))
    }

    @Test fun dashboard() {
        val d = parse(Dashboard.serializer(), """
          { "totals": {"devices":12,"online":7,"offline":4,"unknown":1},
            "relays": {"total":2,"online":2},
            "vpn": {"tailscale":{"installed":true,"running":true,"ip":"100.64.1.2","hostname":"pc.tailnet.ts.net"}, "wireguard_profiles":1, "active_connections":1},
            "pending_schedules": 3, "failed_operations_24h": 1,
            "recent_operations": [{"id":"o1","kind":"wake","device_id":"d1","device_name":"Gaming PC","location_id":"l","state":"succeeded","message":"ok","steps":[{"t":1,"text":"x"}],"requested_by":"admin","source":"manual","schedule_id":null,"delay_secs":0,"started_at":1700000000,"finished_at":1700000034}],
            "upcoming_schedules": [{"schedule_id":"s","name":"n","action":"wake","next_run":1700000000}],
            "locations": [{"id":"l","name":"Home","relay_status":"builtin","device_count":5,"online_count":3,"vpn":"tailscale"}] }""")
        assertEquals(12, d.totals.devices); assertEquals("100.64.1.2", d.vpn.tailscale?.ip)
        assertEquals("succeeded", d.recentOperations[0].state); assertEquals(1700000034L, d.recentOperations[0].finishedAt)
        assertEquals(1700000000L, d.upcomingSchedules[0].nextRun); assertEquals("tailscale", d.locations[0].vpn)
    }

    @Test fun deviceWithNullsAndSysinfo() {
        val d = parse(Device.serializer(), """
          { "id":"d1","location_id":"l","name":"Gaming PC","hostname":"DESKTOP-1","ip":"192.168.1.20","mac":"AA:BB:CC:DD:EE:FF","os":"windows","device_type":"pc",
            "wol_capable":"yes","group_ids":["g1","g2"],"tcp_check_ports":[22,3389],"power_method":"agent","ssh":{"user":"admin","port":22,"key_id":null},"notes":"",
            "status":"online","status_checked_at":1700000000,"last_seen":1700000000,"status_changed_at":null,
            "agent":{"state":"online","version":null,"last_heartbeat":null,"enrolled":true},
            "sysinfo":{"hostname":"x","os_name":"Windows 11","os_version":"10.0","kernel":null,"arch":"x86_64","uptime_secs":1234,"cpu_model":null,"cpu_cores":16,"cpu_usage_percent":12.5,
              "mem_total_bytes":100,"mem_used_bytes":null,"disks":[{"mount":"C:\\","total_bytes":10,"free_bytes":5}],"networks":[{"name":"Ethernet","ips":["1.2.3.4"],"mac":"AA"}],
              "temperatures":[{"label":"CPU","celsius":55.0},{"label":"GPU","celsius":null}],"reported_at":1700000000} }""")
        assertEquals("online", d.status); assertNull(d.statusChangedAt); assertNull(d.sysinfo?.cpuModel); assertNull(d.sysinfo?.memUsedBytes)
        assertEquals(55.0, d.sysinfo!!.temperatures[0].celsius!!, 0.0); assertNull(d.sysinfo!!.temperatures[1].celsius)
        assertEquals("C:\\", d.sysinfo!!.disks[0].mount); assertTrue(d.agent!!.enrolled)
    }

    @Test fun deviceWithMissingFieldsStillParses() {
        val d = parse(Device.serializer(), """{"id":"d","name":"n","sysinfo":null,"os":null}""")
        assertEquals("unknown", d.status); assertNull(d.sysinfo); assertTrue(d.groupIds.isEmpty())
    }

    @Test fun operationTerminal() {
        val o = parse(Operation.serializer(), """{"id":"o","kind":"wake","state":"timeout","message":null,"steps":[],"started_at":5}""")
        assertTrue(o.terminal); assertEquals("Magic packet sent, but the computer did not come online", Fmt.opOutcome(o))
        assertFalse(o.copy(state = "running").terminal)
        assertEquals("Computer is online", Fmt.opOutcome(o.copy(state = "succeeded")))
    }

    @Test fun wakeTimeoutIsNeverReportedAsOnline() {
        listOf("pending", "running", "failed", "timeout").forEach { st ->
            assertNotEquals("Computer is online", Fmt.opOutcome(Operation(kind = "wake", state = st)))
        }
    }

    @Test fun scheduleRoundTrip() {
        val json = """
          { "id":"s1","name":"Wake office PCs","enabled":true,"action":"wake",
            "target": {"device_ids":[],"group_ids":["g"],"location_ids":[]},
            "recurrence": { "kind":"weekly","at":"07:30","weekdays":[1,2,3,4,5] },
            "start_date":null,"end_date":"2026-12-31","exceptions":["2026-12-25"],
            "missed_policy":"run_once","missed_grace_minutes":60,
            "retry":{"max_attempts":3,"interval_secs":60},"executor":"relay","delay_secs":0,
            "next_run":1700000000,"last_run":null,"last_state":null }"""
        val s = parse(Schedule.serializer(), json)
        assertEquals(listOf(1, 2, 3, 4, 5), s.recurrence.weekdays); assertEquals("relay", s.executor); assertEquals("2026-12-31", s.endDate)
        val again = parse(Schedule.serializer(), LwmJson.encodeToString(Schedule.serializer(), s))
        assertEquals(s, again)
        val enc = LwmJson.encodeToString(Schedule.serializer(), s)
        assertTrue(enc.contains("\"missed_grace_minutes\":60")); assertTrue(enc.contains("\"max_attempts\":3")); assertTrue(enc.contains("\"day_of_month\"").not())
    }

    @Test fun customRecurrenceSerialization() {
        val s = Schedule(name = "x", recurrence = Recurrence("custom", "10:00", interval = IntervalSpec(6, "hours")))
        val enc = LwmJson.encodeToString(Schedule.serializer(), s)
        assertTrue(enc.contains("\"interval\":{\"every\":6,\"unit\":\"hours\"}"))
        assertFalse("create must not send an id", enc.contains("\"id\""))
    }

    @Test fun connectivityAndDiagnostics() {
        val c = parse(Connectivity.serializer(), """{"tailscale":{"installed":true,"running":true,"backend_state":"Running","ip":"100.64.1.2","hostname":"pc","magicdns_name":"pc.tail1234.ts.net","tailnet":"tail1234.ts.net","error":null},
          "addresses":[{"address":"192.168.1.5","kind":"lan","interface":"Ethernet"}],"port":8443,"allow_public":false}""")
        assertEquals("pc.tail1234.ts.net", c.tailscale?.magicdnsName); assertEquals("Ethernet", c.addresses[0].iface)
        val d = parse(DiagnosticsReport.serializer(), """{"checks":[{"name":"db","status":"ok","detail":"fine","duration_ms":3}],"generated_at":1}""")
        assertEquals(3L, d.checks[0].durationMs)
        val t = parse(Troubleshoot.serializer(), """{"checks":[{"name":"bios","status":"warn","detail":"d"}],"advice":["enable WoL"]}""")
        assertEquals("enable WoL", t.advice[0])
    }

    @Test fun notificationsAndGroups() {
        val n = parse(NotificationItem.serializer(), """{"id":7,"ts":1,"type":"wake_completed","severity":"info","title":"t","body":"b","device_id":"d"}""")
        assertEquals(7L, n.id)
        val g = parse(Group.serializer(), """{"id":"g","name":"Servers","icon":"server","color":"#fff","device_count":2,"online_count":1}""")
        assertEquals(1, g.onlineCount)
    }

    @Test fun wsEvents() {
        val a = parse(WsEvent.serializer(), """{"type":"device_status","device_id":"d","status":"online","last_seen":5}""")
        assertEquals("online", a.status)
        val b = parse(WsEvent.serializer(), """{"type":"operation","operation":{"id":"o","kind":"wake","state":"running","started_at":1}}""")
        assertEquals("running", b.operation?.state)
        assertEquals("hello", parse(WsEvent.serializer(), """{"type":"hello"}""").type)
    }

    @Test fun errorBody() {
        val e = parse(ApiErrorBody.serializer(), """{ "error": { "code": "not_found", "message": "device not found" } }""")
        assertEquals("not_found", e.error?.code)
    }

    @Test fun bulkRequestOmitsUnsetFields() {
        val j = LwmJson.encodeToString(BulkRequest.serializer(), BulkRequest("shutdown", deviceIds = listOf("a"), confirm = true))
        assertTrue(j.contains("\"confirm\":true")); assertFalse(j.contains("group_id")); assertTrue(j.contains("\"device_ids\":[\"a\"]"))
        val w = LwmJson.encodeToString(BulkRequest.serializer(), BulkRequest("wake", deviceIds = listOf("a")))
        assertFalse(w.contains("confirm"))
    }

    @Test fun cacheRoundTripKeepsData() {
        val c = Sample.cache
        val enc = LwmJson.encodeToString(ControllerCache.serializer(), c)
        assertEquals(c, LwmJson.decodeFromString(ControllerCache.serializer(), enc))
        assertFalse("tokens never belong in the cache", enc.contains("token"))
    }

    @Test fun settingsDefaults() {
        val s = LwmJson.decodeFromString(AppSettings.serializer(), "{}")
        assertTrue(s.notifyEnabled("operations")); assertEquals(ConnMode.AUTOMATIC, s.connectionMode)
        val s2 = LwmJson.decodeFromString(AppSettings.serializer(), """{"connection_mode":"x","connectionMode":"vpn_only"}""")
        assertEquals(ConnMode.VPN_ONLY, s2.connectionMode)
    }
}

class ScheduleDraftTest {
    private val ok = ScheduleDraft(name = "Wake", groupIds = setOf("g"))

    @Test fun validDefaultsPass() { assertNull(ok.validate()) }
    @Test fun requiresName() { assertNotNull(ok.copy(name = " ").validate()) }
    @Test fun requiresTarget() { assertNotNull(ok.copy(groupIds = emptySet()).validate()) }
    @Test fun validatesTime() { assertNotNull(ok.copy(at = "25:00").validate()); assertNotNull(ok.copy(at = "7:30").validate()); assertNull(ok.copy(at = "07:05").validate()) }
    @Test fun onceNeedsDate() { assertNotNull(ok.copy(kind = "once").validate()); assertNull(ok.copy(kind = "once", date = "2026-12-24").validate()); assertNotNull(ok.copy(kind = "once", date = "2026-13-40").validate()) }
    @Test fun weeklyNeedsWeekday() { assertNotNull(ok.copy(kind = "weekly", weekdays = emptySet()).validate()) }
    @Test fun monthlyRange() { assertNotNull(ok.copy(kind = "monthly", dayOfMonth = "0").validate()); assertNull(ok.copy(kind = "monthly", dayOfMonth = "31").validate()) }
    @Test fun customInterval() { assertNotNull(ok.copy(kind = "custom", every = "0").validate()); assertNull(ok.copy(kind = "custom", every = "15", unit = "minutes").validate()) }
    @Test fun endBeforeStart() { assertNotNull(ok.copy(startDate = "2026-05-01", endDate = "2026-04-01").validate()) }
    @Test fun relayOnlyForSingleLocationWake() {
        assertNotNull(ok.copy(executor = "relay").validate())
        assertNull(ok.copy(executor = "relay", groupIds = emptySet(), locationIds = setOf("l")).validate())
        assertNotNull(ok.copy(executor = "relay", action = "shutdown", groupIds = emptySet(), locationIds = setOf("l")).validate())
    }

    @Test fun toScheduleOnlyIncludesRelevantRecurrenceFields() {
        val w = ok.copy(kind = "weekly", weekdays = setOf(5, 1)).toSchedule()
        assertEquals(listOf(1, 5), w.recurrence.weekdays); assertNull(w.recurrence.date); assertNull(w.recurrence.interval); assertNull(w.id)
        val c = ok.copy(kind = "custom", every = "6", unit = "hours", exceptions = listOf("2026-12-25")).toSchedule()
        assertEquals(IntervalSpec(6, "hours"), c.recurrence.interval); assertEquals(listOf("2026-12-25"), c.exceptions)
    }

    @Test fun fromAndBack() {
        val s = Sample.schedules[0]
        val back = ScheduleDraft.from(s).toSchedule()
        assertEquals(s.recurrence, back.recurrence); assertEquals(s.target, back.target); assertEquals(s.executor, back.executor); assertEquals(s.id, back.id)
    }

    @Test fun descriptions() {
        assertEquals("Weekly on Mon, Tue, Wed, Thu, Fri at 07:30", ScheduleText.recurrence(Sample.schedules[0].recurrence))
        assertEquals("Monthly on day 1 at 03:00", ScheduleText.recurrence(Sample.schedules[2].recurrence))
    }
}

class ExplanationsTest {
    private val base = Sample.disconnected
    @Test fun vpnOnlyWithoutVpn() {
        val s = base.copy(settings = base.settings.copy(connectionMode = ConnMode.VPN_ONLY), net = NetSnapshot(wifi = true, vpn = false))
        assertTrue(Explanations.forState(s).any { it.title == "VPN app not connected" })
    }
    @Test fun lanOnlyOffWifi() {
        val s = base.copy(settings = base.settings.copy(connectionMode = ConnMode.LAN_ONLY), net = NetSnapshot(wifi = false, cellular = true))
        assertTrue(Explanations.forState(s).any { it.title.startsWith("Phone not on the LAN") })
    }
    @Test fun revokedOffersRepair() {
        val s = base.copy(conn = com.lanwakemanager.app.data.ConnState(com.lanwakemanager.app.data.ConnStatus.UNAUTHORIZED))
        assertTrue(Explanations.forState(s).any { it.offerRepair && it.title.contains("401") })
    }
    @Test fun certMismatch() {
        val s = base.copy(conn = com.lanwakemanager.app.data.ConnState(com.lanwakemanager.app.data.ConnStatus.CERT_MISMATCH))
        assertTrue(Explanations.forState(s).any { it.title.contains("fingerprint") })
    }
    @Test fun connectedIsOk() { assertEquals("ok", Explanations.forState(Sample.state).first().severity) }
    @Test fun cachedIsNeverLive() {
        assertFalse(Sample.disconnected.isLive); assertTrue(Sample.state.isLive)
        assertEquals("Disconnected", Sample.disconnected.conn.copy(status = com.lanwakemanager.app.data.ConnStatus.DISCONNECTED).chipText())
        assertEquals("Controller unavailable", Sample.disconnected.conn.chipText())
        assertEquals("Connected through Tailscale", Sample.state.conn.chipText())
    }
}
