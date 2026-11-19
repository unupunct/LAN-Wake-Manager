# WireGuard (self-hosted alternative)

Use this if you already run a WireGuard server (router, Raspberry Pi, VPS…) and prefer it over Tailscale. LAN Wake Manager does **not** create
tunnels, does not manage WireGuard keys and never stores a private key: it just uses the tunnel you already have.

## What you need

* A WireGuard server that your phone can reach (UDP port forwarded to it, or a public IP) and a tunnel subnet, e.g. `10.8.0.0/24`.
* The **controller computer** is a peer or sits behind the server so that it has a VPN-reachable address (e.g. `10.8.0.2`), **or** the server routes to the LAN so that
  the phone can reach the controller's LAN address (`AllowedIPs = 10.8.0.0/24, 192.168.1.0/24` on the phone).
* TCP **8443** on the controller must be reachable through the tunnel. **Do not** port-forward 8443 on your router to the internet.

## Setup

1. Install the official *WireGuard* app on the phone and import the profile from your server (QR code or file). Turn the tunnel on.
2. On the controller check `ping 10.8.0.2` (its VPN address) from the phone's network, e.g. with a ping app while the tunnel is on.
3. Dashboard → **Remote & Diagnostics** → **WireGuard profiles** → *Add profile*: name, the controller's VPN address (`10.8.0.2`), port `8443`, the server endpoint
   (informational) and allowed IPs (informational). The controller refuses anything that looks like a key.
4. Dashboard → **Mobile Access** → *Add mobile device*. The QR code lists every address of the controller, including the VPN address (interface names
   containing `wg`/`wireguard` are recognised).
5. In the Android app: **Settings → WireGuard**: add a profile (label + controller VPN address + port) if the controller's VPN address was not in the QR code. Choose connection mode
   *VPN only* to guarantee the app never falls back to LAN/other routes.

## Multiple profiles

Add one profile per tunnel/site. The app tries addresses in the order your connection mode allows and shows which one is active
("Connected through WireGuard").

## Notes

* WireGuard keeps no connection when the phone sleeps; the app reconnects automatically when the tunnel and the network come back.
* If the controller shows `WireGuard: no interface on this computer` in diagnostics, that is fine when the tunnel is terminated on your router and the controller is just on the LAN.
* Troubleshooting: [07-troubleshooting.md](07-troubleshooting.md).
