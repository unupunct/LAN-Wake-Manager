//! Long-running jobs shared by the relay and the controller's built-in relay: wake supervision and discovery.
use crate::discovery::{scan, ScanCtl};
use crate::models::*;
use crate::util::{new_id, now};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Shared<T> = Arc<Mutex<T>>;

#[derive(Default, Clone)]
pub struct WakeJobs {
    jobs: Arc<Mutex<HashMap<String, (Shared<WakeStatus>, Instant)>>>,
}

fn step(st: &Shared<WakeStatus>, text: impl Into<String>) {
    st.lock().unwrap().steps.push(OpStep { t: now(), text: text.into() });
}

impl WakeJobs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, id: &str) -> Option<WakeStatus> {
        self.jobs.lock().unwrap().get(id).map(|(s, _)| s.lock().unwrap().clone())
    }
    fn gc(&self) {
        let mut j = self.jobs.lock().unwrap();
        j.retain(|_, (s, t)| s.lock().unwrap().state == "running" || t.elapsed() < Duration::from_secs(3600));
    }
    /// Start a wake job; returns the job id immediately.
    pub fn start(&self, req: WakeRequest) -> String {
        self.gc();
        let id = new_id();
        let st = Arc::new(Mutex::new(WakeStatus { job_id: id.clone(), state: "running".into(), steps: vec![], online: false, elapsed_secs: 0, detail: None }));
        self.jobs.lock().unwrap().insert(id.clone(), (st.clone(), Instant::now()));
        tokio::spawn(run_wake(req, st));
        id
    }
}

async fn is_up(req: &WakeRequest) -> Option<Vec<String>> {
    let ip = req.ip.as_deref()?;
    let r = crate::probe::probe_one(&ProbeTarget { id: req.device_id.clone(), ip: ip.to_string(), tcp_ports: req.tcp_ports.clone() }, 1000).await;
    if r.alive {
        Some(r.via)
    } else {
        None
    }
}

async fn run_wake(req: WakeRequest, st: Shared<WakeStatus>) {
    let started = Instant::now();
    let finish = |state: &str, online: bool, detail: String| {
        let mut s = st.lock().unwrap();
        s.state = state.to_string();
        s.online = online;
        s.elapsed_secs = started.elapsed().as_secs();
        s.detail = Some(detail);
    };
    if crate::util::parse_mac(&req.mac).is_none() {
        finish("failed", false, format!("invalid MAC address '{}'", req.mac));
        return;
    }
    if let Some(via) = is_up(&req).await {
        step(&st, format!("already reachable ({}), no packet needed", via.join(", ")));
        finish("online", true, format!("already online ({})", via.join(", ")));
        return;
    }
    let ifaces = tokio::task::spawn_blocking(crate::net::list_interfaces).await.unwrap_or_default();
    let paths = crate::wol::plan_paths(&ifaces, req.broadcast_addr.as_deref(), req.ip.as_deref());
    step(&st, format!("target {}; {} transmission path(s): {}", req.mac, paths.len(), paths.iter().map(|p| format!("{}->{}", p.iface, p.dst)).collect::<Vec<_>>().join(", ")));
    let ports = if req.wol_ports.is_empty() { vec![9, 7] } else { req.wol_ports.clone() };
    let attempts = req.attempts.clamp(1, 20);
    let interval = Duration::from_secs(req.interval_secs.clamp(1, 60) as u64);
    let timeout = Duration::from_secs(req.timeout_secs.clamp(5, 900) as u64);
    let can_verify = req.ip.is_some();
    if !can_verify {
        step(&st, "no IP address registered for this computer: the packet can be sent but the start-up cannot be verified");
    }
    let mut sent_attempts = 0u32;
    let mut next_send = Instant::now();
    loop {
        if sent_attempts < attempts && Instant::now() >= next_send {
            sent_attempts += 1;
            match crate::wol::send_round(&req.mac, &paths, &ports) {
                Ok(lines) => {
                    step(&st, format!("attempt {sent_attempts}/{attempts}: {} packet(s) sent", lines.len()));
                    for l in lines.into_iter().take(6) {
                        step(&st, l);
                    }
                }
                Err(e) => {
                    step(&st, format!("attempt {sent_attempts}/{attempts}: {e}"));
                    if sent_attempts == 1 {
                        finish("failed", false, e);
                        return;
                    }
                }
            }
            next_send = Instant::now() + interval;
        }
        if can_verify {
            if let Some(via) = is_up(&req).await {
                step(&st, format!("computer became reachable after {} s ({})", started.elapsed().as_secs(), via.join(", ")));
                finish("online", true, format!("online after {} s, confirmed by {}", started.elapsed().as_secs(), via.join(" and ")));
                return;
            }
        }
        if !can_verify && sent_attempts >= attempts {
            finish("timeout", false, "magic packets sent, but there is no IP address to verify start-up".into());
            return;
        }
        if started.elapsed() >= timeout {
            break;
        }
        {
            st.lock().unwrap().elapsed_secs = started.elapsed().as_secs();
        }
        tokio::time::sleep(Duration::from_millis(1200)).await;
    }
    finish(
        "timeout",
        false,
        format!("{} packet round(s) sent, but the computer did not become reachable within {} s. Check Wake-on-LAN in the BIOS/adapter settings, the MAC address, and that this LAN's relay is on the same subnet.", sent_attempts, timeout.as_secs()),
    );
}

