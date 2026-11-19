use crate::db::Db;
use lwm_core::jobs::{DiscoveryJobs, WakeJobs};
use lwm_core::models::*;
use lwm_core::ratelimit::RateLimiter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{broadcast, oneshot, Notify};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupSettings {
    pub auto: bool,
    pub interval_hours: u32,
    pub keep: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub name: String,
    pub monitor_interval_secs: u32,
    pub allow_public: bool,
    pub backup: BackupSettings,
    pub timezone: Option<String>,
    pub notify_schedule_minutes: u32,
}
impl Settings {
    pub fn default_for(name: String) -> Self {
        Settings { name, monitor_interval_secs: 15, allow_public: false, backup: BackupSettings { auto: true, interval_hours: 24, keep: 14 }, timezone: None, notify_schedule_minutes: 10 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub https_listen: String,
    pub local_listen: String,
}
impl Default for Config {
    fn default() -> Self {
        Config { https_listen: "0.0.0.0:8443".into(), local_listen: "127.0.0.1:8480".into() }
    }
}

pub struct AppState {
    pub db: Arc<Db>,
    pub dir: PathBuf,
    pub controller_id: String,
    pub fingerprint: String,
    pub https_port: u16,
    pub settings: RwLock<Settings>,
    pub allow_public: Arc<AtomicBool>,
    pub events: broadcast::Sender<Value>,
    pub limiter: RateLimiter,
    pub wakes: WakeJobs,
    pub disc: DiscoveryJobs,
    /// controller job id -> (location id, remote relay job id)
    pub remote_disc: Mutex<HashMap<String, (String, String)>>,
    pub agent_queue: Mutex<HashMap<String, VecDeque<AgentCommand>>>,
    pub agent_notify: Notify,
    pub cmd_waiters: Mutex<HashMap<String, oneshot::Sender<AgentCommandResult>>>,
    /// device ids with a running operation (prevents overlapping power operations)
    pub busy: Mutex<HashSet<String>>,
    /// devices whose pending shutdown/restart was cancelled
    pub cancelled: Mutex<HashSet<String>>,
    pub relay_online: Mutex<HashMap<String, bool>>,
    pub started: i64,
    pub shutdown: tokio::sync::watch::Receiver<bool>,
}
pub type St = Arc<AppState>;

impl AppState {
    pub fn emit(&self, v: Value) {
        let _ = self.events.send(v);
    }
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }
    /// Store a notification and push it to connected clients.
    pub fn notify(&self, kind: &str, severity: &str, title: &str, body: &str, device_id: Option<&str>) {
        if let Ok(n) = self.db.notify(kind, severity, title, body, device_id) {
            self.emit(serde_json::json!({"type": "notification", "notification": n}));
        }
    }
}
