//! Availability monitoring: probes every device through its location's relay and merges agent heartbeats.
use crate::state::St;
use crate::views::agent_online;
use lwm_core::models::*;
use lwm_core::util::now;
use std::collections::HashMap;
use std::time::Duration;

pub async fn run(st: St) {
    let mut shutdown = st.shutdown.clone();
    // first pass shortly after start
    tokio::time::sleep(Duration::from_secs(2)).await;
    loop {
        if let Err(e) = cycle(&st).await {
            tracing::warn!("monitor cycle failed: {e:#}");
        }
        let secs = st.settings().monitor_interval_secs.clamp(5, 600) as u64;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
    }
}

pub async fn cycle(st: &St) -> anyhow::Result<()> {
    let locs: Vec<LocationRec> = st.db.doc_list("locations")?;
    let mut handles = vec![];
    for loc in locs {
        let st = st.clone();
        handles.push(tokio::spawn(async move { location_cycle(&st, loc).await }));
    }
    for h in handles {
        let _ = h.await;
    }
    Ok(())
}

pub async fn location_cycle(st: &St, mut loc: LocationRec) {
    let devices = st.db.devices(Some(&loc.id)).unwrap_or_default();
    let relay = match crate::relay::for_location(st, &loc) {
        Ok(r) => r,
        Err(_) => {
            relay_seen(st, &mut loc, false);
            return;
        }
    };
    let targets: Vec<ProbeTarget> = devices.iter().filter_map(|d| d.ip.as_ref().map(|ip| ProbeTarget { id: d.id.clone(), ip: ip.clone(), tcp_ports: crate::ops::effective_ports(d) })).collect();
    let results: HashMap<String, ProbeResult> = if targets.is_empty() {
        let ok = relay.health().await.is_ok();
        relay_seen(st, &mut loc, ok);
        HashMap::new()
    } else {
        match relay.probe(targets).await {
            Ok(r) => {
                relay_seen(st, &mut loc, true);
                r.into_iter().map(|p| (p.id.clone(), p)).collect()
            }
            Err(_) => {
                relay_seen(st, &mut loc, false);
                // relay unreachable: only agent heartbeats can tell us anything; keep statuses otherwise
                HashMap::new()
            }
        }
    };
    let relay_ok = !results.is_empty() || devices.iter().all(|d| d.ip.is_none());
    for d in devices {
        let s = st.db.state_get(&d.id).unwrap_or_default();
        let agent_up = agent_online(&s);
        let probe = results.get(&d.id);
        if probe.is_none() && !agent_up && !relay_ok && d.ip.is_some() {
            continue; // no information this cycle
        }
        let alive = probe.map(|p| p.alive).unwrap_or(false) || agent_up;
        let busy = st.busy.lock().unwrap().contains(&d.id);
        let new_status = match (s.status.as_str(), alive) {
            ("waking", false) if busy => "waking",
            ("shutting_down", true) if busy => "shutting_down",
            (_, true) => "online",
            (_, false) => "offline",
        };
        if let Ok(Some(old)) = st.db.state_set_status(&d.id, new_status, alive) {
            st.emit(serde_json::json!({"type": "device_status", "device_id": d.id, "status": new_status, "last_seen": if alive { Some(now()) } else { None }}));
            if old == "online" && new_status == "offline" && !busy {
                st.notify("device_unavailable", "warn", &format!("{} is offline", d.name), "The computer stopped answering.", Some(&d.id));
            }
        }
    }
}

fn relay_seen(st: &St, loc: &mut LocationRec, ok: bool) {
    if loc.relay_url.is_none() {
        return;
    }
    let was = st.relay_online.lock().unwrap().insert(loc.id.clone(), ok);
    if ok {
        loc.relay_last_seen = Some(now());
        // re-read to avoid overwriting concurrent edits
        if let Ok(Some(mut cur)) = st.db.doc_get::<LocationRec>("locations", &loc.id) {
            cur.relay_last_seen = loc.relay_last_seen;
            let _ = st.db.doc_put("locations", &cur.id, &cur);
        }
    }
    if was != Some(ok) {
        let status = if ok { "online" } else { "offline" };
        st.emit(serde_json::json!({"type": "relay_status", "location_id": loc.id, "relay_status": status}));
        if was.is_some() || !ok {
            if ok {
                st.notify("relay_online", "info", &format!("Wake Relay '{}' is online", loc.name), "The relay is reachable again.", None);
            } else {
                st.notify("relay_offline", "error", &format!("Wake Relay '{}' is offline", loc.name), "Wake requests for this location will fail until it is reachable.", None);
            }
        }
    }
}
