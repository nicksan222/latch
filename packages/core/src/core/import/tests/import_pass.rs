// `pass` store import: decrypts `fixtures/import/pass-store` with the
// test-only key `fixtures/import/pass-test-key.asc` in a temporary
// GNUPGHOME supplied by a configured Rust GPG fixture.

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use latch::core::import;
use latch::protocol::*;
use support::*;

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/import")
        .join(name)
}

/// Some(wrapper) when gpg is available; the keyring lives for the process.
fn gpg_wrapper() -> Option<&'static Path> {
    static W: OnceLock<Option<(tempfile::TempDir, PathBuf)>> = OnceLock::new();
    W.get_or_init(|| {
        let gpg = which::which("gpg").ok()?;
        let home = tempfile::Builder::new().prefix("lg").tempdir().ok()?;
        let gh = home.path().join("g");
        std::fs::create_dir(&gh).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o700)).ok()?;
        }
        let ok = std::process::Command::new(&gpg)
            .env("GNUPGHOME", &gh)
            .args(["--batch", "--quiet", "--import"])
            .arg(fx("pass-test-key.asc"))
            .status()
            .ok()?
            .success();
        if !ok {
            return None;
        }
        let w = home.path().join("gpg-fixture");
        latch_automation::fixtures::install(
            latch_automation::fixtures::FixtureConfig::Gpg(latch_automation::fixtures::GpgConfig {
                gnupghome: Some(gh),
                delegate: Some(gpg),
                ..Default::default()
            }),
            &w,
        )
        .ok()?;
        Some((home, w))
    })
    .as_ref()
    .map(|(_, w)| w.as_path())
}

static ENV: std::sync::Once = std::sync::Once::new();

fn setup_gpg() -> bool {
    setup();
    let w = gpg_wrapper();
    ENV.call_once(|| {
        let v = w
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "/nonexistent/gpg".into());
        // SAFETY: called once before any test thread reads LATCH_GPG.
        unsafe { std::env::set_var("LATCH_GPG", v) };
    });
    w.is_some()
}

#[test]
fn pass_store_import() {
    if !setup_gpg() {
        eprintln!("gpg not available; skipping");
        return;
    }
    let parsed = import::parse(ImportFormat::Pass, &fx("pass-store"), None).unwrap();
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    let names: Vec<_> = parsed
        .entries
        .iter()
        .map(|e| (e.folder.clone(), e.name.clone(), e.entry_type()))
        .collect();
    assert!(names.contains(&(None, "api-token".into(), EntryType::Secret)));
    assert!(names.contains(&(Some("work".into()), "github".into(), EntryType::Login)));
    assert!(names.contains(&(Some("work".into()), "aws - root".into(), EntryType::Login)));

    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = ImportParams {
        format: ImportFormat::Pass,
        path: fx("pass-store").display().to_string(),
        profile_id: None,
        skip_duplicates: true,
        passphrase: None,
    };
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!((r.created, r.profiles_created), (3, 2));
    let work = v.find_profile("work").unwrap();
    let gh = v.find_entry(&work.id, "github").unwrap();
    let e = v
        .get_entry(&GetEntryParams {
            id: gh.id,
            reveal: true,
        })
        .unwrap();
    let EntryPayload::Login(l) = &e.payload else {
        panic!()
    };
    assert_eq!(l.username, "alice");
    assert_eq!(l.password.as_deref(), Some("fixture-github-pass"));
    assert!(l.totp.is_some());
    let aws = v.find_entry(&work.id, "aws - root").unwrap();
    let e = v
        .get_entry(&GetEntryParams {
            id: aws.id,
            reveal: true,
        })
        .unwrap();
    let EntryPayload::Login(l) = &e.payload else {
        panic!()
    };
    assert!(l.custom.iter().any(|c| c.key == "mfa-serial" && c.hidden));
    let pv = import::preview(&v, &p).unwrap();
    assert!(pv.items.iter().all(|i| i.duplicate));
}

#[test]
fn pass_store_edge_cases() {
    setup_gpg();
    // (gpg missing entirely is covered by the unit test in core::import::pass.)
    let t = tempfile::tempdir().unwrap();
    let parsed = import::parse(ImportFormat::Pass, t.path(), None).unwrap();
    assert!(parsed.entries.is_empty());
    assert!(parsed.warnings[0].contains("no .gpg files"));
    let f = t.path().join("file");
    std::fs::write(&f, "x").unwrap();
    assert!(import::parse(ImportFormat::Pass, &f, None).is_err());
}
