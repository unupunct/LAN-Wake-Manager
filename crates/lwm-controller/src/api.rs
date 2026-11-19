//! REST + WebSocket API (docs/API.md).
use crate::auth::{Auth, Role};
use crate::state::{St, Settings};
use crate::{agent_api, auth, backup, connectivity, ops, scheduler, ui, views};
use axum::body::Bytes;
use futures_util::SinkExt;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{middleware, Json, Router};
use lwm_core::models::*;
use lwm_core::server::{ApiError, ApiResult};
use lwm_core::util::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;

type R<T> = Result<T, ApiError>;

pub fn router(st: St) -> Router {
    let public = Router::new()
        .route("/info", get(info))
        .route("/auth/setup", post(auth_setup))
        .route("/auth/login", post(auth_login))
        .route("/pair", post(pair))
        .route("/events", get(ws_events))
        .route("/agent/enroll", post(agent_api::enroll));
    let agent = Router::new()
        .route("/agent/heartbeat", post(agent_api::heartbeat))
        .route("/agent/poll", get(agent_api::poll))
        .route("/agent/commands/{id}/result", post(agent_api::result))
        .layer(middleware::from_fn_with_state(st.clone(), agent_api::auth));
    let authed = Router::new()
        .route("/auth/logout", post(auth_logout))
        .route("/auth/password", post(auth_password))
        .route("/me", get(me))
        .route("/dashboard", get(dashboard))
        .route("/locations", get(locations_list).post(locations_create))
        .route("/locations/{id}", get(location_get).put(location_update).delete(location_delete))
        .route("/locations/{id}/relay/probe", post(relay_probe))
        .route("/locations/{id}/relay/pair", post(relay_pair))
        .route("/locations/{id}/relay", delete(relay_unpair))
        .route("/locations/{id}/interfaces", get(location_interfaces))
        .route("/devices", get(devices_list).post(device_create))
        .route("/devices/{id}", get(device_get).put(device_update).delete(device_delete))
        .route("/devices/{id}/metrics", get(device_metrics))
        .route("/devices/{id}/troubleshoot", get(device_troubleshoot))
        .route("/devices/{id}/refresh", post(device_refresh))
        .route("/devices/{id}/agent-enrollment", post(agent_enrollment))
        .route("/devices/{id}/agent", delete(agent_revoke))
        .route("/devices/{id}/wake", post(device_wake))
        .route("/devices/{id}/shutdown", post(device_shutdown))
        .route("/devices/{id}/restart", post(device_restart))
        .route("/devices/{id}/cancel-shutdown", post(device_cancel))
        .route("/bulk", post(bulk))
        .route("/groups", get(groups_list).post(group_create))
        .route("/groups/{id}", axum::routing::put(group_update).delete(group_delete))
        .route("/groups/{id}/action", post(group_action))
        .route("/operations", get(operations_list))
        .route("/operations/{id}", get(operation_get))
        .route("/schedules", get(schedules_list).post(schedule_create))
        .route("/schedules/{id}", axum::routing::put(schedule_update).delete(schedule_delete))
        .route("/schedules/{id}/run-now", post(schedule_run_now))
        .route("/discovery", post(discovery_start))
        .route("/discovery/{id}", get(discovery_get).delete(discovery_cancel))
        .route("/connectivity", get(connectivity_get))
        .route("/wireguard-profiles", get(wg_list).post(wg_create))
        .route("/wireguard-profiles/{id}", axum::routing::put(wg_update).delete(wg_delete))
        .route("/diagnostics", get(diagnostics))
        .route("/diagnostics/ping", post(diag_ping))
        .route("/diagnostics/dns", post(diag_dns))
        .route("/diagnostics/tcp", post(diag_tcp))
        .route("/pairing", post(pairing_create))
        .route("/mobile-devices", get(mobile_list))
        .route("/mobile-devices/{id}", axum::routing::patch(mobile_patch).delete(mobile_revoke))
        .route("/audit", get(audit_list))
        .route("/notifications", get(notifications_list))
        .route("/settings", get(settings_get).put(settings_put))
        .route("/backups", get(backups_list).post(backups_create))
        .route("/backups/{name}/restore", post(backups_restore))
        .route("/export", get(export_get))
        .route("/import", post(import_post))
        .route("/ssh-keys", get(ssh_keys_list).post(ssh_keys_create))
        .route("/ssh-keys/{id}", delete(ssh_keys_delete))
        .layer(middleware::from_fn_with_state(st.clone(), auth::middleware));
    Router::new()
        .nest("/api/v1", public.merge(agent).merge(authed))
        .fallback(ui::serve)
        .layer(middleware::from_fn(lwm_core::server::security_headers))
        .layer(middleware::from_fn_with_state(st.allow_public.clone(), lwm_core::server::private_only))
        .with_state(st)
}

// ---------------------------------------------------------------- helpers
fn audit(st: &St, a: &Auth, action: &str, target: Option<&str>, result: &str, detail: Option<&str>) {
    st.db.audit(&a.actor(), action, target, result, Some(&a.ip), detail);
}
fn opt_body<T: Default + serde::de::DeserializeOwned>(b: &Bytes) -> R<T> {
    if b.iter().all(|c| c.is_ascii_whitespace()) {
        return Ok(T::default());
    }
    serde_json::from_slice(b).map_err(|e| ApiError::bad_request(format!("invalid JSON body: {e}")))
}
fn ok() -> Json<Value> {
    Json(json!({"ok": true}))
}
fn loc_rec(st: &St, id: &str) -> R<LocationRec> {
    st.db.doc_get("locations", id)?.ok_or_else(|| ApiError::not_found("location not found"))
}
fn dev_rec(st: &St, id: &str) -> R<DeviceRec> {
    st.db.device_get(id)?.ok_or_else(|| ApiError::not_found("device not found"))
}
fn valid_name(n: &str, what: &str) -> R<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(ApiError::bad_request(format!("{what} name must be 1-80 characters")));
    }
    Ok(n.to_string())
}
fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

// ---------------------------------------------------------------- public
async fn info(State(st): State<St>) -> Json<Value> {
    let set = st.settings();
    Json(json!({
        "controller_id": st.controller_id, "name": set.name, "version": lwm_core::VERSION, "api_version": lwm_core::API_VERSION,
        "setup_required": st.db.kv_get("admin_password").ok().flatten().is_none(),
        "tls_fingerprint": st.fingerprint, "time": now()
    }))
}

const SESSION_TTL: i64 = 7 * 86400;

