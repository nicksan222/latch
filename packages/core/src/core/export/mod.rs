//! Exporters (SPEC §12).
//!
//! - `latch_archive`: [`ArchiveDoc`] JSON encrypted with an age passphrase
//!   (scrypt) recipient. Re-importable with `ImportFormat::LatchArchive`.
//! - `json`: the same [`ArchiveDoc`], plaintext (the UI confirms first).
//!   Re-importable with `ImportFormat::Json`.
//! - `dotenv`: every env entry as a dotenv block headed `# <profile>/<entry>`.
//!
//! Files are written atomically with 0600 permissions and never inside the
//! vault repository.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::core::error::IoContext;
use crate::core::format::now;
use crate::core::secrets;
use crate::core::{Error, Result, UnlockedVault};
use crate::protocol::{
    BrowserSettings, EntryPayload, ExportFormat, ExportParams, ExportResult, ListEntriesParams,
};

/// One registered export encoder. File validation and writes belong to the harness.
pub struct ExportKind {
    pub format: ExportFormat,
    pub id: &'static str,
    encode: fn(&ArchiveDoc, &ExportParams) -> Result<EncodedExport>,
}

struct EncodedExport {
    bytes: Zeroizing<Vec<u8>>,
    entries: u32,
}

inventory::collect!(ExportKind);

/// Declare an encoder and attach it to the registry from its own source file.
macro_rules! export_kind {
    ($format:ident { id: $id:literal, encode($doc:ident, $params:ident) $body:block }) => {
        fn encode(
            $doc: &super::super::ArchiveDoc,
            $params: &crate::protocol::ExportParams,
        ) -> crate::core::Result<super::super::EncodedExport> $body

        inventory::submit! {
            super::super::ExportKind {
                format: crate::protocol::ExportFormat::$format,
                id: $id,
                encode,
            }
        }
    };
}

mod kinds;

/// Registry order is stable for inspection; dispatch never depends on link order.
pub fn registered_kinds() -> Vec<&'static ExportKind> {
    let mut kinds: Vec<_> = inventory::iter::<ExportKind>.into_iter().collect();
    for kind in &kinds {
        resolve_kind(kind.format, kinds.iter().copied()).expect("invalid export kind registry");
    }
    kinds.sort_by_key(|kind| kind.id);
    kinds
}

fn resolve_kind<'a>(
    format: ExportFormat,
    kinds: impl IntoIterator<Item = &'a ExportKind>,
) -> Result<&'a ExportKind> {
    let kinds: Vec<_> = kinds.into_iter().collect();
    for (i, kind) in kinds.iter().enumerate() {
        if kind.id.is_empty() {
            return Err(Error::Internal("missing export strategy identity".into()));
        }
        if kinds[..i]
            .iter()
            .any(|other| other.id == kind.id || other.format == kind.format)
        {
            return Err(Error::Internal("duplicate export strategy".into()));
        }
    }
    let mut matches = kinds.into_iter().filter(|kind| kind.format == format);
    let kind = matches
        .next()
        .ok_or_else(|| Error::Internal("missing export strategy".into()))?;
    Ok(kind)
}

fn strategy(format: ExportFormat) -> Result<&'static ExportKind> {
    resolve_kind(format, inventory::iter::<ExportKind>)
}

/// `format` marker of archives and plaintext JSON exports.
pub const ARCHIVE_FORMAT: &str = "latch-archive";
pub const ARCHIVE_VERSION: u32 = 1;
/// Largest decrypted archive document accepted.
const MAX_ARCHIVE_JSON: u64 = 256 * 1024 * 1024;

/// Export document (camelCase, wire payloads).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveDoc {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub vault_id: String,
    pub profiles: Vec<ArchiveProfile>,
    pub entries: Vec<ArchiveEntry>,
}

impl std::fmt::Debug for ArchiveDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveDoc")
            .field("profiles", &self.profiles.len())
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveProfile {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pinned: bool,
    pub browser: BrowserSettings,
    #[serde(default)]
    pub actions: Vec<crate::protocol::LaunchAction>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    pub payload: EntryPayload,
}

