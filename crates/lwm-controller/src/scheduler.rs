//! Persistent schedule engine. Runs inside the service (no GUI needed) and syncs wake schedules to relays.
use crate::state::St;
use lwm_core::models::*;
use lwm_core::schedule::{due, next_after, Due, Spec, Zone};
use lwm_core::util::now;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

static APPROACH_NOTIFIED: Mutex<Option<HashSet<(String, i64)>>> = Mutex::new(None);

pub fn zone_for(st: &St, _s: &Schedule) -> Zone {
    Zone::parse(st.settings().timezone.as_deref())
}

pub fn next_run(st: &St, s: &Schedule) -> Option<i64> {
    if !s.enabled {
        return None;
    }
    let spec = Spec { rec: &s.recurrence, start_date: s.start_date.as_deref(), end_date: s.end_date.as_deref(), exceptions: &s.exceptions };
    next_after(&spec, zone_for(st, s), now())
}

/// Resolve schedule targets to distinct device ids.
pub fn resolve_targets(st: &St, t: &ScheduleTarget) -> Vec<DeviceRec> {
    let all = st.db.devices(None).unwrap_or_default();
    all.into_iter().filter(|d| t.device_ids.contains(&d.id) || t.location_ids.contains(&d.location_id) || d.group_ids.iter().any(|g| t.group_ids.contains(g))).collect()
}

pub async fn run(st: St) {
    let mut shutdown = st.shutdown.clone();
    let mut tick = 0u64;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(15)) => {}
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
        tick += 1;
        if let Err(e) = evaluate(&st).await {
            tracing::warn!("scheduler: {e:#}");
        }
        if tick % 20 == 1 {
            sync_relays(&st).await;
        }
    }
}

pub async fn evaluate(st: &St) -> anyhow::Result<()> {
    let scheds: Vec<Schedule> = st.db.doc_list("schedules")?;
    let t = now();
    let mins = st.settings().notify_schedule_minutes as i64;
    for s in scheds.into_iter().filter(|s| s.enabled && s.executor == "controller") {
        let zone = zone_for(st, &s);
        let spec = Spec { rec: &s.recurrence, start_date: s.start_date.as_deref(), end_date: s.end_date.as_deref(), exceptions: &s.exceptions };
        let mut cursor = st.db.schedule_cursor(&s.id);
        if cursor == 0 {
            cursor = t; // new schedule: never fire for the past
            st.db.schedule_set_cursor(&s.id, cursor);
        }
        let d = due(&spec, zone, cursor, t, s.missed_grace_minutes as i64 * 60, s.missed_policy == "run_once");
        st.db.schedule_set_cursor(&s.id, t);
        if let Due::Fire(occ, missed) = d {
            if st.db.schedule_claim(&s.id, occ) {
                tracing::info!("schedule '{}' fires (occurrence {occ}, missed={missed})", s.name);
                fire(st.clone(), s.clone(), occ);
            }
        } else if mins > 0 {
            if let Some(n) = next_after(&spec, zone, t) {
                if n - t <= mins * 60 {
                    let mut g = APPROACH_NOTIFIED.lock().unwrap();
                    let set = g.get_or_insert_with(HashSet::new);
                    if set.insert((s.id.clone(), n)) {
                        st.notify("schedule_approaching", "info", &format!("Schedule '{}' runs soon", s.name), &format!("{} in about {} min", s.action, ((n - t) / 60).max(1)), None);
                    }
                    if set.len() > 500 {
                        set.clear();
                    }
                }
            }
        }
    }
    Ok(())
}

/// Execute a schedule now (also used by "run now"). Applies the retry policy to failed targets.
pub fn fire(st: St, s: Schedule, occurrence: i64) {
    tokio::spawn(async move {
        let devices = resolve_targets(&st, &s.target);
        if devices.is_empty() {
            update_last(&st, &s.id, "failed", occurrence);
            st.notify("operation_failed", "error", &format!("Schedule '{}' has no targets", s.name), "No computer matches the schedule's targets.", None);
            return;
        }
        let attempts = s.retry.max_attempts.clamp(1, 10);
        let mut pending: Vec<String> = devices.iter().map(|d| d.id.clone()).collect();
        let mut ok = 0usize;
        let total = pending.len();
        for attempt in 1..=attempts {
            let mut ops = vec![];
            for id in &pending {
                match crate::ops::start(&st, id, &s.action, s.delay_secs, crate::ops::Origin { requested_by: format!("schedule:{}", s.name), source: "schedule", schedule_id: Some(s.id.clone()) }) {
                    Ok(o) => ops.push(o),
                    Err(e) => tracing::warn!("schedule target failed: {}", e.message),
                }
            }
            let mut failed = vec![];
            for o in ops {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    match st.db.op_get(&o.id) {
                        Ok(Some(cur)) if cur.state != "pending" && cur.state != "running" => {
                            if cur.state == "succeeded" {
                                ok += 1;
                            } else {
                                failed.push(cur.device_id.clone());
                            }
                            break;
                        }
                        Ok(Some(_)) => {}
                        _ => break,
                    }
                }
            }
            pending = failed;
            if pending.is_empty() || attempt == attempts {
                break;
            }
            tracing::info!("schedule '{}': {} target(s) failed, retry {}/{} in {} s", s.name, pending.len(), attempt, attempts, s.retry.interval_secs);
            tokio::time::sleep(Duration::from_secs(s.retry.interval_secs as u64)).await;
        }
        let state = if ok == total {
            "succeeded"
        } else if ok == 0 {
            "failed"
        } else {
            "partial"
        };
        update_last(&st, &s.id, state, occurrence);
        st.emit(serde_json::json!({"type": "schedule_fired", "schedule_id": s.id}));
    });
}