#[derive(Deserialize)]
struct PwBody {
    password: String,
}
async fn new_session(st: &St) -> R<Value> {
    let token = random_token();
    let exp = st.db.session_create(&sha256_hex(token.as_bytes()), SESSION_TTL)?;
    Ok(json!({"token": token, "expires_at": exp, "role": "admin"}))
}
async fn auth_setup(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(b): Json<PwBody>) -> ApiResult<Value> {
    if !addr.ip().is_loopback() {
        return Err(ApiError::forbidden("the admin password can only be set from the computer running the controller"));
    }
    if st.db.kv_get("admin_password")?.is_some() {
        return Err(ApiError::conflict("an admin password is already set"));
    }
    if b.password.chars().count() < 10 {
        return Err(ApiError::bad_request("the password must be at least 10 characters"));
    }
    st.db.kv_set("admin_password", &hash_password(&b.password)?)?;
    st.db.audit("admin", "setup", None, "ok", Some(&addr.ip().to_string()), Some("admin password created"));
    Ok(Json(new_session(&st).await?))
}
async fn auth_login(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(b): Json<PwBody>) -> ApiResult<Value> {
    let ip = addr.ip().to_string();
    st.limiter.check(&format!("login:{ip}"), 10, Duration::from_secs(300)).map_err(ApiError::rate_limited)?;
    let hash = st.db.kv_get("admin_password")?.ok_or_else(|| ApiError::conflict("no admin password set yet; open the dashboard on the controller computer to create one"))?;
    // verifying is deliberately slow (argon2); do it off the async threads
    let ok = tokio::task::spawn_blocking(move || verify_password(&b.password, &hash)).await.unwrap_or(false);
    if !ok {
        st.db.audit("anonymous", "login", None, "denied", Some(&ip), Some("wrong password"));
        return Err(ApiError::unauthorized("wrong password"));
    }
    st.db.audit("admin", "login", None, "ok", Some(&ip), None);
    Ok(Json(new_session(&st).await?))
}
async fn auth_logout(State(st): State<St>, a: Auth) -> ApiResult<Value> {
    if a.kind == "session" {
        st.db.session_delete(&a.token_hash)?;
    }
    Ok(ok())
}
#[derive(Deserialize)]
struct PwChange {
    old_password: String,
    new_password: String,
}
async fn auth_password(State(st): State<St>, a: Auth, Json(b): Json<PwChange>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    if a.kind != "session" {
        return Err(ApiError::forbidden("the password can only be changed from the dashboard"));
    }
    st.limiter.check(&format!("login:{}", a.ip), 10, Duration::from_secs(300)).map_err(ApiError::rate_limited)?;
    let hash = st.db.kv_get("admin_password")?.unwrap_or_default();
    let old = b.old_password.clone();
    if !tokio::task::spawn_blocking(move || verify_password(&old, &hash)).await.unwrap_or(false) {
        audit(&st, &a, "password_change", None, "denied", None);
        return Err(ApiError::forbidden("the current password is wrong"));
    }
    if b.new_password.chars().count() < 10 {
        return Err(ApiError::bad_request("the new password must be at least 10 characters"));
    }
    st.db.kv_set("admin_password", &hash_password(&b.new_password)?)?;
    st.db.sessions_clear()?;
    audit(&st, &a, "password_change", None, "ok", Some("all dashboard sessions signed out"));
    Ok(Json(new_session(&st).await?))
}

#[derive(Deserialize)]
struct PairBody {
    pairing_token: String,
    device_name: String,
    #[serde(default)]
    platform: String,
}
async fn pair(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(b): Json<PairBody>) -> ApiResult<Value> {
    let ip = addr.ip().to_string();
    st.limiter.check(&format!("pair:{ip}"), 10, Duration::from_secs(300)).map_err(ApiError::rate_limited)?;
    let role = match st.db.pairing_consume(&sha256_hex(b.pairing_token.as_bytes()))? {
        Some(r) => r,
        None => {
            st.db.audit("anonymous", "pair", None, "denied", Some(&ip), Some("invalid, expired or already used pairing token"));
            return Err(ApiError::forbidden("the pairing code is invalid, expired or was already used"));
        }
    };
    let name = valid_name(&b.device_name, "device")?;
    let token = random_token();
    let m = crate::db::MobileDevice { id: new_id(), name: name.clone(), platform: Some(b.platform.chars().take(30).collect()), role: role.clone(), created_at: now(), last_seen: Some(now()), last_ip: Some(ip.clone()), revoked: false };
    st.db.mobile_add(&m, &sha256_hex(token.as_bytes()))?;
    st.db.audit(&name, "pair", Some(&m.id), "ok", Some(&ip), Some(&format!("role {role}")));
    st.notify("controller_notice", "info", "Mobile device paired", &format!("{name} was paired with the {role} role."), None);
    Ok(Json(json!({"device_id": m.id, "token": token, "role": role, "controller": {"id": st.controller_id, "name": st.settings().name}})))
}

// ---------------------------------------------------------------- websocket
async fn ws_events(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |sock| ws_session(st, addr.ip().to_string(), sock))
}
async fn ws_session(st: St, ip: String, mut sock: WebSocket) {
    let first = tokio::time::timeout(Duration::from_secs(5), sock.recv()).await;
    let token = match first {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<Value>(&t).ok().filter(|v| v["type"] == "auth").and_then(|v| v["token"].as_str().map(|s| s.to_string())),
        _ => None,
    };
    let authed = token.as_deref().and_then(|t| auth::resolve(&st, t, &ip));
    if authed.is_none() {
        let _ = st.limiter.check(&format!("authfail:{ip}"), 20, Duration::from_secs(60));
        let _ = sock.send(Message::Text(json!({"type": "error", "error": "unauthorized"}).to_string().into())).await;
        let _ = sock.close().await;
        return;
    }
    let mut rx = st.events.subscribe();
    let mut shutdown = st.shutdown.clone();
    if sock.send(Message::Text(json!({"type": "hello", "controller_id": st.controller_id}).to_string().into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(v) => { if sock.send(Message::Text(v.to_string().into())).await.is_err() { return; } }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { let _ = sock.send(Message::Text(json!({"type":"resync"}).to_string().into())).await; }
                Err(_) => return,
            },
            m = sock.recv() => match m {
                Some(Ok(Message::Ping(p))) => { let _ = sock.send(Message::Pong(p)).await; }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            },
            _ = shutdown.changed() => { if *shutdown.borrow() { return; } }
        }
    }
}

// ---------------------------------------------------------------- me / dashboard
async fn me(a: Auth) -> Json<Value> {
    Json(json!({"kind": a.kind, "role": a.role.as_str(), "device_id": a.device_id, "name": a.name}))
}

async fn dashboard(State(st): State<St>, _a: Auth) -> ApiResult<Value> {
    let devs = views::devices(&st, None, None)?;
    let locs = views::locations(&st).await?;
    let conn = connectivity::get(false).await;
    let scheds: Vec<Schedule> = st.db.doc_list("schedules")?;
    let mut upcoming: Vec<Value> = scheds.iter().filter(|s| s.enabled).filter_map(|s| scheduler::next_run(&st, s).map(|n| json!({"schedule_id": s.id, "name": s.name, "action": s.action, "next_run": n}))).collect();
    upcoming.sort_by_key(|v| v["next_run"].as_i64().unwrap_or(0));
    let relays_total = locs.iter().filter(|l| l.relay_mode == "remote").count() + locs.iter().filter(|l| l.relay_mode == "builtin").count();
    let relays_online = locs.iter().filter(|l| l.relay_status == "online" || l.relay_status == "builtin").count();
    let wg = st.db.doc_list::<Value>("wireguard_profiles")?.len();
    let active_vpn = (conn.tailscale.running as usize) + conn.addresses.iter().filter(|x| x.kind == "wireguard").count();
    Ok(Json(json!({
        "totals": {"devices": devs.len(), "online": devs.iter().filter(|d| d.status == "online").count(), "offline": devs.iter().filter(|d| d.status == "offline").count(), "unknown": devs.iter().filter(|d| d.status != "online" && d.status != "offline").count()},
        "relays": {"total": relays_total, "online": relays_online},
        "vpn": {"tailscale": {"installed": conn.tailscale.installed, "running": conn.tailscale.running, "ip": conn.tailscale.ip, "hostname": conn.tailscale.magicdns_name.clone().or(conn.tailscale.hostname.clone())}, "wireguard_profiles": wg, "active_connections": active_vpn},
        "pending_schedules": upcoming.len(),
        "failed_operations_24h": st.db.ops_failed_since(now() - 86400)?,
        "recent_operations": st.db.ops(10, None, None, None)?,
        "upcoming_schedules": upcoming.into_iter().take(8).collect::<Vec<_>>(),
        "locations": locs
    })))
}

