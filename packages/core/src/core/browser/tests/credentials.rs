use super::*;

#[test]
fn origins_are_scoped_and_never_embed_url_credentials() {
    assert_eq!(
        origin("https://EXAMPLE.com:443/path?q=private#fragment").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        origin("http://localhost:9988/login").as_deref(),
        Some("http://localhost:9988")
    );
    for invalid in [
        "file:///tmp/x",
        "javascript:alert(1)",
        "https://user:pass@example.com/",
        "data:text/plain,x",
        "--evil",
        "example.com",
    ] {
        assert!(origin(invalid).is_none());
    }
}

#[test]
fn acknowledgement_requires_matching_generation_and_cleans_transient_files() {
    let profile = tempfile::tempdir().unwrap();
    super::super::write_file_in(
        profile.path(),
        Path::new(PENDING),
        b"fictional private payload",
    )
    .unwrap();
    super::super::write_file_in(
        profile.path(),
        Path::new(ACK),
        br#"{"generation":"old","ok":true}"#,
    )
    .unwrap();
    assert!(wait(profile.path(), "new", Duration::ZERO).is_err());
    assert!(!profile.path().join(PENDING).exists());
    super::super::write_file_in(
        profile.path(),
        Path::new(ACK),
        br#"{"generation":"new","ok":true}"#,
    )
    .unwrap();
    wait(profile.path(), "new", Duration::ZERO).unwrap();
    assert!(!profile.path().join(ACK).exists());
}

#[test]
fn failed_signing_is_retried_even_when_configuration_bytes_match() {
    let dir = tempfile::tempdir().unwrap();
    let executable = dir.path().join("firefox");
    assert!(
        install_bootstrap_with(&executable, |_| Err(Error::Internal(
            "test signing failure".into()
        )))
        .is_err()
    );
    assert!(dir.path().join(".latch-bootstrap-unsigned").exists());
    let mut finalized = false;
    assert!(
        install_bootstrap_with(&executable, |_| {
            finalized = true;
            Ok(())
        })
        .unwrap()
    );
    assert!(finalized);
    assert!(!dir.path().join(".latch-bootstrap-unsigned").exists());
    assert!(
        !install_bootstrap_with(&executable, |_| panic!(
            "unchanged signed bootstrap needs no signing"
        ))
        .unwrap()
    );
}

#[test]
fn bootstrap_matches_the_actual_linux_and_macos_archive_layouts() {
    let root = tempfile::tempdir().unwrap();
    let linux = root.path().join("linux/firefox");
    std::fs::create_dir_all(linux.parent().unwrap()).unwrap();
    install_bootstrap_with(&linux, |_| Ok(())).unwrap();
    assert!(
        root.path()
            .join("linux/browser/defaults/preferences/00-playwright-prefs.js")
            .is_file()
    );
    let macos = root.path().join("Nightly.app/Contents/MacOS/firefox");
    std::fs::create_dir_all(root.path().join("Nightly.app/Contents/Resources")).unwrap();
    install_bootstrap_with(&macos, |_| Ok(())).unwrap();
    assert!(
        root.path()
            .join("Nightly.app/Contents/Resources/defaults/pref/00-playwright-prefs.js")
            .is_file()
    );
    assert!(
        !root
            .path()
            .join(
                "Nightly.app/Contents/Resources/browser/defaults/preferences/00-playwright-prefs.js"
            )
            .exists()
    );
}
