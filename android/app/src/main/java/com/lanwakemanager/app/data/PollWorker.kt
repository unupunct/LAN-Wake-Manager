package com.lanwakemanager.app.data

import android.content.Context
import androidx.work.*
import com.lanwakemanager.app.AppGraph
import com.lanwakemanager.app.LwmApp
import com.lanwakemanager.app.api.ApiException
import com.lanwakemanager.app.api.isCertificateProblem
import java.util.concurrent.TimeUnit

/**
 * Every 15 minutes: reach each saved controller with the same connection logic as the app and post local
 * notifications for anything new from GET /notifications. Only works while the phone has a network path to the
 * controller (LAN / VPN); there is no push service.
 */
class PollWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {

    override suspend fun doWork(): Result {
        val g = (applicationContext as LwmApp).graph
        val repo = g.repository
        for (rec in g.controllers.controllers.value) {
            try {
                // The foreground WebSocket already covers the active controller while the app is open.
                if (repo.isLiveFor(rec.id)) continue
                pollOne(g, rec)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (_: Exception) { }
        }
        return Result.success()
    }

    private suspend fun pollOne(g: AppGraph, rec: ControllerRecord) {
        val settings = g.settings
        val s = settings.settings.value
        val token = g.controllers.token(rec)
        val r = ConnectionResolver.resolve(rec, token, s.connectionMode, g.net.read())
        val api = r.api
        val wasReachable = s.reachable[rec.id]
        if (api == null || token == null) {
            settings.update { it.copy(reachable = it.reachable + (rec.id to false)) }
            if (wasReachable == true) {
                val cert = r.probes.any { it.certProblem }
                g.notifier.postRaw(
                    NotifyType.CONTROLLER, ("down" + rec.id).hashCode(),
                    if (cert) "Controller certificate changed" else "Controller unavailable",
                    if (cert) "${rec.name}: the certificate no longer matches the pinned fingerprint."
                    else "${rec.name} cannot be reached from this phone right now (VPN off, away from the LAN, or the controller is offline).",
                    important = true,
                )
            }
            return
        }
        settings.update { it.copy(reachable = it.reachable + (rec.id to true)) }
        val last = s.lastNotificationId[rec.id]
        try {
            if (last == null) {
                // First poll: establish a baseline so old history does not flood the phone.
                val max = api.notifications(0, 50).maxOfOrNull { it.id } ?: 0L
                settings.update { it.copy(lastNotificationId = it.lastNotificationId + (rec.id to max)) }
                return
            }
            val items = api.notifications(last, 50).sortedBy { it.id }
            var newest: Long = last
            for (n in items) {
                if (n.id > newest) newest = n.id
                g.notifier.post(rec.id, rec.name, n)
            }
            if (newest != last) settings.update { it.copy(lastNotificationId = it.lastNotificationId + (rec.id to newest)) }
        } catch (e: ApiException) {
            if (e.unauthorized && wasReachable != false) {
                settings.update { it.copy(reachable = it.reachable + (rec.id to false)) }
                g.notifier.postRaw(NotifyType.CONTROLLER, ("revoked" + rec.id).hashCode(), "Pairing revoked",
                    "${rec.name} no longer accepts this phone. Open the app and pair again.", important = true)
            }
        }
    }

    companion object {
        private const val NAME = "lwm_poll"
        fun schedule(context: Context) {
            // No network constraint: a LAN/VPN-only network without Internet must still be polled.
            val req = PeriodicWorkRequestBuilder<PollWorker>(15, TimeUnit.MINUTES)
                .setConstraints(Constraints.NONE)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.KEEP, req)
        }
    }
}