// ---------------------------------------------------------------- locations
async fn locations_list(State(st): State<St>, _a: Auth) -> ApiResult<Vec<Location>> {
    Ok(Json(views::locations(&st).await?))
}
async fn location_get(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Location> {
    views::locations(&st).await?.into_iter().find(|l| l.id == id).map(Json).ok_or_else(|| ApiError::not_found("location not found"))
}
fn apply_location_input(rec: &mut LocationRec, i: LocationInput) -> R<()> {
    rec.name = valid_name(&i.name, "location")?;
    if let Some(c) = i.color {
        if !valid_color(&c) {
            return Err(ApiError::bad_request("color must look like #2ee59d"));
        }
        rec.color = c;
    }
    if let Some(n) = i.notes {
        rec.notes = n.chars().take(500).collect();
    }
    rec.broadcast_addr = match i.broadcast_addr.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(b) => {
            b.parse::<std::net::Ipv4Addr>().map_err(|_| ApiError::bad_request("broadcast address must be an IPv4 address such as 192.168.1.255"))?;
            Some(b.to_string())
        }
    };
    if let Some(p) = i.wol_ports {
        if p.is_empty() || p.len() > 6 || p.iter().any(|x| *x == 0) {
            return Err(ApiError::bad_request("provide 1-6 UDP ports (usually 9 and 7)"));
        }
        rec.wol_ports = p;
    }
    if let Some(a) = i.wake_attempts {
        if !(1..=20).contains(&a) {
            return Err(ApiError::bad_request("attempts must be 1-20"));
        }
        rec.wake_attempts = a;
    }
    if let Some(a) = i.wake_interval_secs {
        if !(1..=60).contains(&a) {
            return Err(ApiError::bad_request("retry interval must be 1-60 seconds"));
        }
        rec.wake_interval_secs = a;
    }
    if let Some(a) = i.wake_timeout_secs {
        if !(5..=900).contains(&a) {
            return Err(ApiError::bad_request("wake timeout must be 5-900 seconds"));
        }
        rec.wake_timeout_secs = a;
    }
    Ok(())
}
fn blank_location() -> LocationRec {
    LocationRec { id: new_id(), name: String::new(), color: "#2ee59d".into(), notes: String::new(), broadcast_addr: None, wol_ports: vec![9, 7], wake_attempts: 3, wake_interval_secs: 2, wake_timeout_secs: 120, relay_url: None, relay_fingerprint: None, relay_token: None, relay_name: None, relay_version: None, relay_last_seen: None, created_at: now() }
}
async fn locations_create(State(st): State<St>, a: Auth, Json(i): Json<LocationInput>) -> R<(StatusCode, Json<Location>)> {
    a.need(Role::Admin)?;
    let mut rec = blank_location();
    apply_location_input(&mut rec, i)?;
    st.db.doc_put("locations", &rec.id, &rec)?;
    audit(&st, &a, "location_create", Some(&rec.name), "ok", None);
    let l = views::locations(&st).await?.into_iter().find(|l| l.id == rec.id).ok_or_else(|| ApiError::internal("lost location"))?;
    Ok((StatusCode::CREATED, Json(l)))
}
async fn location_update(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(i): Json<LocationInput>) -> ApiResult<Location> {
    a.need(Role::Admin)?;
    let mut rec = loc_rec(&st, &id)?;
    apply_location_input(&mut rec, i)?;
    st.db.doc_put("locations", &id, &rec)?;
    audit(&st, &a, "location_update", Some(&rec.name), "ok", None);
    location_get(State(st), a, Path(id)).await
}
async fn location_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let rec = loc_rec(&st, &id)?;
    if !st.db.devices(Some(&id))?.is_empty() {
        return Err(ApiError::conflict("this location still has computers; delete or move them first"));
    }
    st.db.doc_delete("locations", &id)?;
    audit(&st, &a, "location_delete", Some(&rec.name), "ok", None);
    Ok(ok())
}
#[derive(Deserialize)]
struct UrlBody {
    url: String,
}
async fn relay_probe(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(b): Json<UrlBody>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    loc_rec(&st, &id)?;
    let (host, port) = lwm_core::tls::split_https_url(&b.url).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let fp = lwm_core::tls::fetch_fingerprint(&host, port).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "relay_unreachable", format!("cannot reach {host}:{port}: {e}")))?;
    let c = lwm_core::client::ApiClient::new(&b.url, &fp, None, Duration::from_secs(8)).map_err(|e| ApiError::internal(e.to_string()))?;
    let info: RelayInfo = c.get("/v1/info").await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "relay_unreachable", format!("that address is not a LAN Wake Relay: {e}")))?;
    Ok(Json(json!({"fingerprint": fp, "relay_id": info.relay_id, "name": info.name, "version": info.version})))
}
#[derive(Deserialize)]
struct RelayPairBody {
    url: String,
    code: String,
    fingerprint: String,
}
async fn relay_pair(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(b): Json<RelayPairBody>) -> ApiResult<Location> {
    a.need(Role::Admin)?;
    let mut rec = loc_rec(&st, &id)?;
    lwm_core::tls::split_https_url(&b.url).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let fp = lwm_core::tls::normalize_fp(&b.fingerprint);
    let c = lwm_core::client::ApiClient::new(&b.url, &fp, None, Duration::from_secs(10)).map_err(|e| ApiError::internal(e.to_string()))?;
    let r: Value = c.post("/v1/enroll", &json!({"code": b.code, "controller_id": st.controller_id})).await.map_err(|e| {
        audit(&st, &a, "relay_pair", Some(&rec.name), "denied", Some(&e.to_string()));
        ApiError::new(StatusCode::BAD_GATEWAY, "relay_unreachable", format!("pairing failed: {e}"))
    })?;
    rec.relay_url = Some(b.url.trim().trim_end_matches('/').to_string());
    rec.relay_fingerprint = Some(fp);
    rec.relay_token = r["token"].as_str().map(|s| s.to_string());
    rec.relay_name = r["name"].as_str().map(|s| s.to_string());
    rec.relay_version = r["version"].as_str().map(|s| s.to_string());
    rec.relay_last_seen = Some(now());
    st.db.doc_put("locations", &id, &rec)?;
    audit(&st, &a, "relay_pair", Some(&rec.name), "ok", rec.relay_url.as_deref());
    tokio::spawn({
        let st = st.clone();
        async move { scheduler::sync_relays(&st).await }
    });
    location_get(State(st), a, Path(id)).await
}
async fn relay_unpair(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Location> {
    a.need(Role::Admin)?;
    let mut rec = loc_rec(&st, &id)?;
    rec.relay_url = None;
    rec.relay_fingerprint = None;
    rec.relay_token = None;
    rec.relay_name = None;
    rec.relay_version = None;
    rec.relay_last_seen = None;
    st.db.doc_put("locations", &id, &rec)?;
    audit(&st, &a, "relay_unpair", Some(&rec.name), "ok", None);
    location_get(State(st), a, Path(id)).await
}
async fn location_interfaces(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Vec<IfaceInfo>> {
    let rec = loc_rec(&st, &id)?;
    Ok(Json(crate::relay::for_location(&st, &rec)?.interfaces().await?))
}

// ---------------------------------------------------------------- devices
#[derive(Deserialize)]
struct DevQ {
    location_id: Option<String>,
    group_id: Option<String>,
}
async fn devices_list(State(st): State<St>, _a: Auth, Query(q): Query<DevQ>) -> ApiResult<Vec<Device>> {
    Ok(Json(views::devices(&st, q.location_id.as_deref(), q.group_id.as_deref())?))
}
async fn device_get(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Device> {
    views::device(&st, &id)?.map(Json).ok_or_else(|| ApiError::not_found("device not found"))
}
fn apply_device_input(st: &St, rec: &mut DeviceRec, i: DeviceInput) -> R<()> {
    loc_rec(st, &i.location_id)?;
    rec.location_id = i.location_id;
    rec.name = valid_name(&i.name, "computer")?;
    rec.mac = normalize_mac(&i.mac).ok_or_else(|| ApiError::bad_request("invalid MAC address (expected AA:BB:CC:DD:EE:FF)"))?;
    rec.ip = match i.ip.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(ip) => {
            if !is_safe_host(ip) {
                return Err(ApiError::bad_request("invalid IP address or host name"));
            }
            Some(ip.to_string())
        }
    };
    rec.hostname = i.hostname.map(|h| h.trim().to_string()).filter(|h| !h.is_empty());
    if let Some(o) = i.os {
        if !matches!(o.as_str(), "windows" | "linux" | "macos" | "other" | "") {
            return Err(ApiError::bad_request("os must be windows, linux, macos or other"));
        }
        rec.os = if o.is_empty() { None } else { Some(o) };
    }
    if let Some(t) = i.device_type {
        if !matches!(t.as_str(), "pc" | "laptop" | "server" | "nas" | "other") {
            return Err(ApiError::bad_request("device_type must be pc, laptop, server, nas or other"));
        }
        rec.device_type = t;
    }
    if let Some(g) = i.group_ids {
        let known: Vec<Group> = st.db.doc_list("groups")?;
        if g.iter().any(|id| !known.iter().any(|k| &k.id == id)) {
            return Err(ApiError::bad_request("unknown group id"));
        }
        rec.group_ids = g;
    }
    if let Some(p) = i.tcp_check_ports {
        if p.len() > 12 || p.iter().any(|x| *x == 0) {
            return Err(ApiError::bad_request("at most 12 TCP check ports, each 1-65535"));
        }
        rec.tcp_check_ports = p;
    }
    if let Some(m) = i.power_method {
        if !matches!(m.as_str(), "agent" | "ssh" | "winrm" | "none") {
            return Err(ApiError::bad_request("power_method must be agent, ssh, winrm or none"));
        }
        rec.power_method = m;
    }
    rec.ssh = i.ssh;
    if rec.power_method == "ssh" {
        let s = rec.ssh.as_ref().ok_or_else(|| ApiError::bad_request("SSH settings (user, port) are required for the ssh method"))?;
        if !is_safe_user(&s.user) || s.port == 0 {
            return Err(ApiError::bad_request("invalid SSH user name or port"));
        }
    }
    if rec.power_method == "winrm" && rec.ip.is_none() {
        return Err(ApiError::bad_request("WinRM needs the computer's IP address or host name"));
    }
    if let Some(n) = i.notes {
        rec.notes = n.chars().take(500).collect();
    }
    Ok(())
}
async fn device_create(State(st): State<St>, a: Auth, Json(i): Json<DeviceInput>) -> R<(StatusCode, Json<Device>)> {
    a.need(Role::Admin)?;
    let mut rec = DeviceRec { id: new_id(), location_id: String::new(), name: String::new(), hostname: None, ip: None, mac: String::new(), os: None, device_type: "pc".into(), wol_capable: "unknown".into(), group_ids: vec![], tcp_check_ports: vec![], power_method: "none".into(), ssh: None, notes: String::new(), agent_token_hash: None, created_at: now() };
    apply_device_input(&st, &mut rec, i)?;
    if st.db.devices(Some(&rec.location_id))?.iter().any(|d| d.mac == rec.mac) {
        return Err(ApiError::conflict("a computer with this MAC address already exists in this location"));
    }
    st.db.device_put(&rec)?;
    audit(&st, &a, "device_create", Some(&rec.name), "ok", Some(&rec.mac));
    let d = views::device(&st, &rec.id)?.ok_or_else(|| ApiError::internal("lost device"))?;
    let st2 = st.clone();
    tokio::spawn(async move {
        if let Ok(Some(l)) = st2.db.doc_get::<LocationRec>("locations", &d.location_id) {
            crate::monitor::location_cycle(&st2, l).await;
        }
    });
    Ok((StatusCode::CREATED, Json(views::device(&st, &rec.id)?.unwrap())))
}
async fn device_update(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(i): Json<DeviceInput>) -> ApiResult<Device> {
    a.need(Role::Admin)?;
    let mut rec = dev_rec(&st, &id)?;
    apply_device_input(&st, &mut rec, i)?;
    if st.db.devices(Some(&rec.location_id))?.iter().any(|d| d.mac == rec.mac && d.id != rec.id) {
        return Err(ApiError::conflict("another computer with this MAC address exists in this location"));
    }
    st.db.device_put(&rec)?;
    audit(&st, &a, "device_update", Some(&rec.name), "ok", None);
    Ok(Json(views::device(&st, &id)?.unwrap()))
}
async fn device_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let rec = dev_rec(&st, &id)?;
    st.db.device_delete(&id)?;
    audit(&st, &a, "device_delete", Some(&rec.name), "ok", None);
    Ok(ok())
}
#[derive(Deserialize)]
struct MinQ {
    minutes: Option<i64>,
}
async fn device_metrics(State(st): State<St>, _a: Auth, Path(id): Path<String>, Query(q): Query<MinQ>) -> ApiResult<Vec<crate::db::MetricPoint>> {
    dev_rec(&st, &id)?;
    Ok(Json(st.db.metrics(&id, q.minutes.unwrap_or(1440).clamp(5, 10080))?))
}
async fn device_refresh(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Device> {
    let d = dev_rec(&st, &id)?;
    let l = loc_rec(&st, &d.location_id)?;
    crate::monitor::location_cycle(&st, l).await;
    Ok(Json(views::device(&st, &id)?.unwrap()))
}
async fn device_troubleshoot(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    let d = dev_rec(&st, &id)?;
    let l = loc_rec(&st, &d.location_id)?;
    let s = st.db.state_get(&id)?;
    let mut checks: Vec<Value> = vec![];
    let mut advice: Vec<String> = vec![];
    let mut chk = |name: &str, status: &str, detail: String| checks.push(json!({"name": name, "status": status, "detail": detail}));
    chk("MAC address", if normalize_mac(&d.mac).is_some() { "ok" } else { "fail" }, d.mac.clone());
    match &d.ip {
        Some(ip) => chk("IP address", "ok", ip.clone()),
        None => {
            chk("IP address", "warn", "not set: wake packets can be sent but start-up cannot be verified".into());
            advice.push("Enter the computer's IP address (or install the agent, which reports it) so that a wake can be confirmed.".into());
        }
    }
    let relay = crate::relay::for_location(&st, &l);
    match &relay {
        Ok(r) => {
            let t = std::time::Instant::now();
            match r.health().await {
                Ok(()) => chk("Wake Relay", "ok", format!("{} answered in {} ms", if r.is_remote() { "remote relay" } else { "built-in relay" }, t.elapsed().as_millis())),
                Err(e) => {
                    chk("Wake Relay", "fail", e.message);
                    advice.push("The relay of this location is unreachable. Check that it is running and that the VPN is up.".into());
                }
            }
            if let (Some(ip), Ok(ifaces)) = (d.ip.as_deref().and_then(|i| i.parse::<std::net::Ipv4Addr>().ok()), r.interfaces().await) {
                let same = ifaces.iter().find(|i| i.subnet_cidr.parse::<ipnet::Ipv4Net>().map(|n| n.contains(&ip)).unwrap_or(false));
                match same {
                    Some(i) => chk("Same subnet as relay", "ok", format!("{} ({})", i.name, i.subnet_cidr)),
                    None => {
                        chk("Same subnet as relay", "fail", format!("{ip} is not in any subnet of the relay computer"));
                        advice.push("Magic packets are broadcasts and do not cross routers. Put the relay on the same LAN as this computer, or set a subnet-directed broadcast address in the location settings and enable directed broadcasts on the router.".into());
                    }
                }
            }
        }
        Err(e) => chk("Wake Relay", "fail", e.message.clone()),
    }
    chk("Current state", if d.ip.is_none() && s.status == "unknown" { "info" } else { "info" }, format!("{} (checked {})", s.status, s.checked_at.map(|t| format!("{} s ago", now() - t)).unwrap_or_else(|| "never".into())));
    chk("Wake-on-LAN confirmed", if d.wol_capable == "yes" { "ok" } else { "info" }, if d.wol_capable == "yes" { "this computer was woken successfully before".into() } else { "not confirmed yet: it becomes 'yes' after the first successful wake".to_string() });
    chk("Agent", if d.agent_token_hash.is_some() { if views::agent_online(&s) { "ok" } else { "warn" } } else { "info" }, if d.agent_token_hash.is_none() { "not installed (needed for shutdown/restart unless SSH is configured)".into() } else if views::agent_online(&s) { "online".into() } else { "enrolled but offline".into() });
    advice.push("BIOS/UEFI: enable 'Wake on LAN' / 'Power on by PCI-E' and disable 'ErP / EuP / Deep Sleep' power saving.".into());
    advice.push("Windows: Device Manager > network adapter > Power Management: allow 'Wake on Magic Packet'; disable 'Fast Startup' (Control Panel > Power Options).".into());
    advice.push("Linux: run `sudo ethtool -s <iface> wol g` (make it persistent with NetworkManager or a udev rule).".into());
    advice.push("Wi-Fi adapters usually cannot be woken from a powered-off state; use the Ethernet port.".into());
    Ok(Json(json!({"checks": checks, "advice": advice})))
}
async fn agent_enrollment(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let d = dev_rec(&st, &id)?;
    let code = pairing_code();
    let exp = now() + 1800;
    st.db.enroll_add(&sha256_hex(normalize_code(&code).as_bytes()), &id, exp)?;
    let conn = connectivity::get(false).await;
    let host = conn.tailscale.magicdns_name.clone().or(conn.tailscale.ip.clone()).or_else(|| conn.addresses.iter().find(|x| x.kind == "lan").map(|x| x.address.clone())).unwrap_or_else(|| "CONTROLLER-ADDRESS".into());
    let url = format!("https://{host}:{}", st.https_port);
    let tail = format!("--controller {url} --code {code} --fingerprint {}", st.fingerprint);
    audit(&st, &a, "agent_enrollment", Some(&d.name), "ok", Some("one-time code created, valid 30 minutes"));
    Ok(Json(json!({
        "code": code, "expires_at": exp, "controller_url": url, "fingerprint": st.fingerprint,
        "command_windows": format!("& \"C:\\Program Files\\LAN Wake Manager\\Agent\\lwm-agent.exe\" enroll {tail}; & \"C:\\Program Files\\LAN Wake Manager\\Agent\\lwm-agent.exe\" install-service"),
        "command_linux": format!("sudo lwm-agent enroll {tail} && sudo lwm-agent install-service")
    })))
}
async fn agent_revoke(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let mut d = dev_rec(&st, &id)?;
    d.agent_token_hash = None;
    st.db.device_put(&d)?;
    st.agent_queue.lock().unwrap().remove(&id);
    audit(&st, &a, "agent_revoke", Some(&d.name), "ok", None);
    Ok(ok())
}

// ---------------------------------------------------------------- operations
#[derive(Deserialize, Default)]
struct DelayBody {
    #[serde(default)]
    delay_secs: u32,
}
fn origin(a: &Auth, source: &'static str) -> ops::Origin {
    ops::Origin { requested_by: a.actor(), source, schedule_id: None }
}
async fn power(st: St, a: Auth, id: String, kind: &str, body: Bytes) -> R<(StatusCode, Json<Operation>)> {
    a.need(Role::Operator)?;
    let b: DelayBody = opt_body(&body)?;
    if b.delay_secs > 86_400 * 7 {
        return Err(ApiError::bad_request("delay_secs must be at most 7 days"));
    }
    let op = ops::start(&st, &id, kind, b.delay_secs, origin(&a, "manual"))?;
    audit(&st, &a, kind, Some(&op.device_name), "accepted", Some(&op.id));
    Ok((StatusCode::ACCEPTED, Json(op)))
}
async fn device_wake(State(st): State<St>, a: Auth, Path(id): Path<String>, body: Bytes) -> R<(StatusCode, Json<Operation>)> {
    power(st, a, id, "wake", body).await
}
async fn device_shutdown(State(st): State<St>, a: Auth, Path(id): Path<String>, body: Bytes) -> R<(StatusCode, Json<Operation>)> {
    power(st, a, id, "shutdown", body).await
}
async fn device_restart(State(st): State<St>, a: Auth, Path(id): Path<String>, body: Bytes) -> R<(StatusCode, Json<Operation>)> {
    power(st, a, id, "restart", body).await
}
async fn device_cancel(State(st): State<St>, a: Auth, Path(id): Path<String>, body: Bytes) -> R<(StatusCode, Json<Operation>)> {
    power(st, a, id, "cancel_shutdown", body).await
}

#[derive(Deserialize, Default)]
struct BulkBody {
    #[serde(default)]
    action: String,
    #[serde(default)]
    device_ids: Vec<String>,
    group_id: Option<String>,
    location_id: Option<String>,
    #[serde(default)]
    delay_secs: u32,
    #[serde(default)]
    confirm: bool,
}
async fn run_bulk(st: &St, a: &Auth, b: BulkBody) -> R<Json<Value>> {
    a.need(Role::Operator)?;
    if !matches!(b.action.as_str(), "wake" | "shutdown" | "restart") {
        return Err(ApiError::bad_request("action must be wake, shutdown or restart"));
    }
    if (b.action == "shutdown" || b.action == "restart") && !b.confirm {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "confirmation_required", "bulk shutdown/restart needs explicit confirmation (confirm: true)"));
    }
    let all = st.db.devices(None)?;
    let targets: Vec<&DeviceRec> = all.iter().filter(|d| b.device_ids.contains(&d.id) || b.group_id.as_ref().map(|g| d.group_ids.contains(g)).unwrap_or(false) || b.location_id.as_ref().map(|l| &d.location_id == l).unwrap_or(false)).collect();
    if targets.is_empty() {
        return Err(ApiError::bad_request("no computers match the request"));
    }
    if targets.len() > 200 {
        return Err(ApiError::bad_request("too many targets"));
    }
    let mut out = vec![];
    for d in targets {
        out.push(ops::start(st, &d.id, &b.action, b.delay_secs, origin(a, "bulk"))?);
    }
    audit(st, a, &format!("bulk_{}", b.action), None, "accepted", Some(&format!("{} computers", out.len())));
    Ok(Json(json!({"operations": out})))
}
async fn bulk(State(st): State<St>, a: Auth, Json(b): Json<BulkBody>) -> R<Json<Value>> {
    run_bulk(&st, &a, b).await
}
async fn group_action(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(mut b): Json<BulkBody>) -> R<Json<Value>> {
    st.db.doc_get::<Group>("groups", &id)?.ok_or_else(|| ApiError::not_found("group not found"))?;
    b.group_id = Some(id);
    b.device_ids.clear();
    b.location_id = None;
    run_bulk(&st, &a, b).await
}

