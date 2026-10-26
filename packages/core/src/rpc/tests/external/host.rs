//! Test-only server host. Never invoked in the client process.
use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

#[test]
#[ignore = "subprocess fixture; launched only by external gRPC clients"]
fn grpc_test_host() {
    let root = std::path::PathBuf::from(
        std::env::var_os("LATCH_GRPC_TEST_ROOT").expect("subprocess-only fixture"),
    );
    let agent = latch::agent::Agent::new(latch::agent::AgentOptions {
        config_dir: Some(root.join("config")),
        clipboard: Some(Arc::new(latch::agent::clipboard::MemoryClipboard::new())),
        browser_config: Some(latch::agent::browser::ManagerConfig {
            ram_dir: root.join("ram"),
            data_dir: root.join("data"),
            browser_override: (!root.join("native-browser").is_file())
                .then(|| root.join("bin/chromium")),
            path_var: Some(root.join("bin").into_os_string()),
            term_timeout: Duration::from_millis(500),
            archive_wait: Duration::from_secs(2),
        }),
        watch_screen_lock: false,
        headless_idle_exit: None,
        tick: Duration::from_millis(20),
        browser_lock_grace: Duration::ZERO,
    });
    agent.serve_socket().expect("bind isolated gRPC socket");
    println!("LATCH_GRPC_TEST_READY");
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().lock().lines().next();
    agent.shutdown();
}
