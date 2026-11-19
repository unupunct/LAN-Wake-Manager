## LAN Wake Manager 1.0.0

First release. Remote Wake-on-LAN, shutdown and restart for Windows and Linux computers, controlled from a native Android app through your own Tailscale or WireGuard VPN.

### Downloads

| File | For |
|---|---|
| `LAN-Wake-Manager-Setup.msi` | Windows: controller service + desktop window |
| `LAN-Wake-Manager-Relay-Setup.msi` | Windows: Wake Relay service for another LAN |
| `LAN-Wake-Manager-Agent-Setup.msi` | Windows: agent (remote shutdown/restart, system info) |
| `lan-wake-manager-controller_*.deb`, `lan-wake-manager-relay_*.deb`, `lan-wake-manager-agent_*.deb` | Debian/Ubuntu services (systemd) |
| `LAN-Wake-Manager-x86_64.AppImage`, `lan-wake-manager-desktop_amd64.deb` | Linux desktop window |
| `LAN-Wake-Manager.apk` | Android app (signed release) |
| `LAN-Wake-Manager-debug.apk` | Android debug build |

The Linux packages are attached a few minutes after the release by the *Linux packages* workflow. Installers are not code-signed (Windows SmartScreen: *More info → Run anyway*). Start with the [README](https://github.com/unupunct/LAN-Wake-Manager#quick-start-about-10-minutes), [Windows](https://github.com/unupunct/LAN-Wake-Manager/blob/main/docs/INSTALL-WINDOWS.md) and [Linux](https://github.com/unupunct/LAN-Wake-Manager/blob/main/docs/INSTALL-LINUX.md) installation guides.

### Known limitations

* The Android app has not been run on a physical device or emulator; MSI installs and real sleeping-hardware Wake-on-LAN were not tested by the author. See "What has and has not been verified" in the README.
* SSH keys can be uploaded through the API only (no dashboard screen yet).