#[derive(Deserialize)]
struct OpsQ {
    limit: Option<i64>,
    before: Option<i64>,
    device_id: Option<String>,
    state: Option<String>,
}
async fn operations_list(State(st): State<St>, _a: Auth, Query(q): Query<OpsQ>) -> ApiResult<Vec<Operation>> {
    Ok(Json(st.db.ops(q.limit.unwrap_or(50).clamp(1, 500), q.before, q.device_id.as_deref(), q.state.as_deref())?))
}
async fn operation_get(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<Operation> {
    st.db.op_get(&id)?.map(Json).ok_or_else(|| ApiError::not_found("operation not found"))
}

// ---------------------------------------------------------------- groups
const ICONS: &[&str] = &["desktop", "gamepad", "server", "nas", "building", "laptop", "star", "all"];
async fn groups_list(State(st): State<St>, _a: Auth) -> ApiResult<Vec<Group>> {
    Ok(Json(views::groups(&st)?))
}
fn group_from(i: GroupInput, id: String) -> R<Group> {
    let icon = i.icon.unwrap_or_else(|| "desktop".into());
    if !ICONS.contains(&icon.as_str()) {
        return Err(ApiError::bad_request(format!("icon must be one of {}", ICONS.join(", "))));
    }
    let color = i.color.unwrap_or_else(|| "#22d3ee".into());
    if !valid_color(&color) {
        return Err(ApiError::bad_request("color must look like #22d3ee"));
    }
    Ok(Group { id, name: valid_name(&i.name, "group")?, icon, color, device_count: 0, online_count: 0 })
}
async fn group_create(State(st): State<St>, a: Auth, Json(i): Json<GroupInput>) -> R<(StatusCode, Json<Group>)> {
    a.need(Role::Admin)?;
    let g = group_from(i, new_id())?;
    st.db.doc_put("groups", &g.id, &g)?;
    audit(&st, &a, "group_create", Some(&g.name), "ok", None);
    Ok((StatusCode::CREATED, Json(g)))
}
async fn group_update(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(i): Json<GroupInput>) -> ApiResult<Group> {
    a.need(Role::Admin)?;
    st.db.doc_get::<Group>("groups", &id)?.ok_or_else(|| ApiError::not_found("group not found"))?;
    let g = group_from(i, id)?;
    st.db.doc_put("groups", &g.id, &g)?;
    audit(&st, &a, "group_update", Some(&g.name), "ok", None);
    Ok(Json(g))
}
async fn group_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let g: Group = st.db.doc_get("groups", &id)?.ok_or_else(|| ApiError::not_found("group not found"))?;
    for mut d in st.db.devices(None)? {
        if d.group_ids.contains(&id) {
            d.group_ids.retain(|x| x != &id);
            st.db.device_put(&d)?;
        }
    }
    st.db.doc_delete("groups", &id)?;
    audit(&st, &a, "group_delete", Some(&g.name), "ok", None);
    Ok(ok())
}

