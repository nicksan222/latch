// Adversarial regression tests: a hostile git remote, a tampered work tree,
// interrupted rotations, unrelated plaintext next to the vault, `Debug`
// leaks. Each test reproduces a reviewed finding (see the test names).

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use std::path::Path;

use core_support::*;
use latch::core::{Consent, Credentials, TrustState, UnlockedVault, Vault, crypto, format, trust};
use latch::protocol::*;
use secrecy::ExposeSecret;

fn secret(s: &str) -> EntryPayload {
    EntryPayload::Secret(SecretData {
        value: Some(s.into()),
    })
}

fn sync_ok(v: &mut UnlockedVault, mode: SyncMode) {
    let r = v.sync(&SyncParams { mode }).unwrap();
    assert!(r.conflicts.is_empty());
}

struct World {
    t: tempfile::TempDir,
    r: tempfile::TempDir,
    a: UnlockedVault,
    b: UnlockedVault,
    p: String,
    e: String,
}

fn world() -> World {
    let r = bare_remote();
    let t = tempfile::tempdir().unwrap();
    let mut a = new_vault(&t.path().join("a"));
    a.set_remote(r.path().to_str().unwrap()).unwrap();
    let p = a
        .create_profile(&profile_input("sensitive-client"))
        .unwrap()
        .id;
    let e = a
        .create_entry(&entry_input(&p, "shared", secret("v0")))
        .unwrap()
        .id;
    sync_ok(&mut a, SyncMode::Both);
    let b = Vault::clone_from(r.path().to_str().unwrap(), t.path().join("b"))
        .unwrap()
        .unlock_passphrase(PASS)
        .unwrap();
    World { t, r, a, b, p, e }
}

/// Commit everything in `p` bypassing Latch and push (the hostile remote).
fn raw_commit(p: &Path) {
    git(p, &["add", "-A"]);
    git(
        p,
        &[
            "-c",
            "user.name=evil",
            "-c",
            "user.email=evil@example.com",
            "commit",
            "--no-verify",
            "-qm",
            "remote change",
        ],
    );
    git(p, &["push", "-q", "origin", "main"]);
}

fn remote_head(r: &Path) -> String {
    git(r, &["rev-parse", "main"]).trim().to_owned()
}

fn head(p: &Path) -> String {
    git(p, &["rev-parse", "HEAD"]).trim().to_owned()
}

/// Replace the key hierarchy with attacker keys: vault key + signing key
/// encrypted to the PUBLIC recipients, config/manifest consistent and
/// attacker-signed. With `own_chain` the attacker also writes a keychain
/// whose genesis is their key.
fn attacker_takeover(root: &Path, own_chain: bool) -> age::x25519::Identity {
    let mut c = format::ConfigDoc::read(root).unwrap();
    let key = crypto::SecretKey32::new([77; 32]);
    let id = crypto::vault_identity(&key);
    let sk = crypto::generate_signing_key();
    c.vault.signing_public_key = crypto::encode_verifying_key(&sk.verifying_key());
    let kd = format::VaultKeyDoc {
        version: 1,
        vault_key: crypto::b64(key.expose()),
        signing_private_key: crypto::b64(&sk.to_bytes()),
        previous_vault_keys: vec![],
        next: None,
    };
    let recipients: Vec<_> = c
        .recipients
        .iter()
        .map(|r| crypto::parse_recipient(&r.public_key).unwrap())
        .collect();
    let refs: Vec<&dyn age::Recipient> = recipients
        .iter()
        .map(|r| r.recipient.as_ref() as &dyn age::Recipient)
        .collect();
    std::fs::write(
        root.join(format::VAULTKEY_FILE),
        crypto::encrypt_to(&refs, &kd.to_json()).unwrap(),
    )
    .unwrap();
    c.write(root).unwrap();
    if own_chain {
        let g = trust::new_record(
            0,
            &c.vault.id,
            &sk.verifying_key(),
            &id.to_public().to_string(),
            None,
            &sk,
        );
        format::KeychainDoc {
            version: 1,
            records: vec![g],
        }
        .write(root)
        .unwrap();
    }
    let manifest = format::ManifestDoc {
        version: 1,
        updated_at: format::now(),
        config_hash: c.security_hash(),
        profiles: vec![],
    };
    let ct = crypto::encrypt_to_identity(&id, &serde_json::to_vec(&manifest).unwrap()).unwrap();
    std::fs::write(root.join(format::MANIFEST_FILE), &ct).unwrap();
    std::fs::write(
        root.join(format::MANIFEST_SIG_FILE),
        format!("{}\n", crypto::sign(&sk, &ct)),
    )
    .unwrap();
    std::fs::remove_dir_all(root.join("profiles")).unwrap();
    id
}

