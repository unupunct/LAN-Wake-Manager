//! Power operations (wake / shutdown / restart / cancel) and their supervision.
use crate::state::St;
use crate::views::agent_online;
use lwm_core::models::*;
use lwm_core::server::ApiError;
use lwm_core::util::{new_id, now};
use std::time::{Duration, Instant};

pub fn effective_ports(d: &DeviceRec) -> Vec<u16> {
    if !d.tcp_check_ports.is_empty() {
        return d.tcp_check_ports.clone();
    }
    match d.os.as_deref() {
        Some("windows") => vec![445, 3389, 135],
        Some("linux") => vec![22],
        _ => vec![445, 22, 3389, 80],
    }
}

fn save(st: &St, op: &Operation) {
    if let Err(e) = st.db.op_put(op) {
        tracing::error!("cannot save operation: {e}");
    }
    st.emit(serde_json::json!({"type": "operation", "operation": op}));
}
fn step(st: &St, op: &mut Operation, text: impl Into<String>) {
    op.steps.push(OpStep { t: now(), text: text.into() });
    save(st, op);
}
fn finish(st: &St, op: &mut Operation, state: &str, msg: impl Into<String>) {
    op.state = state.into();
    op.message = msg.into();
    op.finished_at = Some(now());
    save(st, op);
}
fn set_status(st: &St, id: &str, status: &str, alive: bool) {
    if let Ok(Some(_)) = st.db.state_set_status(id, status, alive) {
        st.emit(serde_json::json!({"type": "device_status", "device_id": id, "status": status, "last_seen": if alive { Some(now()) } else { None }}));
    }
}

struct BusyGuard(St, String);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.lock().unwrap().remove(&self.1);
    }
}

pub struct Origin {
    pub requested_by: String,
    pub source: &'static str,
    pub schedule_id: Option<String>,
}

/// Create the operation record and run it in the background. Returns the freshly created operation.
pub fn start(st: &St, device_id: &str, kind: &str, delay_secs: u32, origin: Origin) -> Result<Operation, ApiError> {
    let dev = st.db.device_get(device_id).map_err(|e| ApiError::internal(e.to_string()))?.ok_or_else(|| ApiError::not_found("device not found"))?;
    let mut op = Operation {
        id: new_id(),
        kind: kind.into(),
        device_id: dev.id.clone(),
        device_name: dev.name.clone(),
        location_id: dev.location_id.clone(),
        state: "pending".into(),
        message: "queued".into(),
        steps: vec![],
        requested_by: origin.requested_by,
        source: origin.source.into(),
        schedule_id: origin.schedule_id,
        delay_secs,
        started_at: now(),
        finished_at: None,
    };
    let is_cancel = kind == "cancel_shutdown";
    if !is_cancel && !st.busy.lock().unwrap().insert(dev.id.clone()) {
        op.state = "failed".into();
        op.message = "another operation is already running for this computer".into();
        op.finished_at = Some(now());
        save(st, &op);
        return Ok(op);
    }
    save(st, &op);
    let st2 = st.clone();
    let op2 = op.clone();
    tokio::spawn(async move {
        let _guard = if is_cancel { None } else { Some(BusyGuard(st2.clone(), dev.id.clone())) };
        run(st2, op2, dev).await;
    });
    Ok(op)
}

async fn run(st: St, mut op: Operation, dev: DeviceRec) {
    op.state = "running".into();
    save(&st, &op);
    match op.kind.as_str() {
        "wake" => run_wake(&st, &mut op, dev).await,
        _ => run_power(&st, &mut op, dev).await,
    }
}

