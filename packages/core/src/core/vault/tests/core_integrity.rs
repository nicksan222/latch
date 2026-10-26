// Tamper detection, doctor (rebuild, fix, plaintext scan) and the hook.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::{UnlockedVault, Vault, format};
use latch::protocol::*;
use latch_automation::fixtures::{self, FixtureConfig};

fn vault_with_entries(dir: &std::path::Path) -> (UnlockedVault, String, String) {
    let mut v = new_vault(dir);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let mut last = String::new();
    for (name, payload) in all_payloads().into_iter().take(3) {
        last = v
            .create_entry(&entry_input(&p.id, name, payload))
            .unwrap()
            .id;
    }
    (v, p.id, last)
}

fn doctor(v: &mut UnlockedVault, fix: bool, rebuild: bool) -> DoctorReport {
    v.doctor(&DoctorParams {
        fix,
        rebuild_manifest: rebuild,
        staged: false,
    })
    .unwrap()
}

#[test]
fn modified_entry_ciphertext_is_detected() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (v, pid, eid) = vault_with_entries(&path);
    drop(v);
    let f = path.join(format::entry_file(&pid, &eid));
    let mut bytes = std::fs::read(&f).unwrap();
    let n = bytes.len();
    bytes[n - 5] ^= 0x55;
    std::fs::write(&f, &bytes).unwrap();

    let mut u = unlock(&path);
    assert!(!u.integrity_problems().is_empty());
    assert_eq!(u.integrity_problems()[0].code, "hash_mismatch");
    let e = u
        .get_entry(&GetEntryParams {
            id: eid.clone(),
            reveal: false,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity);
    // Writes are blocked too.
    assert_eq!(
        u.create_profile(&profile_input("x")).unwrap_err().code(),
        ErrorCode::Integrity
    );
    assert_eq!(u.list_profiles().unwrap_err().code(), ErrorCode::Integrity);

    let r = doctor(&mut u, false, false);
    assert!(!r.ok);
    assert!(r.issues.iter().any(|i| i.code == "hash_mismatch"));
    // --fix restores the file from HEAD and rebuilds the manifest.
    let r = doctor(&mut u, true, false);
    assert!(r.ok, "{:?}", r.issues);
    assert!(r.fixed.iter().any(|f| f.contains("restored")));
    assert!(u.integrity_problems().is_empty());
    assert!(
        u.get_entry(&GetEntryParams {
            id: eid,
            reveal: true
        })
        .is_ok()
    );
}

#[test]
fn post_unlock_ciphertext_change_blocks_unrelated_write_without_advancing_head() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (mut vault, pid, eid) = vault_with_entries(&path);
    let head = vault.git().head().unwrap();
    let rel = format::entry_file(&pid, &eid);
    let file = path.join(&rel);
    let original = std::fs::read(&file).unwrap();
    let mut altered = original.clone();
    *altered.last_mut().unwrap() ^= 1;
    std::fs::write(&file, &altered).unwrap();

    assert_eq!(
        vault
            .get_entry(&GetEntryParams {
                id: eid,
                reveal: true
            })
            .unwrap_err()
            .code(),
        ErrorCode::Integrity
    );
    assert_eq!(
        vault
            .create_profile(&profile_input("must-not-persist"))
            .unwrap_err()
            .code(),
        ErrorCode::Integrity
    );
    assert_eq!(vault.integrity_problems()[0].code, "hash_mismatch");
    assert_eq!(vault.git().head().unwrap(), head);
    assert_eq!(vault.git().show("HEAD", &rel).unwrap(), Some(original));
}

#[test]
fn modified_manifest_is_detected() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (v, _pid, eid) = vault_with_entries(&path);
    drop(v);
    let f = path.join(format::MANIFEST_FILE);
    let mut bytes = std::fs::read(&f).unwrap();
    let n = bytes.len();
    bytes[n - 1] ^= 1;
    std::fs::write(&f, &bytes).unwrap();
    let mut u = unlock(&path);
    assert_eq!(u.integrity_problems()[0].code, "manifest_signature");
    assert_eq!(
        u.get_entry(&GetEntryParams {
            id: eid.clone(),
            reveal: false
        })
        .unwrap_err()
        .code(),
        ErrorCode::Integrity
    );
    // Explicit rebuild re-derives the manifest from the entries.
    let r = doctor(&mut u, false, true);
    assert!(r.ok, "{:?}", r.issues);
    assert!(
        u.get_entry(&GetEntryParams {
            id: eid,
            reveal: false
        })
        .is_ok()
    );
    assert_eq!(
        u.list_entries(&ListEntriesParams {
            profile_id: None,
            query: None,
            entry_type: None
        })
        .unwrap()
        .len(),
        3
    );
}