/// Collect profiles and fully revealed entries (all, or one profile).
pub fn build_archive(vault: &UnlockedVault, profile_id: Option<&str>) -> Result<ArchiveDoc> {
    let mut profiles = vec![];
    for s in vault.list_profiles()? {
        if profile_id.is_some_and(|p| p != s.id) {
            continue;
        }
        let p = vault.get_profile(&s.id)?;
        let actions = vault.list_actions(&s.id)?;
        profiles.push(ArchiveProfile {
            id: p.id,
            name: p.name,
            color: p.color,
            icon: p.icon,
            description: p.description,
            pinned: p.pinned,
            browser: p.browser,
            actions,
        });
    }
    if profile_id.is_some() && profiles.is_empty() {
        return Err(Error::NotFound("profile".into()));
    }
    let mut out_entries = vec![];
    for p in &profiles {
        let list = vault.list_entries(&ListEntriesParams {
            profile_id: Some(p.id.clone()),
            query: None,
            entry_type: None,
        })?;
        for s in list {
            let doc = vault.read_entry_doc(&s.id)?;
            let input = secrets::to_input(&doc);
            out_entries.push(ArchiveEntry {
                id: doc.id.clone(),
                profile_id: doc.profile_id.clone(),
                name: input.name,
                tags: input.tags,
                notes: input.notes,
                favorite: input.favorite,
                created_at: doc.created_at.clone(),
                updated_at: doc.updated_at.clone(),
                payload: input.payload,
            });
        }
    }
    Ok(ArchiveDoc {
        format: ARCHIVE_FORMAT.into(),
        version: ARCHIVE_VERSION,
        exported_at: now(),
        vault_id: vault.id().to_owned(),
        profiles,
        entries: out_entries,
    })
}

/// Encrypt an archive document with an age passphrase.
pub fn encrypt_archive(doc: &ArchiveDoc, passphrase: &str) -> Result<Vec<u8>> {
    let json = Zeroizing::new(
        serde_json::to_vec(doc).map_err(|_| Error::Internal("serialize archive".into()))?,
    );
    let mut r = age::scrypt::Recipient::new(SecretString::from(passphrase.to_owned()));
    if crate::core::crypto::weak_kdf_allowed() {
        r.set_work_factor(10);
    }
    crate::core::crypto::encrypt_to(&[&r as &dyn age::Recipient], &json)
}

/// Decrypt and parse an archive. Wrong passphrase → `BadCredentials`.
pub fn decrypt_archive(ciphertext: &[u8], passphrase: &str) -> Result<ArchiveDoc> {
    let dec = age::Decryptor::new_buffered(ciphertext)
        .map_err(|_| Error::invalid("not a Latch archive (age header missing)"))?;
    if !dec.is_scrypt() {
        return Err(Error::invalid(
            "this age file is not passphrase-encrypted; not a Latch archive",
        ));
    }
    let id = age::scrypt::Identity::new(SecretString::from(passphrase.to_owned()));
    let mut reader = dec
        .decrypt(std::iter::once(&id as &dyn age::Identity))
        .map_err(|e| match e {
            age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
                Error::BadCredentials
            }
            age::DecryptError::ExcessiveWork { .. } => {
                Error::invalid("archive uses an excessive scrypt work factor")
            }
            _ => Error::integrity("archive cannot be decrypted"),
        })?;
    let mut json = Zeroizing::new(Vec::new());
    // Bounded: an archive cannot expand past the import size limit.
    (&mut reader)
        .take(MAX_ARCHIVE_JSON + 1)
        .read_to_end(&mut json)
        .map_err(|_| Error::integrity("archive is corrupt"))?;
    if json.len() as u64 > MAX_ARCHIVE_JSON {
        return Err(Error::invalid("archive is too large"));
    }
    let doc: ArchiveDoc =
        serde_json::from_slice(&json).map_err(|_| Error::integrity("malformed Latch archive"))?;
    validate_archive(&doc)?;
    Ok(doc)
}

/// Checks shared by encrypted archives and plaintext JSON exports: the
/// format marker and a supported version.
pub fn validate_archive(doc: &ArchiveDoc) -> Result<()> {
    if doc.format != ARCHIVE_FORMAT {
        return Err(Error::invalid("not a Latch archive"));
    }
    if doc.version == 0 || doc.version > ARCHIVE_VERSION {
        return Err(Error::UnsupportedVersion {
            found: doc.version,
            supported: ARCHIVE_VERSION,
        });
    }
    Ok(())
}

/// Resolve the export target; refuse paths inside the vault repository.
pub fn check_target(vault_root: &Path, path: &str) -> Result<PathBuf> {
    if path.trim().is_empty() {
        return Err(Error::invalid("export path is required"));
    }
    let p = PathBuf::from(path);
    let p = if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .ctx(|| "current dir".into())?
            .join(p)
    };
    let file_name = p
        .file_name()
        .ok_or_else(|| Error::invalid("export path must name a file"))?
        .to_owned();
    let parent = p
        .parent()
        .ok_or_else(|| Error::invalid("export path must name a file"))?;
    let parent = parent
        .canonicalize()
        .ctx(|| format!("resolve {}", parent.display()))?;
    let mut target = parent.join(file_name);
    if let Ok(real) = target.canonicalize() {
        target = real;
    }
    if target.is_dir() {
        return Err(Error::invalid("export path is a directory"));
    }
    let root = vault_root
        .canonicalize()
        .unwrap_or_else(|_| vault_root.to_path_buf());
    if target.starts_with(&root) {
        return Err(Error::Denied(
            "refusing to write an export inside the vault repository".into(),
        ));
    }
    Ok(target)
}

