//! On-disk vault format v1 (see README.md). These are snake_case
//! documents, deliberately separate from the camelCase wire types in
//! `crate::protocol`; conversions live next to them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::crypto;
use super::error::{Error, IoContext, Result};
use crate::protocol::{
    AutofillScope, Bookmark, BrowserSettings, EntryType, KdfParams, Profile, ProfileInput,
    RecipientInfo, RecipientKind, TotpAlgorithm,
};

/// Current (and highest supported) vault format version.
pub const FORMAT_VERSION: u32 = 1;
/// Plaintext entry documents larger than this are rejected.
pub const MAX_ENTRY_BYTES: usize = 1024 * 1024;

pub const LATCH_DIR: &str = ".latch";
pub const VERSION_FILE: &str = ".latch/version";
pub const CONFIG_FILE: &str = ".latch/config.toml";
pub const VAULTKEY_FILE: &str = ".latch/vaultkey.age";
pub const MANIFEST_FILE: &str = ".latch/manifest.age";
pub const MANIFEST_SIG_FILE: &str = ".latch/manifest.sig";
/// Signed key-transition chain (see README.md and the format module).
pub const KEYCHAIN_FILE: &str = ".latch/keychain.json";
pub const PROFILES_DIR: &str = "profiles";
pub const SESSIONS_DIR: &str = "sessions";
pub const README_FILE: &str = "README.md";
pub const GITATTRIBUTES_FILE: &str = ".gitattributes";
pub const GITIGNORE_FILE: &str = ".gitignore";

/// Relative path of a profile document.
pub fn profile_file(profile_id: &str) -> String {
    format!("{PROFILES_DIR}/{profile_id}/profile.age")
}

pub fn profile_dir(profile_id: &str) -> String {
    format!("{PROFILES_DIR}/{profile_id}")
}

/// Relative path of an entry document.
pub fn entry_file(profile_id: &str, entry_id: &str) -> String {
    format!("{PROFILES_DIR}/{profile_id}/entries/{entry_id}.age")
}

/// Parse `profiles/<pid>/entries/<eid>.age` → (pid, eid).
pub fn parse_entry_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("profiles/")?;
    let mut it = rest.split('/');
    let pid = it.next()?;
    if it.next()? != "entries" {
        return None;
    }
    let eid = it.next()?.strip_suffix(".age")?;
    if it.next().is_some() || !super::ids::is_valid_id(pid) || !super::ids::is_valid_id(eid) {
        return None;
    }
    Some((pid.to_owned(), eid.to_owned()))
}

/// Parse `profiles/<pid>/profile.age` → pid.
pub fn parse_profile_path(path: &str) -> Option<String> {
    let pid = path
        .strip_prefix("profiles/")?
        .strip_suffix("/profile.age")?;
    super::ids::is_valid_id(pid).then(|| pid.to_owned())
}

/// Fixed (non-layout) files a vault may track.
pub const VAULT_FILES: &[&str] = &[
    VERSION_FILE,
    CONFIG_FILE,
    KEYCHAIN_FILE,
    VAULTKEY_FILE,
    MANIFEST_FILE,
    MANIFEST_SIG_FILE,
    README_FILE,
    GITATTRIBUTES_FILE,
    GITIGNORE_FILE,
];

/// `sessions/<name>.age` (committed browser session archive).
pub fn is_session_archive_path(path: &str) -> bool {
    path.strip_prefix("sessions/").is_some_and(|n| {
        n.len() > 4
            && n.ends_with(".age")
            && !n.starts_with('.')
            && !n.contains('/')
            && n.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    })
}

/// The allowlist of paths Latch writes and commits (and `doctor` accepts as
/// tracked): the fixed files, profile/entry documents with valid ids, and
/// committed session archives. Everything else is foreign.
pub fn is_vault_path(path: &str) -> bool {
    VAULT_FILES.contains(&path)
        || parse_profile_path(path).is_some()
        || parse_entry_path(path).is_some()
        || is_session_archive_path(path)
}

/// RFC 3339 UTC timestamp with second precision.
pub fn now() -> String {
    let t = time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("0 is a valid nanosecond");
    t.format(&time::format_description::well_known::Rfc3339)
        .expect("RFC 3339 formatting cannot fail for UTC")
}

