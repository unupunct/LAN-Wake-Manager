package com.lanwakemanager.app.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lanwakemanager.app.api.*

enum class PairStep { INPUT, PROBING, CONFIRM, PAIRING, DONE }

data class PairingUiState(
    val step: PairStep = PairStep.INPUT,
    val manual: Boolean = false,
    val manualAddr: String = "",
    val manualFp: String = "",
    val manualToken: String = "",
    val controllerId: String = "",
    val controllerName: String = "",
    val fingerprint: String = "",
    val addresses: List<ControllerAddress> = emptyList(),
    val chosen: ControllerAddress? = null,
    val latencyMs: Long? = null,
    val error: String? = null,
    val cameraDenied: Boolean = false,
    val modeNote: String? = null,
)

interface PairingActions {
    fun setManual(on: Boolean) {}
    fun editManual(addr: String? = null, fp: String? = null, token: String? = null) {}
    fun submitManual() {}
    fun confirm() {}
    fun reset() {}
}

object NoPairingActions : PairingActions

@Composable
fun AddControllerScreen(
    st: PairingUiState, actions: PairingActions, mode: ConnMode, onScan: () -> Unit, onDone: () -> Unit, modifier: Modifier = Modifier,
) {
    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        when (st.step) {
            PairStep.INPUT -> {
                LwmCard {
                    Text("Pair with a controller", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                    Text("On the controller's dashboard open Mobile access, choose a role and create a pairing code. The code is single-use and expires in a few minutes.",
                        style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                    Text("Connection mode: ${when (mode) { ConnMode.AUTOMATIC -> "Automatic"; ConnMode.VPN_ONLY -> "VPN only"; ConnMode.LAN_ONLY -> "LAN only" }}. Make sure your VPN is connected if the controller is away from this network.",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                    Spacer(Modifier.height(10.dp))
                    if (!st.manual) {
                        Button(onClick = onScan, modifier = Modifier.fillMaxWidth()) { Icon(Icons.Filled.QrCodeScanner, null); Spacer(Modifier.width(8.dp)); Text("Scan QR code") }
                        if (st.cameraDenied) Text("Camera permission was denied. Allow it in Android settings or use manual setup.", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(top = 6.dp))
                        TextButton(onClick = { actions.setManual(true) }, modifier = Modifier.fillMaxWidth()) { Text("Enter details manually") }
                    }
                }
                if (st.manual) {
                    LwmCard {
                        Text("Manual setup", fontWeight = FontWeight.SemiBold)
                        OutlinedTextField(st.manualAddr, { actions.editManual(addr = it) }, label = { Text("Address (e.g. 100.64.1.2:8443)") }, singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
                        OutlinedTextField(st.manualFp, { actions.editManual(fp = it) }, label = { Text("Certificate fingerprint (SHA-256 hex)") }, minLines = 2, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
                        OutlinedTextField(st.manualToken, { actions.editManual(token = it) }, label = { Text("Pairing token") }, singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
                        Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(onClick = actions::submitManual, enabled = st.manualAddr.isNotBlank() && st.manualFp.isNotBlank() && st.manualToken.isNotBlank()) { Text("Connect") }
                            TextButton(onClick = { actions.setManual(false) }) { Text("Use QR instead") }
                        }
                    }
                }
                st.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            }
            PairStep.PROBING -> LwmCard {
                Text("Contacting the controller...", fontWeight = FontWeight.SemiBold)
                Text("Checking its addresses with the pinned certificate.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 12.dp))
            }
            PairStep.CONFIRM, PairStep.PAIRING -> {
                LwmCard {
                    Text("Confirm pairing", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.height(6.dp))
                    InfoRow("Controller", st.controllerName.ifBlank { "unnamed" })
                    InfoRow("Controller ID", st.controllerId.ifBlank { "(reported on connect)" })
                    InfoRow("Fingerprint", Fingerprint.short(st.fingerprint))
                    InfoRow("Connected via", st.chosen?.let { "${kindLabel(it.kind)}: ${it.url}" })
                    st.latencyMs?.let { InfoRow("Response time", "$it ms") }
                    Text("Check that this fingerprint matches the one shown on the controller before continuing. This phone will be able to control the computers your role allows.",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
                    st.error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 6.dp)) }
                    Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = actions::confirm, enabled = st.step == PairStep.CONFIRM) { Text(if (st.step == PairStep.PAIRING) "Pairing..." else "Pair this phone") }
                        TextButton(onClick = actions::reset, enabled = st.step == PairStep.CONFIRM) { Text("Cancel") }
                    }
                    if (st.step == PairStep.PAIRING) LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 8.dp))
                }
            }
            PairStep.DONE -> LwmCard {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Filled.CheckCircle, null, tint = LocalStatusColors.current.online); Spacer(Modifier.width(10.dp))
                    Text("Paired with ${st.controllerName}", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                }
                Text("The device token is stored encrypted on this phone. You will not need to pair again.", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
                Button(onClick = onDone, modifier = Modifier.padding(top = 12.dp)) { Text("Open dashboard") }
            }
        }
    }
}
