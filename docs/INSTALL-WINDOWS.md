# Installing on Windows 10 / 11

Three installers are attached to the [latest release](https://github.com/unupunct/LAN-Wake-Manager/releases/latest). They are
**not code-signed**, so Windows SmartScreen shows "Windows protected your PC": choose *More info* → *Run anyway*.

| Installer | Install on | What it does |
|---|---|---|
| `LAN-Wake-Manager-Setup.msi` | the always-on computer that acts as **controller** (and any PC where you want the desktop window) | Desktop window + **LAN Wake Manager Controller** Windows service (automatic start, restarts after failures) + firewall rule for TCP 8443 |
| `LAN-Wake-Manager-Relay-Setup.msi` | an always-on computer in an *additional* LAN | **LAN Wake Manager Relay** service (TCP 8444) |
| `LAN-Wake-Manager-Agent-Setup.msi` | each PC you want to shut down / restart remotely | **LAN Wake Manager Agent** service |

Uninstall any of them from *Settings → Apps → Installed apps*. Your data in `C:\ProgramData\LANWakeManager` is kept.

## Controller

1. Run `LAN-Wake-Manager-Setup.msi` (administrator rights are required to register the service). It needs the Microsoft Edge WebView2 runtime
   (part of Windows 11; the installer tells you where to get it on Windows 10).
2. Start **LAN Wake Manager** from the Start menu. Create the admin password on first start.
3. The controller keeps running in the background after you close the window, and starts with Windows. Check with
   `Get-Service LANWakeController` in PowerShell.
4. Install Tailscale (or WireGuard) for access from your phone: [guides/01-tailscale.md](guides/01-tailscale.md).
5. Pair the phone: **Mobile Access** → **Add mobile device**.

## Wake Relay

1. Run `LAN-Wake-Manager-Relay-Setup.msi`.
2. Open PowerShell **as administrator** and run `lwm-relay pair-code`; note the fingerprint and the one-time code.
3. Add the relay to a location in the dashboard ([guides/04-relay.md](guides/04-relay.md)).

## Agent

1. Run `LAN-Wake-Manager-Agent-Setup.msi`.
2. In the dashboard open the computer → **Agent & power** → **Create enrollment code**, copy the Windows command, run it in an administrator PowerShell:
   ```powershell
   lwm-agent enroll --controller https://100.64.0.5:8443 --code ABCD-EFGH --fingerprint 3f9a…
   Restart-Service LANWakeAgent
   ```

## Command line (optional)

The executables can also register their services themselves (administrator): `lwm-controller install-service`, `lwm-relay install-service`,
`lwm-agent install-service`, and `… uninstall-service`. `lwm-controller reset-password` removes the admin password if you forgot it
(you can then set a new one in the dashboard; paired phones keep working).
