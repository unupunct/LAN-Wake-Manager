//! Network helpers: interfaces, ARP, ping (via the OS tool), TCP probing, DNS.
use crate::models::IfaceInfo;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

pub fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || is_cgnat(ip)
}
/// 100.64.0.0/10 (Tailscale and other CGNAT ranges)
pub fn is_cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0xC0) == 0x40
}
/// Source addresses that may talk to the management API unless "allow_public" is on.
pub fn is_private_addr(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || is_private_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4.is_loopback() || is_private_v4(v4);
            }
            v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub fn kind_of_addr(ip: Ipv4Addr, iface_name: &str) -> &'static str {
    let n = iface_name.to_lowercase();
    if is_cgnat(ip) || n.contains("tailscale") {
        "tailscale"
    } else if n.contains("wg") || n.contains("wireguard") {
        "wireguard"
    } else if ip.is_private() || ip.is_link_local() {
        "lan"
    } else {
        "other"
    }
}

/// IPv4 interfaces that are up and not loopback.
pub fn list_interfaces() -> Vec<IfaceInfo> {
    let macs: HashMap<String, String> = {
        let nets = sysinfo::Networks::new_with_refreshed_list();
        nets.iter().map(|(n, d)| (n.clone(), d.mac_address().to_string())).collect()
    };
    let mut out = vec![];
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        if i.is_loopback() {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = &i.addr {
            if v4.ip.is_link_local() {
                continue;
            }
            let prefix = u32::from(v4.netmask).count_ones() as u8;
            let net = u32::from(v4.ip) & u32::from(v4.netmask);
            out.push(IfaceInfo {
                name: i.name.clone(),
                ip: v4.ip.to_string(),
                netmask: v4.netmask.to_string(),
                broadcast: v4.broadcast.map(|b| b.to_string()),
                subnet_cidr: format!("{}/{}", Ipv4Addr::from(net), prefix),
                mac: macs.get(&i.name).filter(|m| *m != "00:00:00:00:00:00").cloned(),
            });
        }
    }
    out
}

