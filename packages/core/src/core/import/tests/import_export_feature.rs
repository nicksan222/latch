// Feature-owned import/export boundary regressions.

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use latch::core::import;
use latch::protocol::{ImportFormat, ImportParams};
use std::fs;
use support::*;

const SECRET_MARKER: &str = "do-not-leak-this-secret-value";

#[test]
fn import_rejects_sparse_file_over_limit_without_reading_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("oversized.env");
    let file = fs::File::create(&path).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();

    let err = import::parse(ImportFormat::Dotenv, &path, None).unwrap_err();
    assert_eq!(err.code(), latch::protocol::ErrorCode::InvalidParams);
    assert!(!err.to_string().contains(SECRET_MARKER));
}

#[test]
fn malformed_json_yaml_and_archive_errors_are_sanitized_and_do_not_mutate_vault() {
    let dir = tempfile::tempdir().unwrap();
    let vault = new_vault(&dir.path().join("vault"));
    let before_profiles = vault.list_profiles().unwrap();
    let before_entries = vault
        .list_entries(&latch::protocol::ListEntriesParams {
            profile_id: None,
            query: None,
            entry_type: None,
        })
        .unwrap();

    for (format, suffix, contents) in [
        (
            ImportFormat::Json,
            "json",
            format!("{{\"password\":\"{SECRET_MARKER}"),
        ),
        (
            ImportFormat::Yaml,
            "yaml",
            format!("password: [{SECRET_MARKER}"),
        ),
        (ImportFormat::LatchArchive, "age", SECRET_MARKER.to_owned()),
    ] {
        let path = dir.path().join(format!("malformed.{suffix}"));
        fs::write(&path, contents).unwrap();
        let err = import::preview(
            &vault,
            &ImportParams {
                format,
                path: path.display().to_string(),
                profile_id: None,
                skip_duplicates: false,
                passphrase: None,
            },
        )
        .unwrap_err();
        assert!(
            !err.to_string().contains(SECRET_MARKER),
            "{format:?}: {err}"
        );
    }

    assert_eq!(vault.list_profiles().unwrap().len(), before_profiles.len());
    assert_eq!(
        vault
            .list_entries(&latch::protocol::ListEntriesParams {
                profile_id: None,
                query: None,
                entry_type: None,
            })
            .unwrap()
            .len(),
        before_entries.len()
    );
}
