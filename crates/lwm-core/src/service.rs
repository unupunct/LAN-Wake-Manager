//! Background-service integration: Windows Service Control Manager and Linux systemd.
use anyhow::Result;

pub struct ServiceSpec<'a> {
    pub name: &'a str,
    pub display: &'a str,
    pub description: &'a str,
    /// absolute path of the executable
    pub exe: std::path::PathBuf,
    /// arguments after the executable (the "service" sub-command etc.)
    pub args: Vec<String>,
}

#[cfg(windows)]
pub mod win {
    use super::*;
    use std::ffi::OsString;
    use std::sync::OnceLock;
    use std::time::Duration;
    use windows_service::service::*;
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_service::{define_windows_service, service_dispatcher};

    static ENTRY: OnceLock<(String, fn(tokio::sync::watch::Receiver<bool>))> = OnceLock::new();
    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        let (name, entry) = ENTRY.get().expect("service entry").clone();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let handler = move |ev| match ev {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = tx.send(true);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let Ok(h) = service_control_handler::register(&name, handler) else { return };
        let status = |state, accepted, wait| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: wait,
            process_id: None,
        };
        let _ = h.set_service_status(status(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN, Duration::default()));
        entry(rx);
        let _ = h.set_service_status(status(ServiceState::Stopped, ServiceControlAccept::empty(), Duration::default()));
    }

    /// Called from `main` when started by the SCM. `entry` blocks until the receiver turns true.
    pub fn run(name: &str, entry: fn(tokio::sync::watch::Receiver<bool>)) -> Result<()> {
        let _ = ENTRY.set((name.to_string(), entry));
        service_dispatcher::start(name, ffi_service_main)?;
        Ok(())
    }

    pub fn install(spec: &ServiceSpec) -> Result<()> {
        let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
        let info = ServiceInfo {
            name: OsString::from(spec.name),
            display_name: OsString::from(spec.display),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: spec.exe.clone(),
            launch_arguments: spec.args.iter().map(OsString::from).collect(),
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let svc = match mgr.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS) {
            Ok(s) => s,
            Err(_) => mgr.open_service(spec.name, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS)?,
        };
        svc.set_description(spec.description)?;
        // automatic recovery after unexpected termination: restart after 5 s, 10 s, 30 s
        let actions = vec![
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(5) },
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(10) },
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(30) },
        ];
        svc.update_failure_actions(ServiceFailureActions { reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86_400)), reboot_msg: None, command: None, actions: Some(actions) })?;
        svc.set_failure_actions_on_non_crash_failures(true)?;
        let _ = svc.start::<&str>(&[]);
        Ok(())
    }

    pub fn uninstall(name: &str) -> Result<()> {
        let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let svc = mgr.open_service(name, ServiceAccess::DELETE | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS)?;
        if svc.query_status()?.current_state != ServiceState::Stopped {
            let _ = svc.stop();
            std::thread::sleep(Duration::from_secs(2));
        }
        svc.delete()?;
        Ok(())
    }
}

#[cfg(not(windows))]
pub mod linux {
    use super::*;

    pub fn unit_text(spec: &ServiceSpec, data_dir_env: Option<&str>) -> String {
        let args = spec.args.join(" ");
        let env = data_dir_env.map(|d| format!("Environment=LWM_DATA_DIR={d}\n")).unwrap_or_default();
        format!(
            "[Unit]\nDescription={}\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart={} {}\n{}Restart=always\nRestartSec=5\nUser=root\nNoNewPrivileges=false\n\n[Install]\nWantedBy=multi-user.target\n",
            spec.description,
            spec.exe.display(),
            args,
            env
        )
    }
    pub fn install(spec: &ServiceSpec) -> Result<()> {
        let unit = format!("/etc/systemd/system/{}.service", spec.name);
        std::fs::write(&unit, unit_text(spec, None))?;
        crate::cmd::run_quiet("systemctl", &["daemon-reload"])?;
        let out = crate::cmd::run_quiet("systemctl", &["enable", "--now", spec.name])?;
        if !out.status.success() {
            anyhow::bail!("systemctl enable failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(())
    }
    pub fn uninstall(name: &str) -> Result<()> {
        let _ = crate::cmd::run_quiet("systemctl", &["disable", "--now", name]);
        let _ = std::fs::remove_file(format!("/etc/systemd/system/{name}.service"));
        let _ = crate::cmd::run_quiet("systemctl", &["daemon-reload"]);
        Ok(())
    }
}

/// Cross-platform helpers used by the three binaries.
pub fn install(spec: &ServiceSpec) -> Result<()> {
    #[cfg(windows)]
    return win::install(spec);
    #[cfg(not(windows))]
    return linux::install(spec);
}
pub fn uninstall(name: &str) -> Result<()> {
    #[cfg(windows)]
    return win::uninstall(name);
    #[cfg(not(windows))]
    return linux::uninstall(name);
}
