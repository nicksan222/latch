//! Native E2E worker; it never calls the container orchestration entrypoint.
pub(crate) mod artifacts;
use anyhow::{Context, Result, ensure};
use std::process::Command;
pub(crate) const ARTIFACT_ARCHIVE: &str = "/tmp/latch-e2e-artifacts.tar";

pub fn run() -> Result<()> {
    let result = run_tests();
    let archive = artifacts::archive(
        &crate::repository().join(".dev/test-results"),
        std::path::Path::new(ARTIFACT_ARCHIVE),
    );
    if result.is_err() && archive.is_err() {
        eprintln!("E2E artifact collection also failed");
    }
    result?;
    archive
}

fn run_tests() -> Result<()> {
    for script in ["tooling/e2e/run.mjs", "apps/landing/tooling/test-site.mjs"] {
        let status = Command::new("node")
            .arg(script)
            .current_dir(crate::repository())
            .env("CI", "true")
            .env("LATCH_E2E_RUN_ID", "container")
            .status()
            .context("run browser acceptance worker")?;
        ensure!(status.success(), "browser acceptance worker failed");
    }
    Ok(())
}
