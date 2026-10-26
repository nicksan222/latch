use super::super::*;

pub(super) fn stop() -> CliResult {
    let pid_path = latch::agent::pid_path();
    let sock = latch::agent::socket_path();
    let Some(pid) = std::fs::read_to_string(&pid_path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
    else {
        outln!("agent not running");
        return Ok(());
    };
    if AgentClient::connect(HelloParams {
        client: ClientKind::Cli,
        version: env!("CARGO_PKG_VERSION").into(),
    })
    .is_err()
    {
        outln!("agent not running");
        let _ = std::fs::remove_file(&pid_path);
        return Ok(());
    }
    #[cfg(unix)]
    {
        // SAFETY: plain kill(2) on a pid we read from our private runtime dir.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        for _ in 0..100 {
            if !sock.exists() {
                eprintln!("agent stopped");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(CliError::user("agent did not stop in time"))
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, sock);
        Err(CliError::user(
            "agent stop is not supported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clap_routes_the_action_with_its_existing_arguments() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["latch", "agent", "stop"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Agent(AgentCmd::Stop)));
    }
}