// ---------------------------------------------------------------------------
// Finding 1: hostile remote replaces the key hierarchy
// ---------------------------------------------------------------------------

#[test]
fn hostile_remote_cannot_replace_key_hierarchy() {
    for own_chain in [false, true] {
        let mut w = world();
        let before = head(w.b.path());
        let attacker = attacker_takeover(w.a.path(), own_chain);
        drop(w.a);
        raw_commit(&w.t.path().join("a"));
        for mode in [SyncMode::PullOnly, SyncMode::Both] {
            let e = w.b.sync(&SyncParams { mode }).unwrap_err();
            assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
        }
        assert_eq!(head(w.b.path()), before, "nothing merged");
        assert!(w.b.integrity_problems().is_empty());
        // New secrets still use the trusted key.
        let e =
            w.b.create_entry(&entry_input(&w.p, "new secret", secret("future")))
                .unwrap();
        let ct = std::fs::read(w.b.path().join(format::entry_file(&w.p, &e.id))).unwrap();
        assert!(crypto::decrypt_with(&[&attacker as &dyn age::Identity], &ct).is_err());

        // Pulling it in by hand: unlock refuses (pinned key history).
        git(w.b.path(), &["fetch", "-q", "origin"]);
        git(w.b.path(), &["reset", "-q", "--hard", "origin/main"]);
        let err = Vault::open(w.b.path())
            .unwrap()
            .unlock_passphrase(PASS)
            .unwrap_err();
        assert_eq!(err.code(), ErrorCode::Integrity, "{err}");
        // The live handle notices before its next write too.
        let err =
            w.b.create_profile(&profile_input("after-takeover"))
                .unwrap_err();
        assert_eq!(err.code(), ErrorCode::Integrity);
        // Doctor reports and repairs nothing with untrusted keys.
        let r =
            w.b.doctor(&DoctorParams {
                fix: true,
                rebuild_manifest: true,
                staged: false,
            })
            .unwrap();
        assert!(!r.ok && r.fixed.is_empty(), "{:?}", r);
        assert!(r.issues.iter().any(|i| i.code == "untrusted_state"));
    }
}

#[test]
fn trust_on_first_use_fingerprint_and_explicit_reset() {
    let mut w = world();
    let fp = w.a.fingerprint();
    assert_eq!(w.b.fingerprint(), fp);
    assert_eq!(w.b.trust_info().key_seq, 0);
    let doc = Vault::init(&{
        let mut p = init_params(&w.t.path().join("kit"));
        p.recovery_kit = true;
        p
    })
    .unwrap();
    let kit = doc.1.recovery_kit.unwrap();
    assert!(kit.document.contains(&doc.0.fingerprint()));

    // A legitimate rotation is a signed extension: accepted and pinned.
    w.a.rotate().unwrap();
    sync_ok(&mut w.a, SyncMode::Both);
    sync_ok(&mut w.b, SyncMode::Both);
    assert_eq!(w.b.trust_info().key_seq, 1);
    assert_eq!(w.b.fingerprint(), fp, "stable across rotations");

    // Forgetting the pin = next unlock is trust-on-first-use.
    let locked = Vault::open(w.b.path()).unwrap();
    assert!(locked.forget_trust().unwrap());
    assert_eq!(locked.trust_info().unwrap().state, TrustState::FirstUse);
    let u = locked.unlock_passphrase(PASS).unwrap();
    assert_eq!(u.trust_info().state, TrustState::FirstUse);
    assert_eq!(u.fingerprint(), fp);
    assert_eq!(
        Vault::open(w.b.path()).unwrap().trust_info().unwrap().state,
        TrustState::Pinned
    );
    drop(w.r);
}

