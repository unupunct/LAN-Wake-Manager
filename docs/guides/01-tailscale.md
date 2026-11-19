# Tailscale (recommended remote access)

Tailscale gives your phone and your controller a private, encrypted network that works from mobile data, hotels and other countries without
opening any router port. LAN Wake Manager uses your **existing** Tailscale installation: it only *reads* the status (`tailscale status --json`)
and the interface address. It never touches Tailscale logins, keys or ACLs.

## 1. Install Tailscale (required)

1. Create a free account at <https://login.tailscale.com> (sign in with the same identity on all devices).
2. **Controller computer** (Windows/Linux): install from <https://tailscale.com/download>, then sign in.
   - Linux: `curl -fsSL https://tailscale.com/install.sh | sh && sudo tailscale up`
   - Windows: run the installer, then click *Log in* in the tray icon.
3. **Android phone**: install *Tailscale* from Google Play, sign in with the same account, switch it **on**.
4. In the Tailscale admin console (<https://login.tailscale.com/admin/machines>) both devices should appear. Turn on **MagicDNS** under *DNS* (optional but
   gives you names like `home-pc.tail1234.ts.net`).

## 2. Check the controller sees Tailscale (required)

Dashboard → **Remote & Diagnostics**. The Tailscale card shows *running*, the controller's `100.x.y.z` address and its MagicDNS name. If it says
"installed but turned off" or "needs login", fix that in Tailscale itself.

The Tailscale address is automatically offered first when you create a pairing QR code (Mobile Access). The phone then talks to
`https://<tailscale-name-or-ip>:8443` — encrypted by Tailscale **and** by the controller's own pinned TLS certificate.

## 3. Multiple locations (home + office)

Install Tailscale on the always-on computer of each location (it also runs that location's Wake Relay, see [04-relay.md](04-relay.md)).
Both appear in your tailnet, so the controller reaches the office relay at `https://<office-tailscale-address>:8444`.

## 4. Tailscale does not automatically expose your whole LAN (important)

Installing Tailscale on one computer only makes **that computer** reachable. Other devices on the same LAN (a sleeping PC, a NAS) are not
reachable unless you do one of the following:

* **Recommended: a Wake Relay on an always-on computer of that LAN.** The relay is the only thing that must be reachable; it sends the Wake-on-LAN
  broadcast locally. No subnet routing is needed for waking computers, and agents connect *out* to the controller by themselves.
* **Optional: a Tailscale subnet router**, only if you also want direct access to LAN devices through the VPN (SSH power-off of machines that have
  no agent, scanning a remote subnet from the controller, NAS web pages). On the always-on Linux machine of that LAN:
  ```bash
  echo 'net.ipv4.ip_forward = 1' | sudo tee -a /etc/sysctl.d/99-tailscale.conf && sudo sysctl -p /etc/sysctl.d/99-tailscale.conf
  sudo tailscale up --advertise-routes=192.168.1.0/24      # your LAN subnet
  ```
  Then **you** approve the route in the admin console (*Machines → … → Edit route settings*). LAN Wake Manager never changes routes or ACLs for you.
  If you use ACLs, allow your phone/controller to reach `tcp:8443` on the controller and `tcp:8444` on relays.

## 5. Access control suggestion

Tag-free setups work out of the box. If you manage ACLs, allow only your phone and controller to talk to `8443/8444`. The controller additionally
requires a pairing (QR) per phone and can revoke each phone individually.
