# LAN Wake Manager – Controller API v1

This is the contract between the **controller** (Rust, `crates/lwm-controller`) and its clients
(the web/desktop dashboard in `ui/`, the Android app in `android/`). Both clients are written
against this document.

## Transport

* Base path: `/api/v1`. All bodies are JSON (`Content-Type: application/json`), field names `snake_case`.
* Controller listens on **HTTPS :8443** (self-signed certificate generated on first start) and, for the
  local desktop window only, on **plain HTTP 127.0.0.1:8480** (loopback only, serves the same API + dashboard).
* The self-signed certificate is **pinned by SHA-256 fingerprint of the leaf certificate (DER)**. Clients must
  not use normal CA validation or hostname checks; they compare the fingerprint obtained from the pairing QR
  code. Fingerprint format: 64 lowercase hex chars (no colons).
* Connections from non-private source addresses are rejected (only loopback, RFC1918, 100.64.0.0/10
  (Tailscale), link-local, fc00::/7 are served) unless the admin enabled `allow_public` in settings.
* Timestamps: integer **Unix epoch seconds** (`*_at`, `last_seen`, `next_run`). Durations: seconds (`*_secs`).
* IDs: opaque strings.

## Authentication

`Authorization: Bearer <token>` on everything except the "public" endpoints below.
Two kinds of tokens:

* **session** – dashboard login with the admin password (`/auth/login`), role `admin`.
* **mobile** – long-lived device token obtained through QR pairing (`/pair`), role chosen at pairing:
  `admin` | `operator` (wake/shutdown/restart/schedules, no configuration) | `viewer` (read-only).

Role checks: viewer = GET only; operator = viewer + operations + schedules + discovery;
admin = everything (locations, relays, devices CRUD, groups, settings, pairing, backups, audit, revocation).
403 `forbidden` if the role is too low. 401 `unauthorized` for a missing/invalid/revoked token.
Failed auth attempts and bursts are rate limited (429 `rate_limited`, `Retry-After` header).

## Errors

```json
{ "error": { "code": "not_found", "message": "device not found" } }
```
Codes: `bad_request`, `unauthorized`, `forbidden`, `not_found`, `conflict`, `rate_limited`,
`confirmation_required`, `relay_unreachable`, `internal`. HTTP status matches (400/401/403/404/409/429/400/502/500).

## Public endpoints

| Method + path | Body | Response |
|---|---|---|
| `GET /info` | – | `{controller_id, name, version, api_version:1, setup_required:bool, tls_fingerprint, time}` |
| `POST /auth/setup` (loopback only, only while `setup_required`) | `{password}` (>= 10 chars) | `{token, expires_at, role}` |
| `POST /auth/login` | `{password}` | `{token, expires_at, role:"admin"}` |
| `POST /pair` | `{pairing_token, device_name, platform}` | `{device_id, token, role, controller:{id, name}}` |

## Authenticated endpoints

### Session / me
* `POST /auth/logout` -> `{ok:true}`
* `POST /auth/password` `{old_password,new_password}` (admin session)
* `GET /me` -> `{kind:"session"|"mobile", role, device_id?:string, name}`

### Dashboard
`GET /dashboard` ->
```json
{ "totals": {"devices":12,"online":7,"offline":4,"unknown":1},
  "relays": {"total":2,"online":2},
  "vpn": {"tailscale":{"installed":true,"running":true,"ip":"100.64.1.2","hostname":"pc.tailnet.ts.net"},
          "wireguard_profiles":1, "active_connections":1},
  "pending_schedules": 3,
  "failed_operations_24h": 1,
  "recent_operations": [Operation],
  "upcoming_schedules": [{"schedule_id":"..","name":"..","action":"wake","next_run":1700000000}],
  "locations": [LocationSummary] }
```

### Locations (network locations; each has its own inventory and relay)
`GET /locations` -> `[Location]`; `POST /locations` (admin) `LocationInput` -> `Location`;
`GET|PUT|DELETE /locations/{id}` (PUT/DELETE admin).

