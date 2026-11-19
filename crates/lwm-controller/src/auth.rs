//! Authentication (admin sessions, paired mobile devices) and role checks.
use crate::state::St;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use lwm_core::server::ApiError;
use lwm_core::util::sha256_hex;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Role {
    Viewer = 1,
    Operator = 2,
    Admin = 3,
}
impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "viewer" => Some(Role::Viewer),
            "operator" => Some(Role::Operator),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Operator => "operator",
            Role::Admin => "admin",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Auth {
    pub kind: &'static str, // session | mobile
    pub role: Role,
    pub device_id: Option<String>,
    pub name: String,
    pub token_hash: String,
    pub ip: String,
}
impl Auth {
    pub fn need(&self, r: Role) -> Result<(), ApiError> {
        if self.role >= r {
            Ok(())
        } else {
            Err(ApiError::forbidden(format!("requires the {} role", r.as_str())))
        }
    }
    /// Actor string for audit log / operations.
    pub fn actor(&self) -> String {
        if self.kind == "session" {
            "admin".to_string()
        } else {
            self.name.clone()
        }
    }
}

pub async fn middleware(State(st): State<St>, ConnectInfo(addr): ConnectInfo<SocketAddr>, mut req: Request, next: Next) -> Result<Response, ApiError> {
    let ip = addr.ip().to_string();
    // generic request ceiling per client
    st.limiter.check(&format!("req:{ip}"), 600, Duration::from_secs(60)).map_err(ApiError::rate_limited)?;
    let token = lwm_core::server::bearer(req.headers());
    let auth = match token {
        Some(t) => {
            let h = sha256_hex(t.as_bytes());
            if st.db.session_valid(&h) {
                Some(Auth { kind: "session", role: Role::Admin, device_id: None, name: "admin".into(), token_hash: h, ip: ip.clone() })
            } else if let Ok(Some(m)) = st.db.mobile_by_token(&h) {
                if m.revoked {
                    None
                } else {
                    st.db.mobile_touch(&m.id, &ip);
                    Some(Auth { kind: "mobile", role: Role::parse(&m.role).unwrap_or(Role::Viewer), device_id: Some(m.id), name: m.name, token_hash: h, ip: ip.clone() })
                }
            } else {
                None
            }
        }
        None => None,
    };
    match auth {
        Some(a) => {
            req.extensions_mut().insert(a);
            Ok(next.run(req).await)
        }
        None => {
            // only failed attempts count against the stricter limiter
            st.limiter.check(&format!("authfail:{ip}"), 20, Duration::from_secs(60)).map_err(ApiError::rate_limited)?;
            st.db.audit("anonymous", "auth", Some(req.uri().path()), "denied", Some(&ip), Some("missing, invalid or revoked token"));
            Err(ApiError::unauthorized("missing, invalid or revoked token"))
        }
    }
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Auth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut axum::http::request::Parts, _s: &S) -> Result<Self, ApiError> {
        parts.extensions.get::<Auth>().cloned().ok_or_else(|| ApiError::unauthorized("not authenticated"))
    }
}

/// Resolve a bearer token to an Auth (used by the websocket handshake).
pub fn resolve(st: &St, token: &str, ip: &str) -> Option<Auth> {
    let h = sha256_hex(token.as_bytes());
    if st.db.session_valid(&h) {
        return Some(Auth { kind: "session", role: Role::Admin, device_id: None, name: "admin".into(), token_hash: h, ip: ip.into() });
    }
    match st.db.mobile_by_token(&h) {
        Ok(Some(m)) if !m.revoked => Some(Auth { kind: "mobile", role: Role::parse(&m.role).unwrap_or(Role::Viewer), device_id: Some(m.id), name: m.name, token_hash: h, ip: ip.into() }),
        _ => None,
    }
}
