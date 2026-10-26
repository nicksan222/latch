// Format compatibility against the committed fixture `fixtures/vault-v1`.
//
// The fixture is a v1 vault with weak KDF parameters and the passphrase
// `correct horse battery staple`, two profiles (`acme`, `personal`) and one
// entry of every type in `acme` (see `core_support::all_payloads`) plus a
// favorite env entry in `personal`. It is stored WITHOUT a `.git` directory:
// reading works on a non-git directory (read-only mode); the write test
// copies it to a temp dir and runs `git init` first.
//
// Regenerate (only when intentionally changing the fixture):
// `LATCH_ALLOW_WEAK_KDF=1 cargo test --no-default-features --test core_fixture -- --ignored generate_fixture`

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::Vault;
use latch::protocol::*;

#[test]
#[ignore = "regenerates fixtures/vault-v1"]
fn generate_fixture() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("vault");
    let mut v = new_vault(&path);
    let acme = v.create_profile(&profile_input("acme")).unwrap();
    let mut personal = profile_input("personal");
    personal.pinned = true;
    personal.color = "#16a34a".into();
    let personal = v.create_profile(&personal).unwrap();
    for (name, payload) in all_payloads() {
        v.create_entry(&entry_input(&acme.id, name, payload))
            .unwrap();
    }
    let mut env = entry_input(
        &personal.id,
        "Home env",
        EntryPayload::Env(EnvData {
            vars: latch::core::environment::parse(
                "# token\nHOME_TOKEN=\"multi\nline\"\nEDITOR=nvim\n",
            )
            .unwrap(),
        }),
    );
    env.favorite = true;
    v.create_entry(&env).unwrap();
    let r = v
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
    drop(v);
    let dest = fixture_dir();
    let _ = std::fs::remove_dir_all(&dest);
    copy_dir(&path, &dest);
}

#[test]
fn fixture_opens_and_reads() {
    setup();
    let dir = fixture_dir();
    let vault = Vault::open(&dir).expect("fixture present (run generate_fixture)");
    assert_eq!(vault.config().version, 1);
    assert!(vault.status().locked);
    assert!(matches!(
        vault.unlock_passphrase("wrong passphrase"),
        Err(latch::core::Error::BadCredentials)
    ));
    let v = vault.unlock_passphrase(PASS).unwrap();
    assert!(
        v.integrity_problems().is_empty(),
        "{:?}",
        v.integrity_problems()
    );

    let profiles = v.list_profiles().unwrap();
    assert_eq!(
        profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["personal", "acme"]
    );
    let acme = v.find_profile("acme").unwrap();
    assert_eq!(acme.entry_count, 8);
    let full = v.get_profile(&acme.id).unwrap();
    assert_eq!(full.browser.bookmarks[0].url, "https://acme.atlassian.net");

    for (name, payload) in all_payloads() {
        let s = v.find_entry(&acme.id, name).unwrap();
        let e = v
            .get_entry(&GetEntryParams {
                id: s.id.clone(),
                reveal: true,
            })
            .unwrap();
        let mut expected = payload;
        if let EntryPayload::File(f) = &mut expected {
            f.size = 20;
        }
        assert_eq!(e.payload, expected, "{name}");
        assert!(
            !v.get_entry(&GetEntryParams {
                id: s.id,
                reveal: false
            })
            .unwrap()
            .redacted_fields
            .is_empty()
        );
    }
    let personal = v.find_profile("personal").unwrap();
    let env = v
        .env_resolve(&EnvResolveParams {
            profile_id: personal.id.clone(),
            entry_ids: None,
        })
        .unwrap();
    assert_eq!(env.vars[0].key, "HOME_TOKEN");
    assert_eq!(env.vars[0].value, "multi\nline");
    let gh = v.find_entry(&acme.id, "GitHub").unwrap();
    assert_eq!(v.totp(&gh.id).unwrap().code.len(), 6);
    // Read-only: no git repo.
    let scan = Vault::open(&dir).unwrap().scan(false).unwrap();
    assert!(scan.ok, "{:?}", scan.issues);
}

#[test]
fn fixture_copy_is_writable_after_git_init() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("v");
    copy_dir(&fixture_dir(), &dir);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(
        &dir,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "add",
            "-A",
        ],
    );
    git(
        &dir,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-qm",
            "import fixture",
        ],
    );
    let mut v = unlock(&dir);
    let acme = v.find_profile("acme").unwrap();
    let e = v
        .create_entry(&entry_input(
            &acme.id,
            "new",
            EntryPayload::Secret(SecretData {
                value: Some("x".into()),
            }),
        ))
        .unwrap();
    assert_eq!(v.history(&e.id).unwrap().len(), 1);
    let r = v
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
}