async fn run_wake(st: &St, op: &mut Operation, dev: DeviceRec) {
    let loc: LocationRec = match st.db.doc_get("locations", &dev.location_id) {
        Ok(Some(l)) => l,
        _ => return finish(st, op, "failed", "network location of this computer no longer exists"),
    };
    let relay = match crate::relay::for_location(st, &loc) {
        Ok(r) => r,
        Err(e) => {
            notify_fail(st, op, &e.message);
            return finish(st, op, "failed", e.message);
        }
    };
    set_status(st, &dev.id, "waking", false);
    step(st, op, format!("asking the Wake Relay of '{}' ({}) to wake {}", loc.name, if relay.is_remote() { "remote relay" } else { "built-in" }, dev.mac));
    let req = WakeRequest {
        device_id: dev.id.clone(),
        mac: dev.mac.clone(),
        ip: dev.ip.clone(),
        tcp_ports: effective_ports(&dev),
        broadcast_addr: loc.broadcast_addr.clone(),
        wol_ports: loc.wol_ports.clone(),
        attempts: loc.wake_attempts,
        interval_secs: loc.wake_interval_secs,
        timeout_secs: loc.wake_timeout_secs,
    };
    let job = match relay.wake_start(req).await {
        Ok(j) => j,
        Err(e) => {
            set_status(st, &dev.id, "offline", false);
            notify_fail(st, op, &e.message);
            return finish(st, op, "failed", e.message);
        }
    };
    let started = Instant::now();
    let limit = Duration::from_secs(loc.wake_timeout_secs as u64 + 45);
    let mut seen = 0usize;
    let mut fails = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        // the agent coming online proves the start-up as well
        if let Ok(s) = st.db.state_get(&dev.id) {
            if s.agent_heartbeat.map(|h| h >= op.started_at).unwrap_or(false) && agent_online(&s) {
                step(st, op, "agent heartbeat received: the computer is up");
                return wake_ok(st, op, &dev, started.elapsed().as_secs(), "confirmed by the agent heartbeat");
            }
        }
        match relay.wake_get(&job).await {
            Ok(w) => {
                fails = 0;
                for s in w.steps.iter().skip(seen) {
                    op.steps.push(s.clone());
                }
                if w.steps.len() > seen {
                    seen = w.steps.len();
                    save(st, op);
                }
                match w.state.as_str() {
                    "online" => return wake_ok(st, op, &dev, started.elapsed().as_secs(), &w.detail.unwrap_or_default()),
                    "timeout" => {
                        set_status(st, &dev.id, "offline", false);
                        let m = w.detail.unwrap_or_else(|| "the computer did not come online".into());
                        notify_fail(st, op, &m);
                        return finish(st, op, "timeout", m);
                    }
                    "failed" => {
                        set_status(st, &dev.id, "offline", false);
                        let m = w.detail.unwrap_or_else(|| "the relay could not send the packet".into());
                        notify_fail(st, op, &m);
                        return finish(st, op, "failed", m);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                fails += 1;
                if fails >= 6 {
                    set_status(st, &dev.id, "offline", false);
                    notify_fail(st, op, &e.message);
                    return finish(st, op, "failed", format!("lost contact with the Wake Relay while waiting: {}", e.message));
                }
            }
        }
        if started.elapsed() > limit {
            set_status(st, &dev.id, "offline", false);
            notify_fail(st, op, "wake timed out");
            return finish(st, op, "timeout", "no result from the relay within the wake timeout");
        }
    }
}

fn wake_ok(st: &St, op: &mut Operation, dev: &DeviceRec, secs: u64, how: &str) {
    set_status(st, &dev.id, "online", true);
    if dev.wol_capable != "yes" {
        if let Ok(Some(mut r)) = st.db.device_get(&dev.id) {
            r.wol_capable = "yes".into();
            let _ = st.db.device_put(&r);
        }
    }
    let msg = if how.is_empty() { format!("Online after {secs} s") } else { format!("Online after {secs} s ({how})") };
    st.notify("wake_completed", "info", &format!("{} is online", dev.name), &msg, Some(&dev.id));
    finish(st, op, "succeeded", msg);
}

fn notify_fail(st: &St, op: &Operation, msg: &str) {
    st.notify("operation_failed", "error", &format!("{} of {} failed", op.kind.replace('_', " "), op.device_name), msg, Some(&op.device_id));
}

async fn run_power(st: &St, op: &mut Operation, dev: DeviceRec) {
    let kind = op.kind.clone();
    let s = st.db.state_get(&dev.id).unwrap_or_default();
    let result: Result<String, String> = match dev.power_method.as_str() {
        "agent" => {
            if !dev.agent_token_hash.is_some() {
                Err("no agent is enrolled on this computer".into())
            } else if !agent_online(&s) {
                Err(format!("the agent on this computer is offline{}", s.agent_heartbeat.map(|h| format!(" (last heartbeat {} s ago)", now() - h)).unwrap_or_default()))
            } else {
                step(st, op, "sending the command to the agent");
                via_agent(st, &dev, &kind, op.delay_secs).await
            }
        }
        "ssh" => via_ssh(st, op, &dev).await,
        "winrm" => via_winrm(st, op, &dev).await,
        _ => Err("no power method is configured for this computer: install the agent or configure SSH".into()),
    };
    match result {
        Err(e) => {
            notify_fail(st, op, &e);
            finish(st, op, "failed", e)
        }
        Ok(msg) => {
            step(st, op, format!("command accepted: {msg}"));
            if kind == "cancel_shutdown" {
                st.cancelled.lock().unwrap().insert(dev.id.clone());
                set_status(st, &dev.id, "online", true);
                return finish(st, op, "succeeded", "pending shutdown/restart cancelled");
            }
            set_status(st, &dev.id, "shutting_down", false);
            supervise_power(st, op, &dev, &kind).await
        }
    }
}

/// Wait until the computer really went down (and, for restart, came back).
async fn supervise_power(st: &St, op: &mut Operation, dev: &DeviceRec, kind: &str) {
    let started = Instant::now();
    let down_deadline = Duration::from_secs(op.delay_secs as u64 + 240);
    let up_deadline = Duration::from_secs(op.delay_secs as u64 + 900);
    let mut went_down: Option<u64> = None;
    let uptime0 = st.db.state_get(&dev.id).ok().and_then(|s| s.sysinfo).and_then(|i| i.uptime_secs);
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if st.cancelled.lock().unwrap().remove(&dev.id) {
            return finish(st, op, "failed", "cancelled before it ran (cancel-shutdown request)");
        }
        // a restart that was faster than the monitor interval is still proven by the agent's uptime counter
        if kind == "restart" {
            if let (Some(u0), Ok(s)) = (uptime0, st.db.state_get(&dev.id)) {
                let u1 = s.sysinfo.as_ref().and_then(|i| i.uptime_secs);
                if s.agent_heartbeat.map(|h| h > op.started_at).unwrap_or(false) && u1.map(|u| u + 5 < u0 + started.elapsed().as_secs()).unwrap_or(false) && started.elapsed().as_secs() > 5 {
                    set_status(st, &dev.id, "online", true);
                    st.notify("restart_executed", "info", &format!("{} restarted", dev.name), "back online (uptime counter reset)", Some(&dev.id));
                    return finish(st, op, "succeeded", format!("Restarted: uptime counter reset, back online after {} s", started.elapsed().as_secs()));
                }
            }
        }
        let status = st.db.state_get(&dev.id).map(|s| s.status).unwrap_or_default();
        let gone = status == "offline";
        if went_down.is_none() && gone {
            went_down = Some(started.elapsed().as_secs());
            step(st, op, format!("computer went offline after {} s", went_down.unwrap()));
            if kind == "shutdown" {
                st.notify("shutdown_executed", "info", &format!("{} was shut down", dev.name), "powered off (confirmed)", Some(&dev.id));
                return finish(st, op, "succeeded", format!("Powered off after {} s (confirmed: no longer reachable)", went_down.unwrap()));
            }
        }
        if let Some(d) = went_down {
            if status == "online" {
                st.notify("restart_executed", "info", &format!("{} restarted", dev.name), "back online", Some(&dev.id));
                return finish(st, op, "succeeded", format!("Restarted: offline after {d} s, back online after {} s", started.elapsed().as_secs()));
            }
            if started.elapsed() > up_deadline {
                notify_fail(st, op, "did not come back after restart");
                return finish(st, op, "timeout", "the computer went down but did not come back online in time");
            }
        } else if started.elapsed() > down_deadline {
            set_status(st, &dev.id, "online", true);
            let m = "the command was accepted but the computer is still online (a program may be blocking the shutdown)";
            notify_fail(st, op, m);
            return finish(st, op, "failed", m);
        }
    }
}

