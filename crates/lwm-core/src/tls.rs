//! Self-signed certificates and fingerprint pinning (no CA, no hostname checks).
use anyhow::{anyhow, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}
/// Must be called once at start-up (axum-server uses the process default provider).
pub fn install_provider() {
    let _ = ring::default_provider().install_default();
}

pub fn fingerprint_der(der: &[u8]) -> String {
    hex::encode(Sha256::digest(der))
}
pub fn normalize_fp(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_lowercase()
}

pub struct CertMaterial {
    pub cert_pem: String,
    pub key_pem: String,
    pub fingerprint: String,
}

/// Load `cert.pem`/`key.pem` from `dir` or generate a fresh self-signed pair (10 years).
pub fn load_or_create(dir: &Path, names: Vec<String>) -> Result<CertMaterial> {
    let cp = dir.join("cert.pem");
    let kp = dir.join("key.pem");
    if cp.exists() && kp.exists() {
        let cert_pem = std::fs::read_to_string(&cp)?;
        let key_pem = std::fs::read_to_string(&kp)?;
        let der = pem_to_der(&cert_pem)?;
        return Ok(CertMaterial { fingerprint: fingerprint_der(&der), cert_pem, key_pem });
    }
    let mut params = rcgen::CertificateParams::new(names)?;
    params.distinguished_name.push(rcgen::DnType::CommonName, "LAN Wake Manager");
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2124, 1, 1);
    let key = rcgen::KeyPair::generate()?;
    let cert = params.self_signed(&key)?;
    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();
    crate::paths::write_secret(&kp, key_pem.as_bytes())?;
    std::fs::write(&cp, cert_pem.as_bytes())?;
    Ok(CertMaterial { fingerprint: fingerprint_der(cert.der()), cert_pem, key_pem })
}

pub fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let body: String = pem
        .lines()
        .skip_while(|l| !l.starts_with("-----BEGIN CERTIFICATE"))
        .skip(1)
        .take_while(|l| !l.starts_with("-----END"))
        .collect();
    base64::engine::general_purpose::STANDARD.decode(body.trim()).map_err(|e| anyhow!("bad certificate pem: {e}"))
}

#[derive(Debug)]
struct PinVerifier {
    fp: String,
    provider: Arc<CryptoProvider>,
    seen: Option<Arc<Mutex<Option<String>>>>,
}
impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint_der(end_entity.as_ref());
        if let Some(s) = &self.seen {
            *s.lock().unwrap() = Some(fp.clone());
            return Ok(ServerCertVerified::assertion());
        }
        if fp == self.fp {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate fingerprint does not match the pinned value".into()))
        }
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

fn client_config(v: PinVerifier) -> Result<rustls::ClientConfig> {
    let p = v.provider.clone();
    Ok(rustls::ClientConfig::builder_with_provider(p)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(v))
        .with_no_client_auth())
}

/// reqwest client that only talks to a server presenting the pinned leaf certificate.
pub fn pinned_client(fingerprint: &str, timeout: std::time::Duration) -> Result<reqwest::Client> {
    let cfg = client_config(PinVerifier { fp: normalize_fp(fingerprint), provider: provider(), seen: None })?;
    Ok(reqwest::Client::builder()
        .use_preconfigured_tls(cfg)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()?)
}

/// Connect to host:port, accept any certificate and return its fingerprint (trust-on-first-use;
/// the user must compare it with the value shown on the other device).
pub async fn fetch_fingerprint(host: &str, port: u16) -> Result<String> {
    let seen = Arc::new(Mutex::new(None));
    let cfg = client_config(PinVerifier { fp: String::new(), provider: provider(), seen: Some(seen.clone()) })?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(cfg));
    let tcp = tokio::time::timeout(std::time::Duration::from_secs(5), tokio::net::TcpStream::connect((host, port)))
        .await
        .map_err(|_| anyhow!("connection to {host}:{port} timed out"))??;
    let name = ServerName::try_from("lan-wake-manager.invalid".to_string()).map_err(|e| anyhow!("{e}"))?;
    let _tls = tokio::time::timeout(std::time::Duration::from_secs(5), connector.connect(name, tcp))
        .await
        .map_err(|_| anyhow!("TLS handshake timed out"))??;
    let fp = seen.lock().unwrap().clone();
    fp.ok_or_else(|| anyhow!("no certificate presented"))
}

/// Split "https://host:port" into (host, port).
pub fn split_https_url(url: &str) -> Result<(String, u16)> {
    let u = url.trim().trim_end_matches('/');
    let rest = u.strip_prefix("https://").ok_or_else(|| anyhow!("URL must start with https://"))?;
    let hostport = rest.split('/').next().unwrap_or(rest);
    if let Some(stripped) = hostport.strip_prefix('[') {
        let (h, p) = stripped.split_once("]:").ok_or_else(|| anyhow!("bad IPv6 URL"))?;
        return Ok((h.to_string(), p.parse()?));
    }
    match hostport.rsplit_once(':') {
        Some((h, p)) => Ok((h.to_string(), p.parse()?)),
        None => Ok((hostport.to_string(), 443)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        assert_eq!(split_https_url("https://100.64.0.1:8444/").unwrap(), ("100.64.0.1".into(), 8444));
        assert!(split_https_url("http://x:1").is_err());
    }
    #[test]
    fn cert_gen_and_fp() {
        let dir = std::env::temp_dir().join(format!("lwm-cert-{}", crate::util::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = load_or_create(&dir, vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
        let b = load_or_create(&dir, vec![]).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.fingerprint.len(), 64);
        std::fs::remove_dir_all(&dir).ok();
    }
}
