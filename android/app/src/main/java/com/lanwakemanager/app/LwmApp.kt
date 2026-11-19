package com.lanwakemanager.app

import android.app.Application
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import com.lanwakemanager.app.data.*

/** Manual dependency graph (no DI framework). */
class AppGraph(app: Application) {
    val controllers = ControllerStore(app.filesDir, KeystoreCipher())
    val settings = SettingsStore(app.filesDir)
    val cache = CacheStore(java.io.File(app.filesDir, "cache"))
    val net = NetInfo(app)
    val notifier = Notifier(app, settings)
    val repository = LwmRepository(app, controllers, settings, cache, net, notifier)
}

class LwmApp : Application() {
    lateinit var graph: AppGraph
        private set

    override fun onCreate() {
        super.onCreate()
        graph = AppGraph(this)
        graph.notifier.createChannels()
        PollWorker.schedule(this)
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) { graph.repository.setForeground(true) }
            override fun onStop(owner: LifecycleOwner) { graph.repository.setForeground(false) }
        })
    }
}
