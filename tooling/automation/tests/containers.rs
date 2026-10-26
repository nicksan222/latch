//! Explicit Docker contract checks; ordinary unit tests never need a daemon.
use anyhow::Result;
use latch_automation::containers::run_image;
use std::time::Duration;
use testcontainers::GenericImage;

#[tokio::test]
#[ignore = "requires Docker"]
async fn failed_workload_propagates_exit_status() -> Result<()> {
    let image = GenericImage::new("alpine", "3.22").with_entrypoint("/bin/sh");
    let error = run_image(image, &["-c", "exit 7"], Duration::from_secs(10))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("status 7"), "{error:#}");
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn stuck_workload_hits_deadline() -> Result<()> {
    let image = GenericImage::new("alpine", "3.22").with_entrypoint("/bin/sh");
    let error = run_image(image, &["-c", "sleep 60"], Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("deadline"), "{error:#}");
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn successful_workloads_can_run_concurrently() -> Result<()> {
    let image = || GenericImage::new("alpine", "3.22").with_entrypoint("/bin/sh");
    let (first, second) = tokio::join!(
        run_image(image(), &["-c", "exit 0"], Duration::from_secs(10)),
        run_image(image(), &["-c", "exit 0"], Duration::from_secs(10))
    );
    first?;
    second?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn custom_image_build_preserves_context() -> Result<()> {
    use testcontainers::{
        GenericBuildableImage, ImageExt,
        runners::{AsyncBuilder, AsyncRunner},
    };
    let _ = env_logger::Builder::new()
        .parse_filters("testcontainers::core::client=info")
        .try_init();
    let image = tokio::time::timeout(
        Duration::from_secs(30),
        GenericBuildableImage::new("latch-tests-build-contract", "context")
            .with_dockerfile_string(
                "FROM alpine:3.22\nCOPY sample /sample\nCMD [\"sleep\", \"60\"]\n",
            )
            .with_data(b"synthetic context".to_vec(), "sample")
            .build_image(),
    )
    .await??;
    let container = image.with_shm_size(1024 * 1024).start().await?;
    let contents = container.copy_file_from("/sample", Vec::new()).await?;
    container.rm().await?;
    assert_eq!(contents, b"synthetic context");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires Docker and the devcontainer Docker CLI"]
async fn interruption_removes_owned_container() -> Result<()> {
    use std::process::{Command, Stdio};
    use std::time::Instant;
    if std::env::var_os("LATCH_CONTAINER_SIGNAL_CHILD").is_some() {
        return run_image(
            GenericImage::new("alpine", "3.22").with_entrypoint("/bin/sh"),
            &["-c", "sleep 60"],
            Duration::from_secs(65),
        )
        .await;
    }
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "interruption_removes_owned_container",
                "--ignored",
            ])
            .env("LATCH_CONTAINER_SIGNAL_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let filter = format!("label=dev.latch.test-owner={}", child.id());
        let exists = || -> Result<bool> {
            let output = Command::new("docker")
                .args(["ps", "-aq", "--filter", &filter])
                .output()?;
            anyhow::ensure!(output.status.success(), "inspect owned test containers");
            Ok(!output.stdout.is_empty())
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        while !exists()? && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !exists()? {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("signal fixture did not start its owned container");
        }
        // SAFETY: this PID belongs to the child process spawned above.
        unsafe {
            libc::kill(child.id() as i32, signal);
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("container signal cleanup did not finish");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        // The watchdog can race the ordinary failure path after stopping the child;
        // both a signal status and a nonzero test exit are valid interruptions.
        assert!(!status.success(), "interruption must fail the run");
        let deadline = Instant::now() + Duration::from_secs(5);
        while exists()? && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!exists()?, "interruption must remove its owned container");
    }
    Ok(())
}