fn update_last(st: &St, id: &str, state: &str, occurrence: i64) {
    if let Ok(Some(mut s)) = st.db.doc_get::<Schedule>("schedules", id) {
        s.last_run = Some(occurrence.max(now().min(occurrence + 86400)));
        s.last_state = Some(state.into());
        let _ = st.db.doc_put("schedules", id, &s);
    }
}

/// Push wake schedules with executor=relay to their relays and pull their run results.
pub async fn sync_relays(st: &St) {
    let scheds: Vec<Schedule> = st.db.doc_list("schedules").unwrap_or_default();
    let locs: Vec<LocationRec> = st.db.doc_list("locations").unwrap_or_default();
    for loc in locs.into_iter().filter(|l| l.relay_url.is_some() && l.relay_token.is_some()) {
        let Ok(relay) = crate::relay::for_location(st, &loc) else { continue };
        let crate::relay::Relay::Remote(client) = relay else { continue };
        let mut list: Vec<RelaySchedule> = vec![];
        for s in scheds.iter().filter(|s| s.executor == "relay" && s.action == "wake") {
            let devs: Vec<DeviceRec> = resolve_targets(st, &s.target).into_iter().filter(|d| d.location_id == loc.id).collect();
            if devs.is_empty() {
                continue;
            }
            list.push(RelaySchedule {
                id: s.id.clone(),
                name: s.name.clone(),
                enabled: s.enabled,
                recurrence: s.recurrence.clone(),
                start_date: s.start_date.clone(),
                end_date: s.end_date.clone(),
                exceptions: s.exceptions.clone(),
                missed_policy: s.missed_policy.clone(),
                missed_grace_minutes: s.missed_grace_minutes,
                retry: s.retry.clone(),
                timezone: st.settings().timezone.clone(),
                targets: devs
                    .iter()
                    .map(|d| WakeRequest { device_id: d.id.clone(), mac: d.mac.clone(), ip: d.ip.clone(), tcp_ports: crate::ops::effective_ports(d), broadcast_addr: loc.broadcast_addr.clone(), wol_ports: loc.wol_ports.clone(), attempts: loc.wake_attempts, interval_secs: loc.wake_interval_secs, timeout_secs: loc.wake_timeout_secs })
                    .collect(),
            });
        }
        if let Err(e) = client.put::<_, serde_json::Value>("/v1/schedules", &list).await {
            tracing::warn!("could not sync schedules to relay '{}': {e}", loc.name);
            continue;
        }
        let since: i64 = st.db.kv_get(&format!("relay_runs_since:{}", loc.id)).ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0);
        if let Ok(runs) = client.get::<Vec<RelayRun>>(&format!("/v1/runs?since={since}")).await {
            let mut max = since;
            for r in runs {
                max = max.max(r.fired_at);
                let okc = r.results.iter().filter(|x| x.1 == "succeeded").count();
                let state = if okc == r.results.len() { "succeeded" } else if okc == 0 { "failed" } else { "partial" };
                if let Ok(Some(mut s)) = st.db.doc_get::<Schedule>("schedules", &r.schedule_id) {
                    s.last_run = Some(r.fired_at);
                    s.last_state = Some(state.into());
                    let _ = st.db.doc_put("schedules", &s.id, &s);
                    st.db.audit(&format!("relay:{}", loc.name), "schedule_run", Some(&s.name), state, None, Some(&format!("{okc}/{} woke up", r.results.len())));
                    if state != "succeeded" {
                        st.notify("operation_failed", "error", &format!("Relay schedule '{}' incomplete", s.name), &format!("{okc}/{} computers came online", r.results.len()), None);
                    }
                }
            }
            let _ = st.db.kv_set(&format!("relay_runs_since:{}", loc.id), &max.to_string());
        }
    }
}
