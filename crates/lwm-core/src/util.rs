use rand::{Rng, RngCore};
use sha2::{Digest, Sha256};

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Random token (256 bit, hex).
pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}
/// Human-friendly one-time code, e.g. "K7QX-4MPD" (no ambiguous characters).
pub fn pairing_code() -> String {
    const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut r = rand::thread_rng();
    let mut s = String::new();
    for i in 0..8 {
        if i == 4 {
            s.push('-');
        }
        s.push(A[r.gen_range(0..A.len())] as char);
    }
    s
}
pub fn normalize_code(c: &str) -> String {
    c.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Constant-time comparison of two strings.
pub fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut OsRng);
    Ok(argon2::Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}
pub fn verify_password(pw: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash)
        .ok()
        .map(|h| argon2::Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// Parse "AA:BB:CC:DD:EE:FF", "aa-bb-..." or "aabbccddeeff".
pub fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let hexs: String = s.chars().filter(|c| !matches!(c, ':' | '-' | '.' | ' ')).collect();
    if hexs.len() != 12 {
        return None;
    }
    let v = hex::decode(&hexs).ok()?;
    let mut m = [0u8; 6];
    m.copy_from_slice(&v);
    Some(m)
}
pub fn format_mac(m: &[u8; 6]) -> String {
    m.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}
pub fn normalize_mac(s: &str) -> Option<String> {
    parse_mac(s).map(|m| format_mac(&m))
}

/// Hostnames/IPs passed to external commands (ssh, ping) must be conservative.
pub fn is_safe_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}
pub fn is_safe_user(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mac_roundtrip() {
        assert_eq!(normalize_mac("aa-bb-cc-dd-ee-ff").unwrap(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(normalize_mac("aabbccddeeff").unwrap(), "AA:BB:CC:DD:EE:FF");
        assert!(normalize_mac("aa:bb:cc").is_none());
        assert!(normalize_mac("zz:bb:cc:dd:ee:ff").is_none());
    }
    #[test]
    fn password_hash() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
    }
    #[test]
    fn safe_host() {
        assert!(is_safe_host("192.168.1.2"));
        assert!(is_safe_host("my-pc.local"));
        assert!(!is_safe_host("-oProxyCommand=x"));
        assert!(!is_safe_host("a b"));
        assert!(!is_safe_host("a;b"));
    }
    #[test]
    fn code_format() {
        let c = pairing_code();
        assert_eq!(c.len(), 9);
        assert_eq!(normalize_code(&c).len(), 8);
    }
}