// ---------------------------------------------------------------- schedules
fn with_next(st: &St, mut s: Schedule) -> Schedule {
    s.next_run = scheduler::next_run(st, &s);
    s
}
async fn schedules_list(State(st): State<St>, _a: Auth) -> ApiResult<Vec<Schedule>> {
    Ok(Json(st.db.doc_list::<Schedule>("schedules")?.into_iter().map(|s| with_next(&st, s)).collect()))
}
fn validate_schedule(st: &St, s: &mut Schedule) -> R<()> {
    s.name = valid_name(&s.name, "schedule")?;
    if !matches!(s.action.as_str(), "wake" | "shutdown" | "restart") {
        return Err(ApiError::bad_request("action must be wake, shutdown or restart"));
    }
    lwm_core::schedule::validate(&s.recurrence, s.start_date.as_deref(), s.end_date.as_deref()).map_err(ApiError::bad_request)?;
    if s.target.device_ids.is_empty() && s.target.group_ids.is_empty() && s.target.location_ids.is_empty() {
        return Err(ApiError::bad_request("choose at least one computer, group or location"));
    }
    if !matches!(s.missed_policy.as_str(), "skip" | "run_once") {
        return Err(ApiError::bad_request("missed_policy must be skip or run_once"));
    }
    if !(1..=10).contains(&s.retry.max_attempts) {
        return Err(ApiError::bad_request("retry.max_attempts must be 1-10"));
    }
    if s.delay_secs > 86_400 {
        return Err(ApiError::bad_request("delay_secs must be at most 24 hours"));
    }
    for d in &s.exceptions {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ApiError::bad_request("exceptions must be YYYY-MM-DD dates"))?;
    }
    let devs = scheduler::resolve_targets(st, &s.target);
    if devs.is_empty() {
        return Err(ApiError::bad_request("the selected targets contain no computers"));
    }
    match s.executor.as_str() {
        "controller" => {}
        "relay" => {
            if s.action != "wake" {
                return Err(ApiError::bad_request("only wake schedules can run on a relay (a sleeping computer cannot run its own schedule; shutdown/restart need the controller)"));
            }
            let locs: std::collections::HashSet<&String> = devs.iter().map(|d| &d.location_id).collect();
            if locs.len() != 1 {
                return Err(ApiError::bad_request("a relay schedule must target computers of exactly one location"));
            }
            let l = loc_rec(st, locs.into_iter().next().unwrap())?;
            if l.relay_url.is_none() {
                return Err(ApiError::bad_request("this location uses the built-in relay; use executor 'controller'"));
            }
        }
        _ => return Err(ApiError::bad_request("executor must be controller or relay")),
    }
    s.next_run = None;
    Ok(())
}
async fn schedule_create(State(st): State<St>, a: Auth, Json(mut s): Json<Schedule>) -> R<(StatusCode, Json<Schedule>)> {
    a.need(Role::Operator)?;
    s.id = new_id();
    s.last_run = None;
    s.last_state = None;
    validate_schedule(&st, &mut s)?;
    st.db.doc_put("schedules", &s.id, &s)?;
    st.db.schedule_set_cursor(&s.id, now());
    audit(&st, &a, "schedule_create", Some(&s.name), "ok", None);
    tokio::spawn({
        let st = st.clone();
        async move { scheduler::sync_relays(&st).await }
    });
    Ok((StatusCode::CREATED, Json(with_next(&st, s))))
}
async fn schedule_update(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(mut s): Json<Schedule>) -> ApiResult<Schedule> {
    a.need(Role::Operator)?;
    let old: Schedule = st.db.doc_get("schedules", &id)?.ok_or_else(|| ApiError::not_found("schedule not found"))?;
    s.id = id.clone();
    s.last_run = old.last_run;
    s.last_state = old.last_state;
    validate_schedule(&st, &mut s)?;
    st.db.doc_put("schedules", &id, &s)?;
    st.db.schedule_set_cursor(&id, now());
    audit(&st, &a, "schedule_update", Some(&s.name), "ok", None);
    tokio::spawn({
        let st = st.clone();
        async move { scheduler::sync_relays(&st).await }
    });
    Ok(Json(with_next(&st, s)))
}
async fn schedule_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Operator)?;
    let s: Schedule = st.db.doc_get("schedules", &id)?.ok_or_else(|| ApiError::not_found("schedule not found"))?;
    st.db.doc_delete("schedules", &id)?;
    audit(&st, &a, "schedule_delete", Some(&s.name), "ok", None);
    tokio::spawn({
        let st = st.clone();
        async move { scheduler::sync_relays(&st).await }
    });
    Ok(ok())
}
async fn schedule_run_now(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Operator)?;
    let s: Schedule = st.db.doc_get("schedules", &id)?.ok_or_else(|| ApiError::not_found("schedule not found"))?;
    if (s.action == "shutdown" || s.action == "restart") && a.kind != "session" && a.role < Role::Operator {
        return Err(ApiError::forbidden("not allowed"));
    }
    audit(&st, &a, "schedule_run_now", Some(&s.name), "accepted", None);
    scheduler::fire(st.clone(), s, now());
    Ok(ok())
}

