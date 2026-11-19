//! Builds the API views (live status merged into stored records).
use crate::db::DeviceState;
use crate::state::St;
use lwm_core::models::*;
use lwm_core::util::now;

pub const AGENT_ONLINE_SECS: i64 = 45;

pub fn agent_online(s: &DeviceState) -> bool {
    s.agent_heartbeat.map(|h| now() - h <= AGENT_ONLINE_SECS).unwrap_or(false)
}

pub fn device_view(r: &DeviceRec, s: &DeviceState) -> Device {
    let enrolled = r.agent_token_hash.is_some();
    let state = if !enrolled {
        "none"
    } else if agent_online(s) {
        "online"
    } else {
        "offline"
    };
    Device {
        id: r.id.clone(),
        location_id: r.location_id.clone(),
        name: r.name.clone(),
        hostname: r.hostname.clone(),
        ip: r.ip.clone(),
        mac: r.mac.clone(),
        os: r.os.clone(),
        device_type: r.device_type.clone(),
        wol_capable: r.wol_capable.clone(),
        group_ids: r.group_ids.clone(),
        tcp_check_ports: r.tcp_check_ports.clone(),
        power_method: r.power_method.clone(),
        ssh: r.ssh.clone(),
        notes: r.notes.clone(),
        status: s.status.clone(),
        status_checked_at: s.checked_at,
        last_seen: s.last_seen,
        status_changed_at: s.changed_at,
        agent: AgentInfo { state: state.into(), version: s.agent_version.clone(), last_heartbeat: s.agent_heartbeat, enrolled },
        sysinfo: if enrolled { s.sysinfo.clone() } else { None },
    }
}

pub fn devices(st: &St, location: Option<&str>, group: Option<&str>) -> anyhow::Result<Vec<Device>> {
    let recs = st.db.devices(location)?;
    let states = st.db.states()?;
    Ok(recs
        .iter()
        .filter(|r| group.map(|g| r.group_ids.iter().any(|x| x == g)).unwrap_or(true))
        .map(|r| device_view(r, states.get(&r.id).unwrap_or(&DeviceState { status: "unknown".into(), ..Default::default() })))
        .collect())
}

pub fn device(st: &St, id: &str) -> anyhow::Result<Option<Device>> {
    let Some(r) = st.db.device_get(id)? else { return Ok(None) };
    let s = st.db.state_get(id)?;
    Ok(Some(device_view(&r, &s)))
}

pub async fn locations(st: &St) -> anyhow::Result<Vec<Location>> {
    let recs: Vec<LocationRec> = st.db.doc_list("locations")?;
    let devs = devices(st, None, None)?;
    let conn = crate::connectivity::get(false).await;
    let local_vpn = if conn.tailscale.running {
        "tailscale"
    } else if conn.addresses.iter().any(|a| a.kind == "wireguard") {
        "wireguard"
    } else {
        "lan"
    };
    Ok(recs
        .iter()
        .map(|l| {
            let mine: Vec<&Device> = devs.iter().filter(|d| d.location_id == l.id).collect();
            let remote = l.relay_url.is_some();
            let status = if !remote {
                "builtin".to_string()
            } else if l.relay_token.is_none() {
                "unpaired".to_string()
            } else if l.relay_last_seen.map(|t| now() - t < 60).unwrap_or(false) {
                "online".to_string()
            } else {
                "offline".to_string()
            };
            let vpn = if remote {
                let host = l.relay_url.as_deref().and_then(|u| lwm_core::tls::split_https_url(u).ok()).map(|x| x.0).unwrap_or_default();
                match host.parse::<std::net::Ipv4Addr>() {
                    Ok(ip) if lwm_core::net::is_cgnat(ip) => "tailscale",
                    Ok(ip) if ip.is_private() => "lan",
                    _ if host.ends_with(".ts.net") => "tailscale",
                    _ => "none",
                }
            } else {
                local_vpn
            };
            Location {
                id: l.id.clone(),
                name: l.name.clone(),
                color: l.color.clone(),
                notes: l.notes.clone(),
                broadcast_addr: l.broadcast_addr.clone(),
                wol_ports: l.wol_ports.clone(),
                wake_attempts: l.wake_attempts,
                wake_interval_secs: l.wake_interval_secs,
                wake_timeout_secs: l.wake_timeout_secs,
                relay_mode: if remote { "remote".into() } else { "builtin".into() },
                relay_url: l.relay_url.clone(),
                relay_status: status,
                relay_last_seen: l.relay_last_seen,
                relay_version: l.relay_version.clone(),
                relay_name: l.relay_name.clone(),
                device_count: mine.len() as u32,
                online_count: mine.iter().filter(|d| d.status == "online").count() as u32,
                controller_status: "online".into(),
                vpn: vpn.into(),
            }
        })
        .collect())
}

pub fn groups(st: &St) -> anyhow::Result<Vec<Group>> {
    let mut gs: Vec<Group> = st.db.doc_list("groups")?;
    let devs = devices(st, None, None)?;
    for g in gs.iter_mut() {
        let mine: Vec<&Device> = devs.iter().filter(|d| d.group_ids.contains(&g.id)).collect();
        g.device_count = mine.len() as u32;
        g.online_count = mine.iter().filter(|d| d.status == "online").count() as u32;
    }
    Ok(gs)
}
