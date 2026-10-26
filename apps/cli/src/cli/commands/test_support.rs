//! Minimal isolated transport for command error-policy tests.
use super::Session;
use latch::agent::browser::ManagerConfig;
use latch::agent::clipboard::MemoryClipboard;
use latch::agent::{Agent, AgentOptions};
use std::sync::Arc;
use std::time::Duration;

pub(super) struct EmptySession {
    pub session: Session,
    _directory: tempfile::TempDir,
}

impl EmptySession {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let agent = Agent::new(AgentOptions {
            config_dir: Some(directory.path().join("config")),
            clipboard: Some(Arc::new(MemoryClipboard::new())),
            browser_config: Some(ManagerConfig {
                ram_dir: directory.path().join("ram"),
                data_dir: directory.path().join("data"),
                browser_override: None,
                term_timeout: Duration::from_secs(1),
                archive_wait: Duration::from_secs(1),
            }),
            ..Default::default()
        });
        Self {
            session: Session::for_test(agent),
            _directory: directory,
        }
    }
}

impl Drop for EmptySession {
    fn drop(&mut self) {
        self.session.local_agent().unwrap().shutdown();
    }
}