// ---------------------------------------------------------------- discovery
#[derive(Deserialize)]
struct DiscBody {
    location_id: String,
    #[serde(flatten)]
    req: DiscoveryRequest,
}
async fn discovery_start(State(st): State<St>, a: Auth, Json(b): Json<DiscBody>) -> ApiResult<Value> {
    a.need(Role::Operator)?;
    let loc = loc_rec(&st, &b.location_id)?;
    let relay = crate::relay::for_location(&st, &loc)?;
    let rjob = relay.discover_start(b.req.clone()).await?;
    let id = new_id();
    st.remote_disc.lock().unwrap().insert(id.clone(), (loc.id.clone(), rjob));
    audit(&st, &a, "discovery", Some(&loc.name), "started", b.req.subnet.as_deref().or(b.req.interface.as_deref()));
    Ok(Json(json!({"job_id": id})))
}
fn disc_job(st: &St, id: &str) -> R<(LocationRec, String)> {
    let (l, j) = st.remote_disc.lock().unwrap().get(id).cloned().ok_or_else(|| ApiError::not_found("unknown discovery job"))?;
    Ok((loc_rec(st, &l)?, j))
}
async fn discovery_get(State(st): State<St>, _a: Auth, Path(id): Path<String>) -> ApiResult<DiscoveryStatus> {
    let (loc, rjob) = disc_job(&st, &id)?;
    let mut s = crate::relay::for_location(&st, &loc)?.discover_get(&rjob).await?;
    let devs = st.db.devices(Some(&loc.id))?;
    for c in s.results.iter_mut() {
        let hit = devs.iter().find(|d| c.mac.as_deref().map(|m| m == d.mac).unwrap_or(false) || d.ip.as_deref() == Some(c.ip.as_str()));
        c.registered = hit.is_some();
        c.device_id = hit.map(|d| d.id.clone());
    }
    s.job_id = id;
    Ok(Json(s))
}
async fn discovery_cancel(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Operator)?;
    let (loc, rjob) = disc_job(&st, &id)?;
    crate::relay::for_location(&st, &loc)?.discover_cancel(&rjob).await?;
    Ok(ok())
}