pub fn parse_time(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

/// Write via temp file + rename in the same directory. Only ciphertext and
/// public files are ever written under the vault path.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::Internal("path without parent".into()))?;
    std::fs::create_dir_all(dir).ctx(|| format!("create {}", dir.display()))?;
    let mut tmp =
        tempfile::NamedTempFile::new_in(dir).ctx(|| format!("temp in {}", dir.display()))?;
    use std::io::Write;
    tmp.write_all(bytes)
        .and_then(|_| tmp.as_file().sync_all())
        .ctx(|| format!("write {}", path.display()))?;
    tmp.persist(path)
        .map_err(|e| Error::io(format!("rename to {}", path.display()), e.error))?;
    Ok(())
}

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).ctx(|| format!("read {}", path.display()))
}

// ---------------------------------------------------------------------------
// .latch/version
// ---------------------------------------------------------------------------

/// Read and check `.latch/version`. Missing → NoVault; newer → UnsupportedVersion.
pub fn read_version(root: &Path) -> Result<u32> {
    let p = root.join(VERSION_FILE);
    let s = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NoVault(root.display().to_string()));
        }
        Err(e) => return Err(Error::io(format!("read {}", p.display()), e)),
    };
    let v: u32 = s
        .trim()
        .parse()
        .map_err(|_| Error::integrity(".latch/version is not an integer"))?;
    if v > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion {
            found: v,
            supported: FORMAT_VERSION,
        });
    }
    if v == 0 {
        return Err(Error::integrity(".latch/version is 0"));
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// .latch/config.toml (public)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigDoc {
    pub version: u32,
    pub kdf: KdfDoc,
    pub vault: VaultMetaDoc,
    #[serde(default)]
    pub recipients: Vec<RecipientDoc>,
    #[serde(default)]
    pub sessions: SessionsDoc,
    #[serde(default)]
    pub sync: SyncDoc,
    #[serde(default)]
    pub hooks: HooksDoc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KdfDoc {
    pub algorithm: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    /// base64 (standard, padded), 16 bytes.
    pub salt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultMetaDoc {
    pub id: String,
    pub created_at: String,
    /// `ed25519:<base64>`
    pub signing_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipientDoc {
    pub label: String,
    /// passphrase | age | ssh | plugin | recovery
    #[serde(rename = "type")]
    pub kind: String,
    pub public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SessionsDoc {
    #[serde(default)]
    pub commit: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SyncDoc {
    #[serde(default)]
    pub names_in_commits: bool,
    #[serde(default)]
    pub auto_push: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HooksDoc {
    #[serde(default = "yes")]
    pub pre_commit_scan: bool,
}

fn yes() -> bool {
    true
}

impl Default for HooksDoc {
    fn default() -> Self {
        HooksDoc {
            pre_commit_scan: true,
        }
    }
}

pub fn recipient_kind_str(k: RecipientKind) -> &'static str {
    match k {
        RecipientKind::Passphrase => "passphrase",
        RecipientKind::Age => "age",
        RecipientKind::Ssh => "ssh",
        RecipientKind::Plugin => "plugin",
        RecipientKind::Recovery => "recovery",
    }
}

pub fn parse_recipient_kind(s: &str) -> RecipientKind {
    match s {
        "passphrase" => RecipientKind::Passphrase,
        "ssh" => RecipientKind::Ssh,
        "recovery" => RecipientKind::Recovery,
        s if s == "plugin" || s.starts_with("age-plugin-") => RecipientKind::Plugin,
        _ => RecipientKind::Age,
    }
}

impl RecipientDoc {
    pub fn to_info(&self) -> RecipientInfo {
        RecipientInfo {
            label: self.label.clone(),
            kind: parse_recipient_kind(&self.kind),
            public_key: self.public_key.clone(),
            added_at: self.added_at.clone(),
        }
    }
}

impl KdfDoc {
    pub fn params(&self) -> KdfParams {
        KdfParams {
            memory_kib: self.memory_kib,
            iterations: self.iterations,
            parallelism: self.parallelism,
        }
    }
    pub fn salt_bytes(&self) -> Result<Vec<u8>> {
        crypto::b64_decode(&self.salt)
            .filter(|s| s.len() >= 8)
            .ok_or_else(|| Error::integrity("malformed KDF salt in config.toml"))
    }
    pub fn new(p: &KdfParams, salt: &[u8]) -> Self {
        KdfDoc {
            algorithm: "argon2id".into(),
            memory_kib: p.memory_kib,
            iterations: p.iterations,
            parallelism: p.parallelism,
            salt: crypto::b64(salt),
        }
    }
}

impl ConfigDoc {
    pub fn read(root: &Path) -> Result<Self> {
        let p = root.join(CONFIG_FILE);
        let s = match std::fs::read_to_string(&p) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::NoVault(root.display().to_string()));
            }
            Err(e) => return Err(Error::io(format!("read {}", p.display()), e)),
        };
        Self::parse(&s)
    }

    /// Parse and check config text (also used for upstream configs read
    /// from git objects).
    pub fn parse(s: &str) -> Result<Self> {
        // config.toml is public, so parse errors may be shown verbatim.
        let c: ConfigDoc = toml::from_str(s)
            .map_err(|e| Error::integrity(format!("config.toml: {}", e.message())))?;
        if c.version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: c.version,
                supported: FORMAT_VERSION,
            });
        }
        if c.kdf.algorithm != "argon2id" {
            return Err(Error::integrity("config.toml: unsupported KDF algorithm"));
        }
        Ok(c)
    }

    pub fn to_toml(&self) -> String {
        let body = toml::to_string_pretty(self).expect("config serializes");
        format!(
            "# Latch vault configuration. Public by design: nothing here is secret.\n\
             # Security-relevant fields are bound to the signed manifest; do not edit\n\
             # [kdf], [vault] or [[recipients]] by hand.\n\n{body}"
        )
    }

    pub fn write(&self, root: &Path) -> Result<()> {
        write_atomic(&root.join(CONFIG_FILE), self.to_toml().as_bytes())
    }

    /// Hash over the WHOLE config (format version, KDF, vault metadata,
    /// recipients and the `[sessions]`/`[sync]`/`[hooks]` settings): the
    /// compact serde_json encoding of the parsed document, field order as
    /// declared. Stored in the signed manifest so any edit of the public
    /// config (an injected recipient, a remote enabling `names_in_commits`)
    /// is detected. Keys unknown to this version are ignored on read and
    /// therefore not bound (they have no effect).
    pub fn security_hash(&self) -> String {
        let canon = serde_json::to_vec(self).expect("serializes");
        crypto::hash(&canon)
    }

    pub fn passphrase_recipient(&self) -> Option<&RecipientDoc> {
        self.recipients.iter().find(|r| r.kind == "passphrase")
    }
}

