// Exporters: Latch archive / JSON / dotenv, permissions, vault-path guard,
// and export → import round trips.

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use latch::core::{UnlockedVault, export, import};
use latch::protocol::*;
use support::*;

const ARCHIVE_PASS: &str = "fixture passphrase";

fn fixture_archive() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/import/archive.latch.age")
}

/// A vault with two profiles and every entry type in `acme`.
fn populated(dir: &Path) -> (UnlockedVault, String) {
    let mut v = new_vault(&dir.join("v"));
    let acme = v.create_profile(&profile_input("acme")).unwrap().id;
    let other = v.create_profile(&profile_input("other")).unwrap().id;
    let mut ids = vec![];
    for (name, payload) in all_payloads() {
        ids.push(
            v.create_entry(&entry_input(&acme, name, payload))
                .unwrap()
                .id,
        );
    }
    v.create_entry(&entry_input(
        &other,
        "Other env",
        EntryPayload::Env(EnvData {
            vars: vec![EnvVar {
                key: "TOKEN".into(),
                value: Some("other-token".into()),
                hidden: true,
                comment: None,
            }],
        }),
    ))
    .unwrap();
    // Autofill restricted to the GitHub login: must survive a round trip.
    let p = v.get_profile(&acme).unwrap();
    let mut input = profile_input("acme");
    input.browser.autofill = AutofillScope::Selected;
    input.browser.autofill_entry_ids = vec![ids[0].clone()];
    input.color = p.color;
    v.update_profile(&ProfileUpdateParams {
        id: acme.clone(),
        input,
    })
    .unwrap();
    (v, acme)
}

fn export_params(format: ExportFormat, path: &Path) -> ExportParams {
    ExportParams {
        format,
        path: path.display().to_string(),
        profile_id: None,
        passphrase: Some(ARCHIVE_PASS.into()),
    }
}

fn all_revealed(v: &UnlockedVault) -> Vec<(String, String, EntryPayload, Vec<String>, String)> {
    let mut out = vec![];
    for p in v.list_profiles().unwrap() {
        for s in v
            .list_entries(&ListEntriesParams {
                profile_id: Some(p.id.clone()),
                query: None,
                entry_type: None,
            })
            .unwrap()
        {
            let e = v
                .get_entry(&GetEntryParams {
                    id: s.id,
                    reveal: true,
                })
                .unwrap();
            out.push((p.name.clone(), e.name, e.payload, e.tags, e.notes));
        }
    }
    out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    out
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn archive_round_trip() {
    let t = tempfile::tempdir().unwrap();
    let (src, _) = populated(t.path());
    let out = t.path().join("backup.latch.age");
    let r = export::run(&src, &export_params(ExportFormat::LatchArchive, &out)).unwrap();
    assert_eq!(r.entries, 9);
    #[cfg(unix)]
    assert_eq!(mode(&out), 0o600);
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1"));
    let text = String::from_utf8_lossy(&bytes);
    for m in SECRET_MARKERS {
        assert!(!text.contains(m), "archive leaks {m}");
    }

    // Wrong passphrase.
    let mut p = ImportParams {
        format: ImportFormat::LatchArchive,
        path: out.display().to_string(),
        profile_id: None,
        skip_duplicates: true,
        passphrase: Some("wrong passphrase".into()),
    };
    let t2 = tempfile::tempdir().unwrap();
    let mut dst = new_vault(&t2.path().join("v"));
    assert_eq!(
        import::preview(&dst, &p).unwrap_err().code(),
        ErrorCode::BadCredentials
    );
    p.passphrase = None;
    assert_eq!(
        import::preview(&dst, &p).unwrap_err().code(),
        ErrorCode::InvalidParams
    );

    p.passphrase = Some(ARCHIVE_PASS.into());
    let r = import::apply(&mut dst, &p).unwrap();
    assert_eq!((r.created, r.profiles_created), (9, 2));
    assert_eq!(all_revealed(&src), all_revealed(&dst));
    // Profile settings and autofill selection survive.
    let acme = dst.find_profile("acme").unwrap();
    let prof = dst.get_profile(&acme.id).unwrap();
    assert_eq!(prof.browser.bookmarks.len(), 1);
    assert_eq!(prof.browser.autofill, AutofillScope::Selected);
    let gh = dst.find_entry(&acme.id, "GitHub").unwrap();
    assert_eq!(prof.browser.autofill_entry_ids, [gh.id]);
    // Re-import into the source: all duplicates.
    let pv = import::preview(&src, &p).unwrap();
    assert!(pv.items.iter().all(|i| i.duplicate));
}

#[test]
fn json_export_round_trip_and_profile_filter() {
    let t = tempfile::tempdir().unwrap();
    let (src, acme) = populated(t.path());
    let out = t.path().join("plain.json");
    let mut ep = export_params(ExportFormat::Json, &out);
    ep.profile_id = Some(acme.clone());
    let r = export::run(&src, &ep).unwrap();
    assert_eq!(r.entries, 8);
    #[cfg(unix)]
    assert_eq!(mode(&out), 0o600);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("gh-password-1") && !text.contains("other-token"));

    let t2 = tempfile::tempdir().unwrap();
    let mut dst = new_vault(&t2.path().join("v"));
    let r = import::apply(
        &mut dst,
        &ImportParams {
            format: ImportFormat::Json,
            path: out.display().to_string(),
            profile_id: None,
            skip_duplicates: true,
            passphrase: None,
        },
    )
    .unwrap();
    assert_eq!((r.created, r.profiles_created), (8, 1));
    let a: Vec<_> = all_revealed(&src)
        .into_iter()
        .filter(|e| e.0 == "acme")
        .collect();
    assert_eq!(a, all_revealed(&dst));
}