#[test]
fn swapped_signature_is_detected() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    let b = t.path().join("b");
    let (va, _, _) = vault_with_entries(&a);
    let (vb, _, _) = vault_with_entries(&b);
    drop((va, vb));
    // Signature from another vault.
    std::fs::copy(
        b.join(format::MANIFEST_SIG_FILE),
        a.join(format::MANIFEST_SIG_FILE),
    )
    .unwrap();
    let u = unlock(&a);
    assert_eq!(u.integrity_problems()[0].code, "manifest_signature");
    assert!(
        u.integrity_problems()[0]
            .message
            .contains("not signed by this vault")
    );
    // Rolled-back manifest + signature pair from an older commit is signed
    // but stale: hashes no longer match.
    let mut u = unlock(&b);
    assert!(u.integrity_problems().is_empty());
    let old = git(&b, &["rev-parse", "HEAD~2"]);
    let old = old.trim();
    let manifest = std::process::Command::new("git")
        .args(["show", &format!("{old}:{}", format::MANIFEST_FILE)])
        .current_dir(&b)
        .output()
        .unwrap()
        .stdout;
    let sig = std::process::Command::new("git")
        .args(["show", &format!("{old}:{}", format::MANIFEST_SIG_FILE)])
        .current_dir(&b)
        .output()
        .unwrap()
        .stdout;
    std::fs::write(b.join(format::MANIFEST_FILE), manifest).unwrap();
    std::fs::write(b.join(format::MANIFEST_SIG_FILE), sig).unwrap();
    // The live handle notices before writing.
    assert_eq!(
        u.create_profile(&profile_input("new")).unwrap_err().code(),
        ErrorCode::Integrity
    );
    let u2 = unlock(&b);
    assert!(
        u2.integrity_problems()
            .iter()
            .any(|p| p.code == "orphan_file"),
        "{:?}",
        u2.integrity_problems()
    );
}

#[test]
fn injected_recipient_in_config_is_detected() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (v, _, _) = vault_with_entries(&path);
    drop(v);
    let cfg = path.join(format::CONFIG_FILE);
    let mut text = std::fs::read_to_string(&cfg).unwrap();
    let attacker = age::x25519::Identity::generate().to_public().to_string();
    text.push_str(&format!(
        "\n[[recipients]]\nlabel = \"evil\"\ntype = \"age\"\npublic_key = \"{attacker}\"\n"
    ));
    std::fs::write(&cfg, text).unwrap();
    let mut u = unlock(&path);
    assert_eq!(u.integrity_problems()[0].code, "config_mismatch");
    // No write (which would re-encrypt the vault key to the attacker) is possible.
    let e = u
        .add_recipient(&AddRecipientParams {
            label: "x".into(),
            public_key: age::x25519::Identity::generate().to_public().to_string(),
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity);
}

#[test]
fn swapped_entry_files_are_detected() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    let p = v.create_profile(&profile_input("acme")).unwrap();
    let a = v
        .create_entry(&entry_input(
            &p.id,
            "a",
            EntryPayload::Secret(SecretData {
                value: Some("A".into()),
            }),
        ))
        .unwrap();
    let b = v
        .create_entry(&entry_input(
            &p.id,
            "b",
            EntryPayload::Secret(SecretData {
                value: Some("B".into()),
            }),
        ))
        .unwrap();
    drop(v);
    let fa = path.join(format::entry_file(&p.id, &a.id));
    let fb = path.join(format::entry_file(&p.id, &b.id));
    let (ba, bb) = (std::fs::read(&fa).unwrap(), std::fs::read(&fb).unwrap());
    std::fs::write(&fa, bb).unwrap();
    std::fs::write(&fb, ba).unwrap();
    let mut u = unlock(&path);
    assert!(!u.integrity_problems().is_empty());
    // Rebuild cannot "launder" swapped files: ids inside do not match paths.
    let r = doctor(&mut u, false, true);
    assert!(
        r.issues.iter().any(|i| i.code == "id_mismatch"),
        "{:?}",
        r.issues
    );
}

