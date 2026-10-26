//! Rust owns container lifecycle; Dockerfiles own the test environment.
use crate::e2e::artifacts;
mod context;
use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use std::time::Duration;
use testcontainers::{
    GenericBuildableImage, GenericImage, ImageExt,
    core::BuildImageOptions,
    runners::{AsyncBuilder, AsyncRunner},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Suite {
    Browser,
    Desktop,
    E2e,
}
impl Suite {
    fn name(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Desktop => "desktop",
            Self::E2e => "e2e",
        }
    }
}

pub fn run(suite: Suite) -> Result<()> {
    let _ = env_logger::Builder::new()
        .parse_filters("testcontainers::core::client=info")
        .try_init();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(run_suite(suite))
}

async fn run_suite(suite: Suite) -> Result<()> {
    let root = crate::repository();
    let snapshot = tempfile::tempdir().context("stage test source")?;
    context::snapshot(&root, snapshot.path())?;
    let digest = context::digest(snapshot.path())?;
    let definitions = snapshot.path().join("tooling/automation/containers");
    let dockerfile = format!(
        "{}\n{}",
        std::fs::read_to_string(definitions.join("Dockerfile.base"))?,
        std::fs::read_to_string(definitions.join(format!("Dockerfile.{}", suite.name())))?
    );
    println!(
        "Building {} test environment ({})",
        suite.name(),
        &digest[..12]
    );
    let image = tokio::time::timeout(
        Duration::from_secs(3600),
        GenericBuildableImage::new(format!("latch-tests-{}", suite.name()), &digest[..16])
            .with_dockerfile_string(dockerfile)
            .with_file(snapshot.path(), "source")
            .build_image_with(BuildImageOptions::new().with_skip_if_exists(true)),
    )
    .await
    .context("test image build exceeded 60 minutes")?
    .context("build test image through Docker; run inside the devcontainer with Docker access")?;
    let output = if matches!(suite, Suite::E2e) {
        Some(artifacts::output_directory(&root)?)
    } else {
        None
    };
    run_workload(image, &[], Duration::from_secs(1800), output.as_deref()).await
}

/// Run a one-shot test image and require successful completion, not merely readiness.
pub async fn run_image(image: GenericImage, command: &[&str], deadline: Duration) -> Result<()> {
    run_workload(image, command, deadline, None).await
}

async fn run_workload(
    image: GenericImage,
    command: &[&str],
    deadline: Duration,
    output: Option<&std::path::Path>,
) -> Result<()> {
    let request = image
        .with_host_config_modifier(|config| config.init = Some(true))
        .with_shm_size(1024 * 1024 * 1024)
        .with_label("dev.latch.tests", "true")
        .with_label("dev.latch.test-owner", std::process::id().to_string());
    let request = if command.is_empty() {
        request
    } else {
        request.with_cmd(command.iter().copied())
    };
    let container = request
        .start()
        .await
        .context("start isolated test workload")?;
    let mut out = container.stdout(true);
    let mut err = container.stderr(true);
    let result = tokio::time::timeout(deadline, async {
        let completion = async {
            loop {
                if let Some(code) = container.exit_code().await? {
                    ensure!(code == 0, "test workload exited with status {code}");
                    return Ok::<(), anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        let diagnostics = async {
            let mut stdout = tokio::io::stdout();
            let mut stderr = tokio::io::stderr();
            tokio::try_join!(
                tokio::io::copy(&mut out, &mut stdout),
                tokio::io::copy(&mut err, &mut stderr)
            )
        };
        // Drain diagnostics even on a nonzero exit; cancel both readers on timeout.
        let (status, logs) = tokio::join!(completion, diagnostics);
        status?;
        logs.context("stream test workload diagnostics")?;
        Ok(())
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("test workload exceeded its deadline")));
    let exported = tokio::time::timeout(Duration::from_secs(30), async {
        if let Some(output) = output {
            let bundle = tempfile::NamedTempFile::new_in(output)?;
            container
                .copy_file_from(crate::e2e::ARTIFACT_ARCHIVE, bundle.path().to_path_buf())
                .await?;
            ensure!(
                bundle.as_file().metadata()?.len() <= 256 * 1024 * 1024,
                "artifact archive exceeds export limit"
            );
            artifacts::extract(bundle.path(), output)?;
            println!("Test artifacts: {}", output.display());
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("test artifact export timed out")));
    let cleanup = container.rm().await;
    if let Err(error) = result {
        if exported.is_err() {
            eprintln!("test artifact export also failed");
        }
        if cleanup.is_err() {
            eprintln!("test workload cleanup also failed");
        }
        bail!(error);
    }
    exported.context("export test artifacts")?;
    cleanup.context("remove completed test workload")?;
    Ok(())
}

#[cfg(test)]
mod tests;
