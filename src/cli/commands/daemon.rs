use tracing::warn;

use crate::daemon::Daemon;
use crate::telemetry::otel::load_otel_config;
use crate::telemetry::tracing::{TracingConfig, init_tracing};

pub async fn handle_start(foreground: bool) -> anyhow::Result<()> {
    let otel_config = load_otel_config();
    if !foreground {
        let config = TracingConfig {
            log_to_file: false,
            log_to_stderr: true,
            ..TracingConfig::default()
        };
        let _guard = init_tracing(&config, otel_config.as_ref())?;
        warn!("Daemonization not yet implemented (daemonize crate required)");
        return Ok(());
    }

    let config = TracingConfig {
        log_to_file: false,
        log_to_stderr: true,
        ..TracingConfig::default()
    };
    let _guard = init_tracing(&config, otel_config.as_ref())?;

    let mut daemon = Daemon::new();
    daemon.run().await?;
    Ok(())
}

pub async fn handle_stop() -> anyhow::Result<()> {
    use crate::daemon::pid::PidFile;
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    use std::thread;
    use std::time::Duration;

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
        use nix::sys::signal::{Signal, kill};
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
