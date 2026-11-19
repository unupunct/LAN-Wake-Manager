# Troubleshooting remote connectivity

Start with **Dashboard → Remote & Diagnostics → Run diagnostics** and, on the phone, **Diagnostics** (VPN status, controller reachability and response time, relay status per location, active connection method, last successful connection).

| Symptom | Likely cause | What to do |
|---|---|---|
| Phone: "Controller unavailable" away from home | VPN app is off, or controller computer is off/asleep | Turn Tailscale/WireGuard on; check the controller is powered and awake (disable sleep on it); check the Tailscale admin console shows it online |
| Works on Wi-Fi at home, not on mobile data | Controller only reachable by LAN address | Pair again after Tailscale/WireGuard is up (the QR then contains the VPN address) or add the VPN address in Settings; use connection mode *Automatic* |
| "VPN only" mode refuses to connect | No VPN address for the controller saved, or VPN is off | Add Tailscale/WireGuard address; switch the VPN on |
| "Certificate fingerprint mismatch" | The controller's certificate changed (data folder deleted/reinstalled) or a different device answers at that address | Verify the address; if you reinstalled the controller, remove the controller in the app and pair again |
| "Token revoked / Pair again" (401) | Phone was revoked or the controller database was restored from before pairing | Dashboard → Mobile Access → pair again |
| Controller diagnostics: Tailscale "installed but turned off / needs login" | Tailscale state on the controller | Sign in / turn on in the Tailscale app |
| Relay "offline" | Relay computer off, service stopped, VPN down, wrong URL | On the relay: `systemctl status lan-wake-relay` / `Get-Service LANWakeRelay`; test `https://<relay>:8444/v1/info` from the controller; re-pair if the relay's data folder was reset |
| Wake result "Timeout" | Packet sent but PC did not start | [05-wake-on-lan.md](05-wake-on-lan.md) checklist; check the troubleshoot report of the device |
| Wake result "Failed – relay unreachable" | Controller cannot reach the location's relay | See "Relay offline" |
| Shutdown says "agent offline" | Agent service stopped, PC asleep, or cannot reach controller | Start the service; check TCP 8443 from that PC to the controller |
| Everything times out only through the VPN | Firewall on the controller blocks the VPN interface | Allow TCP 8443 on the VPN interface (`ufw allow in on tailscale0 to any port 8443 proto tcp`, Windows: the installer adds the rule) |
| Dashboard says "forbidden: only reachable from private networks" | Request came from a public address | Use the VPN. Only enable *Allow public source addresses* (Settings) if you really need it |
| Forgot the admin password | – | On the controller computer, as administrator/root: `lwm-controller reset-password`, then set a new one in the dashboard (paired phones keep working) |

## Where are the logs?

* Windows: Event Viewer → Service control manager events; the services log to the Windows service output only if run in the foreground. To see live logs stop the service and run `lwm-controller run` (or `lwm-relay run`, `lwm-agent run`) in an administrator terminal. Set `LWM_LOG=debug` for more detail.
* Linux: `journalctl -u lan-wake-controller -f` (also `lan-wake-relay`, `lan-wake-agent`).
* The dashboard **Audit Log** records every pairing, login attempt, power operation, denial, revocation and configuration change.

## Useful checks

```bash
curl -k https://<controller-address>:8443/api/v1/info        # answers {"controller_id": ...} when the path works (certificate is self-signed)
tailscale ping <controller-name>
tailscale status
```