// ---------------------------------------------------------------------------
// .latch/vaultkey.age (encrypted to every recipient)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct VaultKeyDoc {
    pub version: u32,
    /// base64, 32 bytes.
    pub vault_key: String,
    /// base64 Ed25519 seed, 32 bytes.
    pub signing_private_key: String,
    /// Vault keys replaced by `rotate`, newest first, so git history stays
    /// readable to current recipients.
    #[serde(default)]
    pub previous_vault_keys: Vec<String>,
    /// A rotation in progress (crash-safe ordering, see FORMAT.md §4.2):
    /// the next keys plus their keychain record signed by the current
    /// signing key. Absent outside a rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<PendingKeysDoc>,
}

/// Keys of a rotation in progress (inside the encrypted `vaultkey.age`).
#[derive(Serialize, Deserialize)]
pub struct PendingKeysDoc {
    /// base64, 32 bytes.
    pub vault_key: String,
    /// base64 Ed25519 seed, 32 bytes.
    pub signing_private_key: String,
    /// The keychain record introducing these keys, signed by the current key.
    pub record: KeyRecord,
}

impl Drop for PendingKeysDoc {
    fn drop(&mut self) {
        self.vault_key.zeroize();
        self.signing_private_key.zeroize();
    }
}

impl Drop for VaultKeyDoc {
    fn drop(&mut self) {
        self.vault_key.zeroize();
        self.signing_private_key.zeroize();
        self.previous_vault_keys.zeroize();
    }
}

impl VaultKeyDoc {
    pub fn to_json(&self) -> Zeroizing<Vec<u8>> {
        // Pre-sized so serialization never reallocates (a reallocation would
        // leave an unzeroized copy of the key material in freed memory).
        let mut out = Zeroizing::new(Vec::with_capacity(
            1024 + 64 * self.previous_vault_keys.len(),
        ));
        serde_json::to_writer(&mut *out, self).expect("serializes");
        out
    }
    pub fn from_json(b: &[u8]) -> Result<Self> {
        serde_json::from_slice(b).map_err(|_| Error::integrity("malformed vaultkey.age contents"))
    }
}

// ---------------------------------------------------------------------------
// .latch/keychain.json (public, signed key-transition chain)
// ---------------------------------------------------------------------------

