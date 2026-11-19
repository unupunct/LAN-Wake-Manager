//! Running external programs with fixed argument lists (never through a shell).
use std::process::Output;
use std::time::Duration;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn run_quiet(prog: &str, args: &[&str]) -> std::io::Result<Output> {
    let mut c = std::process::Command::new(prog);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.output()
}

/// Async run with timeout; returns (exit_ok, stdout+stderr).
pub async fn run(prog: &str, args: &[String], timeout: Duration) -> Result<(bool, String), String> {
    let mut c = tokio::process::Command::new(prog);
    c.args(args).kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c.stdin(std::process::Stdio::null());
    let fut = c.output();
    match tokio::time::timeout(timeout, fut).await {
        Err(_) => Err(format!("{prog} timed out after {}s", timeout.as_secs())),
        Ok(Err(e)) => Err(format!("cannot run {prog}: {e}")),
        Ok(Ok(o)) => {
            let mut text = String::from_utf8_lossy(&o.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            Ok((o.status.success(), text.trim().to_string()))
        }
    }
}