#[test]
fn dotenv_export() {
    let t = tempfile::tempdir().unwrap();
    let (src, _) = populated(t.path());
    let out = t.path().join("all.env");
    let r = export::run(&src, &export_params(ExportFormat::Dotenv, &out)).unwrap();
    assert_eq!(r.entries, 2);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("# acme/App env\n"), "{text}");
    assert!(
        text.contains("# other/Other env\nTOKEN=other-token\n"),
        "{text}"
    );
    assert!(text.contains("DATABASE_URL="));
    // Re-importable as a .env file.
    let parsed = import::parse(ImportFormat::Dotenv, &out, None).unwrap();
    let EntryPayload::Env(e) = &parsed.entries[0].payload else {
        panic!()
    };
    assert_eq!(e.vars.len(), 3);
}

#[test]
fn refuses_vault_path_and_bad_params() {
    let t = tempfile::tempdir().unwrap();
    let (src, _) = populated(t.path());
    let inside = src.path().join("export.json");
    let e = export::run(&src, &export_params(ExportFormat::Json, &inside)).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Denied);
    assert!(!inside.exists());
    let nested = src.path().join("profiles/x.json");
    assert!(export::run(&src, &export_params(ExportFormat::Json, &nested)).is_err());
    // Relative path via `..` that lands in the vault is still caught.
    let sneaky = t.path().join("v/../v/.latch/out.json");
    assert!(export::run(&src, &export_params(ExportFormat::Json, &sneaky)).is_err());
    // Archive without passphrase.
    let mut p = export_params(ExportFormat::LatchArchive, &t.path().join("a.age"));
    p.passphrase = None;
    assert_eq!(
        export::run(&src, &p).unwrap_err().code(),
        ErrorCode::InvalidParams
    );
    // Unknown profile.
    let mut p = export_params(ExportFormat::Json, &t.path().join("b.json"));
    p.profile_id = Some("aaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    assert_eq!(
        export::run(&src, &p).unwrap_err().code(),
        ErrorCode::NotFound
    );
    // Directory target.
    assert!(export::run(&src, &export_params(ExportFormat::Json, t.path())).is_err());
}

#[test]
fn committed_archive_fixture_imports() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let p = ImportParams {
        format: ImportFormat::LatchArchive,
        path: fixture_archive().display().to_string(),
        profile_id: None,
        skip_duplicates: true,
        passphrase: Some(ARCHIVE_PASS.into()),
    };
    let pv = import::preview(&v, &p).unwrap();
    assert_eq!(pv.items.len(), 9);
    let r = import::apply(&mut v, &p).unwrap();
    assert_eq!(r.created, 9);
}