`Location`:
```json
{ "id":"..", "name":"Home Network", "color":"#2ee59d", "notes":"",
  "broadcast_addr": null,              // optional override, e.g. "192.168.1.255"
  "wol_ports": [9,7], "wake_attempts": 3, "wake_interval_secs": 2, "wake_timeout_secs": 120,
  "relay_mode": "builtin"|"remote",    // builtin = the controller sends the packets on its own LAN
  "relay_url": null|"https://100.64.1.9:8444",
  "relay_status": "builtin"|"online"|"offline"|"unpaired",
  "relay_last_seen": null|1700000000, "relay_version": null|"1.0.0", "relay_name": null|"office-nuc",
  "device_count": 5, "online_count": 3,
  "controller_status": "online", "vpn": "tailscale"|"wireguard"|"lan"|"none" }
```
`LocationSummary` = the same object. `LocationInput` has the editable fields (name, color, notes, broadcast_addr, wol_ports, wake_attempts, wake_interval_secs, wake_timeout_secs).

Relay pairing (admin):
* `POST /locations/{id}/relay/probe` `{url}` -> `{fingerprint, relay_id, name, version}` (user compares fingerprint shown on the relay console)
* `POST /locations/{id}/relay/pair` `{url, code, fingerprint}` -> `Location` (sets `relay_mode:"remote"`)
* `DELETE /locations/{id}/relay` -> `Location` (back to builtin)
* `GET /locations/{id}/interfaces` -> `[{name, ip, netmask, broadcast, subnet_cidr, mac}]` (from that location's relay)

### Devices
`GET /devices?location_id=&group_id=` -> `[Device]`; `POST /devices` (admin) `DeviceInput` -> `Device`;
`GET /devices/{id}`; `PUT|DELETE /devices/{id}` (admin).

`Device`:
```json
{ "id":"..", "location_id":"..", "name":"Gaming PC", "hostname":"DESKTOP-1","ip":"192.168.1.20",
  "mac":"AA:BB:CC:DD:EE:FF", "os":"windows"|"linux"|"macos"|"other"|null,
  "device_type":"pc"|"laptop"|"server"|"nas"|"other",
  "wol_capable":"yes"|"no"|"unknown",           // "yes" only after a wake was confirmed
  "group_ids":["g1","g2"], "tcp_check_ports":[22,3389,445],
  "power_method":"agent"|"ssh"|"winrm"|"none",
  "ssh": {"user":"admin","port":22,"key_id":null}|null,
  "notes":"",
  "status":"online"|"offline"|"waking"|"shutting_down"|"unknown",
  "status_checked_at": 1700000000|null, "last_seen": 1700000000|null, "status_changed_at": 1700000000|null,
  "agent": {"state":"online"|"offline"|"none","version":null|"1.0.0","last_heartbeat":null|1700000000,"enrolled":true},
  "sysinfo": null | SysInfo }
```
`SysInfo` (reported by the agent; fields the agent cannot read are `null` and **never invented**):
```json
{ "hostname":"..","os_name":"Windows 11","os_version":"10.0.26200","kernel":null,"arch":"x86_64",
  "uptime_secs":1234,"cpu_model":"..","cpu_cores":16,"cpu_usage_percent":12.5,
  "mem_total_bytes":0,"mem_used_bytes":0,
  "disks":[{"mount":"C:\\","total_bytes":0,"free_bytes":0}],
  "networks":[{"name":"Ethernet","ips":["192.168.1.20"],"mac":"AA:BB.."}],
  "temperatures":[{"label":"CPU","celsius":55.0}],
  "reported_at":1700000000 }
```
`DeviceInput` (create/update): `{location_id, name, hostname?, ip?, mac, os?, device_type?, group_ids?, tcp_check_ports?, power_method?, ssh?, notes?}`.

Other device endpoints:
* `GET /devices/{id}/metrics?minutes=1440` -> `[{t, cpu, mem_used, mem_total, disk_free_min, temp_max}]` (history from heartbeats)
* `GET /devices/{id}/troubleshoot` -> `{checks:[{name,status:"ok"|"warn"|"fail"|"info",detail}], advice:[string]}` (Wake-on-LAN troubleshooting)
* `POST /devices/{id}/refresh` -> `Device` (probe now)
* `POST /devices/{id}/agent-enrollment` (admin) -> `{code, expires_at, controller_url, fingerprint, command_windows, command_linux}`
* `DELETE /devices/{id}/agent` (admin) revokes the agent token

### Power operations (operator+)
* `POST /devices/{id}/wake` `{}` -> **202** `Operation`
* `POST /devices/{id}/shutdown` `{delay_secs?:0}` -> 202 `Operation`
* `POST /devices/{id}/restart` `{delay_secs?:0}` -> 202 `Operation`
* `POST /devices/{id}/cancel-shutdown` `{}` -> 202 `Operation`
* `POST /bulk` `{action:"wake"|"shutdown"|"restart", device_ids?:[], group_id?, location_id?, delay_secs?, confirm?:bool}` -> `{operations:[Operation]}`.
  `shutdown`/`restart` without `confirm:true` -> 400 `confirmation_required`.
* `POST /groups/{id}/action` same body without targets.

`Operation`:
```json
{ "id":"..", "kind":"wake"|"shutdown"|"restart"|"cancel_shutdown",
  "device_id":"..","device_name":"..","location_id":"..",
  "state":"pending"|"running"|"succeeded"|"failed"|"timeout",
  "message":"Online after 34 s (confirmed by ping and agent heartbeat)",
  "steps":[{"t":1700000000,"text":"magic packet sent via 192.168.1.2 -> 192.168.1.255:9 (attempt 1/3)"}],
  "requested_by":"admin|<mobile device name>|schedule:<name>", "source":"manual"|"bulk"|"schedule",
  "schedule_id":null, "delay_secs":0, "started_at":1700000000, "finished_at":null|1700000034 }
```
For `wake`: `succeeded` **only** after the target became reachable; `timeout` means packets were sent but the
machine did not come up in `wake_timeout_secs`; `failed` = could not send / relay unreachable.
* `GET /operations?limit=50&before=<started_at>&device_id=&state=` -> `[Operation]` (newest first)
* `GET /operations/{id}` -> `Operation`

### Groups
`GET /groups` -> `[Group]`; `POST /groups` (admin) `{name, icon?, color?}`; `PUT|DELETE /groups/{id}` (admin).
`Group`: `{id,name,icon,color,device_count,online_count}`. `icon` is one of: `desktop`,`gamepad`,`server`,`nas`,`building`,`laptop`,`star`,`all`.
Membership is edited through `Device.group_ids`. (Seed groups on a fresh install: Workstations, Gaming PCs, Servers, NAS, Office Computers.)

### Schedules (operator+ to manage)
`GET /schedules`, `POST /schedules`, `PUT|DELETE /schedules/{id}`, `POST /schedules/{id}/run-now`.
```json
{ "id":"..","name":"Wake office PCs","enabled":true,"action":"wake"|"shutdown"|"restart",
  "target": {"device_ids":[],"group_ids":["g"],"location_ids":[]},
  "recurrence": { "kind":"once"|"daily"|"weekly"|"monthly"|"custom",
      "at":"07:30",                 // local time of the controller's timezone, HH:MM
      "date":"2026-12-24",          // once only
      "weekdays":[1,2,3,4,5],       // weekly: Mon=1..Sun=7
      "day_of_month":1,             // monthly (clamped to month length)
      "interval":{"every":6,"unit":"minutes"|"hours"|"days"}  // custom
  },
  "start_date":null|"2026-01-01","end_date":null|"2026-12-31",
  "exceptions":["2026-12-25"],
  "missed_policy":"skip"|"run_once", "missed_grace_minutes":60,
  "retry":{"max_attempts":3,"interval_secs":60},
  "executor":"controller"|"relay",  // relay only valid for action=wake and one location; runs even when the controller is down
  "delay_secs":0,
  "next_run":null|1700000000,"last_run":null|1700000000,"last_state":null|"succeeded"|"failed"|"timeout"|"partial" }
```
`shutdown`/`restart` schedules are executed by the controller (needs the agent). A sleeping computer cannot run its own schedule: wake schedules run on the controller/relay.

### Discovery (operator+)
* `POST /discovery` `{location_id, subnet?:"192.168.1.0/24", interface?:"Ethernet", speed:"slow"|"normal"|"fast", timeout_ms?:800, tcp_ports?:[22,445,3389,80,443], methods?:["arp","ping","tcp","mdns"]}` -> `{job_id}`
  (only explicitly configured/selected subnets are scanned; /16 and larger refused)
* `GET /discovery/{job_id}` -> `{job_id, state:"running"|"done"|"cancelled"|"failed", progress:{done,total}, error, results:[Candidate]}`
  `Candidate`: `{ip, mac, hostname, os, device_type, vendor, open_ports:[], methods:["arp","ping"], registered:bool, device_id}`
* `DELETE /discovery/{job_id}` cancels.
Register results by `POST /devices` (a candidate needs a MAC; without a MAC Wake is impossible and the UI says so).

### Connectivity and diagnostics
* `GET /connectivity` ->
```json
{ "tailscale": {"installed":true,"running":true,"backend_state":"Running","ip":"100.64.1.2","hostname":"pc","magicdns_name":"pc.tail1234.ts.net","tailnet":"tail1234.ts.net","error":null},
  "addresses":[{"address":"192.168.1.5","kind":"lan"|"tailscale"|"wireguard"|"other","interface":"Ethernet"}],
  "port":8443, "allow_public":false }
```
* `GET|POST /wireguard-profiles`, `PUT|DELETE /wireguard-profiles/{id}` (admin). Profile: `{id,name,controller_address,port,endpoint,allowed_ips,notes}`. **No private keys are ever stored.**
* `GET /diagnostics?location_id=` -> `{checks:[{name,status:"ok"|"warn"|"fail"|"info",detail,duration_ms}], generated_at}` (controller, VPN, relay reachability + latency, db, scheduler)
* `POST /diagnostics/ping` `{host}`; `POST /diagnostics/dns` `{host}`; `POST /diagnostics/tcp` `{host,port}` -> `{ok,detail,duration_ms}` (run by the location's relay when `location_id` given)

### Mobile access (admin)
* `POST /pairing` `{role:"admin"|"operator"|"viewer", ttl_secs?:300 (max 600)}` -> `{pairing_token, expires_at, qr_payload, addresses:[..]}`
  `qr_payload` is the exact string to encode in the QR:
  `{"v":1,"id":"<controller_id>","name":"Home controller","addrs":["https://100.64.1.2:8443","https://192.168.1.5:8443"],"fp":"<sha256 hex>","token":"<pairing_token>","exp":1700000300}`
  The token is single-use and short-lived; no permanent credential is in the QR.
* `GET /mobile-devices` -> `[{id,name,platform,role,created_at,last_seen,last_ip,revoked:bool}]`
* `PATCH /mobile-devices/{id}` `{name?,role?}`; `DELETE /mobile-devices/{id}` revokes (token stops working immediately).

### Audit, notifications, settings, backups
* `GET /audit?limit=100&before=<ts>` (admin) -> `[{id,ts,actor,action,target,result,ip,detail}]`
* `GET /notifications?after=<id>&limit=50` -> `[{id,ts,type,severity:"info"|"warn"|"error",title,body,device_id}]`
  types: `wake_completed`, `shutdown_executed`, `restart_executed`, `operation_failed`, `device_unavailable`, `relay_offline`, `relay_online`, `controller_notice`, `schedule_approaching`.
  (Android polls this with WorkManager / WebSocket; it cannot be pushed while the phone has no network path to the controller.)
* `GET|PUT /settings` (PUT admin): `{name, monitor_interval_secs:15, allow_public:false, backup:{auto:true,interval_hours:24,keep:14}, timezone:"Europe/Berlin"|null, notify_schedule_minutes:10}`
* `GET /backups` -> `[{name,created_at,size_bytes,kind:"auto"|"manual"}]`; `POST /backups` -> entry; `POST /backups/{name}/restore` (admin; controller restarts its DB handle);
  `GET /export` -> JSON config document (devices, groups, locations, relay config w/o tokens, schedules, settings; **no passwords, keys or tokens**); `POST /import` (same document; merges by id).

### WebSocket events
`GET /events` (upgrade; **auth is the first client message** `{"type":"auth","token":"<token>"}` within 5 s).
Server -> client: `{"type":"hello"}` then events:
`{"type":"device_status","device_id","status","last_seen"}`, `{"type":"operation","operation":Operation}`,
`{"type":"relay_status","location_id","relay_status"}`, `{"type":"notification","notification":{..}}`, `{"type":"schedule_fired","schedule_id"}`.
Clients should also re-fetch via REST after reconnecting; events are hints, REST is the truth.

## Agent and relay protocols (not used by the dashboard / Android)
Documented in `docs/ARCHITECTURE.md` (agent: `/api/v1/agent/*` on the controller; relay: `/v1/*` on port 8444).

## Additional endpoints (SSH keys)
* `GET /ssh-keys` (admin) -> `[{id,name,created_at}]`; `POST /ssh-keys` `{name, private_key}` -> `{id,name}` (unencrypted OpenSSH/PEM key, stored in the controller's private folder, never returned); `DELETE /ssh-keys/{id}`. Reference the key in a device with `ssh.key_id`.
