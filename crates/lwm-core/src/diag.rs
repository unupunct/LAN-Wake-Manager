//! Diagnostics primitives (ping / DNS / TCP) returning user-readable results.
use crate::models::DiagResult;
use std::time::Instant;

pub async fn ping(host: &str) -> DiagResult {
    let t = Instant::now();
    if !crate::util::is_safe_host(host) {
        return DiagResult { ok: false, detail: "invalid host".into(), duration_ms: 0 };
    }
    match crate::net::ping(host, 2000).await {
        Some((rtt, ttl)) => DiagResult { ok: true, detail: format!("reply in {rtt} ms{}", ttl.map(|t| format!(", TTL {t}")).unwrap_or_default()), duration_ms: t.elapsed().as_millis() as u64 },
        None => DiagResult { ok: false, detail: "no ICMP reply (host down, or it blocks ping - try a TCP check)".into(), duration_ms: t.elapsed().as_millis() as u64 },
    }
}
pub async fn dns(host: &str) -> DiagResult {
    let t = Instant::now();
    if !crate::util::is_safe_host(host) {
        return DiagResult { ok: false, detail: "invalid host".into(), duration_ms: 0 };
    }
    match crate::net::resolve(host).await {
        Ok(ips) if !ips.is_empty() => DiagResult { ok: true, detail: ips.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "), duration_ms: t.elapsed().as_millis() as u64 },
        Ok(_) => DiagResult { ok: false, detail: "no addresses returned".into(), duration_ms: t.elapsed().as_millis() as u64 },
        Err(e) => DiagResult { ok: false, detail: e, duration_ms: t.elapsed().as_millis() as u64 },
    }
}
pub async fn tcp(host: &str, port: u16) -> DiagResult {
    let t = Instant::now();
    if !crate::util::is_safe_host(host) || port == 0 {
        return DiagResult { ok: false, detail: "invalid host or port".into(), duration_ms: 0 };
    }
    let ok = crate::net::tcp_open(host, port, 2500).await;
    DiagResult { ok, detail: if ok { format!("port {port} is open") } else { format!("port {port} did not accept a connection (closed, filtered or host down)") }, duration_ms: t.elapsed().as_millis() as u64 }
}