// ---------------------------------------------------------------- connectivity / diagnostics
async fn connectivity_get(State(st): State<St>, _a: Auth) -> ApiResult<Value> {
    let c = connectivity::get(true).await;
    Ok(Json(json!({"tailscale": c.tailscale, "addresses": c.addresses, "port": st.https_port, "allow_public": st.allow_public.load(Ordering::Relaxed)})))
}
#[derive(Serialize, Deserialize, Clone)]
struct WgProfile {
    #[serde(default)]
    id: String,
    name: String,
    controller_address: String,
    #[serde(default = "d_wg_port")]
    port: u16,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    allowed_ips: String,
    #[serde(default)]
    notes: String,
}
fn d_wg_port() -> u16 {
    8443
}
fn check_wg(p: &mut WgProfile) -> R<()> {
    p.name = valid_name(&p.name, "profile")?;
    if !is_safe_host(&p.controller_address) {
        return Err(ApiError::bad_request("controller_address must be a VPN IP address or host name"));
    }
    for f in [&p.endpoint, &p.allowed_ips, &p.notes] {
        let l = f.to_lowercase();
        if l.contains("privatekey") || l.contains("private key") || l.contains("presharedkey") || (f.len() == 44 && f.ends_with('=') && !f.contains(' ')) {
            return Err(ApiError::bad_request("do not enter WireGuard keys here: LAN Wake Manager never stores private or pre-shared keys"));
        }
    }
    if p.port == 0 {
        return Err(ApiError::bad_request("invalid port"));
    }
    Ok(())
}
async fn wg_list(State(st): State<St>, _a: Auth) -> ApiResult<Vec<WgProfile>> {
    Ok(Json(st.db.doc_list("wireguard_profiles")?))
}
async fn wg_create(State(st): State<St>, a: Auth, Json(mut p): Json<WgProfile>) -> R<(StatusCode, Json<WgProfile>)> {
    a.need(Role::Admin)?;
    p.id = new_id();
    check_wg(&mut p)?;
    st.db.doc_put("wireguard_profiles", &p.id, &p)?;
    audit(&st, &a, "wireguard_profile_create", Some(&p.name), "ok", None);
    Ok((StatusCode::CREATED, Json(p)))
}
async fn wg_update(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(mut p): Json<WgProfile>) -> ApiResult<WgProfile> {
    a.need(Role::Admin)?;
    st.db.doc_get::<WgProfile>("wireguard_profiles", &id)?.ok_or_else(|| ApiError::not_found("profile not found"))?;
    p.id = id;
    check_wg(&mut p)?;
    st.db.doc_put("wireguard_profiles", &p.id, &p)?;
    audit(&st, &a, "wireguard_profile_update", Some(&p.name), "ok", None);
    Ok(Json(p))
}
async fn wg_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    st.db.doc_delete("wireguard_profiles", &id)?;
    audit(&st, &a, "wireguard_profile_delete", Some(&id), "ok", None);
    Ok(ok())
}
#[derive(Deserialize)]
struct LocQ {
    location_id: Option<String>,
}
async fn diagnostics(State(st): State<St>, _a: Auth, Query(q): Query<LocQ>) -> ApiResult<Value> {
    let mut checks: Vec<Value> = vec![];
    let mut add = |name: &str, status: &str, detail: String, ms: u64| checks.push(json!({"name": name, "status": status, "detail": detail, "duration_ms": ms}));
    add("Controller", "ok", format!("running version {} (up {} min)", lwm_core::VERSION, (now() - st.started) / 60), 0);
    let t = std::time::Instant::now();
    match st.db.kv_get("controller_id") {
        Ok(_) => add("Database", "ok", "readable and writable".into(), t.elapsed().as_millis() as u64),
        Err(e) => add("Database", "fail", e.to_string(), 0),
    }
    let c = connectivity::get(true).await;
    if c.tailscale.running {
        add("Tailscale", "ok", format!("running; {}", c.tailscale.magicdns_name.clone().or(c.tailscale.ip.clone()).unwrap_or_default()), 0);
    } else if c.tailscale.installed {
        add("Tailscale", "warn", c.tailscale.error.clone().unwrap_or_else(|| "installed but not running".into()), 0);
    } else {
        add("Tailscale", "info", "not detected: remote access then needs WireGuard or another VPN to reach this controller".into(), 0);
    }
    let wg: Vec<_> = c.addresses.iter().filter(|a| a.kind == "wireguard").collect();
    add("WireGuard", if wg.is_empty() { "info" } else { "ok" }, if wg.is_empty() { "no WireGuard interface on this computer (fine if the tunnel is terminated elsewhere)".into() } else { format!("interface {} {}", wg[0].interface, wg[0].address) }, 0);
    add("Management API exposure", if st.allow_public.load(Ordering::Relaxed) { "warn" } else { "ok" }, if st.allow_public.load(Ordering::Relaxed) { "public source addresses are allowed: turn this off unless you know why you need it".into() } else { "only private networks, VPN and loopback may connect".into() }, 0);
    let scheds: Vec<Schedule> = st.db.doc_list("schedules")?;
    add("Scheduler", "ok", format!("{} enabled schedule(s)", scheds.iter().filter(|s| s.enabled).count()), 0);
    let devs = st.db.devices(None)?;
    let states = st.db.states()?;
    let enrolled = devs.iter().filter(|d| d.agent_token_hash.is_some()).count();
    let online = devs.iter().filter(|d| d.agent_token_hash.is_some() && states.get(&d.id).map(views::agent_online).unwrap_or(false)).count();
    add("Agents", if enrolled == online { "ok" } else { "warn" }, format!("{online} of {enrolled} enrolled agents online"), 0);
    for l in st.db.doc_list::<LocationRec>("locations")?.into_iter().filter(|l| q.location_id.as_ref().map(|x| x == &l.id).unwrap_or(true)) {
        let t = std::time::Instant::now();
        match crate::relay::for_location(&st, &l) {
            Ok(r) => match r.health().await {
                Ok(()) => add(&format!("Wake Relay: {}", l.name), "ok", if r.is_remote() { "remote relay reachable".into() } else { "built-in relay (this computer's LAN)".into() }, t.elapsed().as_millis() as u64),
                Err(e) => add(&format!("Wake Relay: {}", l.name), "fail", format!("{} Check that the relay service runs and the VPN route exists.", e.message), t.elapsed().as_millis() as u64),
            },
            Err(e) => add(&format!("Wake Relay: {}", l.name), "fail", e.message, 0),
        }
    }
    Ok(Json(json!({"checks": checks, "generated_at": now()})))
}
#[derive(Deserialize)]
struct DiagBody {
    host: String,
    port: Option<u16>,
    location_id: Option<String>,
}
async fn diag_run(st: St, kind: &str, b: DiagBody) -> ApiResult<DiagResult> {
    match &b.location_id {
        Some(l) => {
            let loc = loc_rec(&st, l)?;
            Ok(Json(crate::relay::for_location(&st, &loc)?.diag(kind, &b.host, b.port).await?))
        }
        None => Ok(Json(crate::relay::Relay::Builtin(st).diag(kind, &b.host, b.port).await?)),
    }
}
async fn diag_ping(State(st): State<St>, a: Auth, Json(b): Json<DiagBody>) -> ApiResult<DiagResult> {
    a.need(Role::Operator)?;
    diag_run(st, "ping", b).await
}
async fn diag_dns(State(st): State<St>, a: Auth, Json(b): Json<DiagBody>) -> ApiResult<DiagResult> {
    a.need(Role::Operator)?;
    diag_run(st, "dns", b).await
}
async fn diag_tcp(State(st): State<St>, a: Auth, Json(b): Json<DiagBody>) -> ApiResult<DiagResult> {
    a.need(Role::Operator)?;
    diag_run(st, "tcp", b).await
}

