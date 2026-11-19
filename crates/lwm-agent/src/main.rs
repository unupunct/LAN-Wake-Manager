//! LAN Wake Manager agent: reports status and executes the fixed set of power commands sent by the controller.
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use lwm_core::client::ApiClient;
use lwm_core::models::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SERVICE_NAME: &str = "LANWakeAgent";

#[derive(Parser)]
#[command(name = "lwm-agent", version, about = "LAN Wake Manager - computer agent")]
struct Cli {
    #[arg(long, global = true)]
    data_dir: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    /// Run in the foreground
    Run,
    /// Entry point used by the Windows Service Control Manager / systemd
    Service,
    /// Connect this computer to a controller using a one-time code from the dashboard
    Enroll {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        code: String,
        /// TLS fingerprint of the controller (shown in the dashboard)
        #[arg(long)]
        fingerprint: String,
    },
    /// Install and start the background service (run as administrator/root)
    InstallService,
    UninstallService,
    Status,
}

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    controller_url: String,
    fingerprint: String,
    token: String,
    device_id: String,
    controller_id: String,
}

fn cfg_path(dir: &Path) -> PathBuf {
    dir.join("agent.json")
}
fn load_cfg(dir: &Path) -> Option<Config> {
    std::fs::read_to_string(cfg_path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = lwm_core::paths::data_dir("agent", cli.data_dir.as_deref());
    lwm_core::paths::ensure_private_dir(&dir)?;
    match cli.cmd {
        Cmd::Run => {
            lwm_core::logging::init("lwm-agent");
            run_foreground(dir)
        }
        Cmd::Service => {
            lwm_core::logging::init("lwm-agent-service");
            #[cfg(windows)]
            {
                SERVICE_DIR.set(dir).ok();
                lwm_core::service::win::run(SERVICE_NAME, service_entry)?;
                Ok(())
            }
            #[cfg(not(windows))]
            run_foreground(dir)
        }
        Cmd::Enroll { controller, code, fingerprint } => {
            lwm_core::tls::install_provider();
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            rt.block_on(enroll(&dir, &controller, &code, &fingerprint))
        }
        Cmd::InstallService => {
            if load_cfg(&dir).is_none() {
                return Err(anyhow!("this computer is not enrolled yet; run `lwm-agent enroll ...` first"));
            }
            let exe = std::env::current_exe()?;
            let mut args = vec![];
            if let Some(d) = &cli.data_dir {
                args.push("--data-dir".to_string());
                args.push(d.clone());
            }
            args.push("service".into());
            lwm_core::service::install(&lwm_core::service::ServiceSpec { name: SERVICE_NAME, display: "LAN Wake Manager Agent", description: "Reports status and performs shutdown/restart requested by LAN Wake Manager", exe, args })?;
            println!("Service '{SERVICE_NAME}' installed and started.");
            Ok(())
        }
        Cmd::UninstallService => {
            lwm_core::service::uninstall(SERVICE_NAME)?;
            println!("Service removed.");
            Ok(())
        }
        Cmd::Status => {
            match load_cfg(&dir) {
                Some(c) => println!("enrolled with controller {} ({}), device id {}", c.controller_url, c.controller_id, c.device_id),
                None => println!("not enrolled"),
            }
            Ok(())
        }
    }
}

fn run_foreground(dir: PathBuf) -> Result<()> {
    lwm_core::tls::install_provider();
    let (tx, rx) = tokio::sync::watch::channel(false);
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        run_agent(dir, rx).await
    })
}

#[cfg(windows)]
static SERVICE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
#[cfg(windows)]
fn service_entry(rx: tokio::sync::watch::Receiver<bool>) {
    lwm_core::tls::install_provider();
    let Some(dir) = SERVICE_DIR.get().cloned() else { return };
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread().enable_all().build() else { return };
    if let Err(e) = rt.block_on(run_agent(dir, rx)) {
        tracing::error!("agent stopped: {e:#}");
    }
}

async fn enroll(dir: &Path, controller: &str, code: &str, fingerprint: &str) -> Result<()> {
    let url = controller.trim_end_matches('/').to_string();
    let api = ApiClient::new(&url, fingerprint, None, Duration::from_secs(15))?;
    let req = AgentEnrollRequest { code: code.to_string(), hostname: lwm_core::net::local_hostname(), os: std::env::consts::OS.to_string(), version: lwm_core::VERSION.into() };
    let r: AgentEnrollResponse = api.post("/api/v1/agent/enroll", &req).await.map_err(|e| anyhow!("enrollment failed: {e}"))?;
    let cfg = Config { controller_url: url, fingerprint: lwm_core::tls::normalize_fp(fingerprint), token: r.token, device_id: r.device_id.clone(), controller_id: r.controller_id };
    lwm_core::paths::write_secret(&cfg_path(dir), serde_json::to_string_pretty(&cfg)?.as_bytes())?;
    println!("Enrolled as device {}. Now run `lwm-agent install-service` (as administrator/root) to start it in the background.", r.device_id);
    Ok(())
}

