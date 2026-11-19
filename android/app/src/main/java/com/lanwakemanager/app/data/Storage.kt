package com.lanwakemanager.app.data

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import com.lanwakemanager.app.api.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Encrypts small secrets (device tokens). */
interface SecretCipher {
    fun encrypt(plain: String): String
    fun decrypt(encoded: String): String
}

/** AES-256-GCM with a non-exportable key in the Android Keystore. Output: base64(iv || ciphertext+tag). */
class KeystoreCipher(private val alias: String = "lwm_token_key") : SecretCipher {
    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(alias, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return gen.generateKey()
    }

    override fun encrypt(plain: String): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        val ct = c.doFinal(plain.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(c.iv + ct, Base64.NO_WRAP)
    }

    override fun decrypt(encoded: String): String {
        val all = Base64.decode(encoded, Base64.NO_WRAP)
        val iv = all.copyOfRange(0, 12)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, iv))
        return String(c.doFinal(all, 12, all.size - 12), Charsets.UTF_8)
    }
}

/** Small JSON file helper with atomic replace. */
class JsonFile<T>(private val file: File, private val ser: KSerializer<T>) {
    fun read(): T? = try {
        if (file.exists()) LwmJson.decodeFromString(ser, file.readText()) else null
    } catch (e: Exception) { null }

    @Synchronized
    fun write(value: T) {
        file.parentFile?.mkdirs()
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeText(LwmJson.encodeToString(ser, value))
        if (!tmp.renameTo(file)) { file.delete(); tmp.renameTo(file) }
    }

    fun delete() { file.delete() }
}

// ------------------------------------------------------------------ controllers

@Serializable
data class ControllerRecord(
    val id: String,
    val name: String,
    val addresses: List<ControllerAddress>,
    val fingerprint: String,
    val role: String = "viewer",
    val deviceId: String? = null,
    /** Base64 AES-GCM ciphertext of the device token; never the token itself. */
    val tokenEnc: String,
    val pairedAt: Long = System.currentTimeMillis() / 1000,
)

@Serializable
private data class ControllerFile(val controllers: List<ControllerRecord> = emptyList())

class ControllerStore(dir: File, private val cipher: SecretCipher) {
    private val file = JsonFile(File(dir, "controllers.json"), ControllerFile.serializer())
    private val _list = MutableStateFlow(file.read()?.controllers ?: emptyList())
    val controllers: StateFlow<List<ControllerRecord>> = _list.asStateFlow()

    private fun persist() = file.write(ControllerFile(_list.value))

    fun get(id: String?): ControllerRecord? = _list.value.firstOrNull { it.id == id }

    fun token(r: ControllerRecord): String? = try { cipher.decrypt(r.tokenEnc) } catch (e: Exception) { null }

    fun encryptToken(token: String): String = cipher.encrypt(token)

    /** Adds or replaces (re-pairing) a controller. */
    @Synchronized
    fun save(r: ControllerRecord) {
        _list.update { l -> l.filter { it.id != r.id } + r }
        persist()
    }

    @Synchronized
    fun update(id: String, block: (ControllerRecord) -> ControllerRecord) {
        _list.update { l -> l.map { if (it.id == id) block(it) else it } }
        persist()
    }

    @Synchronized
    fun remove(id: String) {
        _list.update { l -> l.filter { it.id != id } }
        persist()
    }
}

// ------------------------------------------------------------------ settings

object NotifyType {
    const val OPERATIONS = "operations"
    const val DEVICES = "devices"
    const val RELAY = "relay"
    const val CONTROLLER = "controller"
    const val SCHEDULE = "schedule"
    val all = listOf(OPERATIONS, DEVICES, RELAY, CONTROLLER, SCHEDULE)

    /** Maps a controller notification type to our channel category. */
    fun categoryOf(apiType: String): String = when (apiType) {
        "wake_completed", "shutdown_executed", "restart_executed", "operation_failed" -> OPERATIONS
        "device_unavailable" -> DEVICES
        "relay_offline", "relay_online" -> RELAY
        "schedule_approaching" -> SCHEDULE
        else -> CONTROLLER
    }
}

@Serializable
data class AppSettings(
    val connectionMode: ConnMode = ConnMode.AUTOMATIC,
    /** "system" | "dark" | "light" */
    val theme: String = "dark",
    val refreshSecs: Int = 30,
    val notify: Map<String, Boolean> = emptyMap(),
    val appLock: Boolean = false,
    val lastControllerId: String? = null,
    val selectedLocation: Map<String, String> = emptyMap(),
    val lastNotificationId: Map<String, Long> = emptyMap(),
    /** controller id -> true when the last background poll reached the controller */
    val reachable: Map<String, Boolean> = emptyMap(),
) {
    fun notifyEnabled(type: String) = notify[type] ?: true
}

class SettingsStore(dir: File) {
    private val file = JsonFile(File(dir, "settings.json"), AppSettings.serializer())
    private val _s = MutableStateFlow(file.read() ?: AppSettings())
    val settings: StateFlow<AppSettings> = _s.asStateFlow()

    @Synchronized
    fun update(block: (AppSettings) -> AppSettings) {
        _s.update(block)
        file.write(_s.value)
    }
}

// ------------------------------------------------------------------ cache

@Serializable
data class ControllerCache(
    val savedAt: Long = 0,
    val role: String? = null,
    val dashboard: Dashboard? = null,
    val devices: List<Device> = emptyList(),
    val locations: List<Location> = emptyList(),
    val groups: List<Group> = emptyList(),
    val schedules: List<Schedule> = emptyList(),
    val operations: List<Operation> = emptyList(),
)

class CacheStore(private val dir: File) {
    private fun f(id: String) = JsonFile(File(dir, "cache_${id.filter { it.isLetterOrDigit() || it == '-' || it == '_' }}.json"), ControllerCache.serializer())
    fun load(id: String): ControllerCache? = f(id).read()
    fun save(id: String, c: ControllerCache) = f(id).write(c)
    fun delete(id: String) = f(id).delete()
}
