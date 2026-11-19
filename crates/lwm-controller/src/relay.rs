//! Access to the Wake Relay of a location: the built-in one (this machine's LAN) or a paired remote relay.
use crate::state::St;
use lwm_core::client::ApiClient;
use lwm_core::models::*;
use lwm_core::server::ApiError;
use std::time::Duration;

pub enum Relay {
    Builtin(St),
    Remote(ApiClient),
}

fn unreachable(e: anyhow::Error) -> ApiError {
    ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "relay_unreachable", format!("Wake Relay unreachable: {e}"))
}

pub fn for_location(st: &St, loc: &LocationRec) -> Result<Relay, ApiError> {
    match (&loc.relay_url, &loc.relay_token, &loc.relay_fingerprint) {
        (Some(url), Some(tok), Some(fp)) => ApiClient::new(url, fp, Some(tok.clone()), Duration::from_secs(12)).map(Relay::Remote).map_err(|e| ApiError::internal(e.to_string())),
        (Some(_), _, _) => Err(ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "relay_unreachable", "the relay of this location is not paired yet")),
        _ => Ok(Relay::Builtin(st.clone())),
    }
}

impl Relay {
    pub fn is_remote(&self) -> bool {
        matches!(self, Relay::Remote(_))
    }
    pub async fn wake_start(&self, req: WakeRequest) -> Result<String, ApiError> {
        match self {
            Relay::Builtin(st) => Ok(st.wakes.start(req)),
            Relay::Remote(c) => {
                let v: serde_json::Value = c.post("/v1/wake", &req).await.map_err(unreachable)?;
                v["job_id"].as_str().map(|s| s.to_string()).ok_or_else(|| ApiError::internal("relay returned no job id"))
            }
        }
    }
    pub async fn wake_get(&self, job: &str) -> Result<WakeStatus, ApiError> {
        match self {
            Relay::Builtin(st) => st.wakes.get(job).ok_or_else(|| ApiError::not_found("unknown wake job")),
            Relay::Remote(c) => c.get(&format!("/v1/wake/{job}")).await.map_err(unreachable),
        }
    }
    pub async fn probe(&self, targets: Vec<ProbeTarget>) -> Result<Vec<ProbeResult>, ApiError> {
        match self {
            Relay::Builtin(_) => Ok(lwm_core::probe::probe_many(targets, 1000, 32).await),
            Relay::Remote(c) => c.post("/v1/probe", &serde_json::json!({"targets": targets, "timeout_ms": 1000})).await.map_err(unreachable),
        }
    }
    pub async fn health(&self) -> Result<(), ApiError> {
        match self {
            Relay::Builtin(_) => Ok(()),
            Relay::Remote(c) => c.get::<serde_json::Value>("/v1/health").await.map(|_| ()).map_err(unreachable),
        }
    }
    pub async fn interfaces(&self) -> Result<Vec<IfaceInfo>, ApiError> {
        match self {
            Relay::Builtin(_) => Ok(tokio::task::spawn_blocking(lwm_core::net::list_interfaces).await.unwrap_or_default()),
            Relay::Remote(c) => c.get("/v1/interfaces").await.map_err(unreachable),
        }
    }
    pub async fn discover_start(&self, req: DiscoveryRequest) -> Result<String, ApiError> {
        match self {
            Relay::Builtin(st) => st.disc.start(req).map_err(ApiError::bad_request),
            Relay::Remote(c) => {
                let v: serde_json::Value = c.post("/v1/discover", &req).await.map_err(|e| {
                    let m = e.to_string();
                    match m.strip_prefix("HTTP 400: ") {
                        Some(msg) => ApiError::bad_request(msg.to_string()),
                        None => unreachable(e),
                    }
                })?;
                v["job_id"].as_str().map(|s| s.to_string()).ok_or_else(|| ApiError::internal("relay returned no job id"))
            }
        }
    }
    pub async fn discover_get(&self, job: &str) -> Result<DiscoveryStatus, ApiError> {
        match self {
            Relay::Builtin(st) => st.disc.get(job).ok_or_else(|| ApiError::not_found("unknown discovery job")),
            Relay::Remote(c) => c.get(&format!("/v1/discover/{job}")).await.map_err(unreachable),
        }
    }
    pub async fn discover_cancel(&self, job: &str) -> Result<(), ApiError> {
        match self {
            Relay::Builtin(st) => {
                st.disc.cancel(job);
                Ok(())
            }
            Relay::Remote(c) => c.delete::<serde_json::Value>(&format!("/v1/discover/{job}")).await.map(|_| ()).map_err(unreachable),
        }
    }
    pub async fn diag(&self, kind: &str, host: &str, port: Option<u16>) -> Result<DiagResult, ApiError> {
        match self {
            Relay::Builtin(_) => Ok(match kind {
                "ping" => lwm_core::diag::ping(host).await,
                "dns" => lwm_core::diag::dns(host).await,
                _ => lwm_core::diag::tcp(host, port.unwrap_or(0)).await,
            }),
            Relay::Remote(c) => c.post(&format!("/v1/diag/{kind}"), &serde_json::json!({"host": host, "port": port})).await.map_err(unreachable),
        }
    }
}
