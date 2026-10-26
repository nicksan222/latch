use super::super::*;

pub(super) fn run(headless: bool) -> CliResult {
    crypto::harden_process();
    let agent = latch::agent::Agent::new(latch::agent::AgentOptions {
        headless_idle_exit: headless.then(latch::agent::default_idle_exit),
        watch_screen_lock: true,
        ..Default::default()
    });
    let path = agent.serve_socket().map_err(|e| {
        if e.code() == ErrorCode::AlreadyExists {
            CliError::user("an agent is already running")
        } else {
            CliError::from(e)
        }
    })?;
    if !headless {
        eprintln!("latch agent listening on {}", path.display());
    }
    // Signals → orderly shutdown (lock, close browsers, remove socket).
    {
        let a = agent.clone();
        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async {
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{SignalKind, signal};
                    let (Ok(mut term), Ok(mut int), Ok(mut hup)) = (
                        signal(SignalKind::terminate()),
                        signal(SignalKind::interrupt()),
                        signal(SignalKind::hangup()),
                    ) else {
                        return;
                    };
                    tokio::select! {
                        _ = term.recv() => {}
                        _ = int.recv() => {}
                        _ = hup.recv() => {}
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }
            });
            a.shutdown();
        });
    }
    agent.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clap_routes_the_action_with_its_existing_arguments() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["latch", "agent", "run", "--headless"]).unwrap();
        assert!(matches!(
            cli.cmd,
            Cmd::Agent(AgentCmd::Run { headless: true })
        ));
    }
}
