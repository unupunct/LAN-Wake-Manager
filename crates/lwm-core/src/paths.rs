use std::path::{Path, PathBuf};

/// Default data directory for a role ("controller" | "relay" | "agent").
pub fn data_dir(role: &str, override_dir: Option<&str>) -> PathBuf {
    if let Some(d) = override_dir.map(|s| s.to_string()).or_else(|| std::env::var("LWM_DATA_DIR").ok()) {
        return PathBuf::from(d);
    }
    #[cfg(windows)]
    {
        let base = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
        PathBuf::from(base).join("LANWakeManager").join(role)
    }
    #[cfg(not(windows))]
    {
        if is_root() {
            PathBuf::from("/var/lib/lan-wake-manager").join(role)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".local/share/lan-wake-manager").join(role)
        }
    }
}
#[cfg(not(windows))]
fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .map(|s| s.lines().any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0")))
        .unwrap_or(false)
}

/// Create the directory and restrict it to the service account / administrators.
pub fn ensure_private_dir(p: &Path) -> std::io::Result<()> {
    let existed = p.exists();
    std::fs::create_dir_all(p)?;
    if !existed {
        restrict(p);
    }
    Ok(())
}
#[cfg(windows)]
pub fn restrict(p: &Path) {
    // Drop inherited ACEs and grant only SYSTEM and Administrators plus the current user. Best effort.
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) { (Ok(d), Ok(u)) => format!("{d}\\{u}"), (_, Ok(u)) => u, _ => String::new() };
    let mut args: Vec<String> = vec![p.to_string_lossy().to_string(), "/inheritance:r".into(), "/grant:r".into(), "*S-1-5-18:(OI)(CI)F".into(), "/grant:r".into(), "*S-1-5-32-544:(OI)(CI)F".into()];
    if !user.is_empty() && !user.ends_with('$') && !user.to_uppercase().starts_with("NT AUTHORITY") {
        args.push("/grant:r".into());
        args.push(format!("{user}:(OI)(CI)F"));
    }
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let _ = crate::cmd::run_quiet("icacls", &a);
}
#[cfg(not(windows))]
pub fn restrict(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
}
/// Write a secret file with owner-only permissions on unix (Windows relies on the directory ACL).
pub fn write_secret(p: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::write(p, data)?;
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
