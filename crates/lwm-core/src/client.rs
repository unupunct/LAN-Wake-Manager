//! Small JSON-over-HTTPS client with certificate pinning, used by the controller (to relays) and agents.
use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct ApiClient {
    pub base: String,
    pub token: Option<String>,
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(base: &str, fingerprint: &str, token: Option<String>, timeout: Duration) -> Result<Self> {
        Ok(Self { base: base.trim_end_matches('/').to_string(), token, http: crate::tls::pinned_client(fingerprint, timeout)? })
    }
    fn req(&self, m: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut r = self.http.request(m, format!("{}{}", self.base, path));
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        r
    }
    async fn finish<T: DeserializeOwned>(r: reqwest::RequestBuilder) -> Result<T> {
        let resp = r.send().await.map_err(|e| anyhow!("{}", describe(&e)))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
                .unwrap_or(body);
            return Err(anyhow!("HTTP {}: {}", status.as_u16(), msg));
        }
        Ok(resp.json::<T>().await?)
    }
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Self::finish(self.req(reqwest::Method::GET, path)).await
    }
    pub async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        Self::finish(self.req(reqwest::Method::POST, path).json(body)).await
    }
    pub async fn put<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        Self::finish(self.req(reqwest::Method::PUT, path).json(body)).await
    }
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Self::finish(self.req(reqwest::Method::DELETE, path)).await
    }
    /// GET with a custom per-request timeout (long polling).
    pub async fn get_timeout<T: DeserializeOwned>(&self, path: &str, timeout: Duration) -> Result<T> {
        Self::finish(self.req(reqwest::Method::GET, path).timeout(timeout)).await
    }
}

fn describe(e: &reqwest::Error) -> String {
    let mut s = e.to_string();
    let mut src: Option<&dyn std::error::Error> = std::error::Error::source(e);
    while let Some(x) = src {
        s.push_str(": ");
        s.push_str(&x.to_string());
        src = x.source();
    }
    s
}