/// Atomic write with 0600 permissions.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::Internal("path without parent".into()))?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".latch-export-")
        .tempfile_in(dir)
        .ctx(|| format!("temp in {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600))
            .ctx(|| format!("chmod {}", tmp.path().display()))?;
    }
    tmp.write_all(bytes)
        .and_then(|_| tmp.as_file().sync_all())
        .ctx(|| format!("write {}", path.display()))?;
    tmp.persist(path)
        .map_err(|e| Error::io(format!("rename to {}", path.display()), e.error))?;
    Ok(())
}

pub use kinds::dotenv_text;

pub fn run(vault: &UnlockedVault, params: &ExportParams) -> Result<ExportResult> {
    let target = check_target(vault.path(), &params.path)?;
    let doc = build_archive(vault, params.profile_id.as_deref())?;
    let encoded = (strategy(params.format)?.encode)(&doc, params)?;
    write_private(&target, &encoded.bytes)?;
    Ok(ExportResult {
        path: target.display().to_string(),
        entries: encoded.entries,
    })
}

#[cfg(test)]
mod strategy_tests {
    use super::*;

    fn archive() -> ArchiveDoc {
        ArchiveDoc {
            format: ARCHIVE_FORMAT.into(),
            version: ARCHIVE_VERSION,
            exported_at: "2026-10-25T00:00:00Z".into(),
            vault_id: "test-vault".into(),
            profiles: vec![],
            entries: vec![],
        }
    }

    fn params(format: ExportFormat) -> ExportParams {
        ExportParams {
            format,
            path: "unused".into(),
            profile_id: None,
            passphrase: None,
        }
    }

    #[test]
    fn export_registry_covers_each_format_once_in_stable_order() {
        let kinds = registered_kinds();
        assert_eq!(
            kinds.iter().map(|k| k.id).collect::<Vec<_>>(),
            ["dotenv", "json", "latch_archive"]
        );
        for format in [
            ExportFormat::Dotenv,
            ExportFormat::Json,
            ExportFormat::LatchArchive,
        ] {
            assert_eq!(kinds.iter().filter(|k| k.format == format).count(), 1);
            assert_eq!(strategy(format).unwrap().format, format);
        }
    }

    #[test]
    fn missing_identity_fails_closed() {
        let json = strategy(ExportFormat::Json).unwrap();
        let other = ExportKind {
            format: ExportFormat::Dotenv,
            id: "",
            encode: json.encode,
        };
        assert!(resolve_kind(ExportFormat::Json, [json, &other]).is_err());
    }

    #[test]
    fn duplicate_ids_across_formats_fail_closed() {
        let json = strategy(ExportFormat::Json).unwrap();
        let other = ExportKind {
            format: ExportFormat::Dotenv,
            id: json.id,
            encode: json.encode,
        };
        assert!(resolve_kind(ExportFormat::Json, [json, &other]).is_err());
    }

    #[test]
    fn unrelated_duplicate_formats_fail_closed() {
        let json = strategy(ExportFormat::Json).unwrap();
        let dotenv = strategy(ExportFormat::Dotenv).unwrap();
        assert!(resolve_kind(ExportFormat::Json, [json, dotenv, dotenv]).is_err());
    }

    #[test]
    fn registry_rejects_missing_and_duplicate_handlers() {
        let json = strategy(ExportFormat::Json).unwrap();
        assert!(resolve_kind(ExportFormat::Json, std::iter::empty()).is_err());
        assert!(resolve_kind(ExportFormat::Json, [json, json]).is_err());
    }

    #[test]
    fn registered_encoders_keep_json_and_dotenv_contracts() {
        let doc = archive();
        let json =
            (strategy(ExportFormat::Json).unwrap().encode)(&doc, &params(ExportFormat::Json))
                .unwrap();
        assert_eq!(json.entries, 0);
        assert!(json.bytes.ends_with(b"\n"));
        let decoded: ArchiveDoc = serde_json::from_slice(&json.bytes).unwrap();
        assert_eq!(decoded.format, ARCHIVE_FORMAT);
        assert_eq!(decoded.version, ARCHIVE_VERSION);
        let dotenv =
            (strategy(ExportFormat::Dotenv).unwrap().encode)(&doc, &params(ExportFormat::Dotenv))
                .unwrap();
        assert_eq!(dotenv.entries, 0);
        assert!(dotenv.bytes.is_empty());
    }

    #[test]
    fn encrypted_encoder_requires_a_nonempty_passphrase() {
        let kind = strategy(ExportFormat::LatchArchive).unwrap();
        let doc = archive();
        let mut p = params(ExportFormat::LatchArchive);
        assert!((kind.encode)(&doc, &p).is_err());
        p.passphrase = Some(String::new());
        assert!((kind.encode)(&doc, &p).is_err());
    }
}
