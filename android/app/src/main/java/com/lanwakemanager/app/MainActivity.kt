package com.lanwakemanager.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import com.lanwakemanager.app.ui.*

class MainActivity : FragmentActivity() {
    private val vm: MainViewModel by viewModels()
    private val pairingVm: PairingViewModel by viewModels()

    private var locked by mutableStateOf(false)
    private var stoppedAt = 0L
    private var promptShowing = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        locked = vm.ui.value.settings.appLock
        setContent {
            val s by vm.ui.collectAsStateWithLifecycle()
            val pairing by pairingVm.state.collectAsStateWithLifecycle()
            val snackbar = remember { SnackbarHostState() }
            LaunchedEffect(Unit) { vm.snackbar.collect { snackbar.showSnackbar(it) } }

            val notifLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { vm.refreshDeviceState() }
            LaunchedEffect(Unit) {
                vm.permissionRequests.collect { if (Build.VERSION.SDK_INT >= 33) notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS) }
            }
            // Ask once on first start (API 33+) so notifications can work; the user can decline.
            LaunchedEffect(s.active?.id) {
                if (s.active != null && Build.VERSION.SDK_INT >= 33 && !s.notificationsAllowed && !asked) { asked = true; notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS) }
            }

            val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { r -> r.contents?.let(pairingVm::onScanned) }
            val cameraLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
                if (granted) scanLauncher.launch(scanOptions()) else pairingVm.cameraDenied()
            }
            val onScan = {
                if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) scanLauncher.launch(scanOptions())
                else cameraLauncher.launch(Manifest.permission.CAMERA)
            }

            LwmTheme(s.settings.theme) {
                LaunchedEffect(s.settings.appLock) {
                    if (s.settings.appLock) window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
                    else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                }
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    if (locked && s.settings.appLock) {
                        LockScreen(onUnlock = ::promptUnlock)
                        LaunchedEffect(Unit) { promptUnlock() }
                    } else {
                        LwmRoot(s, vm, pairing, pairingVm, onScanQr = onScan, snackbarHost = snackbar)
                    }
                }
            }
        }
    }

    private var asked = false

    private fun scanOptions() = ScanOptions()
        .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
        .setPrompt("Scan the pairing code shown on the controller")
        .setBeepEnabled(false)
        .setOrientationLocked(false)

    private fun promptUnlock() {
        if (promptShowing) return
        promptShowing = true
        val prompt = BiometricPrompt(this, ContextCompat.getMainExecutor(this), object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) { promptShowing = false; locked = false }
            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) { promptShowing = false }
        })
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Unlock LAN Wake Manager")
            .setAllowedAuthenticators(BIOMETRIC_WEAK or DEVICE_CREDENTIAL)
            .build()
        try { prompt.authenticate(info) } catch (_: Exception) { promptShowing = false }
    }

    override fun onStart() {
        super.onStart()
        if (vm.ui.value.settings.appLock && stoppedAt != 0L && System.currentTimeMillis() - stoppedAt > 15_000) locked = true
        vm.refreshDeviceState()
    }

    override fun onStop() {
        super.onStop()
        stoppedAt = System.currentTimeMillis()
    }
}

@Composable
private fun LockScreen(onUnlock: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(Icons.Filled.Lock, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(56.dp))
        Spacer(Modifier.height(16.dp))
        Text("LAN Wake Manager is locked", style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(16.dp))
        Button(onClick = onUnlock) { Text("Unlock") }
    }
}
