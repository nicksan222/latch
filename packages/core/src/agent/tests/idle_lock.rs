use super::*;

#[test]
fn a_queued_idle_lock_preserves_a_session_with_renewed_activity() {
    let directory = tempfile::tempdir().unwrap();
    let agent = Agent::new(AgentOptions {
        config_dir: Some(directory.path().join("config")),
        clipboard: Some(Arc::new(clipboard::MemoryClipboard::new())),
        browser_config: Some(browser::ManagerConfig {
            ram_dir: directory.path().join("ram"),
            data_dir: directory.path().join("data"),
            browser_override: None,
            path_var: None,
            term_timeout: Duration::from_millis(10),
            archive_wait: Duration::from_millis(10),
        }),
        ..Default::default()
    });
    // A queued worker must recheck the deadline even if it was eligible when
    // enqueued. Renewing activity preserves the unlocked state and generation.
    agent.unlocked.store(true, Ordering::SeqCst);
    agent.set_unlocked(Some(60));
    agent.touch(true);
    let generation = agent.generation.load(Ordering::SeqCst);
    assert!(!agent.lock_with_reason(Some("idle timeout")));
    assert!(agent.is_unlocked());
    assert_eq!(agent.generation.load(Ordering::SeqCst), generation);
    agent.unlocked.store(false, Ordering::SeqCst);
    agent.shutdown();
}
