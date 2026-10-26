use super::super::*;

pub(super) fn status() -> CliResult {
    match AgentClient::connect(HelloParams {
        client: ClientKind::Cli,
        version: env!("CARGO_PKG_VERSION").into(),
    }) {
        Ok((c, h)) => {
            let pid = std::fs::read_to_string(latch::agent::pid_path()).unwrap_or_default();
            outln!(
                "agent running (pid {}, version {}, protocol {})",
                pid.trim(),
                h.agent_version,
                h.protocol
            );
            let mut session = Session::from_remote(c);
            if let Ok(st) = session.vault_status() {
                outln!(
                    "vault: {}",
                    if !st.initialized {
                        "none".to_owned()
                    } else if st.locked {
                        "locked".to_owned()
                    } else {
                        format!("unlocked{}", locks_in(&st))
                    }
                );
            }
            Ok(())
        }
        Err(_) => {
            outln!("agent not running");
            Err(CliError::new(1, ""))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clap_routes_the_action_with_its_existing_arguments() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["latch", "agent", "status"]).unwrap();
        assert!(matches!(cli.cmd, Cmd::Agent(AgentCmd::Status)));
    }
}
