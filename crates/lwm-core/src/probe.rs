//! Availability probing (ICMP via OS ping + configurable TCP checks).
use crate::models::{ProbeResult, ProbeTarget};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub async fn probe_one(t: &ProbeTarget, timeout_ms: u64) -> ProbeResult {
    let ping_fut = crate::net::ping(&t.ip, timeout_ms);
    let tcp_futs = t.tcp_ports.iter().map(|p| {
        let ip = t.ip.clone();
        let p = *p;
        async move { (p, crate::net::tcp_open(&ip, p, timeout_ms).await) }
    });
    let (ping, tcp) = tokio::join!(ping_fut, futures_join(tcp_futs));
    let mut via = vec![];
    if ping.is_some() {
        via.push("ping".to_string());
    }
    for (p, ok) in &tcp {
        if *ok {
            via.push(format!("tcp/{p}"));
        }
    }
    ProbeResult { id: t.id.clone(), alive: !via.is_empty(), via, rtt_ms: ping.map(|p| p.0), ttl: ping.and_then(|p| p.1) }
}

async fn futures_join<I, F, T>(it: I) -> Vec<T>
where
    I: IntoIterator<Item = F>,
    F: std::future::Future<Output = T>,
{
    let mut set = vec![];
    for f in it {
        set.push(f);
    }
    // sequential join keeps core dependency-free; port lists are tiny.
    let mut out = vec![];
    for f in set {
        out.push(f.await);
    }
    out
}

/// Probe many targets with bounded concurrency.
pub async fn probe_many(targets: Vec<ProbeTarget>, timeout_ms: u64, concurrency: usize) -> Vec<ProbeResult> {
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut handles = vec![];
    for t in targets {
        let sem = sem.clone();
        handles.push(tokio::spawn(async move {
            let _p = sem.acquire().await.ok();
            probe_one(&t, timeout_ms).await
        }));
    }
    let mut out = vec![];
    for h in handles {
        if let Ok(r) = h.await {
            out.push(r);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn tcp_probe_detects_listener() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let r = probe_one(&ProbeTarget { id: "x".into(), ip: "127.0.0.1".into(), tcp_ports: vec![port] }, 800).await;
        assert!(r.alive);
        assert!(r.via.iter().any(|v| v == &format!("tcp/{port}")));
    }
    #[tokio::test]
    async fn unreachable_is_not_alive() {
        // TEST-NET-1 is guaranteed not to answer
        let r = probe_one(&ProbeTarget { id: "x".into(), ip: "192.0.2.1".into(), tcp_ports: vec![9] }, 600).await;
        assert!(!r.alive);
    }
}
