// Thin desktop window around the LAN Wake Manager controller dashboard (http://127.0.0.1:8480).
// If the background service is not running, a private controller is started for the lifetime of the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

fn controller_up() -> bool {
    let addr: SocketAddr = "127.0.0.1:8480".parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn start_private_controller() -> Option<Child> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) { "lwm-controller.exe" } else { "lwm-controller" };
    let mut candidates: Vec<PathBuf> = vec![dir.join(name), dir.join("Controller").join(name)];
    candidates.push(PathBuf::from(name));
    let data = if cfg!(windows) {
        PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())).join("LANWakeManager").join("controller")
    } else {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".local/share/lan-wake-manager/controller")
    };
    for c in candidates {
        let mut cmd = Command::new(&c);
        cmd.arg("--data-dir").arg(&data).arg("run");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if let Ok(child) = cmd.spawn() {
            return Some(child);
        }
    }
    None
}

fn main() {
    let mut child = if controller_up() { None } else { start_private_controller() };
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running LAN Wake Manager");
    if let Some(c) = child.as_mut() {
        let _ = c.kill();
    }
}