async fn run_agent(dir: PathBuf, shutdown: tokio::sync::watch::Receiver<bool>) -> Result<()> {
    // The installer registers the service before the computer is enrolled: wait for `lwm-agent enroll`.
    let mut sd0 = shutdown.clone();
    let cfg = loop {
        if let Some(c) = load_cfg(&dir) {
            break c;
        }
        tracing::info!("not enrolled yet: run `lwm-agent enroll --controller https://HOST:8443 --code CODE --fingerprint FP` (waiting)");
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {}
            _ = sd0.changed() => { if *sd0.borrow() { return Ok(()); } }
        }
    };
    tracing::info!("agent {} connecting to {}", lwm_core::VERSION, cfg.controller_url);
    let beat = tokio::spawn(heartbeat_loop(cfg.clone(), shutdown.clone()));
    let cmds = tokio::spawn(command_loop(cfg, shutdown.clone()));
    let mut sd = shutdown;
    while !*sd.borrow() {
        if sd.changed().await.is_err() {
            break;
        }
    }
    beat.abort();
    cmds.abort();
    Ok(())
}

fn backoff(fails: u32) -> Duration {
    Duration::from_secs((2u64.pow(fails.min(5))).min(60))
}

async fn heartbeat_loop(cfg: Config, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let api = match ApiClient::new(&cfg.controller_url, &cfg.fingerprint, Some(cfg.token.clone()), Duration::from_secs(15)) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("cannot create client: {e:#}");
            return;
        }
    };
    let mut collector = lwm_core::sys::Collector::new();
    let mut first = true;
    let mut fails = 0u32;
    loop {
        let info = tokio::task::block_in_place(|| collector.collect(first));
        first = false;
        let hb = AgentHeartbeat { version: lwm_core::VERSION.into(), sysinfo: info };
        let wait = match api.post::<_, serde_json::Value>("/api/v1/agent/heartbeat", &hb).await {
            Ok(_) => {
                if fails > 0 {
                    tracing::info!("reconnected to controller");
                }
                fails = 0;
                Duration::from_secs(15)
            }
            Err(e) => {
                fails += 1;
                tracing::warn!("heartbeat failed ({fails}): {e}");
                backoff(fails)
            }
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
    }
}

async fn command_loop(cfg: Config, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let Ok(api) = ApiClient::new(&cfg.controller_url, &cfg.fingerprint, Some(cfg.token.clone()), Duration::from_secs(40)) else { return };
    let mut fails = 0u32;
    loop {
        if *shutdown.borrow() {
            return;
        }
        match api.get_timeout::<Vec<AgentCommand>>("/api/v1/agent/poll?wait=25", Duration::from_secs(40)).await {
            Ok(list) => {
                fails = 0;
                for c in list {
                    let res = execute(&c).await;
                    tracing::info!("command {} ({}) -> ok={} {}", c.id, c.kind, res.ok, res.message);
                    // the controller may be unreachable right after a shutdown starts; best effort
                    if let Err(e) = api.post::<_, serde_json::Value>(&format!("/api/v1/agent/commands/{}/result", c.id), &res).await {
                        tracing::warn!("could not report result: {e}");
                    }
                }
            }
            Err(e) => {
                fails += 1;
                tracing::warn!("poll failed ({fails}): {e}");
                tokio::select! {
                    _ = tokio::time::sleep(backoff(fails)) => {}
                    _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
                }
            }
        }
    }
}

async fn execute(c: &AgentCommand) -> AgentCommandResult {
    let os = if cfg!(windows) { "windows" } else { "linux" };
    let (prog, args) = match lwm_core::power::local_command(os, &c.kind, c.delay_secs) {
        Ok(x) => x,
        Err(e) => return AgentCommandResult { ok: false, message: e },
    };
    if lwm_core::power::dry_run() {
        return AgentCommandResult { ok: true, message: format!("dry run: would execute `{} {}`", prog, args.join(" ")) };
    }
    match lwm_core::cmd::run(&prog, &args, Duration::from_secs(30)).await {
        Ok((true, out)) => AgentCommandResult { ok: true, message: if out.is_empty() { format!("{} accepted by the operating system", c.kind) } else { out } },
        Ok((false, out)) => AgentCommandResult { ok: false, message: if out.is_empty() { "command failed".into() } else { out } },
        Err(e) => AgentCommandResult { ok: false, message: e },
    }
}
