use tracing::warn;

use crate::daemon::Daemon;
use crate::telemetry::tracing::{init_tracing, TracingConfig};

pub async fn handle_start(foreground: bool) -> anyhow::Result<()> {
    if !foreground {
        let config = TracingConfig {
            log_to_file: false,
            log_to_stderr: true,
            ..TracingConfig::default()
        };
        let _guard = init_tracing(&config)?;
        warn!("Daemonization not yet implemented (daemonize crate required)");
        return Ok(());
    }

    let config = TracingConfig {
        log_to_file: false,
        log_to_stderr: true,
        ..TracingConfig::default()
    };
    let _guard = init_tracing(&config)?;

    let mut daemon = Daemon::new();
    daemon.run().await?;
    Ok(())
}

pub async fn handle_stop() -> anyhow::Result<()> {
    use std::thread;
    use std::time::Duration;
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    use crate::daemon::pid::PidFile;

    let pid_file = PidFile::new();

    let pid = match pid_file.read() {
        Ok(pid) => pid,
        Err(_) => {
            println!("Daemon not running");
            return Ok(());
        }
    };

    if !PidFile::is_process_running(pid)? {
        println!("Daemon not running");
        return Ok(());
    }

    println!("Stopping daemon (PID: {})...", pid);

    let nix_pid = Pid::from_raw(pid as i32);
    kill(nix_pid, Signal::SIGTERM)?;

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if !PidFile::is_process_running(pid)? {
            println!("Daemon stopped");
            return Ok(());
        }
    }

    kill(nix_pid, Signal::SIGKILL)?;
    println!("Daemon stopped");
    Ok(())
}

pub async fn handle_restart() -> anyhow::Result<()> {
    println!("daemon restart not implemented (Story TBD)");
    Ok(())
}

pub async fn handle_reload() -> anyhow::Result<()> {
    use crate::daemon::pid::PidFile;

    let pid_file = PidFile::new();

    let pid = match pid_file.read() {
        Ok(pid) => pid,
        Err(_) => {
            println!("Daemon not running");
            return Ok(());
        }
    };

    if !PidFile::is_process_running(pid)? {
        println!("Daemon not running");
        return Ok(());
    }

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid as i32), Signal::SIGHUP)?;
        println!("Sent reload signal to daemon (PID: {pid})");
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        anyhow::bail!("Reload is not supported on this platform");
    }
}

pub async fn handle_status(json: bool) -> anyhow::Result<()> {
    super::status::handle_status(json).await
}
