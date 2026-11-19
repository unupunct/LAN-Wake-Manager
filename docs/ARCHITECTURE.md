# Architecture

```
 Android app ──(Tailscale / WireGuard / LAN, HTTPS + pinned cert, device token)──┐
 Dashboard (browser / desktop window) ──(admin session)──────────────────────────┤
                                                                                   ▼
                                                       ┌──────────────── Controller ────────────────┐
                                                       │ REST + WebSocket API · SQLite · scheduler   │
                                                       │ monitor · audit log · backups · dashboard   │
                                                       │ built-in relay for its own LAN              │
                                                       └───────┬───────────────────────▲────────────┘
                              HTTPS :8444, relay token         │                       │ HTTPS :8443, agent token (agents connect OUT)
                                  ┌────────────────────────────▼───┐        ┌──────────┴──────────┐
                                  │ Wake Relay (other LAN)         │        │ Agent on each PC    │
                                  │ magic packets · probe · scan   │        │ heartbeat · sysinfo │
                                  │ relay-side wake schedules      │        │ shutdown / restart  │
                                  └──────────────┬─────────────────┘        └─────────────────────┘
                                                 ▼ UDP broadcast 9/7 on the target LAN
                                          sleeping computer
```

## Components

| Path | What |
|---|---|
| `crates/lwm-core` | shared library: models, TLS pinning, WoL packets and path planning, probes, discovery, schedule math, power command templates, service install (Windows SCM / systemd) |
| `crates/lwm-controller` | the controller service (axum, SQLite with migrations, embedded dashboard) |
| `crates/lwm-relay` | the Wake Relay service |
| `crates/lwm-agent` | the computer agent |
| `ui/` | dashboard: React + TypeScript + Vite, built to `ui/dist` and embedded into the controller binary |
| `desktop/` | Tauri window around the dashboard (starts a private controller if the service is not running) |
| `android/` | native Kotlin + Jetpack Compose app |
| `installer/windows`, `packaging/linux`, `.github/workflows` | MSI (WiX v5), `.deb` (cargo-deb), AppImage, CI |

## Ports

| Port | Who | Bound to |
|---|---|---|
| TCP 8443 (HTTPS) | controller API for phones, agents, remote dashboards | all interfaces; connections from public source addresses are rejected unless "allow public" is enabled |
| TCP 8480 (HTTP) | dashboard for the desktop window | **loopback only** |
| TCP 8444 (HTTPS) | relay API | all interfaces, same private-source rule |
| UDP 9 and 7 | magic packets sent by relays | outgoing broadcasts |
| mDNS `_lanwake._tcp` | controller announcement for direct LAN discovery | LAN |

## Trust and pairing

* Every service has a **self-signed certificate**. Peers do not use CAs: they **pin the SHA-256 fingerprint** of the leaf certificate (from the QR code, or shown to the user for comparison). Fingerprint mismatch = connection refused.
* **Phones**: QR = controller addresses + fingerprint + short-lived single-use pairing token. `POST /pair` exchanges it for a long-lived device token (role admin/operator/viewer). Tokens are stored hashed (SHA-256) on the controller; revoking a device invalidates it immediately.
* **Dashboard**: admin password (Argon2id), set on first start from the controller computer only. Sessions are random tokens (7 days).
* **Relays**: one-time code printed on the relay + fingerprint comparison; the controller then holds a relay token, the relay stores its hash.
* **Agents**: one-time code created for a specific device (30 minutes); the agent gets its own token (hash stored).

## Operations

`POST …/wake|shutdown|restart` creates an *Operation* (HTTP 202) executed in the background and persisted:

* **wake**: controller → relay `POST /v1/wake` (job) → the relay sends the packets (all attempts, per interface) and probes ping/TCP every second → `online` / `timeout` / `failed`. The controller also accepts an agent heartbeat as proof. `succeeded` only on proof of life.
* **shutdown/restart**: command queued for the agent (long-poll `GET /api/v1/agent/poll`) → the agent executes the fixed OS command → the controller then watches the monitor until the machine is really offline (and back online for restart; an uptime reset also counts). Alternatives: SSH or WinRM executed by the controller.
* One operation per computer at a time (cancel excepted).

## Schedules

Stored in SQLite and evaluated every 15 s by the service. Recurrence: once, daily, weekly, monthly (clamped to month length), custom interval; start/end dates, exception dates, missed-run policy (`skip` or `run_once` within a grace window), retry policy, duplicate protection (unique `(schedule, occurrence)` row). `executor: relay` pushes wake schedules to the relay (`PUT /v1/schedules`), which runs them autonomously and reports results (`GET /v1/runs`).

## Relay protocol (`/v1`, bearer = relay token)

`GET /v1/info` (public) · `POST /v1/enroll {code, controller_id}` · `GET /v1/interfaces` · `POST /v1/wake` + `GET /v1/wake/{id}` · `POST /v1/probe` · `POST|GET|DELETE /v1/discover…` · `POST /v1/diag/{ping,dns,tcp}` · `PUT|GET /v1/schedules` · `GET /v1/runs?since=` · `GET /v1/health`.

## Agent protocol (controller `/api/v1/agent/*`, bearer = agent token)

`POST /agent/enroll {code,hostname,os,version}` (public, rate limited) · `POST /agent/heartbeat {version, sysinfo}` every 15 s · `GET /agent/poll?wait=25` → commands · `POST /agent/commands/{id}/result`.

## Data

`%ProgramData%\LANWakeManager\controller` or `/var/lib/lan-wake-manager/controller` (restricted to administrators/root): `controller.db` (SQLite WAL), `cert.pem`/`key.pem`, `backups/`, `ssh-keys/`, `config.json`.

Client contract: [API.md](API.md).
