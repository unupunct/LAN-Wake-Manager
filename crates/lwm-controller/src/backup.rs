//! Database backups, restore, and configuration export/import (never contains secrets).
use crate::state::St;
use lwm_core::models::*;
use lwm_core::server::ApiError;
use lwm_core::util::now;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupEntry {
    pub name: String,
    pub created_at: i64,
    pub size_bytes: u64,
    pub kind: String,
}

pub fn dir(st: &St) -> PathBuf {
    st.dir.join("backups")
}

pub fn valid_name(n: &str) -> bool {
    n.starts_with("lwm-") && n.ends_with(".db") && n.len() < 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

pub fn create(st: &St, kind: &str) -> Result<BackupEntry, ApiError> {
    let d = dir(st);
    std::fs::create_dir_all(&d).map_err(|e| ApiError::internal(e.to_string()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("lwm-{stamp}-{kind}.db");
    let path = d.join(&name);
    let p = path.to_string_lossy().replace('\'', "''");
    st.db.with(|c| c.execute_batch(&format!("VACUUM INTO '{p}'"))).map_err(ApiError::from)?;
    prune(st);
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(BackupEntry { name, created_at: now(), size_bytes: size, kind: kind.into() })
}

pub fn list(st: &St) -> Vec<BackupEntry> {
    let mut v = vec![];
    if let Ok(rd) = std::fs::read_dir(dir(st)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !valid_name(&name) {
                continue;
            }
            let md = e.metadata().ok();
            let created = md.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let kind = if name.contains("-auto") { "auto" } else { "manual" };
            v.push(BackupEntry { name, created_at: created, size_bytes: md.map(|m| m.len()).unwrap_or(0), kind: kind.into() });
        }
    }
    v.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.name.cmp(&a.name)));
    v
}

fn prune(st: &St) {
    let keep = st.settings().backup.keep.max(1) as usize;
    let autos: Vec<BackupEntry> = list(st).into_iter().filter(|b| b.kind == "auto").collect();
    for b in autos.into_iter().skip(keep) {
        let _ = std::fs::remove_file(dir(st).join(b.name));
    }
}

pub fn restore(st: &St, name: &str) -> Result<(), ApiError> {
    if !valid_name(name) {
        return Err(ApiError::bad_request("invalid backup name"));
    }
    let path = dir(st).join(name);
    if !path.exists() {
        return Err(ApiError::not_found("backup not found"));
    }
    let src = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| ApiError::bad_request(format!("not a valid backup: {e}")))?;
    let ver: i64 = src.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| ApiError::bad_request(e.to_string()))?;
    if ver < 1 {
        return Err(ApiError::bad_request("file is not a LAN Wake Manager database"));
    }
    create(st, "auto").ok(); // safety copy of the current state
    let mut guard = st.db.conn.lock().unwrap();
    {
        let b = rusqlite::backup::Backup::new(&src, &mut guard).map_err(|e| ApiError::internal(e.to_string()))?;
        b.run_to_completion(64, Duration::from_millis(20), None).map_err(|e| ApiError::internal(e.to_string()))?;
    }
    Ok(())
}

pub async fn auto_loop(st: St) {
    let mut shutdown = st.shutdown.clone();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(600)) => {}
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
        let s = st.settings().backup;
        if !s.auto {
            continue;
        }
        let newest_auto = list(&st).into_iter().find(|b| b.kind == "auto").map(|b| b.created_at).unwrap_or(0);
        if now() - newest_auto >= s.interval_hours.max(1) as i64 * 3600 {
            match create(&st, "auto") {
                Ok(b) => tracing::info!("automatic backup created: {}", b.name),
                Err(e) => tracing::warn!("automatic backup failed: {}", e.message),
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ExportDoc {
    pub format: String,
    pub exported_at: i64,
    pub locations: Vec<serde_json::Value>,
    pub devices: Vec<DeviceRec>,
    pub groups: Vec<Group>,
    pub schedules: Vec<Schedule>,
    pub wireguard_profiles: Vec<serde_json::Value>,
    pub settings: serde_json::Value,
}

pub fn export(st: &St) -> Result<ExportDoc, ApiError> {
    let locs: Vec<LocationRec> = st.db.doc_list("locations")?;
    let mut devices = st.db.devices(None)?;
    for d in devices.iter_mut() {
        d.agent_token_hash = None; // never export credentials
    }
    // relay configuration without tokens or pinned fingerprints: the relay must be paired again after an import
    let locations = locs
        .into_iter()
        .map(|mut l| {
            l.relay_token = None;
            l.relay_fingerprint = None;
            serde_json::to_value(l).unwrap_or_default()
        })
        .collect();
    Ok(ExportDoc {
        format: "lan-wake-manager-config-1".into(),
        exported_at: now(),
        locations,
        devices,
        groups: st.db.doc_list("groups")?,
        schedules: st.db.doc_list("schedules")?,
        wireguard_profiles: st.db.doc_list("wireguard_profiles")?,
        settings: serde_json::to_value(st.settings()).unwrap_or_default(),
    })
}

pub fn import(st: &St, doc: ExportDoc) -> Result<serde_json::Value, ApiError> {
    if doc.format != "lan-wake-manager-config-1" {
        return Err(ApiError::bad_request("unknown configuration format"));
    }
    let (mut l, mut d, mut g, mut s) = (0, 0, 0, 0);
    for v in doc.locations {
        let mut rec: LocationRec = serde_json::from_value(v).map_err(|e| ApiError::bad_request(format!("invalid location: {e}")))?;
        // keep the credentials of an already paired relay with the same id
        if let Ok(Some(old)) = st.db.doc_get::<LocationRec>("locations", &rec.id) {
            if rec.relay_url == old.relay_url {
                rec.relay_token = old.relay_token;
                rec.relay_fingerprint = old.relay_fingerprint;
            }
        }
        st.db.doc_put("locations", &rec.id.clone(), &rec)?;
        l += 1;
    }
    for mut dev in doc.devices {
        lwm_core::util::normalize_mac(&dev.mac).ok_or_else(|| ApiError::bad_request(format!("invalid MAC in device {}", dev.name)))?;
        if let Ok(Some(old)) = st.db.device_get(&dev.id) {
            dev.agent_token_hash = old.agent_token_hash;
        }
        st.db.device_put(&dev)?;
        d += 1;
    }
    for grp in doc.groups {
        st.db.doc_put("groups", &grp.id.clone(), &grp)?;
        g += 1;
    }
    for sc in doc.schedules {
        lwm_core::schedule::validate(&sc.recurrence, sc.start_date.as_deref(), sc.end_date.as_deref()).map_err(ApiError::bad_request)?;
        st.db.doc_put("schedules", &sc.id.clone(), &sc)?;
        s += 1;
    }
    for p in doc.wireguard_profiles {
        if let Some(id) = p["id"].as_str() {
            st.db.doc_put("wireguard_profiles", id, &p)?;
        }
    }
    Ok(serde_json::json!({"locations": l, "devices": d, "groups": g, "schedules": s}))
}
