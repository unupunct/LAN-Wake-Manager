package com.lanwakemanager.app.ui

import com.lanwakemanager.app.api.*
import java.time.LocalDate
import java.time.format.DateTimeParseException

/** Editable form state for a schedule; converts to/from the API model and validates input. */
data class ScheduleDraft(
    val id: String? = null,
    val name: String = "",
    val enabled: Boolean = true,
    val action: String = "wake",
    val deviceIds: Set<String> = emptySet(),
    val groupIds: Set<String> = emptySet(),
    val locationIds: Set<String> = emptySet(),
    val kind: String = "daily",
    val at: String = "07:30",
    val date: String = "",
    val weekdays: Set<Int> = setOf(1, 2, 3, 4, 5),
    val dayOfMonth: String = "1",
    val every: String = "6",
    val unit: String = "hours",
    val startDate: String = "",
    val endDate: String = "",
    val exceptions: List<String> = emptyList(),
    val missedPolicy: String = "skip",
    val graceMinutes: String = "60",
    val retryAttempts: String = "1",
    val retryIntervalSecs: String = "60",
    val executor: String = "controller",
    val delaySecs: String = "0",
) {
    fun validate(): String? {
        if (name.isBlank()) return "Enter a name."
        if (!isTime(at)) return "Time must be HH:MM (24 hour)."
        if (deviceIds.isEmpty() && groupIds.isEmpty() && locationIds.isEmpty()) return "Choose at least one computer, group or location."
        when (kind) {
            "once" -> if (!isDate(date)) return "Enter the date as YYYY-MM-DD."
            "weekly" -> if (weekdays.isEmpty()) return "Choose at least one weekday."
            "monthly" -> if ((dayOfMonth.toIntOrNull() ?: 0) !in 1..31) return "Day of month must be 1 to 31."
            "custom" -> if ((every.toIntOrNull() ?: 0) < 1) return "Repeat interval must be 1 or more."
        }
        if (startDate.isNotBlank() && !isDate(startDate)) return "Start date must be YYYY-MM-DD."
        if (endDate.isNotBlank() && !isDate(endDate)) return "End date must be YYYY-MM-DD."
        if (startDate.isNotBlank() && endDate.isNotBlank() && LocalDate.parse(startDate) > LocalDate.parse(endDate)) return "The end date is before the start date."
        if ((graceMinutes.toIntOrNull() ?: -1) < 0) return "Missed-run grace must be 0 or more minutes."
        if ((retryAttempts.toIntOrNull() ?: 0) !in 1..10) return "Retry attempts must be 1 to 10."
        if ((retryIntervalSecs.toIntOrNull() ?: -1) < 0) return "Retry interval must be 0 or more seconds."
        if ((delaySecs.toIntOrNull() ?: -1) < 0) return "Delay must be 0 or more seconds."
        if (executor == "relay" && (action != "wake" || locationIds.size != 1 || deviceIds.isNotEmpty() || groupIds.isNotEmpty()))
            return "The relay can only run wake schedules that target exactly one location."
        return null
    }

    fun toSchedule(): Schedule = Schedule(
        id = id, name = name.trim(), enabled = enabled, action = action,
        target = ScheduleTarget(deviceIds.toList(), groupIds.toList(), locationIds.toList()),
        recurrence = Recurrence(
            kind = kind, at = at,
            date = if (kind == "once") date else null,
            weekdays = if (kind == "weekly") weekdays.sorted() else emptyList(),
            dayOfMonth = if (kind == "monthly") dayOfMonth.toIntOrNull() else null,
            interval = if (kind == "custom") IntervalSpec(every.toIntOrNull() ?: 1, unit) else null,
        ),
        startDate = startDate.ifBlank { null }, endDate = endDate.ifBlank { null },
        exceptions = exceptions, missedPolicy = missedPolicy, missedGraceMinutes = graceMinutes.toIntOrNull() ?: 60,
        retry = RetryPolicy(retryAttempts.toIntOrNull() ?: 1, retryIntervalSecs.toIntOrNull() ?: 60),
        executor = executor, delaySecs = delaySecs.toIntOrNull() ?: 0,
    )

    companion object {
        fun isTime(s: String) = Regex("^([01]\\d|2[0-3]):[0-5]\\d$").matches(s)
        fun isDate(s: String) = try { LocalDate.parse(s); true } catch (_: DateTimeParseException) { false }

        fun from(s: Schedule?): ScheduleDraft = if (s == null) ScheduleDraft() else ScheduleDraft(
            id = s.id, name = s.name, enabled = s.enabled, action = s.action,
            deviceIds = s.target.deviceIds.toSet(), groupIds = s.target.groupIds.toSet(), locationIds = s.target.locationIds.toSet(),
            kind = s.recurrence.kind, at = s.recurrence.at ?: "07:30", date = s.recurrence.date ?: "",
            weekdays = s.recurrence.weekdays.toSet(), dayOfMonth = (s.recurrence.dayOfMonth ?: 1).toString(),
            every = (s.recurrence.interval?.every ?: 6).toString(), unit = s.recurrence.interval?.unit ?: "hours",
            startDate = s.startDate ?: "", endDate = s.endDate ?: "", exceptions = s.exceptions,
            missedPolicy = s.missedPolicy, graceMinutes = s.missedGraceMinutes.toString(),
            retryAttempts = s.retry.maxAttempts.toString(), retryIntervalSecs = s.retry.intervalSecs.toString(),
            executor = s.executor, delaySecs = s.delaySecs.toString(),
        )
    }
}

object ScheduleText {
    private val days = listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

    fun recurrence(r: Recurrence): String {
        val at = r.at?.let { " at $it" } ?: ""
        return when (r.kind) {
            "once" -> "Once on ${r.date ?: "?"}$at"
            "daily" -> "Every day$at"
            "weekly" -> "Weekly on " + r.weekdays.sorted().joinToString(", ") { days.getOrElse(it - 1) { "?" } } + at
            "monthly" -> "Monthly on day ${r.dayOfMonth ?: "?"}$at"
            "custom" -> "Every ${r.interval?.every ?: "?"} ${r.interval?.unit ?: ""}$at"
            else -> r.kind
        }
    }

    fun target(t: ScheduleTarget, s: AppUiState): String {
        val parts = mutableListOf<String>()
        t.deviceIds.mapNotNull { id -> s.data.devices.firstOrNull { it.id == id }?.name }.takeIf { it.isNotEmpty() }?.let { parts += it.joinToString() }
        t.groupIds.mapNotNull { s.group(it)?.name }.takeIf { it.isNotEmpty() }?.let { parts += "group " + it.joinToString() }
        t.locationIds.mapNotNull { s.location(it)?.name }.takeIf { it.isNotEmpty() }?.let { parts += "location " + it.joinToString() }
        return parts.joinToString("; ").ifBlank { "no target" }
    }
}