#[test]
fn plaintext_scan_and_hook() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (mut v, pid, _) = vault_with_entries(&path);
    let r = doctor(&mut v, false, false);
    assert!(r.ok, "{:?}", r.issues);

    // A stray plaintext file inside the vault.
    std::fs::write(
        path.join("profiles").join(&pid).join("notes.txt"),
        "AKIAIOSFODNN7EXAMPLE",
    )
    .unwrap();
    // A key-looking string in an allowed plaintext file.
    let mut readme = std::fs::read_to_string(path.join("README.md")).unwrap();
    readme.push_str("\nAWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n");
    std::fs::write(path.join("README.md"), readme).unwrap();
    let r = doctor(&mut v, false, false);
    assert!(!r.ok);
    let codes: Vec<_> = r
        .issues
        .iter()
        .map(|i| (i.code.as_str(), i.path.clone().unwrap_or_default()))
        .collect();
    assert!(
        codes
            .iter()
            .any(|(c, p)| *c == "plaintext_file" && p.ends_with("notes.txt")),
        "{codes:?}"
    );
    assert!(
        codes
            .iter()
            .any(|(c, p)| *c == "suspicious_content" && p == "README.md"),
        "{codes:?}"
    );
    for i in &r.issues {
        assert!(!i.message.contains("AKIA"), "{}", i.message);
    }
    // Sync refuses to run while the scan fails.
    let e = v
        .sync(&SyncParams {
            mode: SyncMode::Both,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity);

    // Staged scan (what the pre-commit hook runs) on the locked vault.
    git(&path, &["add", "-A"]);
    let locked = Vault::open(&path).unwrap();
    let r = locked.scan(true).unwrap();
    assert!(!r.ok);
    assert!(
        r.issues
            .iter()
            .any(|i| i.path.as_deref().is_some_and(|p| p.ends_with("notes.txt")))
    );

    // The installed hook blocks a manual commit (hook pointing at an executable
    // that reports failure stands in for the latch binary).
    assert!(v.git().hooks_dir().unwrap().join("pre-commit").exists());
    let fake = t.path().join("fake-latch");
    fixtures::install(FixtureConfig::ExitStatus { status: 1 }, &fake).unwrap();
    #[cfg(unix)]
    {
        v.uninstall_hook().unwrap();
        v.install_hook(Some(&fake)).unwrap();
        let out = std::process::Command::new("git")
            .args(["commit", "-qm", "manual"])
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(!out.status.success(), "hook must block the commit");
    }

    // --fix untracks/excludes the stray file; README content remains an error
    // for the user to fix by hand.
    git(&path, &["reset", "-q"]);
    git(&path, &["checkout", "--", "README.md"]);
    let r = doctor(&mut v, true, false);
    assert!(
        r.fixed.iter().any(|f| f.contains("notes.txt")),
        "{:?}",
        r.fixed
    );
    assert!(r.ok, "{:?}", r.issues);
}

#[test]
fn doctor_reports_missing_hook_and_fixes_it() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let mut v = new_vault(&path);
    assert!(v.uninstall_hook().unwrap());
    let r = doctor(&mut v, false, false);
    assert!(r.ok);
    assert!(
        r.issues
            .iter()
            .any(|i| i.code == "hook_missing" && i.severity == Severity::Warning)
    );
    let r = doctor(&mut v, true, false);
    assert!(r.fixed.iter().any(|f| f.contains("hook")));
    assert!(!r.issues.iter().any(|i| i.code == "hook_missing"));
}

#[test]
fn non_git_vault_is_read_only() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let (v, pid, eid) = vault_with_entries(&path);
    drop(v);
    let copy = t.path().join("copy");
    copy_dir(&path, &copy);
    let mut u = unlock(&copy);
    assert!(u.integrity_problems().is_empty());
    assert!(
        u.get_entry(&GetEntryParams {
            id: eid,
            reveal: true
        })
        .is_ok()
    );
    assert!(u.history(&latch::core::ids::new_id()).unwrap().is_empty());
    let e = u.create_profile(&profile_input("x")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Git);
    assert!(u.get_profile(&pid).is_ok());
    let s = u.status();
    assert!(!s.has_remote && !s.dirty);
}