#[test]
fn upstream_rollback_of_a_rotation_is_refused() {
    let mut w = world();
    w.a.rotate().unwrap();
    sync_ok(&mut w.a, SyncMode::Both);
    sync_ok(&mut w.b, SyncMode::Both);
    // The remote restores the pre-rotation key files (signed by the old key).
    let a = w.a.path().to_owned();
    drop(w.a);
    git(&a, &["checkout", "HEAD~1", "--", ".latch", "profiles"]);
    raw_commit(&a);
    let e =
        w.b.sync(&SyncParams {
            mode: SyncMode::Both,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert!(e.to_string().contains("rolls back"), "{e}");
}

#[test]
fn changed_vault_id_is_refused() {
    let w = world();
    let a = w.a.path().to_owned();
    drop(w.a);
    let mut c = format::ConfigDoc::read(&a).unwrap();
    c.vault.id = latch::core::ids::new_id();
    c.write(&a).unwrap();
    let e = Vault::open(&a)
        .unwrap()
        .unlock_passphrase(PASS)
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
}

#[test]
fn replaced_vaultkey_is_detected_before_writes() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    // Attacker key material encrypted to the public recipients.
    let c = format::ConfigDoc::read(v.path()).unwrap();
    let kd = format::VaultKeyDoc {
        version: 1,
        vault_key: crypto::b64(&[9u8; 32]),
        signing_private_key: crypto::b64(&[8u8; 32]),
        previous_vault_keys: vec![],
        next: None,
    };
    let r = crypto::parse_recipient(&c.recipients[0].public_key).unwrap();
    std::fs::write(
        v.path().join(format::VAULTKEY_FILE),
        crypto::encrypt_to(&[r.recipient.as_ref()], &kd.to_json()).unwrap(),
    )
    .unwrap();
    let e = v.create_profile(&profile_input("x")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert!(!v.integrity_problems().is_empty());
}

// ---------------------------------------------------------------------------
// Finding 2: rebase / fast-forward legitimizes an injected recipient
// ---------------------------------------------------------------------------

fn inject_recipient(a: &mut UnlockedVault, p: &str) -> age::x25519::Identity {
    a.create_entry(&entry_input(p, "remote", secret("remote")))
        .unwrap();
    let attacker = age::x25519::Identity::generate();
    let mut c = format::ConfigDoc::read(a.path()).unwrap();
    c.recipients.push(format::RecipientDoc {
        label: "evil".into(),
        kind: "age".into(),
        public_key: attacker.to_public().to_string(),
        added_at: None,
    });
    c.write(a.path()).unwrap();
    raw_commit(a.path());
    attacker
}

#[test]
fn injected_recipient_is_rejected_on_rebase() {
    let mut w = world();
    let attacker = inject_recipient(&mut w.a, &w.p);
    let local =
        w.b.create_entry(&entry_input(&w.p, "local", secret("local")))
            .unwrap();
    let before = head(w.b.path());
    let e =
        w.b.sync(&SyncParams {
            mode: SyncMode::Both,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert_eq!(head(w.b.path()), before);
    assert!(!w.b.git().rebase_in_progress());
    assert!(!w.b.recipients().iter().any(|r| r.label == "evil"));
    // A later recipient change does not encrypt the vault key to the attacker.
    w.b.add_recipient(&AddRecipientParams {
        label: "innocent".into(),
        public_key: age::x25519::Identity::generate().to_public().to_string(),
    })
    .unwrap();
    let idf = w.t.path().join("attacker.key");
    std::fs::write(&idf, attacker.to_string().expose_secret()).unwrap();
    assert!(
        Vault::open(w.b.path())
            .unwrap()
            .unlock(Credentials::IdentityFile(idf))
            .is_err()
    );
    assert!(w.b.find_entry(&w.p, &local.id).is_ok());
}

#[test]
fn injected_recipient_is_rejected_on_fast_forward() {
    let mut w = world();
    inject_recipient(&mut w.a, &w.p);
    let before = head(w.b.path());
    let e =
        w.b.sync(&SyncParams {
            mode: SyncMode::PullOnly,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert_eq!(head(w.b.path()), before);
    assert!(!w.b.recipients().iter().any(|r| r.label == "evil"));
}

#[test]
fn doctor_does_not_launder_an_injected_recipient() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("v");
    let v = new_vault(&path);
    drop(v);
    let mut c = format::ConfigDoc::read(&path).unwrap();
    c.recipients.push(format::RecipientDoc {
        label: "evil".into(),
        kind: "age".into(),
        public_key: age::x25519::Identity::generate().to_public().to_string(),
        added_at: None,
    });
    c.write(&path).unwrap();
    let mut u = unlock(&path);
    assert_eq!(u.integrity_problems()[0].code, "config_mismatch");
    let r = u
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: true,
            staged: false,
        })
        .unwrap();
    assert!(
        r.issues.iter().any(|i| i.code == "config_untrusted"),
        "{r:?}"
    );
    assert!(!u.integrity_problems().is_empty(), "still blocked");
    // --fix restores the authenticated config from HEAD instead.
    let r = u
        .doctor(&DoctorParams {
            fix: true,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
    assert!(!u.recipients().iter().any(|r| r.label == "evil"));
}

// ---------------------------------------------------------------------------
// Finding 3: unsigned settings / local consent
// ---------------------------------------------------------------------------

#[test]
fn remote_cannot_enable_names_in_commits_or_auto_push() {
    let mut w = world();
    // Unsigned edit by the remote: refused outright.
    let mut c = format::ConfigDoc::read(w.a.path()).unwrap();
    c.sync.names_in_commits = true;
    c.sync.auto_push = true;
    c.write(w.a.path()).unwrap();
    raw_commit(w.a.path());
    let e =
        w.b.sync(&SyncParams {
            mode: SyncMode::PullOnly,
        })
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    let pushed = remote_head(w.r.path());
    w.b.create_entry(&entry_input(&w.p, "private-service", secret("s")))
        .unwrap();
    let msg = git(w.b.path(), &["log", "-1", "--format=%s"]);
    assert!(!msg.contains("private-service") && !msg.contains("sensitive-client"));
    assert_eq!(remote_head(w.r.path()), pushed, "no auto-push");
}

#[test]
fn signed_settings_from_a_teammate_need_local_consent() {
    let mut w = world();
    w.a.set_vault_settings(true, true, false).unwrap();
    assert!(w.a.effective_settings().names_in_commits);
    sync_ok(&mut w.a, SyncMode::Both);
    sync_ok(&mut w.b, SyncMode::PullOnly);
    assert!(w.b.config().sync.names_in_commits && w.b.config().sync.auto_push);
    assert_eq!(w.b.effective_settings(), Consent::default());
    let pushed = remote_head(w.r.path());
    w.b.create_entry(&entry_input(&w.p, "private-service", secret("s")))
        .unwrap();
    let msg = git(w.b.path(), &["log", "-1", "--format=%s"]);
    assert!(!msg.contains("private-service"), "{msg}");
    assert_eq!(
        remote_head(w.r.path()),
        pushed,
        "no auto-push without consent"
    );
    // Local opt-in.
    w.b.set_local_consent(Consent {
        names_in_commits: true,
        auto_push: false,
        sessions_commit: false,
    })
    .unwrap();
    w.b.create_entry(&entry_input(&w.p, "named", secret("s")))
        .unwrap();
    let msg = git(w.b.path(), &["log", "-1", "--format=%s"]);
    assert!(msg.contains("named"), "{msg}");
    assert_eq!(remote_head(w.r.path()), pushed);
}

// ---------------------------------------------------------------------------
// Finding 4: commits contain only vault paths
// ---------------------------------------------------------------------------

#[test]
fn unrelated_staged_and_untracked_plaintext_is_never_committed() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(t.path());
    std::fs::write(t.path().join(".latch/leak.txt"), "secret-sentinel").unwrap();
    std::fs::write(t.path().join("unrelated.txt"), "staged-sentinel").unwrap();
    git(t.path(), &["add", "unrelated.txt"]);
    v.create_profile(&profile_input("test")).unwrap();
    let files = git(t.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        !files.contains("leak.txt") && !files.contains("unrelated.txt"),
        "{files}"
    );
    assert!(files.contains("profile.age"));
    // The user's index keeps their staged file; Latch paths are in sync.
    let staged = git(t.path(), &["diff", "--cached", "--name-only"]);
    assert_eq!(staged.trim(), "unrelated.txt");
    let st = git(t.path(), &["status", "--porcelain"]);
    assert!(!st.contains("profiles/"), "{st}");
}

#[test]
fn foreign_tracked_file_or_secret_content_blocks_commits() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(t.path());
    // A plaintext file committed by hand: Latch refuses to build on it.
    std::fs::write(t.path().join("notes.txt"), "x").unwrap();
    git(t.path(), &["add", "notes.txt"]);
    git(
        t.path(),
        &[
            "-c",
            "user.name=u",
            "-c",
            "user.email=u@e",
            "commit",
            "--no-verify",
            "-qm",
            "manual",
        ],
    );
    let e = v.create_profile(&profile_input("p")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    let r = v
        .doctor(&DoctorParams {
            fix: true,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.fixed.iter().any(|f| f.contains("notes.txt")), "{r:?}");
    let files = git(t.path(), &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(!files.contains("notes.txt"));
    v.create_profile(&profile_input("q")).unwrap();
    // A key-looking string in a vault plaintext file is never committed.
    let mut readme = std::fs::read_to_string(t.path().join("README.md")).unwrap();
    readme.push_str("\nAWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n");
    std::fs::write(t.path().join("README.md"), readme).unwrap();
    let e = v.create_profile(&profile_input("r")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert!(!git(t.path(), &["show", "HEAD:README.md"]).contains("AKIA"));
}

// ---------------------------------------------------------------------------
// Finding 5: live integrity checks cover every authenticated file
// ---------------------------------------------------------------------------

#[test]
fn forged_signature_or_keychain_blocks_writes() {
    for file in [format::MANIFEST_SIG_FILE, format::KEYCHAIN_FILE] {
        let t = tempfile::tempdir().unwrap();
        let mut v = new_vault(t.path());
        std::fs::write(t.path().join(file), "forged").unwrap();
        let e = v.create_profile(&profile_input("test")).unwrap_err();
        assert_eq!(e.code(), ErrorCode::Integrity, "{file}: {e}");
        assert!(!v.integrity_problems().is_empty());
        // Not silently repaired: still forged on disk, still blocked.
        assert_eq!(std::fs::read(t.path().join(file)).unwrap(), b"forged");
        assert!(v.create_profile(&profile_input("again")).is_err());
    }
}

#[test]
fn signature_only_tampering_stays_detected_after_reopen() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(t.path());
    std::fs::write(t.path().join(format::MANIFEST_SIG_FILE), "forged").unwrap();
    assert!(v.create_profile(&profile_input("test")).is_err());
    drop(v);
    assert!(!unlock(t.path()).integrity_problems().is_empty());
}

// ---------------------------------------------------------------------------
// Findings 6 + 8: rotation ordering and archive validation
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn chmod(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn interrupted_rotation_keeps_archives_and_is_completed_by_doctor() {
    // Interrupt at two points: while re-encrypting local session archives
    // and while re-encrypting entries (after the keychain/config switched).
    for stage in ["archives", "entries"] {
        let t = tempfile::tempdir().unwrap();
        let mut v = new_vault(&t.path().join("vault"));
        let p = v.create_profile(&profile_input("p")).unwrap().id;
        let e = v
            .create_entry(&entry_input(&p, "s", secret("value-1")))
            .unwrap()
            .id;
        let dir = latch::core::paths::sessions_dir(v.id());
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join(format!("{}.tar.zst.age", latch::core::ids::new_id()));
        std::fs::write(&archive, v.encrypt_blob(b"unique local session").unwrap()).unwrap();
        let locked_dir = match stage {
            "archives" => dir.clone(),
            _ => v.path().join(format::profile_dir(&p)).join("entries"),
        };
        chmod(&locked_dir, 0o555);
        assert!(v.rotate().is_err(), "{stage}");
        chmod(&locked_dir, 0o755);
        let root = v.path().to_owned();
        drop(v);

        let mut u = unlock(&root);
        assert!(u.rotation_pending(), "{stage}");
        // Every archive is still decryptable.
        assert_eq!(
            &u.decrypt_blob(&std::fs::read(&archive).unwrap()).unwrap()[..],
            b"unique local session"
        );
        let r = u
            .doctor(&DoctorParams {
                fix: true,
                rebuild_manifest: false,
                staged: false,
            })
            .unwrap();
        assert!(r.ok, "{stage}: {:?}", r.issues);
        assert!(
            r.fixed.iter().any(|f| f.contains("rotation")),
            "{stage}: {:?}",
            r.fixed
        );
        assert!(!u.rotation_pending());
        assert_eq!(u.trust_info().key_seq, 1);
        let u = unlock(&root);
        assert!(
            u.integrity_problems().is_empty(),
            "{:?}",
            u.integrity_problems()
        );
        assert_eq!(
            u.reveal(&RevealParams {
                id: e.clone(),
                field: "value".into()
            })
            .unwrap()
            .value,
            "value-1"
        );
        let ct = std::fs::read(&archive).unwrap();
        assert_eq!(&u.decrypt_blob(&ct).unwrap()[..], b"unique local session");
        // Archive and entry are under the NEW key now.
        let new_only = format::VaultKeyDoc::from_json(
            &crypto::decrypt_with(
                &[&crypto::derive_passphrase_identity(
                    &secrecy::SecretString::from(PASS.to_owned()),
                    &u.config().kdf.salt_bytes().unwrap(),
                    &u.config().kdf.params(),
                )
                .unwrap() as &dyn age::Identity],
                &std::fs::read(root.join(format::VAULTKEY_FILE)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(new_only.next.is_none());
        let k = crypto::SecretKey32::new(
            crypto::b64_decode(&new_only.vault_key)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let id = crypto::vault_identity(&k);
        assert!(crypto::decrypt_with(&[&id as &dyn age::Identity], &ct).is_ok());
    }
}

#[test]
fn rotation_aborts_on_an_unreadable_archive() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(t.path());
    v.create_profile(&profile_input("p")).unwrap();
    std::fs::create_dir_all(t.path().join("sessions")).unwrap();
    let path = t.path().join("sessions/test.tar.zst.age");
    std::fs::write(&path, b"damaged archive").unwrap();
    let vk = std::fs::read(t.path().join(format::VAULTKEY_FILE)).unwrap();
    let e = v.rotate().unwrap_err();
    assert_eq!(e.code(), ErrorCode::Integrity, "{e}");
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged archive");
    assert_eq!(
        std::fs::read(t.path().join(format::VAULTKEY_FILE)).unwrap(),
        vk
    );
    assert!(!v.rotation_pending());
    assert_eq!(v.trust_info().key_seq, 0);
}

#[test]
fn failed_rotation_does_not_lose_local_archives() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("vault"));
    v.create_profile(&profile_input("p")).unwrap();
    let dir = latch::core::paths::sessions_dir(v.id());
    std::fs::create_dir_all(&dir).unwrap();
    let archive = dir.join("test.tar.zst.age");
    std::fs::write(&archive, v.encrypt_blob(b"unique local session").unwrap()).unwrap();
    let original = std::fs::read(v.path().join(format::VAULTKEY_FILE)).unwrap();
    std::fs::remove_file(v.path().join(format::VAULTKEY_FILE)).unwrap();
    std::fs::create_dir(v.path().join(format::VAULTKEY_FILE)).unwrap();
    assert!(v.rotate().is_err());
    let root = v.path().to_owned();
    drop(v);
    std::fs::remove_dir(root.join(format::VAULTKEY_FILE)).unwrap();
    std::fs::write(root.join(format::VAULTKEY_FILE), original).unwrap();
    let v = unlock(&root);
    assert_eq!(
        &v.decrypt_blob(&std::fs::read(&archive).unwrap()).unwrap()[..],
        b"unique local session"
    );
}

// ---------------------------------------------------------------------------
// Finding 9: pull-only conflict resolution must not push
// ---------------------------------------------------------------------------

#[test]
fn resolving_a_pull_only_sync_does_not_push() {
    let mut w = world();
    let (p, e) = (w.p.clone(), w.e.clone());
    w.a.update_entry(&EntryUpdateParams {
        id: e.clone(),
        input: entry_input(&p, "shared", secret("a")),
    })
    .unwrap();
    sync_ok(&mut w.a, SyncMode::Both);
    let before = remote_head(w.r.path());
    w.b.update_entry(&EntryUpdateParams {
        id: e.clone(),
        input: entry_input(&p, "shared", secret("b")),
    })
    .unwrap();
    let r =
        w.b.sync(&SyncParams {
            mode: SyncMode::PullOnly,
        })
        .unwrap();
    assert_eq!(r.conflicts.len(), 1);
    let st =
        w.b.resolve(&ResolveParams {
            entry_id: e,
            resolution: Resolution::Mine,
            merged: None,
        })
        .unwrap();
    assert!(st.conflicts.is_empty());
    assert_eq!(remote_head(w.r.path()), before, "pull-only must not push");
    assert_eq!(st.ahead, 1);
}

// ---------------------------------------------------------------------------
// Finding 10: Debug never prints metadata
// ---------------------------------------------------------------------------

#[test]
fn debug_output_has_no_names_or_secrets() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(t.path());
    let mut pi = profile_input("SENTINEL-PROFILE");
    pi.description = "SENTINEL-DESCRIPTION".into();
    pi.browser.bookmarks[0].title = "SENTINEL-BOOKMARK".into();
    pi.browser.start_pages = vec!["https://sentinel-start.example".into()];
    let p = v.create_profile(&pi).unwrap();
    v.create_entry(&entry_input(
        &p.id,
        "SENTINEL-ENTRY",
        EntryPayload::Login(LoginData {
            username: "SENTINEL-USER".into(),
            password: Some("SENTINEL-PASSWORD".into()),
            urls: vec!["https://sentinel-url.example".into()],
            totp: None,
            custom: vec![],
        }),
    ))
    .unwrap();
    let profile_doc = format::ProfileDoc {
        version: 1,
        id: p.id.clone(),
        name: "SENTINEL-PROFILE".into(),
        color: "#000000".into(),
        icon: "x".into(),
        description: "SENTINEL-DESCRIPTION".into(),
        pinned: false,
        browser: format::BrowserDoc::from_protocol(&pi.browser),
        actions: vec![],
        created_at: format::now(),
        updated_at: format::now(),
    };
    let dumps = [
        format!("{:?}", v.manifest()),
        format!("{:#?}", v.manifest()),
        format!("{profile_doc:?}"),
        format!("{:?}", profile_doc.browser),
        format!("{v:?}"),
    ];
    for d in &dumps {
        for s in [
            "SENTINEL-PROFILE",
            "SENTINEL-ENTRY",
            "SENTINEL-USER",
            "SENTINEL-PASSWORD",
            "SENTINEL-DESCRIPTION",
            "SENTINEL-BOOKMARK",
            "sentinel-url",
            "sentinel-start",
        ] {
            assert!(!d.contains(s), "{s} leaked in {d}");
        }
    }
    assert!(dumps[0].contains(&p.id), "ids are fine: {}", dumps[0]);
}
