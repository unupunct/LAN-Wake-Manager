//! Endpoints used by the computer agents (/api/v1/agent/*). Agents authenticate with their own token.
use crate::state::St;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;
use lwm_core::models::*;
use lwm_core::server::{ApiError, ApiResult};
use lwm_core::util::{normalize_code, random_token, sha256_hex};
use serde::Deserialize;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone)]
pub struct AgentCtx {
    pub device_id: String,
}

pub async fn auth(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, mut req: Request, next: Next) -> Result<Response, ApiError> {
    let ip = addr.ip().to_string();
    let found = lwm_core::server::bearer(req.headers()).and_then(|t| st.db.device_by_agent_hash(&sha256_hex(t.as_bytes())).ok().flatten());
    match found {
        Some(d) => {
            req.extensions_mut().insert(AgentCtx { device_id: d.id });
            Ok(next.run(req).await)
        }
        None => {
            st.limiter.check(&format!("authfail:{ip}"), 20, Duration::from_secs(60)).map_err(ApiError::rate_limited)?;
            Err(ApiError::unauthorized("invalid agent token"))
        }
    }
}

pub async fn enroll(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(r): Json<AgentEnrollRequest>) -> ApiResult<AgentEnrollResponse> {
    let ip = addr.ip().to_string();
    st.limiter.check(&format!("enroll:{ip}"), 10, Duration::from_secs(300)).map_err(ApiError::rate_limited)?;
    let Some(device_id) = st.db.enroll_consume(&sha256_hex(normalize_code(&r.code).as_bytes()))? else {
        st.db.audit("agent", "agent_enroll", None, "denied", Some(&ip), Some("invalid or expired code"));
        return Err(ApiError::forbidden("invalid or expired enrollment code"));
    };
    let mut dev = st.db.device_get(&device_id)?.ok_or_else(|| ApiError::not_found("device no longer exists"))?;
    let token = random_token();
    dev.agent_token_hash = Some(sha256_hex(token.as_bytes()));
    if dev.power_method == "none" {
        dev.power_method = "agent".into();
    }
    if dev.os.is_none() {
        dev.os = Some(if r.os == "windows" { "windows".into() } else if r.os == "macos" { "macos".into() } else { "linux".into() });
    }
    if dev.hostname.is_none() {
        dev.hostname = Some(r.hostname.clone());
    }
    st.db.device_put(&dev)?;
    st.db.audit("agent", "agent_enroll", Some(&dev.name), "ok", Some(&ip), Some(&format!("{} agent {}", r.os, r.version)));
    Ok(Json(AgentEnrollResponse { token, device_id, controller_id: st.controller_id.clone() }))
}

pub async fn heartbeat(State(st): State<St>, axum::Extension(ctx): axum::Extension<AgentCtx>, Json(hb): Json<AgentHeartbeat>) -> ApiResult<serde_json::Value> {
    st.db.state_heartbeat(&ctx.device_id, &hb.version, &hb.sysinfo)?;
    // learn the IP address for devices registered without one (first non-loopback address)
    if let Ok(Some(mut d)) = st.db.device_get(&ctx.device_id) {
        if d.ip.is_none() {
            let loc_ifaces = lwm_core::net::list_interfaces();
            let ip = hb.sysinfo.networks.iter().flat_map(|n| n.ips.iter()).find(|ip| ip.parse::<std::net::Ipv4Addr>().map(|a| a.is_private() && !loc_ifaces.iter().any(|i| i.ip == **ip)).unwrap_or(false)).cloned();
            if ip.is_some() {
                d.ip = ip;
                let _ = st.db.device_put(&d);
            }
        }
    }
    st.emit(serde_json::json!({"type": "device_status", "device_id": ctx.device_id, "status": "online", "last_seen": lwm_core::util::now()}));
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct PollQ {
    wait: Option<u64>,
}
pub async fn poll(State(st): State<St>, axum::Extension(ctx): axum::Extension<AgentCtx>, Query(q): Query<PollQ>) -> ApiResult<Vec<AgentCommand>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(q.wait.unwrap_or(0).min(30));
    loop {
        let notified = st.agent_notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let cmds: Vec<AgentCommand> = st.agent_queue.lock().unwrap().get_mut(&ctx.device_id).map(|q| q.drain(..).collect()).unwrap_or_default();
        if !cmds.is_empty() {
            return Ok(Json(cmds));
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(Json(vec![]));
        }
        let _ = tokio::time::timeout(deadline - now, notified).await;
    }
}

pub async fn result(State(st): State<St>, axum::Extension(_ctx): axum::Extension<AgentCtx>, Path(id): Path<String>, Json(r): Json<AgentCommandResult>) -> ApiResult<serde_json::Value> {
    if let Some(tx) = st.cmd_waiters.lock().unwrap().remove(&id) {
        let _ = tx.send(r);
    }
    Ok(Json(serde_json::json!({"ok": true})))
}
