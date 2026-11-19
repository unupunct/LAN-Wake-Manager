//! LAN discovery: ping sweep, TCP checks, ARP table, mDNS. Scans only explicitly selected subnets.
use crate::models::{Candidate, DiscoveryRequest, IfaceInfo};
use ipnet::Ipv4Net;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Default)]
pub struct ScanCtl {
    pub cancel: AtomicBool,
    pub done: AtomicU32,
    pub total: AtomicU32,
}

pub const DEFAULT_PORTS: &[u16] = &[22, 80, 135, 139, 443, 445, 3389, 5000, 5985];
/// Largest network that may be scanned in one job (/20 = 4094 hosts).
pub const MIN_PREFIX: u8 = 20;

pub fn resolve_subnet(req: &DiscoveryRequest, ifaces: &[IfaceInfo]) -> Result<Ipv4Net, String> {
    let net: Ipv4Net = if let Some(s) = &req.subnet {
        s.parse().map_err(|_| format!("invalid subnet '{s}' (expected CIDR like 192.168.1.0/24)"))?
    } else if let Some(n) = &req.interface {
        let i = ifaces.iter().find(|i| &i.name == n).ok_or_else(|| format!("unknown interface '{n}'"))?;
        i.subnet_cidr.parse().map_err(|_| "interface has no usable subnet".to_string())?
    } else {
        return Err("choose a network interface or enter a subnet to scan".into());
    };
    if net.prefix_len() < MIN_PREFIX {
        return Err(format!("refusing to scan a network larger than /{MIN_PREFIX}; choose a smaller subnet"));
    }
    if crate::net::is_cgnat(net.network()) {
        return Err("scanning VPN (100.64.0.0/10) address space is not allowed".into());
    }
    Ok(net)
}

pub fn guess_os(ports: &[u16], ttl: Option<u32>) -> Option<String> {
    let has = |p: u16| ports.contains(&p);
    if has(3389) || has(135) || has(5985) || (has(445) && ttl.map(|t| t > 100).unwrap_or(false)) {
        return Some("windows".into());
    }
    if let Some(t) = ttl {
        if t > 100 {
            return Some("windows".into());
        }
        if t <= 64 && (has(22) || has(80) || has(443) || has(445)) {
            return Some("linux".into());
        }
    }
    if has(22) {
        return Some("linux".into());
    }
    None
}
pub fn guess_type(os: &Option<String>, ports: &[u16]) -> String {
    if ports.contains(&5000) || ports.contains(&5001) {
        return "nas".into();
    }
    match os.as_deref() {
        Some("windows") => "pc".into(),
        Some("linux") if ports.contains(&22) => "server".into(),
        _ => "other".into(),
    }
}

