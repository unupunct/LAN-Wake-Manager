package com.lanwakemanager.app.data

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.lanwakemanager.app.MainActivity
import com.lanwakemanager.app.R
import com.lanwakemanager.app.api.NotificationItem

/** Local notifications only (no push service). One channel per category. */
class Notifier(private val context: Context, private val settings: SettingsStore) {

    private data class Chan(val id: String, val name: String, val desc: String, val importance: Int)

    private val channels = listOf(
        Chan(NotifyType.OPERATIONS, "Operation results", "Wake, shutdown and restart results", NotificationManager.IMPORTANCE_DEFAULT),
        Chan(NotifyType.DEVICES, "Computer unavailable", "A computer stopped responding", NotificationManager.IMPORTANCE_DEFAULT),
        Chan(NotifyType.RELAY, "Relay status", "A location relay went offline or came back", NotificationManager.IMPORTANCE_DEFAULT),
        Chan(NotifyType.CONTROLLER, "Controller unavailable", "The controller cannot be reached from this phone", NotificationManager.IMPORTANCE_DEFAULT),
        Chan(NotifyType.SCHEDULE, "Schedule approaching", "A schedule is about to run", NotificationManager.IMPORTANCE_LOW),
    )

    fun createChannels() {
        if (Build.VERSION.SDK_INT < 26) return
        val nm = context.getSystemService(NotificationManager::class.java)
        channels.forEach { nm.createNotificationChannel(NotificationChannel(it.id, it.name, it.importance).apply { description = it.desc }) }
    }

    fun hasPermission(): Boolean =
        Build.VERSION.SDK_INT < 33 ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    /** Posts an item from the controller if its category is enabled. */
    fun post(controllerId: String, controllerName: String, n: NotificationItem) {
        val cat = NotifyType.categoryOf(n.type)
        postRaw(cat, ("c" + controllerId + n.id).hashCode(), n.title.ifBlank { "LAN Wake Manager" },
            listOfNotNull(n.body, "Controller: $controllerName").joinToString("\n"), n.severity == "error")
    }

    fun postRaw(category: String, id: Int, title: String, text: String, important: Boolean = false) {
        if (!settings.settings.value.notifyEnabled(category) || !hasPermission()) return
        val intent = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        val pi = PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val b = NotificationCompat.Builder(context, category)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(text.lineSequence().firstOrNull().orEmpty())
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setContentIntent(pi)
            .setAutoCancel(true)
            .setPriority(if (important) NotificationCompat.PRIORITY_HIGH else NotificationCompat.PRIORITY_DEFAULT)
        try {
            NotificationManagerCompat.from(context).notify(id, b.build())
        } catch (_: SecurityException) { }
    }
}
