//! Predefined power commands. There is no arbitrary command execution anywhere: only these fixed templates.
use crate::models::SshConfig;
use crate::util::{is_safe_host, is_safe_user};

pub fn dry_run() -> bool {
    std::env::var("LWM_DRY_RUN").map(|v| v == "1").unwrap_or(false)
}

/// (program, args) for the local machine (used by the agent). `kind` = shutdown | restart | cancel_shutdown.
pub fn local_command(os: &str, kind: &str, delay_secs: u32) -> Result<(String, Vec<String>), String> {
    let delay = delay_secs.min(86_400 * 7);
    match (os, kind) {
        ("windows", "shutdown") => Ok(("shutdown".into(), vec!["/s".into(), "/t".into(), delay.to_string(), "/c".into(), "LAN Wake Manager: shutdown requested".into()])),
        ("windows", "restart") => Ok(("shutdown".into(), vec!["/r".into(), "/t".into(), delay.to_string(), "/c".into(), "LAN Wake Manager: restart requested".into()])),
        ("windows", "cancel_shutdown") => Ok(("shutdown".into(), vec!["/a".into()])),
        (_, "shutdown") | (_, "restart") => {
            let flag = if kind == "shutdown" { "-h" } else { "-r" };
            let when = if delay == 0 { "now".to_string() } else { format!("+{}", delay.div_ceil(60)) };
            Ok(("shutdown".into(), vec![flag.into(), when, "LAN Wake Manager requested".into()]))
        }
        (_, "cancel_shutdown") => Ok(("shutdown".into(), vec!["-c".into()])),
        _ => Err(format!("unsupported operation '{kind}'")),
    }
}

/// Remote command line for the ssh method. The remote command is a fixed template chosen by `remote_os`.
pub fn ssh_command(ip: &str, ssh: &SshConfig, key_path: Option<&str>, remote_os: &str, kind: &str, delay_secs: u32) -> Result<(String, Vec<String>), String> {
    if !is_safe_host(ip) {
        return Err("unsafe host name".into());
    }
    if !is_safe_user(&ssh.user) {
        return Err("unsafe ssh user name".into());
    }
    let (_, remote) = local_command(remote_os, kind, delay_secs)?;
    let remote_line = if remote_os == "windows" {
        format!("shutdown {}", remote.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" "))
    } else {
        format!("sudo -n shutdown {}", remote.iter().map(|a| if a.contains(' ') { format!("'{a}'") } else { a.clone() }).collect::<Vec<_>>().join(" "))
    };
    let mut args: Vec<String> = vec![
        "-p".into(),
        ssh.port.to_string(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=8".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
    ];
    if let Some(k) = key_path {
        args.push("-i".into());
        args.push(k.to_string());
        args.push("-o".into());
        args.push("IdentitiesOnly=yes".into());
    }
    args.push(format!("{}@{}", ssh.user, ip));
    args.push("--".into());
    args.push(remote_line);
    Ok(("ssh".into(), args))
}

/// PowerShell (Windows controller only) using the service account / current user for WinRM authentication.
pub fn winrm_command(ip: &str, kind: &str, delay_secs: u32) -> Result<(String, Vec<String>), String> {
    if !is_safe_host(ip) {
        return Err("unsafe host name".into());
    }
    let (_, remote) = local_command("windows", kind, delay_secs)?;
    let inner = remote.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" ");
    let script = format!("Invoke-Command -ComputerName '{ip}' -ScriptBlock {{ shutdown {inner} }} -ErrorAction Stop");
    Ok(("powershell".into(), vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), script]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_commands() {
        let (p, a) = local_command("windows", "shutdown", 30).unwrap();
        assert_eq!(p, "shutdown");
        assert_eq!(&a[..3], &["/s", "/t", "30"]);
        assert_eq!(local_command("windows", "cancel_shutdown", 0).unwrap().1, vec!["/a"]);
        assert_eq!(&local_command("windows", "restart", 0).unwrap().1[..1], &["/r"]);
    }
    #[test]
    fn linux_commands() {
        assert_eq!(&local_command("linux", "shutdown", 0).unwrap().1[..2], &["-h", "now"]);
        assert_eq!(&local_command("linux", "restart", 90).unwrap().1[..2], &["-r", "+2"]);
        assert_eq!(local_command("linux", "cancel_shutdown", 0).unwrap().1, vec!["-c"]);
        assert!(local_command("linux", "format_disk", 0).is_err());
    }
    #[test]
    fn ssh_is_validated() {
        let cfg = SshConfig { user: "admin".into(), port: 2222, key_id: None };
        let (p, a) = ssh_command("192.168.1.9", &cfg, Some("/k/id"), "linux", "shutdown", 0).unwrap();
        assert_eq!(p, "ssh");
        assert!(a.iter().any(|x| x == "admin@192.168.1.9"));
        assert!(a.last().unwrap().starts_with("sudo -n shutdown -h now"));
        assert!(ssh_command("-oProxyCommand=evil", &cfg, None, "linux", "shutdown", 0).is_err());
        let bad = SshConfig { user: "a;rm -rf".into(), port: 22, key_id: None };
        assert!(ssh_command("1.2.3.4", &bad, None, "linux", "shutdown", 0).is_err());
    }
}
