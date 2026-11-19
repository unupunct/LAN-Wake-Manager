//! Wake-on-LAN magic packets.
use crate::models::IfaceInfo;
use crate::util::parse_mac;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

/// 6 x 0xFF followed by the MAC address repeated 16 times (102 bytes).
pub fn magic_packet(mac: &[u8; 6]) -> Vec<u8> {
    let mut p = vec![0xFFu8; 6];
    for _ in 0..16 {
        p.extend_from_slice(mac);
    }
    p
}

/// A single transmission path: bind to `src` (None = OS choice) and send to `dst`.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub src: Option<Ipv4Addr>,
    pub dst: Ipv4Addr,
    pub iface: String,
}

fn in_subnet(ip: Ipv4Addr, i: &IfaceInfo) -> bool {
    let (Ok(addr), Ok(mask)) = (i.ip.parse::<Ipv4Addr>(), i.netmask.parse::<Ipv4Addr>()) else { return false };
    (u32::from(ip) & u32::from(mask)) == (u32::from(addr) & u32::from(mask))
}

/// Decide where to send packets. Prefers the interface that is on the target's subnet and
/// uses the subnet-directed broadcast plus the limited broadcast. VPN (CGNAT) interfaces are skipped.
pub fn plan_paths(ifaces: &[IfaceInfo], override_bcast: Option<&str>, device_ip: Option<&str>) -> Vec<Path> {
    let dev: Option<Ipv4Addr> = device_ip.and_then(|s| s.parse().ok());
    let usable: Vec<&IfaceInfo> = ifaces
        .iter()
        .filter(|i| i.ip.parse::<Ipv4Addr>().map(|a| !crate::net::is_cgnat(a) && !a.is_link_local()).unwrap_or(false))
        .collect();
    let mut chosen: Vec<&IfaceInfo> = match dev {
        Some(d) => usable.iter().copied().filter(|i| in_subnet(d, i)).collect(),
        None => vec![],
    };
    if chosen.is_empty() {
        chosen = usable.clone();
    }
    let mut out: Vec<Path> = vec![];
    let mut push = |p: Path| {
        if !out.contains(&p) {
            out.push(p)
        }
    };
    if let Some(b) = override_bcast.and_then(|b| b.parse::<Ipv4Addr>().ok()) {
        let src = usable.iter().find(|i| i.broadcast.as_deref().and_then(|x| x.parse::<Ipv4Addr>().ok()) == Some(b) || in_subnet(b, i));
        push(Path { src: src.and_then(|i| i.ip.parse().ok()), dst: b, iface: src.map(|i| i.name.clone()).unwrap_or_else(|| "default".into()) });
        return out;
    }
    for i in chosen {
        let src: Option<Ipv4Addr> = i.ip.parse().ok();
        if let Some(b) = i.broadcast.as_deref().and_then(|b| b.parse::<Ipv4Addr>().ok()) {
            push(Path { src, dst: b, iface: i.name.clone() });
        }
        push(Path { src, dst: Ipv4Addr::BROADCAST, iface: i.name.clone() });
    }
    if out.is_empty() {
        out.push(Path { src: None, dst: Ipv4Addr::BROADCAST, iface: "default".into() });
    }
    out
}

pub fn send_to(src: Option<Ipv4Addr>, dst: SocketAddr, packet: &[u8]) -> std::io::Result<()> {
    let bind = SocketAddr::V4(SocketAddrV4::new(src.unwrap_or(Ipv4Addr::UNSPECIFIED), 0));
    let sock = UdpSocket::bind(bind)?;
    sock.set_broadcast(true)?;
    sock.send_to(packet, dst)?;
    Ok(())
}

/// Send one round of magic packets (every path x every port). Returns human readable step lines.
pub fn send_round(mac: &str, paths: &[Path], ports: &[u16]) -> Result<Vec<String>, String> {
    let m = parse_mac(mac).ok_or_else(|| format!("invalid MAC address {mac}"))?;
    let pkt = magic_packet(&m);
    let mut lines = vec![];
    let mut sent = 0;
    for p in paths {
        for port in ports {
            match send_to(p.src, SocketAddr::V4(SocketAddrV4::new(p.dst, *port)), &pkt) {
                Ok(()) => {
                    sent += 1;
                    lines.push(format!("magic packet sent via {} ({}) -> {}:{}", p.iface, p.src.map(|s| s.to_string()).unwrap_or_else(|| "auto".into()), p.dst, port));
                }
                Err(e) => lines.push(format!("send via {} -> {}:{} failed: {}", p.iface, p.dst, port, e)),
            }
        }
    }
    if sent == 0 {
        Err(format!("could not transmit any packet: {}", lines.join("; ")))
    } else {
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn iface(name: &str, ip: &str, mask: &str, b: &str) -> IfaceInfo {
        IfaceInfo { name: name.into(), ip: ip.into(), netmask: mask.into(), broadcast: Some(b.into()), subnet_cidr: String::new(), mac: None }
    }
    #[test]
    fn packet_layout() {
        let mac = [1, 2, 3, 4, 5, 6];
        let p = magic_packet(&mac);
        assert_eq!(p.len(), 102);
        assert!(p[..6].iter().all(|b| *b == 0xFF));
        for i in 0..16 {
            assert_eq!(&p[6 + i * 6..12 + i * 6], &mac);
        }
    }
    #[test]
    fn planning_prefers_target_subnet_and_skips_vpn() {
        let ifs = vec![
            iface("eth0", "192.168.1.5", "255.255.255.0", "192.168.1.255"),
            iface("eth1", "10.0.0.5", "255.255.255.0", "10.0.0.255"),
            iface("tailscale0", "100.64.0.5", "255.192.0.0", "100.127.255.255"),
        ];
        let p = plan_paths(&ifs, None, Some("10.0.0.20"));
        assert!(p.iter().all(|x| x.iface == "eth1"));
        assert!(p.iter().any(|x| x.dst == "10.0.0.255".parse::<Ipv4Addr>().unwrap()));
        let all = plan_paths(&ifs, None, None);
        assert!(all.iter().all(|x| x.iface != "tailscale0"));
        assert!(all.iter().any(|x| x.iface == "eth0") && all.iter().any(|x| x.iface == "eth1"));
        let o = plan_paths(&ifs, Some("192.168.1.255"), None);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].src, Some("192.168.1.5".parse().unwrap()));
    }
    #[test]
    fn packet_really_goes_over_udp() {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        rx.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
        let port = rx.local_addr().unwrap().port();
        let mac = parse_mac("AA:BB:CC:DD:EE:FF").unwrap();
        send_to(None, format!("127.0.0.1:{port}").parse().unwrap(), &magic_packet(&mac)).unwrap();
        let mut buf = [0u8; 200];
        let (n, _) = rx.recv_from(&mut buf).unwrap();
        assert_eq!(n, 102);
        assert_eq!(&buf[6..12], &mac);
    }
}