// ---------------- discovery jobs ----------------

struct DiscJob {
    ctl: Arc<ScanCtl>,
    state: String,
    error: Option<String>,
    results: Vec<Candidate>,
    created: Instant,
}

#[derive(Default, Clone)]
pub struct DiscoveryJobs {
    jobs: Arc<Mutex<HashMap<String, Shared<DiscJob>>>>,
}

impl DiscoveryJobs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn start(&self, req: DiscoveryRequest) -> Result<String, String> {
        let ifaces = crate::net::list_interfaces();
        // validate synchronously so the caller gets a proper error
        crate::discovery::resolve_subnet(&req, &ifaces)?;
        {
            let mut j = self.jobs.lock().unwrap();
            j.retain(|_, v| {
                let v = v.lock().unwrap();
                v.state == "running" || v.created.elapsed() < Duration::from_secs(1800)
            });
            if j.values().filter(|v| v.lock().unwrap().state == "running").count() >= 2 {
                return Err("too many scans running; wait for one to finish or cancel it".into());
            }
        }
        let id = new_id();
        let ctl = Arc::new(ScanCtl::default());
        let job = Arc::new(Mutex::new(DiscJob { ctl: ctl.clone(), state: "running".into(), error: None, results: vec![], created: Instant::now() }));
        self.jobs.lock().unwrap().insert(id.clone(), job.clone());
        tokio::spawn(async move {
            let r = scan(&req, &ifaces, ctl.clone()).await;
            let mut j = job.lock().unwrap();
            match r {
                Ok(res) => {
                    j.results = res;
                    j.state = "done".into();
                }
                Err(e) if e == "cancelled" => j.state = "cancelled".into(),
                Err(e) => {
                    j.state = "failed".into();
                    j.error = Some(e);
                }
            }
        });
        Ok(id)
    }
    pub fn get(&self, id: &str) -> Option<DiscoveryStatus> {
        let j = self.jobs.lock().unwrap();
        let job = j.get(id)?.lock().unwrap();
        Some(DiscoveryStatus {
            job_id: id.to_string(),
            state: job.state.clone(),
            progress: Progress { done: job.ctl.done.load(Ordering::Relaxed), total: job.ctl.total.load(Ordering::Relaxed) },
            error: job.error.clone(),
            results: job.results.clone(),
        })
    }
    pub fn cancel(&self, id: &str) -> bool {
        match self.jobs.lock().unwrap().get(id) {
            Some(j) => {
                j.lock().unwrap().ctl.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn wake_confirms_when_target_comes_up() {
        std::env::set_var("LWM_NO_ICMP", "1"); // loopback always answers ping
        // The "computer" is a local TCP listener that only starts after the first packet round.
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe); // closed -> target initially down
        let jobs = WakeJobs::new();
        let id = jobs.start(WakeRequest { device_id: "d".into(), mac: "AA:BB:CC:DD:EE:FF".into(), ip: Some("127.0.0.1".into()), tcp_ports: vec![port], broadcast_addr: None, wol_ports: vec![9], attempts: 2, interval_secs: 1, timeout_secs: 20 });
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(jobs.get(&id).unwrap().state, "running", "must not claim success before the host is reachable");
        let _l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap(); // machine "boots"
        let mut state = String::new();
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(700)).await;
            state = jobs.get(&id).unwrap().state;
            if state != "running" {
                break;
            }
        }
        let s = jobs.get(&id).unwrap();
        assert_eq!(state, "online", "{:?}", s);
        assert!(s.online);
        assert!(s.steps.iter().any(|x| x.text.contains("packet")));
    }
    #[tokio::test]
    async fn wake_times_out_honestly() {
        let jobs = WakeJobs::new();
        let id = jobs.start(WakeRequest { device_id: "d".into(), mac: "AA:BB:CC:DD:EE:FF".into(), ip: Some("192.0.2.55".into()), tcp_ports: vec![9], broadcast_addr: Some("127.0.0.1".into()), wol_ports: vec![9], attempts: 1, interval_secs: 1, timeout_secs: 5 });
        let mut s = jobs.get(&id).unwrap();
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            s = jobs.get(&id).unwrap();
            if s.state != "running" {
                break;
            }
        }
        assert_eq!(s.state, "timeout", "{:?}", s);
        assert!(!s.online);
    }
}
