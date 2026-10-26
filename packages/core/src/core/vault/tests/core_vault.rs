// Vault acceptance cases; shared imports/helpers come from core_vault_support.rs.

#[test]
fn init_open_unlock_lock() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("vault");
    let (v, res) = Vault::init(&init_params(&path)).unwrap();
    assert!(res.recovery_kit.is_none());
    assert!(!res.status.locked && res.status.initialized);
    assert_eq!(res.status.recipients.len(), 1);
    assert_eq!(res.status.recipients[0].kind, RecipientKind::Passphrase);
    assert!(!res.status.dirty, "init commits everything");
    for f in [
        format::VERSION_FILE,
        format::CONFIG_FILE,
        format::VAULTKEY_FILE,
        format::MANIFEST_FILE,
        format::MANIFEST_SIG_FILE,
        "README.md",
        ".gitattributes",
        ".gitignore",
    ] {
        assert!(path.join(f).exists(), "{f}");
    }
    assert!(path.join(".git/hooks/pre-commit").exists());
    let id = v.id().to_owned();
    let locked = v.lock();
    assert!(locked.status().locked);

    let opened = Vault::open(&path).unwrap();
    assert_eq!(opened.id(), id);
    assert!(matches!(
        opened.unlock_passphrase("wrong passphrase!"),
        Err(Error::BadCredentials)
    ));
    let err = opened.unlock_passphrase("wrong passphrase!").unwrap_err();
    assert_eq!(err.code(), ErrorCode::BadCredentials);
    let u = opened
        .unlock_with(&UnlockParams {
            passphrase: Some(PASS.into()),
            identity_file: None,
            timeout_secs: None,
        })
        .unwrap();
    assert!(u.integrity_problems().is_empty());
    assert!(u.list_profiles().unwrap().is_empty());
    let log = u.git().log(10).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].message, "vault: init");
    assert_eq!(log[0].author, "Alice");
}

#[test]
fn init_refuses_existing_and_weak_rules() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    new_vault(&path);
    assert!(matches!(
        Vault::init(&init_params(&path)),
        Err(Error::AlreadyExists(_))
    ));
    let other = t.path().join("nonempty");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("x"), "y").unwrap();
    assert!(matches!(
        Vault::init(&init_params(&other)),
        Err(Error::InvalidParams(_))
    ));
    let mut p = init_params(&t.path().join("short"));
    p.passphrase = "short".into();
    assert!(matches!(Vault::init(&p), Err(Error::InvalidParams(_))));
}

#[test]
fn newer_format_version_refused() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    new_vault(&path);
    std::fs::write(path.join(format::VERSION_FILE), "2\n").unwrap();
    let e = Vault::open(&path).unwrap_err();
    assert_eq!(e.code(), ErrorCode::UnsupportedVersion);
}

#[test]
fn recovery_kit_unlocks() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut p = init_params(&path);
    p.recovery_kit = true;
    let (_v, res) = Vault::init(&p).unwrap();
    let kit = res.recovery_kit.unwrap();
    assert!(kit.recovery_secret_key.starts_with("AGE-SECRET-KEY-1"));
    assert!(kit.document.contains(&kit.recovery_secret_key));
    assert_eq!(res.status.recipients.len(), 2);
    assert_eq!(res.status.recipients[1].kind, RecipientKind::Recovery);
    // The secret key is never written to the vault.
    for f in walk(&path) {
        let bytes = std::fs::read(&f).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(&kit.recovery_secret_key),
            "{f:?}"
        );
    }
    let idf = t.path().join("recovery.txt");
    std::fs::write(&idf, format!("{}\n", kit.recovery_secret_key)).unwrap();
    let u = Vault::open(&path)
        .unwrap()
        .unlock_with(&UnlockParams {
            passphrase: None,
            identity_file: Some(idf.display().to_string()),
            timeout_secs: None,
        })
        .unwrap();
    assert!(u.integrity_problems().is_empty());
}

#[test]
fn vault_home_paths_expand_at_all_boundaries() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let work = t.path().join("work");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    // HOME and cwd change only in a fresh child; parallel tests keep their own
    // process environment and no real user vault path is touched.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "vault_home_paths_subprocess_fixture",
            "--nocapture",
        ])
        .env("LATCH_TEST_HOME_PATHS", "1")
        .env("HOME", &home)
        .env("LATCH_CONFIG_DIR", t.path().join("config"))
        .env("LATCH_DATA_DIR", t.path().join("data"))
        .current_dir(&work)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!work.join("~").exists());
}

#[test]
fn vault_home_paths_subprocess_fixture() {
    if std::env::var_os("LATCH_TEST_HOME_PATHS").is_none() {
        return;
    }
    setup();
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let (vault, result) =
        Vault::init(&init_params(std::path::Path::new("~/vault-source"))).unwrap();
    let source = home.join("vault-source").canonicalize().unwrap();
    assert!(vault.path().is_absolute());
    assert_eq!(vault.path(), source);
    assert_eq!(result.status.path.as_deref(), source.to_str());
    let opened = Vault::open("~/vault-source").unwrap();
    assert_eq!(opened.path(), source);
    let cloned = Vault::clone_from(source.to_str().unwrap(), "~/vault-clone").unwrap();
    assert_eq!(
        cloned.path(),
        home.join("vault-clone").canonicalize().unwrap()
    );
    for path in ["~alice/vault", "~.user/vault"] {
        assert_eq!(
            Vault::init(&init_params(std::path::Path::new(path)))
                .err()
                .unwrap()
                .code(),
            ErrorCode::InvalidParams
        );
        assert_eq!(
            Vault::open(path).err().unwrap().code(),
            ErrorCode::InvalidParams
        );
        assert_eq!(
            Vault::clone_from(source.to_str().unwrap(), path)
                .err()
                .unwrap()
                .code(),
            ErrorCode::InvalidParams
        );
    }
    assert!(!std::path::Path::new("~").exists());
}
