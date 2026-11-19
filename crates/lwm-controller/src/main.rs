//! LAN Wake Manager controller: management API, scheduler, monitor, built-in wake relay and dashboard.
mod agent_api;
mod api;
mod auth;
mod backup;
mod connectivity;
mod db;
mod monitor;
mod ops;
mod relay;
mod scheduler;
mod state;
mod ui;
mod views;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use lwm_core::models::*;
use lwm_core::util::{new_id, now};
use state::{AppState, Config, Settings};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

const SERVICE_NAME: &str = "LANWakeController";

#[derive(Parser)]
#[command(name = "lwm-controller", version, about = "LAN Wake Manager - controller service")]
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
        /// HTTPS listen address for remote clients (default 0.0.0.0:8443)
        #[arg(long)]
        listen: Option<String>,
        /// Loopback HTTP address used by the desktop window (default 127.0.0.1:8480)
        #[arg(long)]
        local_listen: Option<String>,
    },
    /// Entry point used by the Windows Service Control Manager / systemd
    Service,
    /// Install and start the background service (run as administrator/root)
    InstallService {
        #[arg(long)]
        listen: Option<String>,
    },
    UninstallService,
    /// Remove the admin password so a new one can be set from the dashboard (local administrators only)
    ResetPassword,
    /// Print controller id and TLS fingerprint
    Info,
}