async fn via_agent(st: &St, dev: &DeviceRec, kind: &str, delay: u32) -> Result<String, String> {
    let cmd = AgentCommand { id: new_id(), kind: kind.into(), delay_secs: delay };
    let (tx, rx) = tokio::sync::oneshot::channel();
    st.cmd_waiters.lock().unwrap().insert(cmd.id.clone(), tx);
    st.agent_queue.lock().unwrap().entry(dev.id.clone()).or_default().push_back(cmd.clone());
    st.agent_notify.notify_waiters();
    let r = tokio::time::timeout(Duration::from_secs(45), rx).await;
    st.cmd_waiters.lock().unwrap().remove(&cmd.id);
    match r {
        Ok(Ok(res)) if res.ok => Ok(res.message),
        Ok(Ok(res)) => Err(format!("agent reported an error: {}", res.message)),
        _ => {
            if let Some(q) = st.agent_queue.lock().unwrap().get_mut(&dev.id) {
                q.retain(|c| c.id != cmd.id);
            }
            Err("the agent did not confirm the command within 45 s".into())
        }
    }
}

async fn via_ssh(st: &St, op: &mut Operation, dev: &DeviceRec) -> Result<String, String> {
    let ssh = dev.ssh.clone().ok_or("SSH is not configured for this computer")?;
    let ip = dev.ip.clone().ok_or("this computer has no IP address")?;
    let key = ssh.key_id.as_ref().map(|k| st.dir.join("ssh-keys").join(k)).filter(|p| p.exists());
    if ssh.key_id.is_some() && key.is_none() {
        return Err("the configured SSH key no longer exists".into());
    }
    let os = dev.os.clone().unwrap_or_else(|| "linux".into());
    let (prog, args) = lwm_core::power::ssh_command(&ip, &ssh, key.as_ref().map(|p| p.to_string_lossy().to_string()).as_deref(), &os, &op.kind, op.delay_secs)?;
    step(st, op, format!("ssh {}@{}:{}", ssh.user, ip, ssh.port));
    if lwm_core::power::dry_run() {
        return Ok(format!("dry run: {prog} {}", args.join(" ")));
    }
    match lwm_core::cmd::run(&prog, &args, Duration::from_secs(40)).await? {
        (true, out) => Ok(if out.is_empty() { "executed over SSH".into() } else { out }),
        (false, out) => Err(format!("ssh failed: {}", out.lines().last().unwrap_or("unknown error"))),
    }
}

async fn via_winrm(st: &St, op: &mut Operation, dev: &DeviceRec) -> Result<String, String> {
    if !cfg!(windows) {
        return Err("Windows Remote Management can only be used from a Windows controller".into());
    }
    let ip = dev.ip.clone().ok_or("this computer has no IP address")?;
    let (prog, args) = lwm_core::power::winrm_command(&ip, &op.kind, op.delay_secs)?;
    step(st, op, format!("WinRM to {ip}"));
    if lwm_core::power::dry_run() {
        return Ok(format!("dry run: {prog} {}", args.join(" ")));
    }
    match lwm_core::cmd::run(&prog, &args, Duration::from_secs(60)).await? {
        (true, _) => Ok("executed over WinRM".into()),
        (false, out) => Err(format!("WinRM failed (check the prerequisites in the documentation): {}", out.lines().next().unwrap_or("unknown error"))),
    }
}