/// Regenerates `fixtures/import/archive.latch.age`.
#[test]
#[ignore]
fn regenerate_archive_fixture() {
    let t = tempfile::tempdir().unwrap();
    let (src, _) = populated(t.path());
    export::run(
        &src,
        &export_params(ExportFormat::LatchArchive, &fixture_archive()),
    )
    .unwrap();
}

#[test]
fn saved_actions_roundtrip_remaps_environment_sources_and_restores_browser_only_profiles() {
    let t = tempfile::tempdir().unwrap();
    let (mut source, pid) = populated(t.path());
    let env = source
        .list_entries(&ListEntriesParams {
            profile_id: Some(pid.clone()),
            query: None,
            entry_type: Some(EntryType::Env),
        })
        .unwrap()[0]
        .id
        .clone();
    source
        .save_action(&ActionSaveParams {
            profile_id: pid,
            id: None,
            input: ActionInput {
                name: "Develop".into(),
                command: "bun run dev".into(),
                env_entry_ids: vec![env],
                env_keys: Some(vec!["PORT".into()]),
                urls: vec![],
            },
        })
        .unwrap();
    let empty = source
        .create_profile(&profile_input("Consoles only"))
        .unwrap();
    source
        .save_action(&ActionSaveParams {
            profile_id: empty.id,
            id: None,
            input: ActionInput {
                name: "Console".into(),
                command: "".into(),
                env_entry_ids: vec![],
                env_keys: None,
                urls: vec!["https://example.com/".into()],
            },
        })
        .unwrap();
    let path = t.path().join("backup.json");
    export::run(&source, &export_params(ExportFormat::Json, &path)).unwrap();
    let mut dest = new_vault(&t.path().join("restored"));
    import::apply(
        &mut dest,
        &ImportParams {
            format: ImportFormat::Json,
            path: path.to_string_lossy().into(),
            profile_id: None,
            passphrase: None,
            skip_duplicates: true,
        },
    )
    .unwrap();
    let p = dest.find_profile("acme").unwrap();
    let actions = dest.list_actions(&p.id).unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(
        dest.action_environment(&p.id, &actions[0].id).unwrap().vars[0].key,
        "PORT"
    );
    assert!(
        dest.get_entry(&GetEntryParams {
            id: actions[0].env_entry_ids[0].clone(),
            reveal: false
        })
        .is_ok()
    );
    let p = dest.find_profile("Consoles only").unwrap();
    assert_eq!(
        dest.list_actions(&p.id).unwrap()[0].urls,
        vec!["https://example.com/"]
    );
}

#[cfg(unix)]
#[test]
fn failed_action_profile_restore_leaves_no_orphan_profile_files() {
    use std::os::unix::fs::PermissionsExt;
    // Root bypasses directory permissions; exercised on normal developer/CI users.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let mut source = new_vault(&t.path().join("source"));
    let p = source
        .create_profile(&profile_input("Browser project"))
        .unwrap();
    source
        .save_action(&ActionSaveParams {
            profile_id: p.id,
            id: None,
            input: ActionInput {
                name: "Open".into(),
                command: "".into(),
                env_entry_ids: vec![],
                env_keys: None,
                urls: vec!["https://example.com".into()],
            },
        })
        .unwrap();
    let backup = t.path().join("backup.json");
    export::run(&source, &export_params(ExportFormat::Json, &backup)).unwrap();
    let path = t.path().join("dest");
    let mut dest = new_vault(&path);
    let meta = path.join(".latch");
    std::fs::set_permissions(&meta, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = import::apply(
        &mut dest,
        &ImportParams {
            format: ImportFormat::Json,
            path: backup.to_string_lossy().into(),
            profile_id: None,
            passphrase: None,
            skip_duplicates: true,
        },
    );
    std::fs::set_permissions(&meta, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_dir(path.join("profiles"))
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );
    drop(dest);
    assert!(unlock(&path).integrity_problems().is_empty());
}
