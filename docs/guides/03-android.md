# Android app: install, pair, use from anywhere

## Install the APK

1. Download `LAN-Wake-Manager.apk` from the [latest release](https://github.com/unupunct/LAN-Wake-Manager/releases/latest) on the phone (Android 8.0+).
2. Open it. Android asks to allow installs from that source (your browser/files app): allow it, then *Install*. The APK is signed with the project's own key (not from Google Play).
3. Updates: install the next APK over the old one; it keeps your controllers (same signing key required).

Adb alternative: `adb install LAN-Wake-Manager.apk`.

## Pair with a controller

1. Make sure the phone can reach the controller (Tailscale/WireGuard on, or same Wi-Fi) — see the [Tailscale](01-tailscale.md) / [WireGuard](02-wireguard.md) guides.
2. Dashboard (on the controller): **Mobile Access → Add mobile device**, choose a role (*Operator* is right for most phones), *Show pairing QR*.
   The code is valid for 5 minutes and works once. It contains the controller's addresses, its certificate fingerprint and a one-time token — no password.
3. App: **Add controller → Scan QR**. The app connects, shows the controller name and certificate fingerprint; confirm to pair. Prefer typing? Use *Manual*: address, fingerprint, pairing code.
4. Done. The phone remembers the controller (token stored encrypted with the Android Keystore). You do not pair again when you reopen the app.

Revoke a phone any time: Dashboard → Mobile Access → *Revoke*. Its token stops working immediately.

## Using it away from home

* **Connection mode** (Settings): *Automatic* (prefers the local LAN, then Tailscale, then WireGuard), *VPN only*, or *LAN only*. The status chip on every screen says how you are connected:
  "Connected through Tailscale / WireGuard / Local LAN", "Disconnected", "Controller unavailable". The app never silently drops to an unencrypted route; all connections are HTTPS with a pinned certificate.
* **Wake a sleeping PC**: Computers → tap *Wake*. The app shows the real result: **Online** only when the relay confirmed that the computer answered; "magic packet sent, but the computer did not come online" otherwise.
* **Shutdown / Restart**: needs the agent (or SSH) on that computer; always asks for confirmation, bulk actions list every affected computer.
* **Offline**: the last known data stays visible, clearly labelled *Cached – x minutes ago*. The app reconnects automatically when connectivity returns and refreshes.
* **Notifications** are local notifications created while the phone can reach the controller (live connection while the app is open, plus a check about every 15 minutes in the background). They cannot arrive while the phone has no network path to the controller.

## Building and signing yourself

```powershell
cd android
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.21.9-hotspot"
.\gradlew assembleDebug assembleRelease
```
Create your own signing key (never commit it):
```bash
keytool -genkeypair -v -keystore lanwake.jks -alias lanwake -keyalg RSA -keysize 4096 -validity 10000
```
Put `storeFile`, `storePassword`, `keyAlias`, `keyPassword` into `android/keystore.properties` (git-ignored). Without that file the release build is unsigned.
Verify: `apksigner verify --verbose app/build/outputs/apk/release/app-release.apk`. Back up the key: Android refuses updates signed with a different key.