// ---------------------------------------------------------------- mobile access
#[derive(Deserialize)]
struct PairingBody {
    role: String,
    ttl_secs: Option<i64>,
}
async fn pairing_create(State(st): State<St>, a: Auth, Json(b): Json<PairingBody>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    Role::parse(&b.role).ok_or_else(|| ApiError::bad_request("role must be admin, operator or viewer"))?;
    let ttl = b.ttl_secs.unwrap_or(300).clamp(30, 600);
    let token = random_token();
    let exp = now() + ttl;
    st.db.pairing_add(&sha256_hex(token.as_bytes()), &b.role, exp)?;
    let c = connectivity::get(true).await;
    let mut addrs: Vec<String> = vec![];
    if let Some(n) = &c.tailscale.magicdns_name {
        addrs.push(format!("https://{n}:{}", st.https_port));
    }
    for kind in ["tailscale", "wireguard", "lan", "other"] {
        for x in c.addresses.iter().filter(|x| x.kind == kind) {
            let u = format!("https://{}:{}", x.address, st.https_port);
            if !addrs.contains(&u) {
                addrs.push(u);
            }
        }
    }
    let payload = json!({"v": 1, "id": st.controller_id, "name": st.settings().name, "addrs": addrs, "fp": st.fingerprint, "token": token, "exp": exp});
    audit(&st, &a, "pairing_create", None, "ok", Some(&format!("role {}, valid {ttl} s", b.role)));
    Ok(Json(json!({"pairing_token": token, "expires_at": exp, "qr_payload": payload.to_string(), "addresses": addrs})))
}
async fn mobile_list(State(st): State<St>, a: Auth) -> ApiResult<Vec<crate::db::MobileDevice>> {
    a.need(Role::Admin)?;
    Ok(Json(st.db.mobile_list()?))
}
#[derive(Deserialize)]
struct MobilePatch {
    name: Option<String>,
    role: Option<String>,
}
async fn mobile_patch(State(st): State<St>, a: Auth, Path(id): Path<String>, Json(b): Json<MobilePatch>) -> ApiResult<crate::db::MobileDevice> {
    a.need(Role::Admin)?;
    if let Some(r) = &b.role {
        Role::parse(r).ok_or_else(|| ApiError::bad_request("invalid role"))?;
    }
    let name = b.name.as_deref().map(|n| valid_name(n, "device")).transpose()?;
    st.db.mobile_update(&id, name.as_deref(), b.role.as_deref())?;
    audit(&st, &a, "mobile_update", Some(&id), "ok", None);
    st.db.mobile_get(&id)?.map(Json).ok_or_else(|| ApiError::not_found("mobile device not found"))
}
async fn mobile_revoke(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    if !st.db.mobile_revoke(&id)? {
        return Err(ApiError::not_found("mobile device not found"));
    }
    audit(&st, &a, "mobile_revoke", Some(&id), "ok", None);
    Ok(ok())
}

// ---------------------------------------------------------------- audit / notifications / settings
#[derive(Deserialize)]
struct AuditQ {
    limit: Option<i64>,
    before: Option<i64>,
}
async fn audit_list(State(st): State<St>, a: Auth, Query(q): Query<AuditQ>) -> ApiResult<Vec<crate::db::AuditRow>> {
    a.need(Role::Admin)?;
    Ok(Json(st.db.audit_list(q.limit.unwrap_or(100).clamp(1, 1000), q.before)?))
}
#[derive(Deserialize)]
struct NotifQ {
    after: Option<i64>,
    limit: Option<i64>,
}
async fn notifications_list(State(st): State<St>, _a: Auth, Query(q): Query<NotifQ>) -> ApiResult<Vec<crate::db::Notification>> {
    Ok(Json(st.db.notifications(q.after.unwrap_or(0), q.limit.unwrap_or(50).clamp(1, 200))?))
}
async fn settings_get(State(st): State<St>, _a: Auth) -> Json<Settings> {
    Json(st.settings())
}
async fn settings_put(State(st): State<St>, a: Auth, Json(s): Json<Settings>) -> ApiResult<Settings> {
    a.need(Role::Admin)?;
    valid_name(&s.name, "controller")?;
    if !(5..=600).contains(&s.monitor_interval_secs) || !(1..=100).contains(&s.backup.keep) || !(1..=720).contains(&s.backup.interval_hours) || s.notify_schedule_minutes > 1440 {
        return Err(ApiError::bad_request("a setting is out of range"));
    }
    if let Some(tz) = &s.timezone {
        tz.parse::<chrono_tz::Tz>().map_err(|_| ApiError::bad_request("unknown time zone (use an IANA name such as Europe/Berlin)"))?;
    }
    let before = st.allow_public.load(Ordering::Relaxed);
    st.allow_public.store(s.allow_public, Ordering::Relaxed);
    st.db.kv_set("settings", &serde_json::to_string(&s).map_err(|e| ApiError::internal(e.to_string()))?)?;
    *st.settings.write().unwrap() = s.clone();
    audit(&st, &a, "settings_update", None, "ok", if before != s.allow_public { Some(if s.allow_public { "allow_public ENABLED" } else { "allow_public disabled" }) } else { None });
    Ok(Json(s))
}

// ---------------------------------------------------------------- backups / export / import / ssh keys
async fn backups_list(State(st): State<St>, a: Auth) -> ApiResult<Vec<backup::BackupEntry>> {
    a.need(Role::Admin)?;
    Ok(Json(backup::list(&st)))
}
async fn backups_create(State(st): State<St>, a: Auth) -> R<(StatusCode, Json<backup::BackupEntry>)> {
    a.need(Role::Admin)?;
    let b = backup::create(&st, "manual")?;
    audit(&st, &a, "backup_create", Some(&b.name), "ok", None);
    Ok((StatusCode::CREATED, Json(b)))
}
async fn backups_restore(State(st): State<St>, a: Auth, Path(name): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    backup::restore(&st, &name)?;
    audit(&st, &a, "backup_restore", Some(&name), "ok", None);
    Ok(ok())
}
async fn export_get(State(st): State<St>, a: Auth) -> ApiResult<backup::ExportDoc> {
    a.need(Role::Admin)?;
    audit(&st, &a, "export", None, "ok", None);
    Ok(Json(backup::export(&st)?))
}
async fn import_post(State(st): State<St>, a: Auth, Json(doc): Json<backup::ExportDoc>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let r = backup::import(&st, doc)?;
    audit(&st, &a, "import", None, "ok", Some(&r.to_string()));
    Ok(Json(r))
}
async fn ssh_keys_list(State(st): State<St>, a: Auth) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    let rows: Vec<Value> = st.db.with(|c| {
        let mut s = c.prepare("SELECT id,name,created_at FROM ssh_keys ORDER BY created_at")?;
        let r = s.query_map([], |r| Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "created_at": r.get::<_, i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>();
        r
    })?;
    Ok(Json(json!(rows)))
}
#[derive(Deserialize)]
struct KeyBody {
    name: String,
    private_key: String,
}
async fn ssh_keys_create(State(st): State<St>, a: Auth, Json(b): Json<KeyBody>) -> R<(StatusCode, Json<Value>)> {
    a.need(Role::Admin)?;
    let name = valid_name(&b.name, "key")?;
    let k = b.private_key.trim().replace("\r\n", "\n");
    if !(k.starts_with("-----BEGIN") && k.contains("PRIVATE KEY-----")) || k.len() > 16_384 {
        return Err(ApiError::bad_request("expected an unencrypted OpenSSH/PEM private key"));
    }
    let id = new_id();
    let dir = st.dir.join("ssh-keys");
    lwm_core::paths::ensure_private_dir(&dir).map_err(|e| ApiError::internal(e.to_string()))?;
    lwm_core::paths::write_secret(&dir.join(&id), format!("{k}\n").as_bytes()).map_err(|e| ApiError::internal(e.to_string()))?;
    st.db.with(|c| c.execute("INSERT INTO ssh_keys(id,name,created_at) VALUES(?1,?2,?3)", rusqlite::params![id, name, now()]).map(|_| ()))?;
    audit(&st, &a, "ssh_key_create", Some(&name), "ok", None);
    Ok((StatusCode::CREATED, Json(json!({"id": id, "name": name}))))
}
async fn ssh_keys_delete(State(st): State<St>, a: Auth, Path(id): Path<String>) -> ApiResult<Value> {
    a.need(Role::Admin)?;
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request("invalid id"));
    }
    let _ = std::fs::remove_file(st.dir.join("ssh-keys").join(&id));
    st.db.with(|c| c.execute("DELETE FROM ssh_keys WHERE id=?1", [&id]).map(|_| ()))?;
    audit(&st, &a, "ssh_key_delete", Some(&id), "ok", None);
    Ok(ok())
}

