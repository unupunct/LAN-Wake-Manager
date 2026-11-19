//! Data models shared by the controller, relay and agent. JSON shapes follow docs/API.md.
use serde::{Deserialize, Serialize};

pub type Ts = i64;

fn d_wol_ports() -> Vec<u16> {
    vec![9, 7]
}
fn d_attempts() -> u32 {
    3
}
fn d_interval() -> u32 {
    2
}
fn d_timeout() -> u32 {
    120
}
fn d_color() -> String {
    "#2ee59d".into()
}

/// Wake parameters of a network location.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WakeSettings {
    pub broadcast_addr: Option<String>,
    pub wol_ports: Vec<u16>,
    pub wake_attempts: u32,
    pub wake_interval_secs: u32,
    pub wake_timeout_secs: u32,
}

/// Stored location record (controller DB). `relay_token` / `relay_fingerprint` never leave the controller.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LocationRec {
    pub id: String,
    pub name: String,
    #[serde(default = "d_color")]
    pub color: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub broadcast_addr: Option<String>,
    #[serde(default = "d_wol_ports")]
    pub wol_ports: Vec<u16>,
    #[serde(default = "d_attempts")]
    pub wake_attempts: u32,
    #[serde(default = "d_interval")]
    pub wake_interval_secs: u32,
    #[serde(default = "d_timeout")]
    pub wake_timeout_secs: u32,
    #[serde(default)]
    pub relay_url: Option<String>,
    #[serde(default)]
    pub relay_fingerprint: Option<String>,
    #[serde(default)]
    pub relay_token: Option<String>,
    #[serde(default)]
    pub relay_name: Option<String>,
    #[serde(default)]
    pub relay_version: Option<String>,
    #[serde(default)]
    pub relay_last_seen: Option<Ts>,
    #[serde(default)]
    pub created_at: Ts,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub color: String,
    pub notes: String,
    pub broadcast_addr: Option<String>,
    pub wol_ports: Vec<u16>,
    pub wake_attempts: u32,
    pub wake_interval_secs: u32,
    pub wake_timeout_secs: u32,
    pub relay_mode: String,
    pub relay_url: Option<String>,
    pub relay_status: String,
    pub relay_last_seen: Option<Ts>,
    pub relay_version: Option<String>,
    pub relay_name: Option<String>,
    pub device_count: u32,
    pub online_count: u32,
    pub controller_status: String,
    pub vpn: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LocationInput {
    pub name: String,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub broadcast_addr: Option<String>,
    pub wol_ports: Option<Vec<u16>>,
    pub wake_attempts: Option<u32>,
    pub wake_interval_secs: Option<u32>,
    pub wake_timeout_secs: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SshConfig {
    pub user: String,
    #[serde(default = "d_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub key_id: Option<String>,
}
fn d_ssh_port() -> u16 {
    22
}

/// Stored device (static configuration).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceRec {
    pub id: String,
    pub location_id: String,
    pub name: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    pub mac: String,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default = "d_dtype")]
    pub device_type: String,
    #[serde(default = "d_unknown")]
    pub wol_capable: String,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub tcp_check_ports: Vec<u16>,
    #[serde(default = "d_pm")]
    pub power_method: String,
    #[serde(default)]
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub agent_token_hash: Option<String>,
    #[serde(default)]
    pub created_at: Ts,
}
fn d_dtype() -> String {
    "pc".into()
}
fn d_unknown() -> String {
    "unknown".into()
}
fn d_pm() -> String {
    "none".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceInput {
    pub location_id: String,
    pub name: String,
    pub hostname: Option<String>,
    pub ip: Option<String>,
    pub mac: String,
    pub os: Option<String>,
    pub device_type: Option<String>,
    pub group_ids: Option<Vec<String>>,
    pub tcp_check_ports: Option<Vec<u16>>,
    pub power_method: Option<String>,
    pub ssh: Option<SshConfig>,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AgentInfo {
    pub state: String, // online | offline | none
    pub version: Option<String>,
    pub last_heartbeat: Option<Ts>,
    pub enrolled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Device {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub hostname: Option<String>,
    pub ip: Option<String>,
    pub mac: String,
    pub os: Option<String>,
    pub device_type: String,
    pub wol_capable: String,
    pub group_ids: Vec<String>,
    pub tcp_check_ports: Vec<u16>,
    pub power_method: String,
    pub ssh: Option<SshConfig>,
    pub notes: String,
    pub status: String,
    pub status_checked_at: Option<Ts>,
    pub last_seen: Option<Ts>,
    pub status_changed_at: Option<Ts>,
    pub agent: AgentInfo,
    pub sysinfo: Option<SysInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DiskInfo {
    pub mount: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NetInfo {
    pub name: String,
    pub ips: Vec<String>,
    pub mac: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TempInfo {
    pub label: String,
    pub celsius: f32,
}
/// Reported by the agent; values that cannot be read are `None` (never invented).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SysInfo {
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel: Option<String>,
    pub arch: Option<String>,
    pub uptime_secs: Option<u64>,
    pub cpu_model: Option<String>,
    pub cpu_cores: Option<u32>,
    pub cpu_usage_percent: Option<f32>,
    pub mem_total_bytes: Option<u64>,
    pub mem_used_bytes: Option<u64>,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetInfo>,
    pub temperatures: Vec<TempInfo>,
    pub reported_at: Ts,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    #[serde(default)]
    pub device_count: u32,
    #[serde(default)]
    pub online_count: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupInput {
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpStep {
    pub t: Ts,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Operation {
    pub id: String,
    pub kind: String,  // wake | shutdown | restart | cancel_shutdown
    pub device_id: String,
    pub device_name: String,
    pub location_id: String,
    pub state: String, // pending | running | succeeded | failed | timeout
    pub message: String,
    pub steps: Vec<OpStep>,
    pub requested_by: String,
    pub source: String, // manual | bulk | schedule
    pub schedule_id: Option<String>,
    pub delay_secs: u32,
    pub started_at: Ts,
    pub finished_at: Option<Ts>,
}

// ---------- schedules ----------
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ScheduleTarget {
    #[serde(default)]
    pub device_ids: Vec<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub location_ids: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interval {
    pub every: u32,
    pub unit: String, // minutes | hours | days
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Recurrence {
    pub kind: String, // once | daily | weekly | monthly | custom
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub weekdays: Vec<u32>,
    #[serde(default)]
    pub day_of_month: Option<u32>,
    #[serde(default)]
    pub interval: Option<Interval>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub interval_secs: u32,
}
impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 1, interval_secs: 60 }
    }
}
fn d_true() -> bool {
    true
}
fn d_skip() -> String {
    "skip".into()
}
fn d_grace() -> u32 {
    60
}
fn d_controller() -> String {
    "controller".into()
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Schedule {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default = "d_true")]
    pub enabled: bool,
    pub action: String,
    #[serde(default)]
    pub target: ScheduleTarget,
    pub recurrence: Recurrence,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub exceptions: Vec<String>,
    #[serde(default = "d_skip")]
    pub missed_policy: String,
    #[serde(default = "d_grace")]
    pub missed_grace_minutes: u32,
    #[serde(default)]
    pub retry: RetryPolicy,
    #[serde(default = "d_controller")]
    pub executor: String,
    #[serde(default)]
    pub delay_secs: u32,
    #[serde(default)]
    pub next_run: Option<Ts>,
    #[serde(default)]
    pub last_run: Option<Ts>,
    #[serde(default)]
    pub last_state: Option<String>,
}

// ---------- discovery ----------
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Candidate {
    pub ip: String,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub device_type: String,
    pub vendor: Option<String>,
    pub open_ports: Vec<u16>,
    pub methods: Vec<String>,
    #[serde(default)]
    pub registered: bool,
    #[serde(default)]
    pub device_id: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscoveryRequest {
    pub subnet: Option<String>,
    pub interface: Option<String>,
    pub speed: Option<String>,
    pub timeout_ms: Option<u64>,
    pub tcp_ports: Option<Vec<u16>>,
    pub methods: Option<Vec<String>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscoveryStatus {
    pub job_id: String,
    pub state: String, // running | done | cancelled | failed
    pub progress: Progress,
    pub error: Option<String>,
    pub results: Vec<Candidate>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IfaceInfo {
    pub name: String,
    pub ip: String,
    pub netmask: String,
    pub broadcast: Option<String>,
    pub subnet_cidr: String,
    pub mac: Option<String>,
}

// ---------- relay protocol ----------
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WakeRequest {
    pub device_id: String,
    pub mac: String,
    pub ip: Option<String>,
    pub tcp_ports: Vec<u16>,
    pub broadcast_addr: Option<String>,
    pub wol_ports: Vec<u16>,
    pub attempts: u32,
    pub interval_secs: u32,
    pub timeout_secs: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WakeStatus {
    pub job_id: String,
    pub state: String, // running | online | timeout | failed
    pub steps: Vec<OpStep>,
    pub online: bool,
    pub elapsed_secs: u64,
    pub detail: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProbeTarget {
    pub id: String,
    pub ip: String,
    pub tcp_ports: Vec<u16>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProbeResult {
    pub id: String,
    pub alive: bool,
    pub via: Vec<String>,
    pub rtt_ms: Option<u64>,
    pub ttl: Option<u32>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiagResult {
    pub ok: bool,
    pub detail: String,
    pub duration_ms: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RelayInfo {
    pub relay_id: String,
    pub name: String,
    pub version: String,
    pub paired: bool,
    pub tls_fingerprint: String,
}
/// A wake schedule synchronised to a relay so that it runs even if the controller is down.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RelaySchedule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub recurrence: Recurrence,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub exceptions: Vec<String>,
    pub missed_policy: String,
    pub missed_grace_minutes: u32,
    pub retry: RetryPolicy,
    pub timezone: Option<String>,
    pub targets: Vec<WakeRequest>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RelayRun {
    pub schedule_id: String,
    pub fired_at: Ts,
    pub finished_at: Ts,
    pub results: Vec<(String, String)>, // device_id, state
}

// ---------- agent protocol ----------
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentEnrollRequest {
    pub code: String,
    pub hostname: String,
    pub os: String,
    pub version: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentEnrollResponse {
    pub token: String,
    pub device_id: String,
    pub controller_id: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentCommand {
    pub id: String,
    pub kind: String, // shutdown | restart | cancel_shutdown
    pub delay_secs: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentHeartbeat {
    pub version: String,
    pub sysinfo: SysInfo,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentCommandResult {
    pub ok: bool,
    pub message: String,
}