pub fn parse_ping_ttl(out: &str) -> Option<u32> {
    let low = out.to_lowercase();
    let idx = low.find("ttl=")?;
    low[idx + 4..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// ICMP echo through the OS `ping` tool (no raw sockets / elevated rights needed).
/// Returns (rtt, ttl) when the host answered.
pub async fn ping(ip: &str, timeout_ms: u64) -> Option<(u64, Option<u32>)> {
    if std::env::var("LWM_NO_ICMP").map(|v| v == "1").unwrap_or(false) {
        return None;
    }
    if !crate::util::is_safe_host(ip) {
        return None;
    }
    let args: Vec<String> = if cfg!(windows) {
        vec!["-n".into(), "1".into(), "-w".into(), timeout_ms.to_string(), ip.into()]
    } else {
        vec!["-c".into(), "1".into(), "-W".into(), (timeout_ms.div_ceil(1000)).max(1).to_string(), ip.into()]
    };
    let started = Instant::now();
    let (ok, out) = crate::cmd::run("ping", &args, Duration::from_millis(timeout_ms + 2500)).await.ok()?;
    // Windows prints "Destination host unreachable" with exit code 0 in some cases; require a TTL.
    let ttl = parse_ping_ttl(&out);
    if ok && ttl.is_some() {
        Some((started.elapsed().as_millis() as u64, ttl))
    } else {
        None
    }
}

pub async fn tcp_open(ip: &str, port: u16, timeout_ms: u64) -> bool {
    let Ok(addr) = format!("{ip}:{port}").parse::<SocketAddr>().or_else(|_| ip.parse::<IpAddr>().map(|i| SocketAddr::new(i, port))) else {
        return false;
    };
    matches!(tokio::time::timeout(Duration::from_millis(timeout_ms), tokio::net::TcpStream::connect(addr)).await, Ok(Ok(_)))
}

pub fn parse_arp_windows(out: &str) -> HashMap<Ipv4Addr, String> {
    let mut m = HashMap::new();
    for l in out.lines() {
        let p: Vec<&str> = l.split_whitespace().collect();
        if p.len() >= 3 {
            if let (Ok(ip), Some(mac)) = (p[0].parse::<Ipv4Addr>(), crate::util::normalize_mac(p[1])) {
                if mac != "FF:FF:FF:FF:FF:FF" && !mac.starts_with("01:00:5E") && p[2].eq_ignore_ascii_case("dynamic") {
                    m.insert(ip, mac);
                }
            }
        }
    }
    m
}
pub fn parse_arp_linux(out: &str) -> HashMap<Ipv4Addr, String> {
    // /proc/net/arp: IP HWtype Flags HWaddress Mask Device
    let mut m = HashMap::new();
    for l in out.lines().skip(1) {
        let p: Vec<&str> = l.split_whitespace().collect();
        if p.len() >= 4 && p[2] != "0x0" {
            if let (Ok(ip), Some(mac)) = (p[0].parse::<Ipv4Addr>(), crate::util::normalize_mac(p[3])) {
                if mac != "00:00:00:00:00:00" {
                    m.insert(ip, mac);
                }
            }
        }
    }
    m
}
/// Current ARP/neighbour table (only hosts this machine recently talked to).
pub fn arp_table() -> HashMap<Ipv4Addr, String> {
    if cfg!(windows) {
        crate::cmd::run_quiet("arp", &["-a"]).map(|o| parse_arp_windows(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
    } else {
        std::fs::read_to_string("/proc/net/arp").map(|s| parse_arp_linux(&s)).unwrap_or_default()
    }
}

pub async fn reverse_dns(ip: Ipv4Addr) -> Option<String> {
    let r = tokio::time::timeout(Duration::from_millis(1500), tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&IpAddr::V4(ip)))).await;
    match r {
        Ok(Ok(Ok(name))) if name != ip.to_string() => Some(name),
        _ => None,
    }
}

pub async fn resolve(host: &str) -> Result<Vec<IpAddr>, String> {
    let h = format!("{host}:0");
    tokio::time::timeout(Duration::from_secs(4), tokio::net::lookup_host(h))
        .await
        .map_err(|_| "DNS lookup timed out".to_string())?
        .map(|i| i.map(|s| s.ip()).collect())
        .map_err(|e| e.to_string())
}

pub fn local_hostname() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "controller".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_ranges() {
        assert!(is_private_addr("192.168.1.5".parse().unwrap()));
        assert!(is_private_addr("10.1.2.3".parse().unwrap()));
        assert!(is_private_addr("100.64.1.2".parse().unwrap()));
        assert!(is_private_addr("127.0.0.1".parse().unwrap()));
        assert!(is_private_addr("fd00::1".parse().unwrap()));
        assert!(!is_private_addr("8.8.8.8".parse().unwrap()));
        assert!(!is_private_addr("100.128.0.1".parse().unwrap()));
        assert!(!is_private_addr("2001:db8::1".parse().unwrap()));
    }
    #[test]
    fn ttl_parse() {
        assert_eq!(parse_ping_ttl("Reply from 1.2.3.4: bytes=32 time<1ms TTL=128"), Some(128));
        assert_eq!(parse_ping_ttl("64 bytes from x: icmp_seq=1 ttl=63 time=0.4 ms"), Some(63));
        assert_eq!(parse_ping_ttl("Destination host unreachable"), None);
    }
    #[test]
    fn arp_parse() {
        let w = "  192.168.1.1           aa-bb-cc-dd-ee-ff     dynamic   \n  192.168.1.255         ff-ff-ff-ff-ff-ff     static    \n";
        let m = parse_arp_windows(w);
        assert_eq!(m.len(), 1);
        assert_eq!(m[&"192.168.1.1".parse::<Ipv4Addr>().unwrap()], "AA:BB:CC:DD:EE:FF");
        let l = "IP address       HW type     Flags       HW address            Mask     Device\n192.168.1.7    0x1         0x2         11:22:33:44:55:66     *        eth0\n192.168.1.9    0x1         0x0         00:00:00:00:00:00     *        eth0\n";
        assert_eq!(parse_arp_linux(l).len(), 1);
    }
}