async fn mdns_browse(window: Duration) -> HashMap<Ipv4Addr, String> {
    tokio::task::spawn_blocking(move || {
        let mut out = HashMap::new();
        let Ok(d) = mdns_sd::ServiceDaemon::new() else { return out };
        let types = ["_workstation._tcp.local.", "_ssh._tcp.local.", "_smb._tcp.local.", "_device-info._tcp.local.", "_http._tcp.local."];
        let rxs: Vec<_> = types.iter().filter_map(|t| d.browse(t).ok()).collect();
        let end = std::time::Instant::now() + window;
        while std::time::Instant::now() < end {
            for rx in &rxs {
                if let Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) = rx.try_recv() {
                    let host = info.get_hostname().trim_end_matches('.').trim_end_matches(".local").to_string();
                    for a in info.get_addresses() {
                        if let IpAddr::V4(v4) = a {
                            out.insert(*v4, host.clone());
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = d.shutdown();
        out
    })
    .await
    .unwrap_or_default()
}

pub async fn scan(req: &DiscoveryRequest, ifaces: &[IfaceInfo], ctl: Arc<ScanCtl>) -> Result<Vec<Candidate>, String> {
    let net = resolve_subnet(req, ifaces)?;
    let methods: HashSet<String> = req.methods.clone().unwrap_or_else(|| vec!["arp".into(), "ping".into(), "tcp".into(), "mdns".into()]).into_iter().collect();
    let timeout = req.timeout_ms.unwrap_or(800).clamp(100, 5000);
    let conc = match req.speed.as_deref() {
        Some("slow") => 16,
        Some("fast") => 192,
        _ => 64,
    };
    let ports = req.tcp_ports.clone().unwrap_or_else(|| DEFAULT_PORTS.to_vec());
    let own: HashSet<Ipv4Addr> = ifaces.iter().filter_map(|i| i.ip.parse().ok()).collect();
    let hosts: Vec<Ipv4Addr> = net.hosts().filter(|h| !own.contains(h)).collect();
    ctl.total.store(hosts.len() as u32 + 1, Ordering::Relaxed);

    // mDNS runs alongside the sweep
    let mdns_handle = if methods.contains("mdns") { Some(tokio::spawn(mdns_browse(Duration::from_secs(3)))) } else { None };

    // poke every host with UDP so the OS fills its ARP table (works even if ping/TCP are filtered)
    if methods.contains("arp") {
        tokio::task::spawn_blocking({
            let hosts = hosts.clone();
            move || {
                if let Ok(s) = std::net::UdpSocket::bind("0.0.0.0:0") {
                    for h in hosts {
                        let _ = s.send_to(&[0u8], (h, 9));
                    }
                }
            }
        });
    }

    let sem = Arc::new(Semaphore::new(conc));
    let mut handles = vec![];
    for h in hosts.iter().copied() {
        let sem = sem.clone();
        let ctl = ctl.clone();
        let ports = ports.clone();
        let methods = methods.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            if ctl.cancel.load(Ordering::Relaxed) {
                return None;
            }
            let ip = h.to_string();
            let mut found_methods = vec![];
            let mut ttl = None;
            let mut open = vec![];
            if methods.contains("ping") {
                if let Some((_, t)) = crate::net::ping(&ip, timeout).await {
                    found_methods.push("ping".to_string());
                    ttl = t;
                }
            }
            if methods.contains("tcp") {
                for p in &ports {
                    if crate::net::tcp_open(&ip, *p, timeout.min(600)).await {
                        open.push(*p);
                    }
                }
                if !open.is_empty() {
                    found_methods.push("tcp".to_string());
                }
            }
            ctl.done.fetch_add(1, Ordering::Relaxed);
            Some((h, found_methods, ttl, open))
        }));
    }
    let mut hits: HashMap<Ipv4Addr, (Vec<String>, Option<u32>, Vec<u16>)> = HashMap::new();
    for hd in handles {
        if let Ok(Some((h, m, ttl, open))) = hd.await {
            if !m.is_empty() {
                hits.insert(h, (m, ttl, open));
            }
        }
    }
    if ctl.cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }

    let arp = if methods.contains("arp") { crate::net::arp_table() } else { HashMap::new() };
    let mdns = match mdns_handle {
        Some(h) => h.await.unwrap_or_default(),
        None => HashMap::new(),
    };
    let mut ips: HashSet<Ipv4Addr> = hits.keys().copied().collect();
    for (ip, _) in arp.iter() {
        if net.contains(ip) && !own.contains(ip) && *ip != net.broadcast() {
            ips.insert(*ip);
        }
    }
    for ip in mdns.keys() {
        if net.contains(ip) && !own.contains(ip) {
            ips.insert(*ip);
        }
    }
    let mut out = vec![];
    for ip in ips {
        let (mut m, ttl, open) = hits.get(&ip).cloned().unwrap_or_default();
        let mac = arp.get(&ip).cloned();
        if mac.is_some() {
            m.push("arp".into());
        }
        let mut hostname = mdns.get(&ip).cloned();
        if hostname.is_some() {
            m.push("mdns".into());
        } else if methods.contains("arp") || methods.contains("ping") {
            hostname = crate::net::reverse_dns(ip).await;
        }
        let os = guess_os(&open, ttl);
        let device_type = guess_type(&os, &open);
        out.push(Candidate { ip: ip.to_string(), mac, hostname, os, device_type, vendor: None, open_ports: open, methods: m, registered: false, device_id: None });
    }
    out.sort_by_key(|c| c.ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(0));
    ctl.done.store(ctl.total.load(Ordering::Relaxed), Ordering::Relaxed);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(s: Option<&str>) -> DiscoveryRequest {
        DiscoveryRequest { subnet: s.map(|s| s.to_string()), interface: None, speed: None, timeout_ms: None, tcp_ports: None, methods: None }
    }
    #[test]
    fn subnet_rules() {
        assert!(resolve_subnet(&req(Some("192.168.1.0/24")), &[]).is_ok());
        assert!(resolve_subnet(&req(Some("10.0.0.0/8")), &[]).is_err());
        assert!(resolve_subnet(&req(Some("100.64.0.0/24")), &[]).is_err());
        assert!(resolve_subnet(&req(None), &[]).is_err());
        assert!(resolve_subnet(&req(Some("nonsense")), &[]).is_err());
    }
    #[test]
    fn os_guess() {
        assert_eq!(guess_os(&[445, 3389], Some(128)).as_deref(), Some("windows"));
        assert_eq!(guess_os(&[22], Some(64)).as_deref(), Some("linux"));
        assert_eq!(guess_os(&[], None), None);
        assert_eq!(guess_type(&Some("linux".into()), &[22]), "server");
    }
    #[tokio::test]
    async fn scans_loopback_range_for_listener() {
        // a /30 around 127.0.0.0 contains 127.0.0.1 and .2; listen on 127.0.0.1 only.
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let r = DiscoveryRequest { subnet: Some("127.0.0.0/30".into()), interface: None, speed: Some("fast".into()), timeout_ms: Some(300), tcp_ports: Some(vec![port]), methods: Some(vec!["tcp".into()]) };
        let res = scan(&r, &[], Arc::new(ScanCtl::default())).await.unwrap();
        assert!(res.iter().any(|c| c.ip == "127.0.0.1" && c.open_ports == vec![port]));
        assert!(!res.iter().any(|c| c.ip == "127.0.0.2"));
    }
}
