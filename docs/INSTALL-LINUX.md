# Installing on Linux

Everything is attached to the [latest release](https://github.com/unupunct/LAN-Wake-Manager/releases/latest). Debian/Ubuntu-family
systems (22.04+, Debian 12+, Raspberry Pi OS 64-bit on x86-64) use the `.deb` packages; every other distribution can use the
AppImage for the desktop window or build from source (see the bottom of this page).

Which package do I need?

| Role | Install on | Package |
|---|---|---|
| **Controller** (brain, schedules, remote access, dashboard) | one always-on computer | `lan-wake-manager-controller_*_amd64.deb` |
| **Desktop window** (optional, a native window around the dashboard) | any desktop | `LAN-Wake-Manager-x86_64.AppImage` or `lan-wake-manager-desktop_amd64.deb` |
| **Wake Relay** (only for *additional* LANs, e.g. the office) | one always-on computer in that LAN | `lan-wake-manager-relay_*_amd64.deb` |
| **Agent** (needed for remote shutdown / restart and system info) | every computer you want to power off remotely | `lan-wake-manager-agent_*_amd64.deb` |

The controller already contains a Wake Relay for its own LAN, so a single-location setup needs no separate relay.

## Step by step: controller (the always-on computer)

1. Download the `.deb` and install it. `apt` pulls in the dependencies (`systemd`, `iputils-ping`):
   ```bash
   sudo apt install ./lan-wake-manager-controller_1.0.0-1_amd64.deb
   ```
2. The package registers and starts the systemd service `lan-wake-controller` (starts at boot, restarts automatically after a crash). Check it:
   ```bash
   systemctl status lan-wake-controller
   curl http://127.0.0.1:8480/api/v1/info
   ```
3. Open the dashboard **on that computer**: <http://127.0.0.1:8480>. The first visit asks you to create the admin password
   (this can only be done from the controller computer itself).
4. Optional firewall rule (ufw): allow the management port **only from your LAN/VPN**, never from the internet:
   ```bash
   sudo ufw allow from 192.168.0.0/16 to any port 8443 proto tcp   # your LAN
   sudo ufw allow in on tailscale0 to any port 8443 proto tcp       # Tailscale
   ```
5. Install Tailscale (recommended) or set up WireGuard so that your phone can reach the controller from anywhere:
   [guides/01-tailscale.md](guides/01-tailscale.md), [guides/02-wireguard.md](guides/02-wireguard.md).
6. Pair your phone: Dashboard → **Mobile Access** → **Add mobile device** → scan the QR code with the Android app
   ([guides/03-android.md](guides/03-android.md)).

Logs: `journalctl -u lan-wake-controller -f`. Data (database, certificate, backups): `/var/lib/lan-wake-manager/controller` (root only).

## Desktop window (AppImage)

```bash
chmod +x LAN-Wake-Manager-x86_64.AppImage
./LAN-Wake-Manager-x86_64.AppImage
```
The window opens the dashboard of the controller on this computer. If no controller service is running it starts a private one for as
long as the window is open (data in `~/.local/share/lan-wake-manager/controller`). If the AppImage does not start because FUSE is
missing (`dlopen(): error loading libfuse.so.2`): `sudo apt install libfuse2` or run
`./LAN-Wake-Manager-x86_64.AppImage --appimage-extract-and-run`. Requires `libwebkit2gtk-4.1` (Ubuntu 22.04+).

## Wake Relay on another LAN

1. `sudo apt install ./lan-wake-manager-relay_1.0.0-1_amd64.deb` (starts service `lan-wake-relay` on TCP 8444).
2. Print the pairing information:
   ```bash
   sudo lwm-relay pair-code
   ```
   It shows the **TLS fingerprint** and a **one-time code** (valid 15 minutes).
3. Make sure the controller can reach this computer (same Tailscale network, or a Tailscale subnet route to it). See [guides/04-relay.md](guides/04-relay.md).
4. In the dashboard: **Locations** → add *Office Network* → **Relay** → enter `https://<relay-address>:8444` → *Check* → compare the fingerprint → enter the code → *Pair*.

## Agent on a computer you want to shut down / restart remotely

1. `sudo apt install ./lan-wake-manager-agent_1.0.0-1_amd64.deb` (the service waits until the computer is enrolled).
2. In the dashboard open the computer → **Agent & power** → **Create enrollment code**. Copy the Linux command it shows; it looks like
   ```bash
   sudo lwm-agent enroll --controller https://100.64.0.5:8443 --code ABCD-EFGH --fingerprint 3f9a…
   sudo systemctl restart lan-wake-agent
   ```
3. Within about 15 seconds the dashboard shows the agent as *online* with CPU, memory, disks and uptime.

## Uninstall

```bash
sudo apt remove lan-wake-manager-controller lan-wake-manager-relay lan-wake-manager-agent lan-wake-manager-desktop
sudo rm -rf /var/lib/lan-wake-manager      # only if you also want to delete the data (devices, schedules, backups)
```

## Other distributions / building from source

```bash
sudo apt install build-essential pkg-config curl nodejs npm     # or the equivalents of your distribution
git clone https://github.com/unupunct/LAN-Wake-Manager && cd LAN-Wake-Manager
(cd ui && npm ci && npm run build)
cargo build --release
sudo install -m755 target/release/lwm-controller target/release/lwm-relay target/release/lwm-agent /usr/local/bin/
sudo lwm-controller install-service        # writes /etc/systemd/system/LANWakeController.service and starts it
```
`lwm-relay install-service` and `lwm-agent install-service` work the same way. `uninstall-service` removes them.
