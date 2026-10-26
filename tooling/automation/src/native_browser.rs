//! Native browser acceptance command. Real scenarios use the normal Cargo harness.
#[cfg(any(test, feature = "browser-acceptance"))]
#[cfg_attr(all(test, not(feature = "browser-acceptance")), allow(dead_code))]
mod fixture;
#[cfg(any(test, feature = "browser-acceptance"))]
mod marionette;
use crate::repository as repository_root;
use anyhow::{Context, Result};
#[cfg(feature = "browser-acceptance")]
pub use fixture::BrowserFixture;
use std::process::{Command, Stdio};
pub fn run() -> Result<()> {
    run_cargo_tests(&[
        "-p",
        "latch-automation",
        "--features",
        "browser-acceptance",
        "--test",
        "native_browser",
    ])?;
    run_external_acceptance()?;
    run_cargo_tests(&[
        "-p",
        "latch-core",
        "--test",
        "browser_sessions",
        "real_managed_firefox_smoke",
        "--",
        "--ignored",
    ])
}

fn require_tests(listing: &str) -> Result<()> {
    anyhow::ensure!(
        listing.lines().any(|line| line.ends_with(": test")),
        "native-browser selection discovered no tests"
    );
    Ok(())
}

fn run_cargo_tests(args: &[&str]) -> Result<()> {
    let mut selection = vec!["test", "--locked", "--no-default-features"];
    selection.extend_from_slice(args);
    let listing = Command::new("cargo")
        .args(&selection)
        .args(if selection.contains(&"--") {
            vec!["--list"]
        } else {
            vec!["--", "--list"]
        })
        .current_dir(repository_root())
        .stderr(Stdio::inherit())
        .output()
        .context("discover native-browser acceptance tests")?;
    anyhow::ensure!(
        listing.status.success(),
        "native-browser test discovery failed"
    );
    require_tests(&String::from_utf8(listing.stdout)?)?;
    let status = Command::new("cargo")
        .args(&selection)
        .current_dir(repository_root())
        .stdin(Stdio::null())
        // Each Firefox owns several processes; bound native acceptance resource use.
        .env("RUST_TEST_THREADS", "1")
        .status()?;
    anyhow::ensure!(status.success(), "native-browser acceptance tests failed");
    Ok(())
}

fn run_external_acceptance() -> Result<()> {
    run_cargo_tests(&[
        "-p",
        "latch-core",
        "--test",
        "rpc_external",
        "native_browser::managed_firefox",
        "--",
        "--ignored",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_acceptance_selection_is_an_error() {
        assert!(require_tests("0 tests, 0 benchmarks\n").is_err());
        assert!(
            require_tests("native_browser::managed_firefox: test\n1 test, 0 benchmarks\n").is_ok()
        );
    }
}
