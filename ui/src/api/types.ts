// Types mirror docs/API.md. All optional/nullable fields are tolerated defensively.
export type Role = 'admin' | 'operator' | 'viewer';
export type DeviceStatus = 'online' | 'offline' | 'waking' | 'shutting_down' | 'unknown';
export type OpState = 'pending' | 'running' | 'succeeded' | 'failed' | 'timeout';
export type OpKind = 'wake' | 'shutdown' | 'restart' | 'cancel_shutdown';
export type Action = 'wake' | 'shutdown' | 'restart';

export interface Info {
  controller_id: string;
  name: string;
  version: string;
  api_version: number;
  setup_required: boolean;
  tls_fingerprint?: string | null;
  time?: number;
}
export interface AuthResult { token: string; expires_at?: number | null; role: Role }
export interface Me { kind: 'session' | 'mobile'; role: Role; device_id?: string; name: string }

export interface LocationInput {
  name: string;
  color?: string | null;
  notes?: string | null;
  broadcast_addr?: string | null;
  wol_ports: number[];
  wake_attempts: number;
  wake_interval_secs: number;
  wake_timeout_secs: number;
}
export interface Location extends LocationInput {
  id: string;
  relay_mode: 'builtin' | 'remote';
  relay_url: string | null;
  relay_status: 'builtin' | 'online' | 'offline' | 'unpaired';
  relay_last_seen: number | null;
  relay_version: string | null;
  relay_name: string | null;
  device_count: number;
  online_count: number;
  controller_status: string;
  vpn: 'tailscale' | 'wireguard' | 'lan' | 'none';
}
export interface NetInterface { name: string; ip: string; netmask?: string; broadcast?: string; subnet_cidr: string; mac?: string | null }
export interface RelayProbe { fingerprint: string; relay_id: string; name: string; version: string }

export interface DiskInfo { mount: string; total_bytes: number | null; free_bytes: number | null }
export interface NetworkInfo { name: string; ips?: string[]; mac?: string | null }
export interface SysInfo {
  hostname?: string | null; os_name?: string | null; os_version?: string | null; kernel?: string | null; arch?: string | null;
  uptime_secs?: number | null; cpu_model?: string | null; cpu_cores?: number | null; cpu_usage_percent?: number | null;
  mem_total_bytes?: number | null; mem_used_bytes?: number | null;
  disks?: DiskInfo[] | null; networks?: NetworkInfo[] | null;
  temperatures?: { label: string; celsius: number | null }[] | null;
  reported_at?: number | null;
}
export interface AgentInfo { state: 'online' | 'offline' | 'none'; version: string | null; last_heartbeat: number | null; enrolled: boolean }
export interface SshConfig { user: string; port: number; key_id: string | null }
export type PowerMethod = 'agent' | 'ssh' | 'winrm' | 'none';
export type OsKind = 'windows' | 'linux' | 'macos' | 'other';
export type DeviceType = 'pc' | 'laptop' | 'server' | 'nas' | 'other';

export interface DeviceInput {
  location_id: string; name: string; hostname?: string | null; ip?: string | null; mac: string;
  os?: OsKind | null; device_type?: DeviceType; group_ids?: string[]; tcp_check_ports?: number[];
  power_method?: PowerMethod; ssh?: SshConfig | null; notes?: string | null;
}
export interface Device {
  id: string; location_id: string; name: string; hostname: string | null; ip: string | null; mac: string;
  os: OsKind | null; device_type: DeviceType; wol_capable: 'yes' | 'no' | 'unknown';
  group_ids: string[]; tcp_check_ports: number[]; power_method: PowerMethod; ssh: SshConfig | null; notes: string | null;
  status: DeviceStatus; status_checked_at: number | null; last_seen: number | null; status_changed_at: number | null;
  agent: AgentInfo | null; sysinfo: SysInfo | null;
}
export interface MetricPoint { t: number; cpu?: number | null; mem_used?: number | null; mem_total?: number | null; disk_free_min?: number | null; temp_max?: number | null }
export interface Check { name: string; status: 'ok' | 'warn' | 'fail' | 'info'; detail?: string | null; duration_ms?: number | null }
export interface Troubleshoot { checks: Check[]; advice: string[] }
export interface Enrollment { code: string; expires_at: number; controller_url: string; fingerprint: string; command_windows: string; command_linux: string }

export interface OpStep { t: number; text: string }
export interface Operation {
  id: string; kind: OpKind; device_id: string; device_name: string; location_id?: string | null;
  state: OpState; message: string | null; steps: OpStep[] | null; requested_by: string | null;
  source: 'manual' | 'bulk' | 'schedule' | string; schedule_id: string | null; delay_secs: number | null;
  started_at: number; finished_at: number | null;
}
export interface BulkRequest { action: Action; device_ids?: string[]; group_id?: string; location_id?: string; delay_secs?: number; confirm?: boolean }

