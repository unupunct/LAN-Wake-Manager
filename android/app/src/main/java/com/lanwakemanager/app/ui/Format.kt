package com.lanwakemanager.app.ui

import com.lanwakemanager.app.api.Device
import com.lanwakemanager.app.api.Operation
import java.text.DateFormat
import java.util.Date
import java.util.Locale

object Fmt {
    fun ago(ts: Long?, now: Long): String {
        if (ts == null || ts <= 0) return "never"
        val d = (now - ts).coerceAtLeast(0)
        return when {
            d < 10 -> "just now"
            d < 60 -> "${d}s ago"
            d < 3600 -> "${d / 60} min ago"
            d < 86400 -> "${d / 3600} h ago"
            else -> "${d / 86400} d ago"
        }
    }

    fun until(ts: Long?, now: Long): String {
        if (ts == null) return "not scheduled"
        val d = ts - now
        if (d <= 0) return "due now"
        return when {
            d < 3600 -> "in ${(d / 60).coerceAtLeast(1)} min"
            d < 86400 -> "in ${d / 3600} h ${(d % 3600) / 60} min"
            else -> "in ${d / 86400} d ${(d % 86400) / 3600} h"
        }
    }

    fun dateTime(ts: Long?): String =
        if (ts == null || ts <= 0) "-" else DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(ts * 1000))

    fun time(ts: Long?): String =
        if (ts == null || ts <= 0) "-" else DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(ts * 1000))

    fun bytes(b: Long?): String {
        if (b == null) return "not reported"
        val units = arrayOf("B", "KB", "MB", "GB", "TB")
        var v = b.toDouble(); var i = 0
        while (v >= 1024 && i < units.size - 1) { v /= 1024; i++ }
        return if (i == 0) "$b B" else String.format(Locale.US, "%.1f %s", v, units[i])
    }

    fun duration(secs: Long?): String {
        if (secs == null) return "not reported"
        val d = secs / 86400; val h = (secs % 86400) / 3600; val m = (secs % 3600) / 60
        return when {
            d > 0 -> "${d}d ${h}h ${m}m"
            h > 0 -> "${h}h ${m}m"
            else -> "${m}m ${secs % 60}s"
        }
    }

    fun osLabel(os: String?) = when (os) {
        "windows" -> "Windows"; "linux" -> "Linux"; "macos" -> "macOS"; "other" -> "Other"; else -> "OS unknown"
    }

    fun statusLabel(s: String) = when (s) {
        "online" -> "Online"; "offline" -> "Offline"; "waking" -> "Waking"; "shutting_down" -> "Shutting down"; else -> "Unknown"
    }

    fun kindLabel(kind: String) = when (kind) {
        "wake" -> "Wake"; "shutdown" -> "Shutdown"; "restart" -> "Restart"; "cancel_shutdown" -> "Cancel shutdown"; else -> kind
    }

    fun wolLabel(d: Device) = when (d.wolCapable) {
        "yes" -> "Wake-on-LAN confirmed"; "no" -> "Wake-on-LAN not working"; else -> "Wake-on-LAN not verified"
    }

    fun agentLabel(d: Device) = when (d.agent?.state) {
        "online" -> "Agent online"; "offline" -> "Agent offline"; else -> "No agent"
    }

    /** True wording for an operation outcome. Only "succeeded" means the computer is up. */
    fun opOutcome(op: Operation): String = when (op.state) {
        "pending" -> "Waiting to start"
        "running" -> when (op.kind) { "wake" -> "Magic packet sent, waiting for the computer to come online"; else -> "In progress" }
        "succeeded" -> when (op.kind) {
            "wake" -> "Computer is online"
            "shutdown" -> "Shutdown confirmed"
            "restart" -> "Restart confirmed"
            else -> "Done"
        }
        "timeout" -> if (op.kind == "wake") "Magic packet sent, but the computer did not come online" else "Timed out"
        "failed" -> "Failed"
        else -> op.state
    }
}
