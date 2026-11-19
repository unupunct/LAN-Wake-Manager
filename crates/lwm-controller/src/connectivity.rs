//! Detects Tailscale / WireGuard / LAN addresses of this controller. Read-only: it never touches
//! Tailscale credentials, keys or ACLs.
use lwm_core::net::kind_of_addr;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Tailscale {
    pub installed: bool,
    pub running: bool,
    pub backend_state: Option<String>,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub magicdns_name: Option<String>,
    pub tailnet: Option<String>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Addr {
    pub address: String,
    pub kind: String,
    pub interface: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Connectivity {
    pub tailscale: Tailscale,
    pub addresses: Vec<Addr>,
}

fn tailscale_binary() -> Option<String> {
    let candidates: Vec<&str> = if cfg!(windows) {
        vec!["tailscale", "C:\\Program Files\\Tailscale\\tailscale.exe", "C:\\Program Files (x86)\\Tailscale\\tailscale.exe"]
    } else {
        vec!["tailscale", "/usr/bin/tailscale", "/usr/local/bin/tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"]
    };
    for c in candidates {
        if let Ok(o) = lwm_core::cmd::run_quiet(c, &["version"]) {
            if o.status.success() {
                return Some(c.to_string());
            }
        }
    }
    None
}

/// Parse the relevant parts of `tailscale status --json`.
pub fn parse_status(json: &str) -> Tailscale {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return Tailscale { installed: true, error: Some(format!("unreadable status output: {e}")), ..Default::default() },
    };
    let backend = v["BackendState"].as_str().map(|s| s.to_string());
    let me = &v["Self"];
    let ip = me["TailscaleIPs"].as_array().and_then(|a| a.iter().filter_map(|x| x.as_str()).find(|s| s.parse::<Ipv4Addr>().is_ok())).map(|s| s.to_string());
    let dns = me["DNSName"].as_str().map(|s| s.trim_end_matches('.').to_string()).filter(|s| !s.is_empty());
    let running = backend.as_deref() == Some("Running");
    Tailscale {
        installed: true,
        running,
        backend_state: backend.clone(),
        ip,
        hostname: me["HostName"].as_str().map(|s| s.to_string()),
        magicdns_name: dns,
        tailnet: v["CurrentTailnet"]["Name"].as_str().map(|s| s.to_string()),
        error: if running { None } else { Some(match backend.as_deref() { Some("NeedsLogin") => "Tailscale needs you to log in".to_string(), Some("Stopped") => "Tailscale is installed but turned off".to_string(), Some(o) => format!("Tailscale state: {o}"), None => "Tailscale reported no state".to_string() }) },
    }
}

async fn detect_tailscale() -> Tailscale {
    let bin = tokio::task::spawn_blocking(tailscale_binary).await.ok().flatten();
    let mut t = match bin {
        Some(b) => match lwm_core::cmd::run(&b, &["status".to_string(), "--json".to_string()], Duration::from_secs(6)).await {
            Ok((_, out)) if out.trim_start().starts_with('{') => parse_status(&out),
            Ok((_, out)) => Tailscale { installed: true, error: Some(format!("tailscale status failed: {}", out.lines().next().unwrap_or(""))), ..Default::default() },
            Err(e) => Tailscale { installed: true, error: Some(e), ..Default::default() },
        },
        None => Tailscale { installed: false, error: Some("the tailscale command was not found".into()), ..Default::default() },
    };
    // Fall back to the interface address: Tailscale (even the GUI-only client) always owns a 100.64/10 address.
    if t.ip.is_none() {
        if let Some(i) = lwm_core::net::list_interfaces().into_iter().find(|i| i.ip.parse::<Ipv4Addr>().map(lwm_core::net::is_cgnat).unwrap_or(false)) {
            t.ip = Some(i.ip);
            if !t.installed {
                t.installed = true;
                t.running = true;
                t.error = Some("detected through the network interface (tailscale command not available)".into());
            }
        }
    }
    t
}

static CACHE: Mutex<Option<(Instant, Connectivity)>> = Mutex::new(None);

pub async fn get(force: bool) -> Connectivity {
    if !force {
        if let Some((t, c)) = CACHE.lock().unwrap().as_ref() {
            if t.elapsed() < Duration::from_secs(30) {
                return c.clone();
            }
        }
    }
    let tailscale = detect_tailscale().await;
    let addresses = tokio::task::spawn_blocking(|| {
        lwm_core::net::list_interfaces()
            .into_iter()
            .filter_map(|i| {
                let ip: Ipv4Addr = i.ip.parse().ok()?;
                Some(Addr { address: i.ip.clone(), kind: kind_of_addr(ip, &i.name).to_string(), interface: i.name })
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    let c = Connectivity { tailscale, addresses };
    *CACHE.lock().unwrap() = Some((Instant::now(), c.clone()));
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_running_status() {
        let j = r#"{"BackendState":"Running","Self":{"HostName":"pc","DNSName":"pc.tail1234.ts.net.","TailscaleIPs":["100.64.1.2","fd7a::1"]},"CurrentTailnet":{"Name":"me@example.com"}}"#;
        let t = parse_status(j);
        assert!(t.running && t.installed);
        assert_eq!(t.ip.as_deref(), Some("100.64.1.2"));
        assert_eq!(t.magicdns_name.as_deref(), Some("pc.tail1234.ts.net"));
        assert!(t.error.is_none());
    }
    #[test]
    fn explains_stopped() {
        let t = parse_status(r#"{"BackendState":"Stopped","Self":{}}"#);
        assert!(!t.running);
        assert!(t.error.unwrap().contains("turned off"));
    }
}
