use super::*;

#[tokio::test]
#[ignore = "requires Docker"]
async fn failure_artifacts_are_exported_without_replacing_the_exit_error() -> Result<()> {
    for valid in [true, false] {
        let mut archive = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(15);
        header.set_mode(0o600);
        header.set_cksum();
        archive.append_data(&mut header, "case/trace.zip", &b"synthetic trace"[..])?;
        let bytes = if valid {
            archive.into_inner()?
        } else {
            b"not a tar archive".to_vec()
        };
        let image = GenericBuildableImage::new("latch-tests-artifact-contract", if valid { "valid" } else { "invalid" })
            .with_dockerfile_string("FROM alpine:3.22\nCOPY artifacts.tar /tmp/latch-e2e-artifacts.tar\nCMD [\"sh\", \"-c\", \"exit 7\"]\n")
            .with_data(bytes, "artifacts.tar").build_image().await?;
        let output = tempfile::tempdir()?;
        let error = run_workload(image, &[], Duration::from_secs(10), Some(output.path()))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("status 7"), "{error:#}");
        if valid {
            assert_eq!(
                std::fs::read(output.path().join("case/trace.zip"))?,
                b"synthetic trace"
            );
        }
    }
    Ok(())
}