fn load_config(dir: &Path) -> Config {
    std::fs::read_to_string(dir.join("config.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save_config(dir: &Path, c: &Config) -> Result<()> {
    std::fs::write(dir.join("config.json"), serde_json::to_string_pretty(c)?)?;
    Ok(())
}

fn run_foreground(dir: PathBuf, cfg: Config) -> Result<()> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        serve(dir, cfg, rx).await
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = lwm_core::paths::data_dir("controller", cli.data_dir.as_deref());
    lwm_core::paths::ensure_private_dir(&dir)?;
    match cli.cmd {
        Cmd::Run { listen, local_listen } => {
            lwm_core::logging::init("lwm-controller");
            let mut cfg = load_config(&dir);
            if let Some(l) = listen {
                cfg.https_listen = l;
            }
            if let Some(l) = local_listen {
                cfg.local_listen = l;
            }
            run_foreground(dir, cfg)
        }
        Cmd::Service => {
            lwm_core::logging::init("lwm-controller-service");
            #[cfg(windows)]
            {
                SERVICE_DIR.set(dir).ok();
                lwm_core::service::win::run(SERVICE_NAME, service_entry)?;
                Ok(())
            }
            #[cfg(not(windows))]
            {
                let cfg = load_config(&dir);
                run_foreground(dir, cfg)
            }
        }
        Cmd::InstallService { listen } => {
            let mut cfg = load_config(&dir);
            if let Some(l) = listen {
                cfg.https_listen = l;
            }
            save_config(&dir, &cfg)?;
            let exe = std::env::current_exe()?;
            let mut args = vec![];
            if let Some(d) = &cli.data_dir {
                args.push("--data-dir".to_string());
                args.push(d.clone());
            }
            args.push("service".into());
            lwm_core::service::install(&lwm_core::service::ServiceSpec { name: SERVICE_NAME, display: "LAN Wake Manager Controller", description: "Manages computers, schedules and remote access for LAN Wake Manager", exe, args })?;
            println!("Service '{SERVICE_NAME}' installed and started. Dashboard: http://127.0.0.1:8480  Remote API: https://<this-computer>:8443");
            Ok(())
        }
        Cmd::UninstallService => {
            lwm_core::service::uninstall(SERVICE_NAME)?;
            println!("Service removed (your data in {} is kept).", dir.display());
            Ok(())
        }
        Cmd::ResetPassword => {
            let db = db::Db::open(&dir.join("controller.db"))?;
            db.with(|c| c.execute("DELETE FROM kv WHERE key='admin_password'", []).map(|_| ()))?;
            db.sessions_clear()?;
            db.audit("local-admin", "password_reset", None, "ok", Some("local"), Some("admin password removed from the command line"));
            println!("Admin password removed. Open the dashboard on this computer to set a new one. Paired phones keep working.");
            Ok(())
        }
        Cmd::Info => {
            let names = cert_names();
            let cert = lwm_core::tls::load_or_create(&dir, names)?;
            let db = db::Db::open(&dir.join("controller.db"))?;
            println!("controller id   : {}\nTLS fingerprint : {}\ndata directory  : {}", db.kv_get("controller_id")?.unwrap_or_default(), cert.fingerprint, dir.display());
            Ok(())
        }
    }
}

#[cfg(windows)]
static SERVICE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
#[cfg(windows)]
fn service_entry(rx: tokio::sync::watch::Receiver<bool>) {
    let Some(dir) = SERVICE_DIR.get().cloned() else { return };
    let cfg = load_config(&dir);
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread().enable_all().build() else { return };
    if let Err(e) = rt.block_on(serve(dir, cfg, rx)) {
        tracing::error!("controller stopped: {e:#}");
    }
}

fn cert_names() -> Vec<String> {
    let mut n = vec!["localhost".to_string(), "127.0.0.1".to_string(), lwm_core::net::local_hostname()];
    for i in lwm_core::net::list_interfaces() {
        n.push(i.ip);
    }
    n.sort();
    n.dedup();
    n
}

fn seed(db: &db::Db) -> Result<()> {
    if db.doc_list::<LocationRec>("locations")?.is_empty() {
        let l = LocationRec { id: new_id(), name: "Home Network".into(), color: "#2ee59d".into(), notes: "The LAN this controller is connected to (built-in Wake Relay).".into(), broadcast_addr: None, wol_ports: vec![9, 7], wake_attempts: 3, wake_interval_secs: 2, wake_timeout_secs: 120, relay_url: None, relay_fingerprint: None, relay_token: None, relay_name: None, relay_version: None, relay_last_seen: None, created_at: now() };
        db.doc_put("locations", &l.id, &l)?;
    }
    if db.kv_get("groups_seeded")?.is_none() {
        for (n, icon, color) in [("Workstations", "desktop", "#2ee59d"), ("Gaming PCs", "gamepad", "#a78bfa"), ("Servers", "server", "#22d3ee"), ("NAS", "nas", "#f59e0b"), ("Office Computers", "building", "#60a5fa")] {
            let g = Group { id: new_id(), name: n.into(), icon: icon.into(), color: color.into(), device_count: 0, online_count: 0 };
            db.doc_put("groups", &g.id, &g)?;
        }
        db.kv_set("groups_seeded", "1")?;
    }
    Ok(())
}

async fn serve(dir: PathBuf, cfg: Config, shutdown: tokio::sync::watch::Receiver<bool>) -> Result<()> {
    lwm_core::tls::install_provider();
    let db = Arc::new(db::Db::open(&dir.join("controller.db"))?);
    seed(&db)?;
    db.ops_fail_orphans()?;
    let controller_id = match db.kv_get("controller_id")? {
        Some(id) => id,
        None => {
            let id = new_id();
            db.kv_set("controller_id", &id)?;
            id
        }
    };
    let settings: Settings = db.kv_json("settings").unwrap_or_else(|| Settings::default_for(lwm_core::net::local_hostname()));
    let cert = lwm_core::tls::load_or_create(&dir, cert_names())?;
    let https: SocketAddr = cfg.https_listen.parse().map_err(|_| anyhow!("invalid HTTPS listen address {}", cfg.https_listen))?;
    let local: SocketAddr = cfg.local_listen.parse().map_err(|_| anyhow!("invalid local listen address {}", cfg.local_listen))?;
    if !local.ip().is_loopback() {
        return Err(anyhow!("the plain-HTTP dashboard listener must be bound to a loopback address"));
    }
    let (events, _) = tokio::sync::broadcast::channel(256);
    let st: state::St = Arc::new(AppState {
        db: db.clone(),
        dir: dir.clone(),
        controller_id: controller_id.clone(),
        fingerprint: cert.fingerprint.clone(),
        https_port: https.port(),
        allow_public: Arc::new(AtomicBool::new(settings.allow_public)),
        settings: RwLock::new(settings),
        events,
        limiter: lwm_core::ratelimit::RateLimiter::new(),
        wakes: lwm_core::jobs::WakeJobs::new(),
        disc: lwm_core::jobs::DiscoveryJobs::new(),
        remote_disc: Mutex::new(Default::default()),
        agent_queue: Mutex::new(Default::default()),
        agent_notify: tokio::sync::Notify::new(),
        cmd_waiters: Mutex::new(Default::default()),
        busy: Mutex::new(Default::default()),
        cancelled: Mutex::new(Default::default()),
        relay_online: Mutex::new(Default::default()),
        started: now(),
        shutdown: shutdown.clone(),
    });
    tokio::spawn(monitor::run(st.clone()));
    tokio::spawn(scheduler::run(st.clone()));
    tokio::spawn(backup::auto_loop(st.clone()));
    advertise(&st);
    let app = api::router(st.clone());
    tracing::info!("controller {controller_id} ready: https://{https} (fingerprint {}), dashboard http://{local}", cert.fingerprint);
    st.db.audit("system", "start", None, "ok", None, Some(lwm_core::VERSION));
    let a = lwm_core::server::serve_tls(https, app.clone(), &cert, shutdown.clone());
    let b = lwm_core::server::serve_plain(local, app, shutdown.clone());
    let (ra, rb) = tokio::join!(a, b);
    if let Err(e) = &ra {
        tracing::error!("HTTPS listener stopped: {e:#}");
    }
    if let Err(e) = &rb {
        tracing::error!("local listener stopped: {e:#}");
    }
    ra.and(rb)
}

/// Announce the controller on the LAN (mDNS) so that phones on the same network can find it (Direct LAN mode).
fn advertise(st: &state::St) {
    let Ok(d) = mdns_sd::ServiceDaemon::new() else { return };
    let host = format!("{}.local.", lwm_core::net::local_hostname().replace(' ', "-"));
    let props = [("id", st.controller_id.as_str()), ("fp", &st.fingerprint[..16]), ("v", "1")];
    match mdns_sd::ServiceInfo::new("_lanwake._tcp.local.", &format!("LANWake-{}", &st.controller_id[..8]), &host, "", st.https_port, &props[..]) {
        Ok(info) => {
            if let Err(e) = d.register(info.enable_addr_auto()) {
                tracing::debug!("mDNS announce failed: {e}");
            }
            // keep the daemon alive for the life of the process
            Box::leak(Box::new(d));
        }
        Err(e) => tracing::debug!("mDNS info error: {e}"),
    }
}