/// One key epoch: the signing public key and a hash of the vault-identity
/// recipient in force from `seq` on. Record 0 (genesis) is self-signed;
/// record n is signed by record n-1's key. Verification lives in
/// `core::trust`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRecord {
    pub seq: u64,
    pub vault_id: String,
    /// `ed25519:<base64>`
    pub signing_public_key: String,
    /// `blake3:<hex>` of the vault identity's `age1…` recipient (the
    /// recipient itself stays secret so nobody can forge ciphertexts).
    pub vault_recipient_hash: String,
    pub created_at: String,
    /// Hash of the previous record (absent for genesis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
    /// `ed25519:<base64>` over [`KeyRecord::signed_bytes`].
    pub signature: String,
}

impl KeyRecord {
    /// Bytes covered by the signature: a domain-separation prefix plus the
    /// compact JSON of every field except `signature`.
    pub fn signed_bytes(&self) -> Vec<u8> {
        #[derive(Serialize)]
        struct Unsigned<'a> {
            seq: u64,
            vault_id: &'a str,
            signing_public_key: &'a str,
            vault_recipient_hash: &'a str,
            created_at: &'a str,
            prev: &'a Option<String>,
        }
        let mut v = b"latch/v1/keychain-record\n".to_vec();
        serde_json::to_writer(
            &mut v,
            &Unsigned {
                seq: self.seq,
                vault_id: &self.vault_id,
                signing_public_key: &self.signing_public_key,
                vault_recipient_hash: &self.vault_recipient_hash,
                created_at: &self.created_at,
                prev: &self.prev,
            },
        )
        .expect("serializes");
        v
    }

    /// `blake3:` hash of the whole record (including the signature).
    pub fn hash(&self) -> String {
        crypto::hash(&serde_json::to_vec(self).expect("serializes"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeychainDoc {
    pub version: u32,
    pub records: Vec<KeyRecord>,
}

impl KeychainDoc {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let k: KeychainDoc = serde_json::from_slice(b)
            .map_err(|_| Error::integrity("keychain.json is malformed"))?;
        if k.version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: k.version,
                supported: FORMAT_VERSION,
            });
        }
        if k.records.is_empty() {
            return Err(Error::integrity("keychain.json has no records"));
        }
        Ok(k)
    }
    pub fn read(root: &Path) -> Result<Self> {
        match std::fs::read(root.join(KEYCHAIN_FILE)) {
            Ok(b) => Self::parse(&b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::integrity("keychain.json is missing"))
            }
            Err(e) => Err(Error::io(format!("read {KEYCHAIN_FILE}"), e)),
        }
    }
    pub fn to_json(&self) -> Vec<u8> {
        let mut v = serde_json::to_vec_pretty(self).expect("serializes");
        v.push(b'\n');
        v
    }
    pub fn write(&self, root: &Path) -> Result<()> {
        write_atomic(&root.join(KEYCHAIN_FILE), &self.to_json())
    }
    pub fn head(&self) -> &KeyRecord {
        self.records.last().expect("non-empty (checked on parse)")
    }
}

// ---------------------------------------------------------------------------
// .latch/manifest.age
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDoc {
    pub version: u32,
    pub updated_at: String,
    /// [`ConfigDoc::security_hash`] at signing time.
    pub config_hash: String,
    pub profiles: Vec<ManifestProfile>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pinned: bool,
    /// BLAKE3 of `profile.age` ciphertext.
    pub hash: String,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub has_totp: bool,
    #[serde(default)]
    pub created_at: String,
    pub updated_at: String,
    /// BLAKE3 of the entry ciphertext.
    pub hash: String,
}

impl ManifestDoc {
    pub fn profile(&self, id: &str) -> Option<&ManifestProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }
    pub fn profile_mut(&mut self, id: &str) -> Option<&mut ManifestProfile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }
    /// (profile, entry) for an entry id.
    pub fn entry(&self, id: &str) -> Option<(&ManifestProfile, &ManifestEntry)> {
        self.profiles
            .iter()
            .find_map(|p| p.entries.iter().find(|e| e.id == id).map(|e| (p, e)))
    }
    pub fn from_json(b: &[u8]) -> Result<Self> {
        let m: ManifestDoc =
            serde_json::from_slice(b).map_err(|_| Error::integrity("malformed manifest"))?;
        if m.version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: m.version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(m)
    }
}

// ---------------------------------------------------------------------------
// profiles/<id>/profile.age
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileDoc {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub browser: BrowserDoc,
    #[serde(default)]
    pub actions: Vec<ActionDoc>,
    pub created_at: String,
    pub updated_at: String,
}