export type GroupIcon = 'desktop' | 'gamepad' | 'server' | 'nas' | 'building' | 'laptop' | 'star' | 'all';
export interface GroupInput { name: string; icon?: GroupIcon | string; color?: string }
export interface Group { id: string; name: string; icon: GroupIcon | string; color: string; device_count: number; online_count: number }

export type RecKind = 'once' | 'daily' | 'weekly' | 'monthly' | 'custom';
export interface Recurrence {
  kind: RecKind; at?: string | null; date?: string | null; weekdays?: number[] | null; day_of_month?: number | null;
  interval?: { every: number; unit: 'minutes' | 'hours' | 'days' } | null;
}
export interface ScheduleInput {
  name: string; enabled: boolean; action: Action;
  target: { device_ids: string[]; group_ids: string[]; location_ids: string[] };
  recurrence: Recurrence; start_date: string | null; end_date: string | null; exceptions: string[];
  missed_policy: 'skip' | 'run_once'; missed_grace_minutes: number;
  retry: { max_attempts: number; interval_secs: number };
  executor: 'controller' | 'relay'; delay_secs: number;
}
export interface Schedule extends ScheduleInput {
  id: string; next_run: number | null; last_run: number | null; last_state: 'succeeded' | 'failed' | 'timeout' | 'partial' | null;
}

export interface Candidate {
  ip: string; mac: string | null; hostname: string | null; os: OsKind | null; device_type: DeviceType | null; vendor: string | null;
  open_ports: number[]; methods: string[]; registered: boolean; device_id: string | null;
}
export interface DiscoveryRequest {
  location_id: string; subnet?: string; interface?: string; speed: 'slow' | 'normal' | 'fast'; timeout_ms?: number; tcp_ports?: number[]; methods?: string[];
}
export interface DiscoveryJob {
  job_id: string; state: 'running' | 'done' | 'cancelled' | 'failed'; progress: { done: number; total: number } | null; error: string | null; results: Candidate[] | null;
}

export interface Tailscale {
  installed: boolean; running: boolean; backend_state?: string | null; ip?: string | null; hostname?: string | null;
  magicdns_name?: string | null; tailnet?: string | null; error?: string | null;
}
export interface Connectivity {
  tailscale: Tailscale | null; addresses: { address: string; kind: 'lan' | 'tailscale' | 'wireguard' | 'other'; interface?: string | null }[];
  port: number; allow_public: boolean;
}
export interface WgProfileInput { name: string; controller_address: string; port: number; endpoint: string; allowed_ips: string; notes: string }
export interface WgProfile extends WgProfileInput { id: string }
export interface Diagnostics { checks: Check[]; generated_at: number }
export interface ToolResult { ok: boolean; detail: string | null; duration_ms: number | null }

export interface Dashboard {
  totals: { devices: number; online: number; offline: number; unknown: number };
  relays: { total: number; online: number };
  vpn: { tailscale: Tailscale | null; wireguard_profiles: number; active_connections: number } | null;
  pending_schedules: number; failed_operations_24h: number;
  recent_operations: Operation[];
  upcoming_schedules: { schedule_id: string; name: string; action: Action; next_run: number | null }[];
  locations: Location[];
}

export interface Pairing { pairing_token: string; expires_at: number; qr_payload: string; addresses: string[] }
export interface MobileDevice { id: string; name: string; platform: string; role: Role; created_at: number; last_seen: number | null; last_ip: string | null; revoked: boolean }
export interface AuditEntry { id: string | number; ts: number; actor: string; action: string; target: string | null; result: string; ip: string | null; detail: string | null }
export interface Settings {
  name: string; monitor_interval_secs: number; allow_public: boolean;
  backup: { auto: boolean; interval_hours: number; keep: number };
  timezone: string | null; notify_schedule_minutes: number;
}
export interface Notification { id: number | string; ts: number; type: string; severity: 'info' | 'warn' | 'error'; title: string; body: string | null; device_id: string | null }
export interface BackupEntry { name: string; created_at: number; size_bytes: number; kind: 'auto' | 'manual' }

export type LiveEvent =
  | { type: 'hello' }
  | { type: 'device_status'; device_id: string; status: DeviceStatus; last_seen?: number | null }
  | { type: 'operation'; operation: Operation }
  | { type: 'relay_status'; location_id: string; relay_status: Location['relay_status'] }
  | { type: 'notification'; notification: Notification }
  | { type: 'schedule_fired'; schedule_id: string };
