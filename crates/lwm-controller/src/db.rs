//! SQLite persistence with versioned migrations. Entities are stored as JSON documents next to the
//! columns that are queried; everything runs inside short transactions.
use anyhow::{anyhow, Result};
use lwm_core::models::*;
use lwm_core::util::now;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    pub conn: Mutex<Connection>,
}

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE locations(id TEXT PRIMARY KEY, data TEXT NOT NULL);
     CREATE TABLE devices(id TEXT PRIMARY KEY, location_id TEXT NOT NULL, agent_hash TEXT, data TEXT NOT NULL);
     CREATE INDEX devices_loc ON devices(location_id);
     CREATE INDEX devices_agent ON devices(agent_hash);
     CREATE TABLE device_state(device_id TEXT PRIMARY KEY, status TEXT NOT NULL DEFAULT 'unknown', checked_at INTEGER, last_seen INTEGER, changed_at INTEGER,
        agent_version TEXT, agent_heartbeat INTEGER, sysinfo TEXT);
     CREATE TABLE groups(id TEXT PRIMARY KEY, data TEXT NOT NULL);
     CREATE TABLE schedules(id TEXT PRIMARY KEY, data TEXT NOT NULL, cursor INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE schedule_runs(schedule_id TEXT NOT NULL, occurrence INTEGER NOT NULL, fired_at INTEGER NOT NULL, PRIMARY KEY(schedule_id, occurrence));
     CREATE TABLE operations(id TEXT PRIMARY KEY, started_at INTEGER NOT NULL, device_id TEXT NOT NULL, state TEXT NOT NULL, data TEXT NOT NULL);
     CREATE INDEX operations_started ON operations(started_at);
     CREATE TABLE metrics(device_id TEXT NOT NULL, t INTEGER NOT NULL, cpu REAL, mem_used INTEGER, mem_total INTEGER, disk_free_min INTEGER, temp_max REAL);
     CREATE INDEX metrics_dev ON metrics(device_id, t);
     CREATE TABLE audit(id INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, actor TEXT NOT NULL, action TEXT NOT NULL, target TEXT, result TEXT NOT NULL, ip TEXT, detail TEXT);
     CREATE TABLE mobile_devices(id TEXT PRIMARY KEY, name TEXT NOT NULL, platform TEXT, role TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
        created_at INTEGER NOT NULL, last_seen INTEGER, last_ip TEXT, revoked INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE sessions(token_hash TEXT PRIMARY KEY, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL);
     CREATE TABLE pairing_tokens(token_hash TEXT PRIMARY KEY, role TEXT NOT NULL, expires_at INTEGER NOT NULL, used INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE enrollments(code_hash TEXT PRIMARY KEY, device_id TEXT NOT NULL, expires_at INTEGER NOT NULL);
     CREATE TABLE notifications(id INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, type TEXT NOT NULL, severity TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, device_id TEXT);
     CREATE TABLE wireguard_profiles(id TEXT PRIMARY KEY, data TEXT NOT NULL);
     CREATE TABLE ssh_keys(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL);",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Notification {
    pub id: i64,
    pub ts: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub body: String,
    pub device_id: Option<String>,
}
use serde::Deserialize;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditRow {
    pub id: i64,
    pub ts: i64,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub result: String,
    pub ip: Option<String>,
    pub detail: Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MobileDevice {
    pub id: String,
    pub name: String,
    pub platform: Option<String>,
    pub role: String,
    pub created_at: i64,
    pub last_seen: Option<i64>,
    pub last_ip: Option<String>,
    pub revoked: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DeviceState {
    pub status: String,
    pub checked_at: Option<i64>,
    pub last_seen: Option<i64>,
    pub changed_at: Option<i64>,
    pub agent_version: Option<String>,
    pub agent_heartbeat: Option<i64>,
    pub sysinfo: Option<SysInfo>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricPoint {
    pub t: i64,
    pub cpu: Option<f64>,
    pub mem_used: Option<i64>,
    pub mem_total: Option<i64>,
    pub disk_free_min: Option<i64>,
    pub temp_max: Option<f64>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Db { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let db = Db { conn: Mutex::new(Connection::open_in_memory()?) };
        db.migrate()?;
        Ok(db)
    }
    fn migrate(&self) -> Result<()> {
        let mut c = self.conn.lock().unwrap();
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
        let v: usize = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(v) {
            let tx = c.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
            tx.commit()?;
        }
        Ok(())
    }
    pub fn with<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut c = self.conn.lock().unwrap();
        f(&mut c).map_err(|e| anyhow!("database error: {e}"))
    }

    // ---- kv ----
    pub fn kv_get(&self, k: &str) -> Result<Option<String>> {
        self.with(|c| c.query_row("SELECT value FROM kv WHERE key=?1", [k], |r| r.get(0)).optional())
    }
    pub fn kv_set(&self, k: &str, v: &str) -> Result<()> {
        self.with(|c| c.execute("INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![k, v]).map(|_| ()))
    }
    pub fn kv_json<T: DeserializeOwned>(&self, k: &str) -> Option<T> {
        self.kv_get(k).ok().flatten().and_then(|s| serde_json::from_str(&s).ok())
    }

    // ---- generic documents ----
    pub fn doc_put<T: Serialize>(&self, table: &str, id: &str, v: &T) -> Result<()> {
        let s = serde_json::to_string(v)?;
        self.with(|c| c.execute(&format!("INSERT INTO {table}(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data"), params![id, s]).map(|_| ()))
    }
    pub fn doc_get<T: DeserializeOwned>(&self, table: &str, id: &str) -> Result<Option<T>> {
        let s: Option<String> = self.with(|c| c.query_row(&format!("SELECT data FROM {table} WHERE id=?1"), [id], |r| r.get(0)).optional())?;
        Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
    }
    pub fn doc_list<T: DeserializeOwned>(&self, table: &str) -> Result<Vec<T>> {
        let rows: Vec<String> = self.with(|c| {
            let mut st = c.prepare(&format!("SELECT data FROM {table} ORDER BY rowid"))?;
            let r = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })?;
        Ok(rows.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }
    pub fn doc_delete(&self, table: &str, id: &str) -> Result<bool> {
        self.with(|c| c.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id]).map(|n| n > 0))
    }

    // ---- devices ----
    pub fn device_put(&self, d: &DeviceRec) -> Result<()> {
        let s = serde_json::to_string(d)?;
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("INSERT INTO devices(id,location_id,agent_hash,data) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET location_id=excluded.location_id, agent_hash=excluded.agent_hash, data=excluded.data", params![d.id, d.location_id, d.agent_token_hash, s])?;
            tx.execute("INSERT OR IGNORE INTO device_state(device_id, status) VALUES(?1,'unknown')", [&d.id])?;
            tx.commit()
        })
    }
    pub fn device_get(&self, id: &str) -> Result<Option<DeviceRec>> {
        self.doc_get("devices", id)
    }
    pub fn devices(&self, location: Option<&str>) -> Result<Vec<DeviceRec>> {
        let rows: Vec<String> = self.with(|c| {
            let (sql, p): (&str, Vec<String>) = match location {
                Some(l) => ("SELECT data FROM devices WHERE location_id=?1 ORDER BY rowid", vec![l.to_string()]),
                None => ("SELECT data FROM devices ORDER BY rowid", vec![]),
            };
            let mut st = c.prepare(sql)?;
            let r = st.query_map(rusqlite::params_from_iter(p), |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })?;
        Ok(rows.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }
    pub fn device_delete(&self, id: &str) -> Result<bool> {
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("DELETE FROM device_state WHERE device_id=?1", [id])?;
            tx.execute("DELETE FROM metrics WHERE device_id=?1", [id])?;
            let n = tx.execute("DELETE FROM devices WHERE id=?1", [id])?;
            tx.commit()?;
            Ok(n > 0)
        })
    }
    pub fn device_by_agent_hash(&self, h: &str) -> Result<Option<DeviceRec>> {
        let s: Option<String> = self.with(|c| c.query_row("SELECT data FROM devices WHERE agent_hash=?1", [h], |r| r.get(0)).optional())?;
        Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
    }

    // ---- device state ----
    pub fn state_get(&self, id: &str) -> Result<DeviceState> {
        let r = self.with(|c| {
            c.query_row("SELECT status,checked_at,last_seen,changed_at,agent_version,agent_heartbeat,sysinfo FROM device_state WHERE device_id=?1", [id], |r| {
                let si: Option<String> = r.get(6)?;
                Ok(DeviceState { status: r.get(0)?, checked_at: r.get(1)?, last_seen: r.get(2)?, changed_at: r.get(3)?, agent_version: r.get(4)?, agent_heartbeat: r.get(5)?, sysinfo: si.and_then(|s| serde_json::from_str(&s).ok()) })
            })
            .optional()
        })?;
        Ok(r.unwrap_or(DeviceState { status: "unknown".into(), ..Default::default() }))
    }
    pub fn states(&self) -> Result<std::collections::HashMap<String, DeviceState>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT device_id,status,checked_at,last_seen,changed_at,agent_version,agent_heartbeat,sysinfo FROM device_state")?;
            let r = st
                .query_map([], |r| {
                    let si: Option<String> = r.get(7)?;
                    Ok((r.get::<_, String>(0)?, DeviceState { status: r.get(1)?, checked_at: r.get(2)?, last_seen: r.get(3)?, changed_at: r.get(4)?, agent_version: r.get(5)?, agent_heartbeat: r.get(6)?, sysinfo: si.and_then(|s| serde_json::from_str(&s).ok()) }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>();
            r.map(|v| v.into_iter().collect())
        })
    }
    /// Set status; returns Some(old status) when it changed.
    pub fn state_set_status(&self, id: &str, status: &str, alive: bool) -> Result<Option<String>> {
        let t = now();
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("INSERT OR IGNORE INTO device_state(device_id,status) VALUES(?1,'unknown')", [id])?;
            let old: String = tx.query_row("SELECT status FROM device_state WHERE device_id=?1", [id], |r| r.get(0))?;
            if old != status {
                tx.execute("UPDATE device_state SET status=?2, checked_at=?3, changed_at=?3 WHERE device_id=?1", params![id, status, t])?;
            } else {
                tx.execute("UPDATE device_state SET checked_at=?2 WHERE device_id=?1", params![id, t])?;
            }
            if alive {
                tx.execute("UPDATE device_state SET last_seen=?2 WHERE device_id=?1", params![id, t])?;
            }
            tx.commit()?;
            Ok(if old != status { Some(old) } else { None })
        })
    }
    pub fn state_heartbeat(&self, id: &str, version: &str, si: &SysInfo) -> Result<()> {
        let t = now();
        let s = serde_json::to_string(si)?;
        let free_min = si.disks.iter().map(|d| d.free_bytes as i64).min();
        let temp_max = si.temperatures.iter().map(|t| t.celsius as f64).fold(None, |a: Option<f64>, x| Some(a.map_or(x, |y| y.max(x))));
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("INSERT OR IGNORE INTO device_state(device_id,status) VALUES(?1,'unknown')", [id])?;
            tx.execute("UPDATE device_state SET agent_version=?2, agent_heartbeat=?3, sysinfo=?4, last_seen=?3 WHERE device_id=?1", params![id, version, t, s])?;
            tx.execute("INSERT INTO metrics(device_id,t,cpu,mem_used,mem_total,disk_free_min,temp_max) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id, t, si.cpu_usage_percent.map(|x| x as f64), si.mem_used_bytes.map(|x| x as i64), si.mem_total_bytes.map(|x| x as i64), free_min, temp_max])?;
            tx.execute("DELETE FROM metrics WHERE device_id=?1 AND t<?2", params![id, t - 7 * 86400])?;
            tx.commit()
        })
    }
    pub fn metrics(&self, id: &str, minutes: i64) -> Result<Vec<MetricPoint>> {
        let since = now() - minutes * 60;
        self.with(|c| {
            let mut st = c.prepare("SELECT t,cpu,mem_used,mem_total,disk_free_min,temp_max FROM metrics WHERE device_id=?1 AND t>=?2 ORDER BY t")?;
            let r = st.query_map(params![id, since], |r| Ok(MetricPoint { t: r.get(0)?, cpu: r.get(1)?, mem_used: r.get(2)?, mem_total: r.get(3)?, disk_free_min: r.get(4)?, temp_max: r.get(5)? }))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })
    }

    // ---- operations ----
    pub fn op_put(&self, o: &Operation) -> Result<()> {
        let s = serde_json::to_string(o)?;
        self.with(|c| c.execute("INSERT INTO operations(id,started_at,device_id,state,data) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET state=excluded.state, data=excluded.data", params![o.id, o.started_at, o.device_id, o.state, s]).map(|_| ()))
    }
    pub fn op_get(&self, id: &str) -> Result<Option<Operation>> {
        self.doc_get("operations", id)
    }
    pub fn ops(&self, limit: i64, before: Option<i64>, device: Option<&str>, state: Option<&str>) -> Result<Vec<Operation>> {
        let rows: Vec<String> = self.with(|c| {
            let mut st = c.prepare("SELECT data FROM operations WHERE (?1 IS NULL OR started_at<?1) AND (?2 IS NULL OR device_id=?2) AND (?3 IS NULL OR state=?3) ORDER BY started_at DESC, rowid DESC LIMIT ?4")?;
            let r = st.query_map(params![before, device, state, limit], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })?;
        Ok(rows.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect())
    }
    pub fn ops_failed_since(&self, since: i64) -> Result<i64> {
        self.with(|c| c.query_row("SELECT COUNT(*) FROM operations WHERE started_at>=?1 AND state IN ('failed','timeout')", [since], |r| r.get(0)))
    }
    /// Operations left running by a previous process are marked failed on start-up.
    pub fn ops_fail_orphans(&self) -> Result<()> {
        let list: Vec<Operation> = self.ops(500, None, None, Some("running"))?.into_iter().chain(self.ops(500, None, None, Some("pending"))?).collect();
        for mut o in list {
            o.state = "failed".into();
            o.message = "interrupted by a controller restart".into();
            o.finished_at = Some(now());
            self.op_put(&o)?;
        }
        Ok(())
    }

    // ---- audit / notifications ----
    pub fn audit(&self, actor: &str, action: &str, target: Option<&str>, result: &str, ip: Option<&str>, detail: Option<&str>) {
        let r = self.with(|c| c.execute("INSERT INTO audit(ts,actor,action,target,result,ip,detail) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![now(), actor, action, target, result, ip, detail]).map(|_| ()));
        if let Err(e) = r {
            tracing::error!("audit write failed: {e}");
        }
    }
    pub fn audit_list(&self, limit: i64, before: Option<i64>) -> Result<Vec<AuditRow>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id,ts,actor,action,target,result,ip,detail FROM audit WHERE (?1 IS NULL OR ts<?1) ORDER BY id DESC LIMIT ?2")?;
            let r = st.query_map(params![before, limit], |r| Ok(AuditRow { id: r.get(0)?, ts: r.get(1)?, actor: r.get(2)?, action: r.get(3)?, target: r.get(4)?, result: r.get(5)?, ip: r.get(6)?, detail: r.get(7)? }))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })
    }
    pub fn notify(&self, kind: &str, severity: &str, title: &str, body: &str, device_id: Option<&str>) -> Result<Notification> {
        let t = now();
        let id = self.with(|c| {
            c.execute("INSERT INTO notifications(ts,type,severity,title,body,device_id) VALUES(?1,?2,?3,?4,?5,?6)", params![t, kind, severity, title, body, device_id])?;
            let id = c.last_insert_rowid();
            c.execute("DELETE FROM notifications WHERE id<?1", [id - 1000])?;
            Ok(id)
        })?;
        Ok(Notification { id, ts: t, kind: kind.into(), severity: severity.into(), title: title.into(), body: body.into(), device_id: device_id.map(|s| s.to_string()) })
    }
    pub fn notifications(&self, after: i64, limit: i64) -> Result<Vec<Notification>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id,ts,type,severity,title,body,device_id FROM notifications WHERE id>?1 ORDER BY id ASC LIMIT ?2")?;
            let r = st.query_map(params![after, limit], |r| Ok(Notification { id: r.get(0)?, ts: r.get(1)?, kind: r.get(2)?, severity: r.get(3)?, title: r.get(4)?, body: r.get(5)?, device_id: r.get(6)? }))?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })
    }

    // ---- sessions / mobile devices / pairing ----
    pub fn session_create(&self, hash: &str, ttl: i64) -> Result<i64> {
        let exp = now() + ttl;
        self.with(|c| {
            c.execute("DELETE FROM sessions WHERE expires_at<?1", [now()])?;
            c.execute("INSERT INTO sessions(token_hash,created_at,expires_at) VALUES(?1,?2,?3)", params![hash, now(), exp])?;
            Ok(exp)
        })
    }
    pub fn session_valid(&self, hash: &str) -> bool {
        self.with(|c| c.query_row("SELECT 1 FROM sessions WHERE token_hash=?1 AND expires_at>?2", params![hash, now()], |_| Ok(())).optional().map(|o| o.is_some())).unwrap_or(false)
    }
    pub fn session_delete(&self, hash: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM sessions WHERE token_hash=?1", [hash]).map(|_| ()))
    }
    pub fn sessions_clear(&self) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM sessions", []).map(|_| ()))
    }
    pub fn mobile_add(&self, m: &MobileDevice, token_hash: &str) -> Result<()> {
        self.with(|c| c.execute("INSERT INTO mobile_devices(id,name,platform,role,token_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6)", params![m.id, m.name, m.platform, m.role, token_hash, m.created_at]).map(|_| ()))
    }
    fn mobile_from_row(r: &rusqlite::Row) -> rusqlite::Result<MobileDevice> {
        Ok(MobileDevice { id: r.get(0)?, name: r.get(1)?, platform: r.get(2)?, role: r.get(3)?, created_at: r.get(4)?, last_seen: r.get(5)?, last_ip: r.get(6)?, revoked: r.get::<_, i64>(7)? != 0 })
    }
    pub fn mobile_by_token(&self, hash: &str) -> Result<Option<MobileDevice>> {
        self.with(|c| c.query_row("SELECT id,name,platform,role,created_at,last_seen,last_ip,revoked FROM mobile_devices WHERE token_hash=?1", [hash], Self::mobile_from_row).optional())
    }
    pub fn mobile_list(&self) -> Result<Vec<MobileDevice>> {
        self.with(|c| {
            let mut st = c.prepare("SELECT id,name,platform,role,created_at,last_seen,last_ip,revoked FROM mobile_devices ORDER BY created_at DESC")?;
            let r = st.query_map([], Self::mobile_from_row)?.collect::<rusqlite::Result<Vec<_>>>();
            r
        })
    }
    pub fn mobile_get(&self, id: &str) -> Result<Option<MobileDevice>> {
        self.with(|c| c.query_row("SELECT id,name,platform,role,created_at,last_seen,last_ip,revoked FROM mobile_devices WHERE id=?1", [id], Self::mobile_from_row).optional())
    }
    pub fn mobile_touch(&self, id: &str, ip: &str) {
        let _ = self.with(|c| c.execute("UPDATE mobile_devices SET last_seen=?2,last_ip=?3 WHERE id=?1", params![id, now(), ip]).map(|_| ()));
    }
    pub fn mobile_update(&self, id: &str, name: Option<&str>, role: Option<&str>) -> Result<bool> {
        self.with(|c| {
            let mut n = 0;
            if let Some(nm) = name {
                n += c.execute("UPDATE mobile_devices SET name=?2 WHERE id=?1", params![id, nm])?;
            }
            if let Some(r) = role {
                n += c.execute("UPDATE mobile_devices SET role=?2 WHERE id=?1", params![id, r])?;
            }
            Ok(n > 0)
        })
    }
    pub fn mobile_revoke(&self, id: &str) -> Result<bool> {
        self.with(|c| c.execute("UPDATE mobile_devices SET revoked=1 WHERE id=?1", [id]).map(|n| n > 0))
    }
    pub fn pairing_add(&self, hash: &str, role: &str, exp: i64) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM pairing_tokens WHERE expires_at<?1", [now() - 3600])?;
            c.execute("INSERT INTO pairing_tokens(token_hash,role,expires_at) VALUES(?1,?2,?3)", params![hash, role, exp]).map(|_| ())
        })
    }
    /// Atomically consume a pairing token; returns its role when valid.
    pub fn pairing_consume(&self, hash: &str) -> Result<Option<String>> {
        self.with(|c| {
            let tx = c.transaction()?;
            let r: Option<(String, i64, i64)> = tx.query_row("SELECT role,expires_at,used FROM pairing_tokens WHERE token_hash=?1", [hash], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
            let out = match r {
                Some((role, exp, 0)) if exp > now() => {
                    tx.execute("UPDATE pairing_tokens SET used=1 WHERE token_hash=?1", [hash])?;
                    Some(role)
                }
                _ => None,
            };
            tx.commit()?;
            Ok(out)
        })
    }
    pub fn enroll_add(&self, code_hash: &str, device_id: &str, exp: i64) -> Result<()> {
        self.with(|c| {
            c.execute("DELETE FROM enrollments WHERE device_id=?1 OR expires_at<?2", params![device_id, now()])?;
            c.execute("INSERT INTO enrollments(code_hash,device_id,expires_at) VALUES(?1,?2,?3)", params![code_hash, device_id, exp]).map(|_| ())
        })
    }
    pub fn enroll_consume(&self, code_hash: &str) -> Result<Option<String>> {
        self.with(|c| {
            let tx = c.transaction()?;
            let r: Option<(String, i64)> = tx.query_row("SELECT device_id,expires_at FROM enrollments WHERE code_hash=?1", [code_hash], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            let out = match r {
                Some((d, exp)) if exp > now() => {
                    tx.execute("DELETE FROM enrollments WHERE code_hash=?1", [code_hash])?;
                    Some(d)
                }
                _ => None,
            };
            tx.commit()?;
            Ok(out)
        })
    }

    // ---- schedules ----
    pub fn schedule_cursor(&self, id: &str) -> i64 {
        self.with(|c| c.query_row("SELECT cursor FROM schedules WHERE id=?1", [id], |r| r.get(0))).unwrap_or(0)
    }
    pub fn schedule_set_cursor(&self, id: &str, cur: i64) {
        let _ = self.with(|c| c.execute("UPDATE schedules SET cursor=?2 WHERE id=?1", params![id, cur]).map(|_| ()));
    }
    /// Duplicate protection: returns false if this occurrence was already executed.
    pub fn schedule_claim(&self, id: &str, occurrence: i64) -> bool {
        self.with(|c| c.execute("INSERT OR IGNORE INTO schedule_runs(schedule_id,occurrence,fired_at) VALUES(?1,?2,?3)", params![id, occurrence, now()])).map(|n| n > 0).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrations_and_documents() {
        let db = Db::open_memory().unwrap();
        db.kv_set("a", "b").unwrap();
        assert_eq!(db.kv_get("a").unwrap().unwrap(), "b");
        let d = DeviceRec { id: "d1".into(), location_id: "l1".into(), name: "PC".into(), hostname: None, ip: Some("10.0.0.2".into()), mac: "AA:BB:CC:DD:EE:FF".into(), os: None, device_type: "pc".into(), wol_capable: "unknown".into(), group_ids: vec![], tcp_check_ports: vec![], power_method: "none".into(), ssh: None, notes: String::new(), agent_token_hash: Some("h".into()), created_at: 1 };
        db.device_put(&d).unwrap();
        assert_eq!(db.devices(Some("l1")).unwrap().len(), 1);
        assert_eq!(db.device_by_agent_hash("h").unwrap().unwrap().id, "d1");
        assert_eq!(db.state_set_status("d1", "online", true).unwrap().as_deref(), Some("unknown"));
        assert!(db.state_set_status("d1", "online", true).unwrap().is_none());
        assert!(db.schedule_claim("s", 5));
        assert!(!db.schedule_claim("s", 5), "duplicate execution must be refused");
        let tok = "abc";
        db.pairing_add(tok, "operator", now() + 60).unwrap();
        assert_eq!(db.pairing_consume(tok).unwrap().as_deref(), Some("operator"));
        assert!(db.pairing_consume(tok).unwrap().is_none(), "pairing tokens are single use");
        db.pairing_add("old", "admin", now() - 5).unwrap();
        assert!(db.pairing_consume("old").unwrap().is_none());
        assert!(db.device_delete("d1").unwrap());
    }
}
