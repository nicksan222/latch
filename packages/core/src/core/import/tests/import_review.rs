// Regression tests for import review findings: all-or-nothing apply,
// plaintext archive version check, bounded expansion (KeePass gzip
// attachments, pass/gpg output), KeePass nesting limits.

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use std::path::Path;

use base64::Engine;
use latch::protocol::*;
use support::*;

fn params(format: ImportFormat, path: &Path) -> ImportParams {
    ImportParams {
        format,
        path: path.display().to_string(),
        profile_id: None,
        skip_duplicates: false,
        passphrase: None,
    }
}

#[test]
fn failed_import_commits_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut source = new_vault(&t.path().join("source"));
    for name in ["alpha", "beta"] {
        let p = source.create_profile(&profile_input(name)).unwrap();
        source
            .create_entry(&entry_input(
                &p.id,
                "secret",
                EntryPayload::Secret(SecretData {
                    value: Some("pw".into()),
                }),
            ))
            .unwrap();
    }
    let mut doc = latch::core::export::build_archive(&source, None).unwrap();
    doc.profiles[1].color = "invalid".into();
    let f = t.path().join("export.json");
    std::fs::write(&f, serde_json::to_vec(&doc).unwrap()).unwrap();
    let mut target = new_vault(&t.path().join("target"));
    let head = git(target.path(), &["rev-parse", "HEAD"]);
    let err = latch::core::import::apply(&mut target, &params(ImportFormat::Json, &f)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidParams);
    let all = ListEntriesParams {
        profile_id: None,
        query: None,
        entry_type: None,
    };
    assert!(target.list_entries(&all).unwrap().is_empty());
    assert!(target.list_profiles().unwrap().is_empty());
    assert_eq!(git(target.path(), &["rev-parse", "HEAD"]), head);
    assert!(git(target.path(), &["status", "--porcelain"]).is_empty());
    // The handle stays healthy and writable.
    assert!(target.integrity_problems().is_empty());
    target.create_profile(&profile_input("after")).unwrap();

    // A valid archive imports, with autofill selections restored in the
    // same single commit.
    let mut doc = latch::core::export::build_archive(&source, None).unwrap();
    let alpha_entry = doc
        .entries
        .iter()
        .find(|e| e.profile_id == doc.profiles[0].id)
        .unwrap()
        .id
        .clone();
    doc.profiles[0].browser.autofill = AutofillScope::Selected;
    doc.profiles[0].browser.autofill_entry_ids = vec![alpha_entry];
    std::fs::write(&f, serde_json::to_vec(&doc).unwrap()).unwrap();
    let before = git(target.path(), &["rev-list", "--count", "HEAD"]);
    let r = latch::core::import::apply(&mut target, &params(ImportFormat::Json, &f)).unwrap();
    assert_eq!((r.created, r.profiles_created), (2, 2));
    let after = git(target.path(), &["rev-list", "--count", "HEAD"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
    assert!(git(target.path(), &["status", "--porcelain"]).is_empty());
    let alpha = target.find_profile(&doc.profiles[0].name).unwrap();
    let p = target.get_profile(&alpha.id).unwrap();
    assert_eq!(p.browser.autofill_entry_ids.len(), 1);
}

#[test]
fn plaintext_archive_version_is_checked() {
    let t = tempfile::tempdir().unwrap();
    let v = new_vault(&t.path().join("vault"));
    let mut doc = latch::core::export::build_archive(&v, None).unwrap();
    doc.version = 99;
    let f = t.path().join("future.json");
    std::fs::write(&f, serde_json::to_vec(&doc).unwrap()).unwrap();
    let err = latch::core::import::preview(&v, &params(ImportFormat::Json, &f)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::UnsupportedVersion);
}

fn keepass_xml(groups: &str, entry_extra: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><KeePassFile><Meta></Meta><Root><Group><Name>Root</Name>{groups}<Entry><String><Key>Title</Key><Value>T</Value></String>{entry_extra}</Entry></Group></Root></KeePassFile>"#
    )
}

#[test]
fn keepass_gzip_bomb_is_bounded() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let v = new_vault(&t.path().join("vault"));
    let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::best());
    std::io::Write::write_all(&mut gz, &vec![0u8; 256 * 1024 * 1024]).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(gz.finish().unwrap());
    let xml = keepass_xml(
        "",
        &format!(r#"<Binary><Key>bomb.bin</Key><Value Compressed="True">{b64}</Value></Binary>"#),
    );
    let f = t.path().join("kp.xml");
    std::fs::write(&f, xml).unwrap();
    let p = latch::core::import::preview(&v, &params(ImportFormat::KeepassXml, &f)).unwrap();
    assert!(
        p.warnings.iter().any(|w| w.contains("bomb.bin")),
        "{:?}",
        p.warnings
    );
    assert!(p.items.iter().all(|i| i.entry_type != EntryType::File));
}

#[test]
fn keepass_deep_nesting_is_rejected_without_overflow() {
    setup();
    let t = tempfile::tempdir().unwrap();
    let v = new_vault(&t.path().join("vault"));
    let n = 50_000;
    let groups = format!(
        "{}{}",
        "<Group><Name>g</Name>".repeat(n),
        "</Group>".repeat(n)
    );
    let f = t.path().join("deep.xml");
    std::fs::write(&f, keepass_xml(&groups, "")).unwrap();
    let err = latch::core::import::preview(&v, &params(ImportFormat::KeepassXml, &f)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidParams);
    // Moderate nesting still imports (iterative traversal).
    let n = 60;
    let groups = format!(
        "{}<Entry><String><Key>Title</Key><Value>deep</Value></String></Entry>{}",
        "<Group><Name>g</Name>".repeat(n),
        "</Group>".repeat(n)
    );
    std::fs::write(&f, keepass_xml(&groups, "")).unwrap();
    let p = latch::core::import::preview(&v, &params(ImportFormat::KeepassXml, &f)).unwrap();
    assert!(p.items.iter().any(|i| i.name == "deep"));
}

#[test]
fn pass_gpg_output_is_bounded() {
    use std::os::unix::fs::PermissionsExt;
    setup();
    let t = tempfile::tempdir().unwrap();
    let v = new_vault(&t.path().join("vault"));
    // A "gpg" that never stops writing.
    let gpg = t.path().join("gpg");
    std::fs::write(&gpg, "#!/bin/sh\nexec yes 0123456789abcdef\n").unwrap();
    std::fs::set_permissions(&gpg, std::fs::Permissions::from_mode(0o755)).unwrap();
    let store = t.path().join("store");
    std::fs::create_dir_all(store.join("acme")).unwrap();
    std::fs::write(store.join("acme/huge.gpg"), b"x").unwrap();
    // This test binary owns LATCH_GPG (no other test reads it).
    // SAFETY: set before the parse below; no other thread reads LATCH_GPG.
    unsafe { std::env::set_var("LATCH_GPG", &gpg) };
    let p = latch::core::import::preview(&v, &params(ImportFormat::Pass, &store)).unwrap();
    assert!(p.items.is_empty());
    assert!(
        p.warnings.iter().any(|w| w.contains("acme/huge")),
        "{:?}",
        p.warnings
    );
}