/// On-disk action metadata, deliberately separate from camelCase RPC types.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDoc {
    pub id: String,
    pub name: String,
    pub command: String,
    pub env_entry_ids: Vec<String>,
    pub env_keys: Option<Vec<String>>,
    pub urls: Vec<String>,
}

impl ActionDoc {
    pub fn to_protocol(&self) -> crate::protocol::LaunchAction {
        crate::protocol::LaunchAction {
            id: self.id.clone(),
            name: self.name.clone(),
            command: self.command.clone(),
            env_entry_ids: self.env_entry_ids.clone(),
            env_keys: self.env_keys.clone(),
            urls: self.urls.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserDoc {
    #[serde(default)]
    pub start_pages: Vec<String>,
    #[serde(default)]
    pub bookmarks: Vec<BookmarkDoc>,
    /// all | selected | none
    #[serde(default = "autofill_all")]
    pub autofill_entries: String,
    #[serde(default)]
    pub autofill_entry_ids: Vec<String>,
}

fn autofill_all() -> String {
    "all".into()
}

impl Default for BrowserDoc {
    fn default() -> Self {
        BrowserDoc {
            start_pages: vec![],
            bookmarks: vec![],
            autofill_entries: autofill_all(),
            autofill_entry_ids: vec![],
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkDoc {
    pub title: String,
    pub url: String,
}

impl BrowserDoc {
    pub fn from_protocol(b: &BrowserSettings) -> Self {
        BrowserDoc {
            start_pages: b.start_pages.clone(),
            bookmarks: b
                .bookmarks
                .iter()
                .map(|b| BookmarkDoc {
                    title: b.title.clone(),
                    url: b.url.clone(),
                })
                .collect(),
            autofill_entries: match b.autofill {
                AutofillScope::All => "all",
                AutofillScope::Selected => "selected",
                AutofillScope::None => "none",
            }
            .into(),
            autofill_entry_ids: b.autofill_entry_ids.clone(),
        }
    }
    pub fn to_protocol(&self) -> BrowserSettings {
        BrowserSettings {
            start_pages: self.start_pages.clone(),
            bookmarks: self
                .bookmarks
                .iter()
                .map(|b| Bookmark {
                    title: b.title.clone(),
                    url: b.url.clone(),
                })
                .collect(),
            autofill: match self.autofill_entries.as_str() {
                "selected" => AutofillScope::Selected,
                "none" => AutofillScope::None,
                _ => AutofillScope::All,
            },
            autofill_entry_ids: self.autofill_entry_ids.clone(),
        }
    }
}

impl ProfileDoc {
    pub fn to_protocol(&self) -> Profile {
        Profile {
            id: self.id.clone(),
            name: self.name.clone(),
            color: self.color.clone(),
            icon: self.icon.clone(),
            description: self.description.clone(),
            pinned: self.pinned,
            browser: self.browser.to_protocol(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    /// Apply validated input (name/color/icon/description/pinned/browser).
    pub fn apply_input(&mut self, input: &ProfileInput) {
        self.name = input.name.trim().to_owned();
        self.color = input.color.trim().to_ascii_lowercase();
        self.icon = input.icon.trim().to_owned();
        self.description = input.description.clone();
        self.pinned = input.pinned;
        self.browser = BrowserDoc::from_protocol(&input.browser);
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("serializes")
    }
    pub fn from_json(b: &[u8]) -> Result<Self> {
        let p: ProfileDoc = serde_json::from_slice(b)
            .map_err(|_| Error::integrity("malformed profile document"))?;
        if p.version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: p.version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(p)
    }
}

/// Non-placeholder start page rows, retaining their original form indices.
/// Keep the row-count bound on the original input before using this iterator.
pub(crate) fn profile_start_page_rows(input: &ProfileInput) -> impl Iterator<Item = (usize, &str)> {
    input
        .browser
        .start_pages
        .iter()
        .enumerate()
        .filter(|(_, page)| !page.trim().is_empty())
        .map(|(index, page)| (index, page.as_str()))
}

/// Validate the original draft before removing empty editor placeholder rows.
/// Create and update use this same normalization, including batch imports.
pub fn normalize_profile_input(input: &ProfileInput) -> Result<ProfileInput> {
    validate_profile_input(input)?;
    let mut normalized = input.clone();
    normalized.browser.start_pages = profile_start_page_rows(input)
        .map(|(_, page)| page.to_owned())
        .collect();
    Ok(normalized)
}

pub fn validate_profile_input(input: &ProfileInput) -> Result<()> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(Error::invalid("profile name must be 1-128 characters"));
    }
    if name.contains('/') || name.chars().any(char::is_control) {
        return Err(Error::invalid(
            "profile name must not contain `/` or control characters",
        ));
    }
    let c = input.color.trim();
    if !(c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err(Error::invalid("profile color must be #rrggbb"));
    }
    if input
        .browser
        .autofill_entry_ids
        .iter()
        .any(|i| !super::ids::is_valid_id(i))
    {
        return Err(Error::invalid("autofill entry ids must be entry ids"));
    }
    if input.browser.start_pages.len() > 64 {
        return Err(Error::invalid("too many start pages"));
    }
    for (_, u) in profile_start_page_rows(input) {
        // http(s) only: start pages end up on the browser command line.
        super::browser::normalize_start_url(u)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// profiles/<pid>/entries/<eid>.age
// ---------------------------------------------------------------------------

/// Entry envelope. Secret strings are zeroized on drop; `Debug` never prints
/// payload values.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryDoc {
    pub version: u32,
    pub id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    /// `"type"` + `"data"` at the top level of the document.
    #[serde(flatten)]
    pub body: EntryBody,
}

impl std::fmt::Debug for EntryDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntryDoc")
            .field("id", &self.id)
            .field("profile_id", &self.profile_id)
            .field("type", &self.body.entry_type())
            .finish_non_exhaustive()
    }
}

impl Drop for EntryDoc {
    fn drop(&mut self) {
        self.notes.zeroize();
        self.body.zeroize_secrets();
    }
}

impl EntryDoc {
    pub fn entry_type(&self) -> EntryType {
        self.body.entry_type()
    }
    pub fn to_json(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(serde_json::to_vec(self).expect("serializes"))
    }
    pub fn from_json(b: &[u8]) -> Result<Self> {
        // Never surface serde's message: it can quote secret values.
        let d: EntryDoc =
            serde_json::from_slice(b).map_err(|_| Error::integrity("malformed entry document"))?;
        if d.version > FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: d.version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(d)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum EntryBody {
    Login(LoginDoc),
    Secret(SecretDoc),
    Env(EnvDoc),
    File(FileDoc),
    Note(NoteDoc),
    SshKey(SshKeyDoc),
    Database(DatabaseDoc),
    ApiKey(ApiKeyDoc),
}

impl EntryBody {
    pub fn entry_type(&self) -> EntryType {
        match self {
            EntryBody::Login(_) => EntryType::Login,
            EntryBody::Secret(_) => EntryType::Secret,
            EntryBody::Env(_) => EntryType::Env,
            EntryBody::File(_) => EntryType::File,
            EntryBody::Note(_) => EntryType::Note,
            EntryBody::SshKey(_) => EntryType::SshKey,
            EntryBody::Database(_) => EntryType::Database,
            EntryBody::ApiKey(_) => EntryType::ApiKey,
        }
    }

    fn zeroize_secrets(&mut self) {
        match self {
            EntryBody::Login(d) => {
                d.username.zeroize();
                d.password.zeroize();
                if let Some(t) = d.totp.as_mut() {
                    t.secret.zeroize();
                }
                for c in &mut d.custom {
                    c.value.zeroize();
                }
            }
            EntryBody::Secret(d) => d.value.zeroize(),
            EntryBody::Env(d) => {
                for v in &mut d.vars {
                    v.value.zeroize();
                }
            }
            EntryBody::File(d) => d.content_base64.zeroize(),
            EntryBody::Note(d) => d.body.zeroize(),
            EntryBody::SshKey(d) => {
                d.private_key.zeroize();
                d.passphrase.zeroize();
            }
            EntryBody::Database(d) => d.password.zeroize(),
            EntryBody::ApiKey(d) => {
                d.key.zeroize();
                d.secret.zeroize();
            }
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginDoc {
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub totp: Option<TotpDoc>,
    #[serde(default)]
    pub custom: Vec<CustomFieldDoc>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct TotpDoc {
    /// Base32, uppercase, no padding.
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: u8,
    pub period: u32,
    #[serde(default)]
    pub issuer: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomFieldDoc {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SecretDoc {
    pub value: String,
    #[serde(default = "yes")]
    pub hidden: bool,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvDoc {
    pub vars: Vec<EnvVarDoc>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVarDoc {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDoc {
    pub filename: String,
    pub mime: String,
    pub content_base64: String,
    pub size: u64,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteDoc {
    pub body: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SshKeyDoc {
    pub private_key: String,
    pub public_key: String,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseDoc {
    pub engine: String,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub options: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiKeyDoc {
    pub key: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// `Debug` that never prints contents (secret-bearing documents).
macro_rules! redacted_debug {
    ($($t:ty),* $(,)?) => {
        $(impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}(<redacted>)", stringify!($t))
            }
        })*
    };
}

redacted_debug!(
    LoginDoc,
    TotpDoc,
    CustomFieldDoc,
    SecretDoc,
    EnvDoc,
    EnvVarDoc,
    FileDoc,
    NoteDoc,
    SshKeyDoc,
    DatabaseDoc,
    ApiKeyDoc,
    VaultKeyDoc,
);

// Metadata-bearing documents (names, usernames, URLs, bookmarks are
// confidential, SPEC §3.1): `Debug` shows ids, counts and types only.
impl std::fmt::Debug for ManifestDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManifestDoc")
            .field("version", &self.version)
            .field("profiles", &self.profiles)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ManifestProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManifestProfile")
            .field("id", &self.id)
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ManifestEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManifestEntry")
            .field("id", &self.id)
            .field("type", &self.entry_type)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ProfileDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileDoc")
            .field("id", &self.id)
            .field("browser", &self.browser)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for BrowserDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserDoc")
            .field("start_pages", &self.start_pages.len())
            .field("bookmarks", &self.bookmarks.len())
            .finish_non_exhaustive()
    }
}

redacted_debug!(BookmarkDoc, PendingKeysDoc);

impl std::fmt::Debug for EntryBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntryBody::{:?}(<redacted>)", self.entry_type())
    }
}

// ---------------------------------------------------------------------------
// Generated plaintext files
// ---------------------------------------------------------------------------

pub const GITATTRIBUTES: &str = "# Generated by Latch. Encrypted files are binary; never diff or merge them as text.\n\
*.age binary\n\
.latch/manifest.sig -diff -merge\n";

pub const GITIGNORE: &str = "# Generated by Latch. Nothing is ignored on purpose: `latch doctor` verifies\n\
# that every tracked file is ciphertext or public configuration.\n";

pub fn readme(vault_id: &str) -> String {
    format!(
        "# Latch vault\n\n\
This repository is a [Latch](https://github.com/) secret vault (vault id `{vault_id}`).\n\n\
Everything under `profiles/` and every `*.age` file is encrypted with\n\
[age](https://age-encryption.org). Names, usernames, URLs and values are inside\n\
the ciphertext. The repository is designed to be safe to publish: a reader learns\n\
only the number and approximate size of entries, the KDF parameters, the\n\
recipient public keys and commit metadata.\n\n\
## Layout\n\n\
- `.latch/version` - format version\n\
- `.latch/config.toml` - public configuration (KDF parameters, recipients)\n\
- `.latch/keychain.json` - signed history of the vault signing keys\n\
- `.latch/vaultkey.age` - vault key, encrypted to every recipient\n\
- `.latch/manifest.age`, `.latch/manifest.sig` - signed, encrypted index\n\
- `profiles/<id>/profile.age`, `profiles/<id>/entries/<id>.age` - one encrypted file each\n\n\
## Usage\n\n\
```\n\
latch unlock\n\
latch ls\n\
latch env <profile> -- <command>\n\
```\n\n\
Do not edit files in this repository by hand. Use `latch doctor` to verify it.\n"
    )
}

pub fn root_path(root: &Path, rel: &str) -> PathBuf {
    root.join(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let p = super::super::ids::new_id();
        let e = super::super::ids::new_id();
        assert_eq!(parse_entry_path(&entry_file(&p, &e)), Some((p.clone(), e)));
        assert_eq!(parse_profile_path(&profile_file(&p)), Some(p.clone()));
        assert_eq!(parse_entry_path("profiles/x/entries/y.age"), None);
        assert_eq!(parse_entry_path(&profile_file(&p)), None);
        assert_eq!(parse_profile_path("profiles/../profile.age"), None);
    }

    #[test]
    fn version_file() {
        let t = tempfile::tempdir().unwrap();
        assert!(matches!(read_version(t.path()), Err(Error::NoVault(_))));
        std::fs::create_dir_all(t.path().join(LATCH_DIR)).unwrap();
        std::fs::write(t.path().join(VERSION_FILE), "1\n").unwrap();
        assert_eq!(read_version(t.path()).unwrap(), 1);
        std::fs::write(t.path().join(VERSION_FILE), "2\n").unwrap();
        assert!(matches!(
            read_version(t.path()),
            Err(Error::UnsupportedVersion { found: 2, .. })
        ));
        std::fs::write(t.path().join(VERSION_FILE), "x").unwrap();
        assert!(matches!(read_version(t.path()), Err(Error::Integrity(_))));
    }

    fn sample_config() -> ConfigDoc {
        ConfigDoc {
            version: 1,
            kdf: KdfDoc::new(&crypto::WEAK_KDF, &[1u8; 16]),
            vault: VaultMetaDoc {
                id: "abc".into(),
                created_at: now(),
                signing_public_key: "ed25519:xx".into(),
            },
            recipients: vec![RecipientDoc {
                label: "passphrase".into(),
                kind: "passphrase".into(),
                public_key: "age1xyz".into(),
                added_at: Some(now()),
            }],
            sessions: SessionsDoc::default(),
            sync: SyncDoc::default(),
            hooks: HooksDoc::default(),
        }
    }

    #[test]
    fn config_roundtrip_and_hash() {
        let t = tempfile::tempdir().unwrap();
        let mut c = sample_config();
        c.write(t.path()).unwrap();
        let text = std::fs::read_to_string(t.path().join(CONFIG_FILE)).unwrap();
        assert!(text.contains("[[recipients]]"));
        assert!(text.contains("[sync]"));
        let back = ConfigDoc::read(t.path()).unwrap();
        assert_eq!(back, c);
        let h = c.security_hash();
        c.sync.auto_push = true;
        assert_ne!(h, c.security_hash(), "settings are bound too");
        c.sync.auto_push = false;
        assert_eq!(h, c.security_hash());
        c.recipients[0].public_key = "age1evil".into();
        assert_ne!(h, c.security_hash());
    }

    #[test]
    fn config_newer_version_rejected() {
        let t = tempfile::tempdir().unwrap();
        let mut c = sample_config();
        c.version = 9;
        c.write(t.path()).unwrap();
        assert!(matches!(
            ConfigDoc::read(t.path()),
            Err(Error::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn entry_doc_shape() {
        let d = EntryDoc {
            version: 1,
            id: "i".into(),
            profile_id: "p".into(),
            name: "n".into(),
            tags: vec![],
            notes: String::new(),
            favorite: false,
            created_at: now(),
            updated_at: now(),
            body: EntryBody::Secret(SecretDoc {
                value: "hunter2".into(),
                hidden: true,
            }),
        };
        let v: serde_json::Value = serde_json::from_slice(&d.to_json()).unwrap();
        assert_eq!(v["type"], "secret");
        assert_eq!(v["data"]["value"], "hunter2");
        assert!(!format!("{d:?}").contains("hunter2"));
        assert!(!format!("{:?}", d.body).contains("hunter2"));
        let back = EntryDoc::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
        // Malformed documents never echo their contents.
        let err =
            EntryDoc::from_json(br#"{"type": "secret", "data": {"value": 5}, "x": "hunter2"}"#)
                .unwrap_err();
        assert!(!err.to_string().contains("hunter2"));
    }

    #[test]
    fn pinned_enum_spellings() {
        // On-disk docs reuse these protocol enums; their spellings are format.
        assert_eq!(
            serde_json::to_string(&EntryType::SshKey).unwrap(),
            "\"ssh_key\""
        );
        assert_eq!(
            serde_json::to_string(&EntryType::ApiKey).unwrap(),
            "\"api_key\""
        );
        assert_eq!(
            serde_json::to_string(&TotpAlgorithm::Sha256).unwrap(),
            "\"SHA256\""
        );
        assert_eq!(
            serde_json::to_string(&crate::protocol::BrowserEngine::Firefox).unwrap(),
            "\"firefox\""
        );
    }

    #[test]
    fn profile_validation() {
        let mut input = ProfileInput {
            name: "acme".into(),
            color: "#2563EB".into(),
            icon: "briefcase".into(),
            description: String::new(),
            pinned: false,
            browser: BrowserDoc::default().to_protocol(),
        };
        assert!(validate_profile_input(&input).is_ok());
        input.color = "blue".into();
        assert!(validate_profile_input(&input).is_err());
        input.color = "#000000".into();
        input.name = " ".into();
        assert!(validate_profile_input(&input).is_err());
        input.name = "a/b".into();
        assert!(validate_profile_input(&input).is_err());
    }

    #[test]
    fn atomic_write() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("a/b/c.txt");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn time_format() {
        let n = now();
        assert!(n.ends_with('Z'), "{n}");
        assert!(parse_time(&n).is_some());
    }
}
