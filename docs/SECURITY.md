# Security model

* **No unauthenticated power operations.** Everything except `GET /info`, first-run password setup (loopback only), login, pairing and agent enrollment needs a token; those are rate limited and audited.
* **Private by default.** The API only answers loopback, RFC 1918, link-local, Tailscale/CGNAT (100.64.0.0/10) and unique-local IPv6 sources. Never forward port 8443/8444 on your router; use Tailscale or WireGuard.
* **TLS everywhere** with pinned self-signed certificates (SHA-256 of the leaf). The pairing QR carries the fingerprint, so phones have no trust-on-first-use over the network. Relay pairing asks you to compare the fingerprint.
* **No permanent credentials in QR codes**: 5-minute (max 10), single-use pairing tokens. Long-lived device tokens are stored encrypted on the phone (Android Keystore AES-GCM) and as SHA-256 hashes on the controller. Revocation is immediate.
* **Roles**: viewer (read only), operator (power operations, schedules, discovery), admin (everything). Bulk shutdown/restart requires an explicit `confirm: true`, which the dashboard and the app only send after a confirmation dialog.
* **No arbitrary command execution.** The agent, SSH and WinRM paths use fixed command templates; host and user names are validated against a conservative character set before they reach a process argument list (no shell is involved).
* **Least-privilege agents** are limited to those fixed commands and only talk to their enrolled controller (pinned fingerprint). They run as SYSTEM/root solely because the OS requires it to power off.
* **Rate limiting** per client IP: login, pairing, enrollment, failed authentication, overall request rate.
* **Audit log** for logins (also failed), pairing, wake/shutdown/restart, schedule changes, revocations, settings changes and denied requests.
* **Secrets at rest**: the data directory is restricted to administrators/root. Admin password: Argon2id. Phone/agent/session tokens: SHA-256 hashes. The relay token the controller needs to call a relay, and SSH keys it uses, are stored in that restricted directory (not separately encrypted). JSON export never contains passwords, keys, tokens or token hashes; **database backups (`backups/*.db`) do contain relay tokens and token hashes** — treat them like the data directory.
* **Tailscale / WireGuard** are used as they are; the application never reads their private keys or changes ACLs/routes.

Report problems privately through GitHub security advisories of this repository.
