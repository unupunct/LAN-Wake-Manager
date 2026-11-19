//! HTTP server plumbing shared by controller and relay: TLS listener, error type, source-address guard.
use crate::tls::CertMaterial;
use anyhow::Result;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Router;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retry_after: Option<u64>,
}
impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into(), retry_after: None }
    }
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", m)
    }
    pub fn unauthorized(m: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", m)
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", m)
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", m)
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", m)
    }
    pub fn rate_limited(retry: u64) -> Self {
        let mut e = Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "too many requests, slow down");
        e.retry_after = Some(retry);
        e
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("internal error: {e:#}");
        ApiError::internal(format!("{e}"))
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut r = (self.status, axum::Json(json!({"error": {"code": self.code, "message": self.message}}))).into_response();
        if let Some(s) = self.retry_after {
            if let Ok(v) = HeaderValue::from_str(&s.to_string()) {
                r.headers_mut().insert(header::RETRY_AFTER, v);
            }
        }
        r
    }
}
pub type ApiResult<T> = Result<axum::Json<T>, ApiError>;

/// Rejects requests whose source address is not private/loopback/VPN unless `allow_public` is set.
pub async fn private_only(State(allow_public): State<Arc<AtomicBool>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, req: Request, next: Next) -> Response {
    if !allow_public.load(Ordering::Relaxed) && !crate::net::is_private_addr(addr.ip()) {
        return ApiError::forbidden("management API is only reachable from private networks and VPNs").into_response();
    }
    next.run(req).await
}

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut r = next.run(req).await;
    let h = r.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert(header::CACHE_CONTROL, h.get(header::CACHE_CONTROL).cloned().unwrap_or(HeaderValue::from_static("no-store")));
    r
}

/// Serve `app` over TLS with the given certificate until `shutdown` turns true.
pub async fn serve_tls(addr: SocketAddr, app: Router, cert: &CertMaterial, mut shutdown: tokio::sync::watch::Receiver<bool>) -> Result<()> {
    crate::tls::install_provider();
    let cfg = axum_server::tls_rustls::RustlsConfig::from_pem(cert.cert_pem.clone().into_bytes(), cert.key_pem.clone().into_bytes()).await?;
    let handle = axum_server::Handle::new();
    let h2 = handle.clone();
    tokio::spawn(async move {
        while !*shutdown.borrow() {
            if shutdown.changed().await.is_err() {
                break;
            }
        }
        h2.graceful_shutdown(Some(Duration::from_secs(5)));
    });
    axum_server::bind_rustls(addr, cfg).handle(handle).serve(app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

pub async fn serve_plain(addr: SocketAddr, app: Router, mut shutdown: tokio::sync::watch::Receiver<bool>) -> Result<()> {
    let l = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(l, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            while !*shutdown.borrow() {
                if shutdown.changed().await.is_err() {
                    break;
                }
            }
        })
        .await?;
    Ok(())
}

pub fn bearer(req_headers: &axum::http::HeaderMap) -> Option<String> {
    let v = req_headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(|s| s.trim().to_string())
}
