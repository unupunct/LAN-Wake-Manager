//! LAN Wake Relay: sends Wake-on-LAN packets inside one LAN on behalf of the controller.
use anyhow::{anyhow, Result};
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use lwm_core::jobs::{DiscoveryJobs, WakeJobs};
use lwm_core::models::*;
use lwm_core::ratelimit::RateLimiter;
use lwm_core::server::{ApiError, ApiResult};
use lwm_core::util::{new_id, normalize_code, now, pairing_code, random_token, sha256_hex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SERVICE_NAME: &str = "LANWakeRelay";

#[derive(Parser)]
#[command(name = "lwm-relay", version, about = "LAN Wake Manager - Wake Relay")]
struct Cli {
    #[arg(long, global = true)]
    data_dir: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    /// Run in the foreground
    Run {
        #[arg(long)]
        listen: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Entry point used by the Windows Service Control Manager / systemd
    Service,
    /// Install and start the background service (Windows service / systemd unit)
    InstallService {
        #[arg(long)]
        listen: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    UninstallService,
    /// Create a new one-time pairing code (valid 15 minutes) and print it with the TLS fingerprint
    PairCode,
    /// Show relay identity, pairing state and fingerprint
    Status,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Config {
    relay_id: String,
    name: String,
    listen: String,
    controller_token_hash: Option<String>,
    controller_id: Option<String>,
    #[serde(default)]
    schedules: Vec<RelaySchedule>,
}
#[derive(Serialize, Deserialize)]
struct Pairing {
    code_hash: String,
    expires: i64,
}

struct St {
    dir: PathBuf,
    cfg: Mutex<Config>,
    fingerprint: String,
    wakes: WakeJobs,
    disc: DiscoveryJobs,
    limiter: RateLimiter,
    runs: Mutex<Vec<RelayRun>>,
}

fn load_config(dir: &std::path::Path) -> Config {
    std::fs::read_to_string(dir.join("relay.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_config(dir: &std::path::Path, c: &Config) -> Result<()> {
    lwm_core::paths::write_secret(&dir.join("relay.json"), serde_json::to_string_pretty(c)?.as_bytes())?;
    Ok(())
}
fn new_pairing(dir: &std::path::Path) -> Result<String> {
    let code = pairing_code();
    let p = Pairing { code_hash: sha256_hex(normalize_code(&code).as_bytes()), expires: now() + 900 };
    lwm_core::paths::write_secret(&dir.join("pairing.json"), serde_json::to_string(&p)?.as_bytes())?;
    Ok(code)
}

fn run_foreground(dir: PathBuf, cfg: Config) -> Result<()> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        run_relay(dir, cfg, rx).await
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = lwm_core::paths::data_dir("relay", cli.data_dir.as_deref());
    lwm_core::paths::ensure_private_dir(&dir)?;
    match cli.cmd {
        Cmd::Run { listen, name } => {
            lwm_core::logging::init("lwm-relay");
            let cfg = prepare_config(&dir, listen, name)?;
            run_foreground(dir, cfg)
        }
        Cmd::Service => {
            lwm_core::logging::init("lwm-relay-service");
            #[cfg(windows)]
            {
                SERVICE_DIR.set(dir).ok();
                lwm_core::service::win::run(SERVICE_NAME, service_entry)?;
                Ok(())
            }
            #[cfg(not(windows))]
            {
                let cfg = prepare_config(&dir, None, None)?;
                run_foreground(dir, cfg)
            }
        }
        Cmd::InstallService { listen, name } => {
            let cfg = prepare_config(&dir, listen, name)?;
            let exe = std::env::current_exe()?;
            let mut args = vec![];
            if let Some(d) = &cli.data_dir {
                args.push("--data-dir".to_string());
                args.push(d.clone());
            }
            args.push("service".into());
            lwm_core::service::install(&lwm_core::service::ServiceSpec { name: SERVICE_NAME, display: "LAN Wake Manager Relay", description: "Sends Wake-on-LAN packets inside this LAN for LAN Wake Manager", exe, args })?;
            println!("Service '{SERVICE_NAME}' installed and started (listening on {}).", cfg.listen);
            println!("Run `lwm-relay pair-code` to get the pairing code for the controller.");
            Ok(())
        }
        Cmd::UninstallService => {
            lwm_core::service::uninstall(SERVICE_NAME)?;
            println!("Service removed.");
            Ok(())
        }
        Cmd::PairCode => {
            let cfg = prepare_config(&dir, None, None)?;
            let cert = lwm_core::tls::load_or_create(&dir, cert_names())?;
            let code = new_pairing(&dir)?;
            println!("Relay name       : {}", cfg.name);
            println!("Relay URL        : https://<this-computer-VPN-or-LAN-address>{}", port_suffix(&cfg.listen));
            println!("TLS fingerprint  : {}", cert.fingerprint);
            println!("Pairing code     : {code}   (valid 15 minutes, single use)");
            println!("Enter the URL and code in the controller (Locations > Relay) and compare the fingerprint it shows.");
            Ok(())
        }
        Cmd::Status => {
            let cfg = prepare_config(&dir, None, None)?;
            let cert = lwm_core::tls::load_or_create(&dir, cert_names())?;
            println!("relay id    : {}\nname        : {}\nlisten      : {}\npaired      : {}\nfingerprint : {}\nschedules   : {}", cfg.relay_id, cfg.name, cfg.listen, cfg.controller_token_hash.is_some(), cert.fingerprint, cfg.schedules.len());
            Ok(())
        }
    }
}

fn port_suffix(listen: &str) -> String {
    listen.rsplit_once(':').map(|(_, p)| format!(":{p}")).unwrap_or_default()
}
fn cert_names() -> Vec<String> {
    let mut n = vec!["localhost".to_string(), lwm_core::net::local_hostname()];
    for i in lwm_core::net::list_interfaces() {
        n.push(i.ip);
    }
    n.sort();
    n.dedup();
    n
}

fn prepare_config(dir: &std::path::Path, listen: Option<String>, name: Option<String>) -> Result<Config> {
    let mut c = load_config(dir);
    if c.relay_id.is_empty() {
        c.relay_id = new_id();
    }
    if c.name.is_empty() {
        c.name = lwm_core::net::local_hostname();
    }
    if c.listen.is_empty() {
        c.listen = "0.0.0.0:8444".into();
    }
    if let Some(l) = listen {
        c.listen = l;
    }
    if let Some(n) = name {
        c.name = n;
    }
    save_config(dir, &c)?;
    Ok(c)
}

#[cfg(windows)]
static SERVICE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
#[cfg(windows)]
fn service_entry(rx: tokio::sync::watch::Receiver<bool>) {
    let Some(dir) = SERVICE_DIR.get().cloned() else { return };
    let Ok(cfg) = prepare_config(&dir, None, None) else { return };
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread().enable_all().build() else { return };
    if let Err(e) = rt.block_on(run_relay(dir, cfg, rx)) {
        tracing::error!("relay stopped: {e:#}");
    }
}

async fn run_relay(dir: PathBuf, cfg: Config, shutdown: tokio::sync::watch::Receiver<bool>) -> Result<()> {
    let cert = lwm_core::tls::load_or_create(&dir, cert_names())?;
    let listen: SocketAddr = cfg.listen.parse().map_err(|_| anyhow!("invalid listen address {}", cfg.listen))?;
    let st = Arc::new(St { dir: dir.clone(), cfg: Mutex::new(cfg.clone()), fingerprint: cert.fingerprint.clone(), wakes: WakeJobs::new(), disc: DiscoveryJobs::new(), limiter: RateLimiter::new(), runs: Mutex::new(load_runs(&dir)) });
    if cfg.controller_token_hash.is_none() {
        let code = new_pairing(&dir)?;
        tracing::info!("relay is not paired. Pairing code: {code} (15 min). TLS fingerprint: {}", cert.fingerprint);
    }
    tokio::spawn(scheduler(st.clone(), shutdown.clone()));
    let allow_public = Arc::new(AtomicBool::new(false));
    let authed = Router::new()
        .route("/v1/interfaces", get(interfaces))
        .route("/v1/wake", post(wake_start))
        .route("/v1/wake/{id}", get(wake_get))
        .route("/v1/probe", post(probe))
        .route("/v1/discover", post(discover_start))
        .route("/v1/discover/{id}", get(discover_get).delete(discover_cancel))
        .route("/v1/diag/ping", post(diag_ping))
        .route("/v1/diag/dns", post(diag_dns))
        .route("/v1/diag/tcp", post(diag_tcp))
        .route("/v1/schedules", put(put_schedules).get(get_schedules))
        .route("/v1/runs", get(get_runs))
        .route("/v1/health", get(health))
        .layer(middleware::from_fn_with_state(st.clone(), auth));
    let app = Router::new()
        .route("/v1/info", get(info))
        .route("/v1/enroll", post(enroll))
        .merge(authed)
        .layer(middleware::from_fn(lwm_core::server::security_headers))
        .layer(middleware::from_fn_with_state(allow_public, lwm_core::server::private_only))
        .with_state(st);
    tracing::info!("relay listening on https://{listen} (fingerprint {})", cert.fingerprint);
    lwm_core::server::serve_tls(listen, app, &cert, shutdown).await
}

async fn auth(State(st): State<Arc<St>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, req: Request, next: Next) -> Result<Response, ApiError> {
    let token = lwm_core::server::bearer(req.headers()).ok_or_else(|| ApiError::unauthorized("missing token"))?;
    let expected = st.cfg.lock().unwrap().controller_token_hash.clone();
    match expected {
        Some(h) if lwm_core::util::ct_eq(&sha256_hex(token.as_bytes()), &h) => Ok(next.run(req).await),
        _ => {
            // only failed attempts count against the limiter
            st.limiter.check(&format!("auth-fail:{}", addr.ip()), 20, Duration::from_secs(60)).map_err(ApiError::rate_limited)?;
            Err(ApiError::unauthorized("invalid token"))
        }
    }
}

async fn info(State(st): State<Arc<St>>) -> Json<RelayInfo> {
    let c = st.cfg.lock().unwrap();
    Json(RelayInfo { relay_id: c.relay_id.clone(), name: c.name.clone(), version: lwm_core::VERSION.into(), paired: c.controller_token_hash.is_some(), tls_fingerprint: st.fingerprint.clone() })
}

#[derive(Deserialize)]
struct EnrollReq {
    code: String,
    controller_id: String,
}
async fn enroll(State(st): State<Arc<St>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(r): Json<EnrollReq>) -> ApiResult<serde_json::Value> {
    st.limiter.check(&format!("enroll:{}", addr.ip()), 5, Duration::from_secs(300)).map_err(ApiError::rate_limited)?;
    let p: Pairing = std::fs::read_to_string(st.dir.join("pairing.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).ok_or_else(|| ApiError::forbidden("no pairing code active; run `lwm-relay pair-code` on the relay"))?;
    if p.expires < now() || !lwm_core::util::ct_eq(&sha256_hex(normalize_code(&r.code).as_bytes()), &p.code_hash) {
        return Err(ApiError::forbidden("invalid or expired pairing code"));
    }
    let _ = std::fs::remove_file(st.dir.join("pairing.json")); // single use
    let token = random_token();
    let (relay_id, name) = {
        let mut c = st.cfg.lock().unwrap();
        c.controller_token_hash = Some(sha256_hex(token.as_bytes()));
        c.controller_id = Some(r.controller_id.clone());
        save_config(&st.dir, &c)?;
        (c.relay_id.clone(), c.name.clone())
    };
    tracing::info!("paired with controller {} from {}", r.controller_id, addr.ip());
    Ok(Json(serde_json::json!({"token": token, "relay_id": relay_id, "name": name, "version": lwm_core::VERSION})))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true, "time": now()}))
}
async fn interfaces() -> Json<Vec<IfaceInfo>> {
    Json(tokio::task::spawn_blocking(lwm_core::net::list_interfaces).await.unwrap_or_default())
}
async fn wake_start(State(st): State<Arc<St>>, Json(r): Json<WakeRequest>) -> ApiResult<serde_json::Value> {
    if lwm_core::util::parse_mac(&r.mac).is_none() {
        return Err(ApiError::bad_request("invalid MAC address"));
    }
    let id = st.wakes.start(r);
    Ok(Json(serde_json::json!({"job_id": id})))
}
async fn wake_get(State(st): State<Arc<St>>, Path(id): Path<String>) -> ApiResult<WakeStatus> {
    st.wakes.get(&id).map(Json).ok_or_else(|| ApiError::not_found("unknown job"))
}
#[derive(Deserialize)]
struct ProbeReq {
    targets: Vec<ProbeTarget>,
    timeout_ms: Option<u64>,
}
async fn probe(Json(r): Json<ProbeReq>) -> ApiResult<Vec<ProbeResult>> {
    if r.targets.len() > 512 {
        return Err(ApiError::bad_request("too many targets"));
    }
    Ok(Json(lwm_core::probe::probe_many(r.targets, r.timeout_ms.unwrap_or(1000).clamp(200, 5000), 32).await))
}
async fn discover_start(State(st): State<Arc<St>>, Json(r): Json<DiscoveryRequest>) -> ApiResult<serde_json::Value> {
    let id = st.disc.start(r).map_err(ApiError::bad_request)?;
    Ok(Json(serde_json::json!({"job_id": id})))
}
async fn discover_get(State(st): State<Arc<St>>, Path(id): Path<String>) -> ApiResult<DiscoveryStatus> {
    st.disc.get(&id).map(Json).ok_or_else(|| ApiError::not_found("unknown job"))
}
async fn discover_cancel(State(st): State<Arc<St>>, Path(id): Path<String>) -> ApiResult<serde_json::Value> {
    if st.disc.cancel(&id) {
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err(ApiError::not_found("unknown job"))
    }
}
#[derive(Deserialize)]
struct HostReq {
    host: String,
    port: Option<u16>,
}
async fn diag_ping(Json(r): Json<HostReq>) -> ApiResult<DiagResult> {
    Ok(Json(lwm_core::diag::ping(&r.host).await))
}
async fn diag_dns(Json(r): Json<HostReq>) -> ApiResult<DiagResult> {
    Ok(Json(lwm_core::diag::dns(&r.host).await))
}
async fn diag_tcp(Json(r): Json<HostReq>) -> ApiResult<DiagResult> {
    Ok(Json(lwm_core::diag::tcp(&r.host, r.port.unwrap_or(0)).await))
}
async fn put_schedules(State(st): State<Arc<St>>, Json(list): Json<Vec<RelaySchedule>>) -> ApiResult<serde_json::Value> {
    for s in &list {
        lwm_core::schedule::validate(&s.recurrence, s.start_date.as_deref(), s.end_date.as_deref()).map_err(ApiError::bad_request)?;
    }
    let n = list.len();
    {
        let mut c = st.cfg.lock().unwrap();
        c.schedules = list;
        save_config(&st.dir, &c)?;
    }
    Ok(Json(serde_json::json!({"ok": true, "count": n})))
}
async fn get_schedules(State(st): State<Arc<St>>) -> Json<Vec<RelaySchedule>> {
    Json(st.cfg.lock().unwrap().schedules.clone())
}
#[derive(Deserialize)]
struct SinceQ {
    since: Option<i64>,
}
async fn get_runs(State(st): State<Arc<St>>, Query(q): Query<SinceQ>) -> Json<Vec<RelayRun>> {
    let since = q.since.unwrap_or(0);
    Json(st.runs.lock().unwrap().iter().filter(|r| r.fired_at > since).cloned().collect())
}

fn load_runs(dir: &std::path::Path) -> Vec<RelayRun> {
    std::fs::read_to_string(dir.join("runs.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Executes the wake schedules pushed by the controller, independently of the controller being reachable.
async fn scheduler(st: Arc<St>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut cursors: HashMap<String, i64> = std::fs::read_to_string(st.dir.join("cursors.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(15)) => {}
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
        let scheds = st.cfg.lock().unwrap().schedules.clone();
        let t = now();
        for s in scheds.into_iter().filter(|s| s.enabled) {
            let cursor = *cursors.entry(s.id.clone()).or_insert(t);
            let zone = lwm_core::schedule::Zone::parse(s.timezone.as_deref());
            let spec = lwm_core::schedule::Spec { rec: &s.recurrence, start_date: s.start_date.as_deref(), end_date: s.end_date.as_deref(), exceptions: &s.exceptions };
            let d = lwm_core::schedule::due(&spec, zone, cursor, t, s.missed_grace_minutes as i64 * 60, s.missed_policy == "run_once");
            cursors.insert(s.id.clone(), t);
            if let lwm_core::schedule::Due::Fire(occ, missed) = d {
                tracing::info!("schedule '{}' fires (occurrence {occ}, missed={missed})", s.name);
                let st2 = st.clone();
                tokio::spawn(async move { run_schedule(st2, s, occ).await });
            }
        }
        let _ = std::fs::write(st.dir.join("cursors.json"), serde_json::to_string(&cursors).unwrap_or_default());
    }
}

async fn run_schedule(st: Arc<St>, s: RelaySchedule, occurrence: i64) {
    let attempts = s.retry.max_attempts.clamp(1, 10);
    let mut pending: Vec<WakeRequest> = s.targets.clone();
    let mut results: HashMap<String, String> = s.targets.iter().map(|t| (t.device_id.clone(), "pending".to_string())).collect();
    for attempt in 1..=attempts {
        let jobs: Vec<(String, String)> = pending.iter().map(|t| (t.device_id.clone(), st.wakes.start(t.clone()))).collect();
        for (dev, job) in jobs {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                match st.wakes.get(&job) {
                    Some(w) if w.state != "running" => {
                        results.insert(dev.clone(), if w.online { "succeeded".into() } else { w.state.clone() });
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        }
        pending.retain(|t| results.get(&t.device_id).map(|r| r != "succeeded").unwrap_or(true));
        if pending.is_empty() || attempt == attempts {
            break;
        }
        tokio::time::sleep(Duration::from_secs(s.retry.interval_secs as u64)).await;
    }
    let run = RelayRun { schedule_id: s.id.clone(), fired_at: occurrence, finished_at: now(), results: results.into_iter().collect() };
    let mut runs = st.runs.lock().unwrap();
    runs.push(run);
    let len = runs.len();
    if len > 200 {
        runs.drain(0..len - 200);
    }
    let _ = std::fs::write(st.dir.join("runs.json"), serde_json::to_string(&*runs).unwrap_or_default());
}
