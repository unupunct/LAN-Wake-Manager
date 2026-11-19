package com.lanwakemanager.app.api

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.io.IOException
import javax.net.ssl.SSLException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** A well-formed HTTP error answer from the controller. */
class ApiException(val status: Int, val code: String, override val message: String) : IOException(message) {
    val unauthorized get() = status == 401
    val forbidden get() = status == 403
}

/** True when a failure is a TLS certificate problem (fingerprint mismatch) rather than "unreachable". */
fun Throwable.isCertificateProblem(): Boolean {
    var t: Throwable? = this
    while (t != null) {
        if (t is java.security.cert.CertificateException) return true
        if (t is SSLException && (t.message ?: "").contains("fingerprint", ignoreCase = true)) return true
        t = t.cause
    }
    return false
}

/**
 * REST client for one controller at one base URL (e.g. https://100.64.1.2:8443), with TLS pinned to the
 * controller's fingerprint. All calls are suspend functions executed off the main thread.
 */
class ControllerApi(
    baseUrl: String,
    val fingerprint: String,
    var token: String? = null,
    val client: OkHttpClient = PinnedClient.create(fingerprint),
) {
    val base: String = Addresses.normalizeUrl(baseUrl)
    private val json = LwmJson
    private val jsonType = "application/json; charset=utf-8".toMediaType()

    /** Same pinned client family, different timeouts (used for quick LAN probes). */
    fun withTimeouts(connectMs: Long, readMs: Long): ControllerApi =
        ControllerApi(base, fingerprint, token, client.newBuilder()
            .connectTimeout(connectMs, java.util.concurrent.TimeUnit.MILLISECONDS)
            .readTimeout(readMs, java.util.concurrent.TimeUnit.MILLISECONDS).build())

    private fun url(path: String) = "$base/api/v1$path"

    private suspend fun Call.await(): Response = suspendCancellableCoroutine { cont ->
        enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) { if (cont.isActive) cont.resumeWithException(e) }
            override fun onResponse(call: Call, response: Response) { cont.resume(response) }
        })
        cont.invokeOnCancellation { try { cancel() } catch (_: Throwable) {} }
    }

    private suspend fun <T> exec(method: String, path: String, body: String?, ser: KSerializer<T>?, auth: Boolean = true): T? =
        withContext(Dispatchers.IO) {
            val rb = Request.Builder().url(url(path))
            if (auth) token?.let { rb.header("Authorization", "Bearer $it") }
            rb.header("Accept", "application/json")
            val reqBody = when {
                body != null -> body.toRequestBody(jsonType)
                method == "POST" || method == "PUT" || method == "PATCH" -> "{}".toRequestBody(jsonType)
                else -> null
            }
            rb.method(method, reqBody)
            val resp = client.newCall(rb.build()).await()
            resp.use { r ->
                val text = r.body?.string().orEmpty()
                if (!r.isSuccessful) {
                    val err = try { json.decodeFromString(ApiErrorBody.serializer(), text).error } catch (_: Exception) { null }
                    throw ApiException(r.code, err?.code ?: "http_${r.code}", err?.message?.ifBlank { null } ?: "HTTP ${r.code}")
                }
                if (ser == null || text.isBlank()) null else json.decodeFromString(ser, text)
            }
        }

    private suspend fun <T> get(path: String, ser: KSerializer<T>, auth: Boolean = true): T =
        exec("GET", path, null, ser, auth) ?: throw IOException("Empty response")

    private suspend fun <T, B> send(method: String, path: String, bodySer: KSerializer<B>, body: B, ser: KSerializer<T>): T =
        exec(method, path, json.encodeToString(bodySer, body), ser) ?: throw IOException("Empty response")

    // ---- public
    suspend fun info(): InfoResponse = get("/info", InfoResponse.serializer(), auth = false)

    suspend fun pair(req: PairRequest): PairResponse =
        exec("POST", "/pair", json.encodeToString(PairRequest.serializer(), req), PairResponse.serializer(), auth = false)
            ?: throw IOException("Empty response")

    // ---- read
    suspend fun me() = get("/me", MeResponse.serializer())
    suspend fun dashboard() = get("/dashboard", Dashboard.serializer())
    suspend fun locations() = get("/locations", ListSerializer(Location.serializer()))
    suspend fun devices(locationId: String? = null) =
        get("/devices" + (locationId?.let { "?location_id=$it" } ?: ""), ListSerializer(Device.serializer()))
    suspend fun device(id: String) = get("/devices/$id", Device.serializer())
    suspend fun refreshDevice(id: String): Device = exec("POST", "/devices/$id/refresh", "{}", Device.serializer())!!
    suspend fun troubleshoot(id: String) = get("/devices/$id/troubleshoot", Troubleshoot.serializer())
    suspend fun groups() = get("/groups", ListSerializer(Group.serializer()))
    suspend fun schedules() = get("/schedules", ListSerializer(Schedule.serializer()))
    suspend fun operations(limit: Int = 50, deviceId: String? = null): List<Operation> =
        get("/operations?limit=$limit" + (deviceId?.let { "&device_id=$it" } ?: ""), ListSerializer(Operation.serializer()))
    suspend fun operation(id: String) = get("/operations/$id", Operation.serializer())
    suspend fun connectivity() = get("/connectivity", Connectivity.serializer())
    suspend fun diagnostics(locationId: String? = null) =
        get("/diagnostics" + (locationId?.let { "?location_id=$it" } ?: ""), DiagnosticsReport.serializer())
    suspend fun notifications(afterId: Long, limit: Int = 50) =
        get("/notifications?after=$afterId&limit=$limit", ListSerializer(NotificationItem.serializer()))

    // ---- power operations
    suspend fun wake(deviceId: String): Operation =
        exec("POST", "/devices/$deviceId/wake", "{}", Operation.serializer())!!

    suspend fun shutdown(deviceId: String, delaySecs: Int = 0): Operation =
        send("POST", "/devices/$deviceId/shutdown", DelayBody.serializer(), DelayBody(delaySecs), Operation.serializer())

    suspend fun restart(deviceId: String, delaySecs: Int = 0): Operation =
        send("POST", "/devices/$deviceId/restart", DelayBody.serializer(), DelayBody(delaySecs), Operation.serializer())

    suspend fun cancelShutdown(deviceId: String): Operation =
        exec("POST", "/devices/$deviceId/cancel-shutdown", "{}", Operation.serializer())!!

    suspend fun bulk(req: BulkRequest): List<Operation> =
        send("POST", "/bulk", BulkRequest.serializer(), req, BulkResponse.serializer()).operations

    suspend fun groupAction(groupId: String, req: GroupActionRequest): List<Operation> =
        send("POST", "/groups/$groupId/action", GroupActionRequest.serializer(), req, BulkResponse.serializer()).operations

    // ---- groups / devices (admin)
    suspend fun createGroup(g: GroupInput): Group = send("POST", "/groups", GroupInput.serializer(), g, Group.serializer())
    suspend fun updateGroup(id: String, g: GroupInput): Group = send("PUT", "/groups/$id", GroupInput.serializer(), g, Group.serializer())
    suspend fun deleteGroup(id: String) { exec<Unit>("DELETE", "/groups/$id", null, null) }
    suspend fun updateDevice(id: String, d: DeviceInput): Device = send("PUT", "/devices/$id", DeviceInput.serializer(), d, Device.serializer())

    // ---- schedules
    suspend fun createSchedule(s: Schedule): Schedule = send("POST", "/schedules", Schedule.serializer(), s, Schedule.serializer())
    suspend fun updateSchedule(id: String, s: Schedule): Schedule = send("PUT", "/schedules/$id", Schedule.serializer(), s, Schedule.serializer())
    suspend fun deleteSchedule(id: String) { exec<Unit>("DELETE", "/schedules/$id", null, null) }
    suspend fun runScheduleNow(id: String) { exec<Unit>("POST", "/schedules/$id/run-now", "{}", null) }

    // ---- websocket
    /** Opens /events; auth is the first client message. */
    fun openEvents(listener: WebSocketListener): WebSocket {
        val req = Request.Builder().url("$base/api/v1/events".toHttpUrl()).build()
        val tok = token.orEmpty()
        return client.newWebSocket(req, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                webSocket.send(json.encodeToString(kotlinx.serialization.json.JsonObject.serializer(),
                    kotlinx.serialization.json.buildJsonObject {
                        put("type", kotlinx.serialization.json.JsonPrimitive("auth"))
                        put("token", kotlinx.serialization.json.JsonPrimitive(tok))
                    }))
                listener.onOpen(webSocket, response)
            }
            override fun onMessage(webSocket: WebSocket, text: String) = listener.onMessage(webSocket, text)
            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) = listener.onClosing(webSocket, code, reason)
            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = listener.onClosed(webSocket, code, reason)
            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = listener.onFailure(webSocket, t, response)
        })
    }
}

/** Human readable message for any failure from the API layer. */
fun Throwable.userMessage(): String = when {
    this is CancellationException -> "Cancelled"
    this is ApiException -> when (status) {
        401 -> "The controller no longer accepts this device (token revoked). Pair again."
        403 -> "Your role is not allowed to do this."
        429 -> "Too many requests. Try again in a moment."
        else -> message
    }
    isCertificateProblem() -> "The controller's certificate does not match the pinned fingerprint."
    this is java.net.SocketTimeoutException -> "The controller did not answer in time."
    this is java.net.ConnectException -> "Connection refused or unreachable."
    this is java.net.UnknownHostException -> "Unknown host."
    else -> message ?: javaClass.simpleName
}
