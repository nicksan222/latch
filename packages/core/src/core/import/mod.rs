//! Importers (SPEC §12).
//!
//! Every format parses into a [`Parsed`] list of [`ImportedEntry`] values
//! (wire payloads plus the source folder/collection), which is then planned
//! against the vault: target profile resolution, duplicate detection and
//! name de-duplication. `preview` returns the plan; `apply` executes it and
//! commits once.
//!
//! Profile mapping when `ImportParams::profile_id` is `None`: the importer's
//! top-level folder/collection/group becomes the profile (matched by name,
//! created when missing); entries without one go to `imported`. Deeper folder
//! levels become tags.
//!
//! Duplicates: same profile + same name + same type, or (logins) same
//! username and a common URL host, compared with existing entries and with
//! earlier items of the same import.

// Defined before `kinds` so every format declares its own registration locally.
macro_rules! import_kind {
    (format: $format:ident, id: $id:literal, parse: $parse:path $(,)?) => {
        inventory::submit! {
            crate::core::import::ImportKind {
                format: crate::protocol::ImportFormat::$format,
                id: $id,
                parse: $parse,
            }
        }
    };
}

mod csv_formats;
mod generic;
mod kinds;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use zeroize::Zeroizing;

use crate::core::export::ArchiveDoc;
use crate::core::format::BrowserDoc;
use crate::core::secrets;
use crate::core::{Error, Result, UnlockedVault};
use crate::protocol::{
    CustomField, EntryInput, EntryPayload, EntryType, ImportApplyResult, ImportFormat, ImportItem,
    ImportParams, ImportPreview, ListEntriesParams, ProfileInput, ProfileUpdateParams,
    TotpAlgorithm, TotpConfig,
};

/// Default profile for entries without a folder.
pub const DEFAULT_PROFILE: &str = "imported";

/// Largest input file accepted (the per-entry 1 MiB limit still applies).
const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;

/// One parsed item, not yet bound to a profile.
#[derive(Clone)]
pub struct ImportedEntry {
    /// Top-level folder / collection / group (→ profile name).
    pub folder: Option<String>,
    pub name: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub payload: EntryPayload,
    /// Id in the source (Latch archives: used to remap autofill selections).
    pub source_id: Option<String>,
}

impl std::fmt::Debug for ImportedEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportedEntry")
            .field("folder", &self.folder)
            .field("name", &self.name)
            .field("type", &payload_type(&self.payload))
            .finish_non_exhaustive()
    }
}

impl ImportedEntry {
    pub fn new(name: impl Into<String>, payload: EntryPayload) -> Self {
        ImportedEntry {
            folder: None,
            name: name.into(),
            tags: vec![],
            notes: String::new(),
            favorite: false,
            payload,
            source_id: None,
        }
    }
    pub fn entry_type(&self) -> EntryType {
        payload_type(&self.payload)
    }
}

/// Result of parsing a source file.
#[derive(Debug, Default)]
pub struct Parsed {
    pub entries: Vec<ImportedEntry>,
    /// Full profile definitions (Latch archives), keyed by name.
    pub profiles: Vec<ProfileInput>,
    /// Autofill selections by profile name (source entry ids).
    pub autofill_ids: HashMap<String, Vec<String>>,
    /// Saved launches keyed by source profile name; env references are remapped on restore.
    pub actions: HashMap<String, Vec<crate::protocol::LaunchAction>>,
    pub warnings: Vec<String>,
}

pub fn payload_type(p: &EntryPayload) -> EntryType {
    crate::core::secrets::payload_type(p)
}

/// A concrete source format and its parser. Registration order is irrelevant.
pub struct ImportKind {
    pub format: ImportFormat,
    pub id: &'static str,
    parse: fn(&Path, Option<&str>) -> Result<Parsed>,
}

inventory::collect!(ImportKind);

/// Registered import formats, sorted by stable identifier.
pub fn registered_kinds() -> Vec<&'static ImportKind> {
    let mut kinds: Vec<_> = inventory::iter::<ImportKind>.into_iter().collect();
    for kind in &kinds {
        resolve_strategy(kind.format, kinds.iter().copied()).expect("invalid import kind registry");
    }
    kinds.sort_by_key(|kind| kind.id);
    kinds
}

fn resolve_strategy(
    format: ImportFormat,
    kinds: impl IntoIterator<Item = &'static ImportKind>,
) -> Result<&'static ImportKind> {
    let kinds: Vec<_> = kinds.into_iter().collect();
    for (i, kind) in kinds.iter().enumerate() {
        if kind.id.is_empty() {
            return Err(Error::Internal("missing import strategy identity".into()));
        }
        if kinds[..i]
            .iter()
            .any(|other| other.id == kind.id || other.format == kind.format)
        {
            return Err(Error::Internal("duplicate import strategy".into()));
        }
    }
    let mut matches = kinds.into_iter().filter(|kind| kind.format == format);
    let kind = matches
        .next()
        .ok_or_else(|| Error::Internal("missing import strategy".into()))?;
    Ok(kind)
}

fn strategy(format: ImportFormat) -> Result<&'static ImportKind> {
    resolve_strategy(format, inventory::iter::<ImportKind>)
}

/// Parse a source without touching any vault.
pub fn parse(format: ImportFormat, path: &Path, passphrase: Option<&str>) -> Result<Parsed> {
    let mut parsed = (strategy(format)?.parse)(path, passphrase)?;
    for e in &mut parsed.entries {
        e.name = sanitize_name(&e.name, 256, "Untitled");
        e.folder = e
            .folder
            .as_deref()
            .map(|f| sanitize_name(f, 128, DEFAULT_PROFILE))
            .filter(|f| !f.is_empty());
    }
    Ok(parsed)
}

/// Convert a decrypted Latch archive (or plaintext Latch JSON export).
pub(crate) fn from_archive(doc: ArchiveDoc) -> Parsed {
    let mut out = Parsed::default();
    let names: HashMap<&str, &str> = doc
        .profiles
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();
    for p in &doc.profiles {
        if !p.actions.is_empty() {
            out.actions.insert(p.name.to_lowercase(), p.actions.clone());
        }
        let mut browser = p.browser.clone();
        if !browser.autofill_entry_ids.is_empty() {
            out.autofill_ids
                .insert(p.name.to_lowercase(), browser.autofill_entry_ids.clone());
        }
        browser.autofill_entry_ids.clear();
        out.profiles.push(ProfileInput {
            name: p.name.clone(),
            color: p.color.clone(),
            icon: p.icon.clone(),
            description: p.description.clone(),
            pinned: p.pinned,
            browser,
        });
    }
    for e in &doc.entries {
        let folder = names.get(e.profile_id.as_str()).map(|n| n.to_string());
        if folder.is_none() {
            out.warnings.push(format!(
                "entry `{}` references an unknown profile; importing into `{DEFAULT_PROFILE}`",
                e.name
            ));
        }
        out.entries.push(ImportedEntry {
            folder,
            name: e.name.clone(),
            tags: e.tags.clone(),
            notes: e.notes.clone(),
            favorite: e.favorite,
            payload: e.payload.clone(),
            source_id: Some(e.id.clone()),
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Existing { id: String, name: String },
    New { name: String },
}

impl Target {
    fn name(&self) -> &str {
        match self {
            Target::Existing { name, .. } | Target::New { name } => name,
        }
    }
    fn key(&self) -> String {
        self.name().to_lowercase()
    }
}

#[derive(Debug)]
struct PlanItem {
    idx: usize,
    target: Target,
    name: String,
    duplicate: bool,
}

struct Plan {
    items: Vec<PlanItem>,
    warnings: Vec<String>,
}

/// Login identity used for duplicate detection.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoginKey {
    username: String,
    hosts: Vec<String>,
}

impl LoginKey {
    fn matches(&self, other: &LoginKey) -> bool {
        !self.username.is_empty()
            && self.username == other.username
            && self.hosts.iter().any(|h| other.hosts.contains(h))
    }
}

#[derive(Debug, Clone)]
struct Known {
    name: String,
    ty: EntryType,
    login: Option<LoginKey>,
}

fn login_key(p: &EntryPayload) -> Option<LoginKey> {
    match p {
        EntryPayload::Login(l) => Some(LoginKey {
            username: l.username.trim().to_owned(),
            hosts: l.urls.iter().filter_map(|u| host_of(u)).collect(),
        }),
        _ => None,
    }
}

/// Lowercased host of a URL (scheme optional), without `www.`.
pub(crate) fn host_of(u: &str) -> Option<String> {
    let u = u.trim();
    if u.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(u)
        .ok()
        .filter(|p| p.host_str().is_some())
        .or_else(|| url::Url::parse(&format!("https://{u}")).ok())?;
    let h = parsed.host_str()?.to_ascii_lowercase();
    Some(h.strip_prefix("www.").unwrap_or(&h).to_owned())
}

fn existing_known(vault: &UnlockedVault, profile_id: &str) -> Result<Vec<Known>> {
    let list = vault.list_entries(&ListEntriesParams {
        profile_id: Some(profile_id.to_owned()),
        query: None,
        entry_type: None,
    })?;
    let mut out = Vec::with_capacity(list.len());
    for s in list {
        let login = if s.entry_type == EntryType::Login {
            let doc = vault.read_entry_doc(&s.id)?;
            login_key(&secrets::payload(&doc.body, true))
        } else {
            None
        };
        out.push(Known {
            name: s.name.to_lowercase(),
            ty: s.entry_type,
            login,
        });
    }
    Ok(out)
}

/// Validate an item as `create_entry` would (without a profile).
fn validate(e: &ImportedEntry) -> Result<()> {
    let input = EntryInput {
        profile_id: String::new(),
        name: e.name.clone(),
        tags: e.tags.clone(),
        notes: e.notes.clone(),
        favorite: e.favorite,
        payload: e.payload.clone(),
    };
    secrets::validate_input(&input)?;
    let body = secrets::body_from_payload(&e.payload, None)?;
    let json =
        Zeroizing::new(serde_json::to_vec(&body).map_err(|_| Error::Internal("serialize".into()))?);
    secrets::check_size(&json)?;
    Ok(())
}

fn plan(vault: &UnlockedVault, params: &ImportParams, parsed: &Parsed) -> Result<Plan> {
    let mut warnings = parsed.warnings.clone();
    let profiles = vault.list_profiles()?;
    let fixed = match &params.profile_id {
        Some(id) => {
            let p = profiles
                .iter()
                .find(|p| &p.id == id)
                .ok_or_else(|| Error::NotFound("profile".into()))?;
            Some(Target::Existing {
                id: p.id.clone(),
                name: p.name.clone(),
            })
        }
        None => None,
    };
    let resolve = |folder: Option<&str>| -> Target {
        if let Some(t) = &fixed {
            return t.clone();
        }
        let name = folder.unwrap_or(DEFAULT_PROFILE);
        match profiles.iter().find(|p| p.name.eq_ignore_ascii_case(name)) {
            Some(p) => Target::Existing {
                id: p.id.clone(),
                name: p.name.clone(),
            },
            None => Target::New {
                name: name.to_owned(),
            },
        }
    };

    // Known entries per target profile (existing + already planned).
    let mut known: HashMap<String, Vec<Known>> = HashMap::new();
    // Names taken per target profile, lowercased.
    let mut taken: HashMap<String, HashSet<String>> = HashMap::new();
    let mut items = vec![];
    for (idx, e) in parsed.entries.iter().enumerate() {
        if let Err(err) = validate(e) {
            warnings.push(format!("skipped `{}`: {err}", e.name));
            continue;
        }
        let target = resolve(e.folder.as_deref());
        let key = target.key();
        if !known.contains_key(&key) {
            let k = match &target {
                Target::Existing { id, .. } => existing_known(vault, id)?,
                Target::New { .. } => vec![],
            };
            taken.insert(key.clone(), k.iter().map(|k| k.name.clone()).collect());
            known.insert(key.clone(), k);
        }
        let ty = e.entry_type();
        let lk = login_key(&e.payload);
        let lname = e.name.to_lowercase();
        let ks = known.get_mut(&key).expect("inserted above");
        let duplicate = ks.iter().any(|k| {
            let same_name = k.name == lname && k.ty == ty;
            let same_login = match (&k.login, &lk) {
                (Some(a), Some(b)) => a.matches(b),
                _ => false,
            };
            same_name || same_login
        });
        let names = taken.get_mut(&key).expect("inserted above");
        let mut name = e.name.clone();
        if !(duplicate && params.skip_duplicates) {
            let mut n = 2;
            while names.contains(&name.to_lowercase()) {
                name = format!("{} ({n})", e.name);
                n += 1;
            }
            names.insert(name.to_lowercase());
            ks.push(Known {
                name: lname,
                ty,
                login: lk,
            });
        }
        items.push(PlanItem {
            idx,
            target,
            name,
            duplicate,
        });
    }
    Ok(Plan { items, warnings })
}

pub fn preview(vault: &UnlockedVault, params: &ImportParams) -> Result<ImportPreview> {
    let parsed = parse(
        params.format,
        Path::new(&params.path),
        params.passphrase.as_deref(),
    )?;
    let plan = plan(vault, params, &parsed)?;
    Ok(ImportPreview {
        items: plan
            .items
            .iter()
            .map(|i| ImportItem {
                name: i.name.clone(),
                entry_type: parsed.entries[i.idx].entry_type(),
                profile_name: i.target.name().to_owned(),
                duplicate: i.duplicate,
            })
            .collect(),
        warnings: plan.warnings,
    })
}

const PALETTE: &[&str] = &[
    "#2563eb", "#16a34a", "#db2777", "#ea580c", "#7c3aed", "#0891b2", "#ca8a04", "#dc2626",
];

fn default_profile_input(name: &str, n: usize) -> ProfileInput {
    ProfileInput {
        name: name.to_owned(),
        color: PALETTE[n % PALETTE.len()].to_owned(),
        icon: "folder".into(),
        description: "Imported".into(),
        pinned: false,
        browser: BrowserDoc::default().to_protocol(),
    }
}

/// Input for a profile the plan creates (archive definition or default).
fn new_profile_input(parsed: &Parsed, name: &str, n: usize) -> ProfileInput {
    parsed
        .profiles
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .cloned()
        .map(|mut p| {
            // The planned (sanitized) name wins.
            p.name = name.to_owned();
            p
        })
        .unwrap_or_else(|| default_profile_input(name, n))
}

/// Apply an import all-or-nothing: the whole plan (every new profile
/// definition) is validated first; writes are uncommitted and tracked, and
/// any failure (including the commit) restores the work tree and the
/// in-memory state. Autofill selections of created profiles are restored
/// before the single commit.
pub fn apply(vault: &mut UnlockedVault, params: &ImportParams) -> Result<ImportApplyResult> {
    let parsed = parse(
        params.format,
        Path::new(&params.path),
        params.passphrase.as_deref(),
    )?;
    let plan = plan(vault, params, &parsed)?;

    // Validate every profile the plan would create.
    let mut new_inputs: BTreeMap<String, ProfileInput> = BTreeMap::new();
    let base = vault.manifest().profiles.len();
    for item in &plan.items {
        if item.duplicate && params.skip_duplicates {
            continue;
        }
        if let Target::New { name } = &item.target {
            let key = name.to_lowercase();
            if !new_inputs.contains_key(&key) {
                let input = new_profile_input(&parsed, name, base + new_inputs.len());
                crate::core::format::validate_profile_input(&input).map_err(|e| {
                    Error::invalid(format!("import: profile `{name}` is invalid: {e}"))
                })?;
                new_inputs.insert(key, input);
            }
        }
    }

    if params.profile_id.is_none() {
        let existing = vault.list_profiles()?;
        for p in &parsed.profiles {
            let key = p.name.to_lowercase();
            if parsed.actions.contains_key(&key)
                && !existing
                    .iter()
                    .any(|e| e.name.eq_ignore_ascii_case(&p.name))
            {
                crate::core::format::validate_profile_input(p)?;
                new_inputs.entry(key).or_insert_with(|| p.clone());
            }
        }
    }

    vault.ensure_writable()?;
    let mut txn = vault.begin_txn();
    let mut res = ImportApplyResult {
        created: 0,
        skipped: (parsed.entries.len() - plan.items.len()) as u32,
        profiles_created: 0,
    };
    let mut new_profiles: BTreeMap<String, String> = BTreeMap::new();
    let mut id_map: HashMap<String, String> = HashMap::new();
    let outcome = (|| -> Result<()> {
        for (key, input) in &new_inputs {
            let (p, _) = vault.create_profile_uncommitted(input)?;
            txn.created_dirs
                .push(crate::core::format::profile_dir(&p.id));
            res.profiles_created += 1;
            new_profiles.insert(key.clone(), p.id);
        }
        for item in &plan.items {
            if item.duplicate && params.skip_duplicates {
                res.skipped += 1;
                continue;
            }
            let pid = match &item.target {
                Target::Existing { id, .. } => id.clone(),
                Target::New { name } => match new_profiles.get(&name.to_lowercase()) {
                    Some(id) => id.clone(),
                    None => {
                        let input = new_inputs
                            .get(&name.to_lowercase())
                            .cloned()
                            .ok_or_else(|| Error::Internal("import plan".into()))?;
                        let (p, _) = vault.create_profile_uncommitted(&input)?;
                        txn.created_dirs
                            .push(crate::core::format::profile_dir(&p.id));
                        res.profiles_created += 1;
                        new_profiles.insert(name.to_lowercase(), p.id.clone());
                        p.id
                    }
                },
            };
            let e = &parsed.entries[item.idx];
            let (created, _) = vault.create_entry_uncommitted(&EntryInput {
                profile_id: pid.clone(),
                name: item.name.clone(),
                tags: e.tags.clone(),
                notes: e.notes.clone(),
                favorite: e.favorite,
                payload: e.payload.clone(),
            })?;
            txn.created_files
                .push(crate::core::format::entry_file(&pid, &created.id));
            if let Some(src) = &e.source_id {
                id_map.insert(src.clone(), created.id);
            }
            res.created += 1;
        }
        // Launch actions are definitions only. Import never executes commands.
        // Restore on new profiles, consistent with browser-setting restore policy.
        for (name, pid) in &new_profiles {
            if let Some(actions) = parsed.actions.get(name) {
                for a in actions {
                    let env_entry_ids = a.env_entry_ids.iter().map(|old|id_map.get(old).cloned().ok_or_else(||Error::invalid("an imported action environment source was skipped or is missing"))).collect::<Result<Vec<_>>>()?;
                    vault.save_action_uncommitted(&crate::protocol::ActionSaveParams {
                        profile_id: pid.clone(),
                        id: None,
                        input: crate::protocol::ActionInput {
                            name: a.name.clone(),
                            command: a.command.clone(),
                            env_entry_ids,
                            env_keys: a.env_keys.clone(),
                            urls: a.urls.clone(),
                        },
                    })?;
                }
            }
        }
        // Latch archives: restore autofill selections on created profiles.
        for (lname, pid) in &new_profiles {
            let Some(old_ids) = parsed.autofill_ids.get(lname) else {
                continue;
            };
            let ids: Vec<String> = old_ids
                .iter()
                .filter_map(|o| id_map.get(o).cloned())
                .collect();
            if ids.is_empty() {
                continue;
            }
            let p = vault.get_profile(pid)?;
            let mut browser = p.browser.clone();
            browser.autofill_entry_ids = ids;
            vault.update_profile_uncommitted(&ProfileUpdateParams {
                id: pid.clone(),
                input: ProfileInput {
                    name: p.name,
                    color: p.color,
                    icon: p.icon,
                    description: p.description,
                    pinned: p.pinned,
                    browser,
                },
            })?;
        }
        if res.created > 0 || res.profiles_created > 0 {
            let msg = format!(
                "import: {} entries, {} profiles",
                res.created, res.profiles_created
            );
            vault.commit(&msg)?;
        }
        Ok(())
    })();
    if let Err(e) = outcome {
        vault.rollback(txn);
        return Err(e);
    }
    Ok(res)
}

// ---------------------------------------------------------------------------
// Shared helpers for the parsers
// ---------------------------------------------------------------------------

pub(crate) fn read_bytes(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let meta =
        std::fs::metadata(path).map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    if meta.is_dir() {
        return Err(Error::invalid(format!("{} is a directory", path.display())));
    }
    if meta.len() > MAX_INPUT_BYTES {
        return Err(Error::invalid("import file is larger than 64 MiB"));
    }
    std::fs::read(path)
        .map(Zeroizing::new)
        .map_err(|e| Error::io(format!("read {}", path.display()), e))
}

pub(crate) fn read_text(path: &Path) -> Result<Zeroizing<String>> {
    let bytes = read_bytes(path)?;
    let s = std::str::from_utf8(&bytes)
        .map_err(|_| Error::invalid(format!("{} is not valid UTF-8", path.display())))?;
    // Strip a UTF-8 BOM (common in CSV exports).
    Ok(Zeroizing::new(s.trim_start_matches('\u{feff}').to_owned()))
}

/// File name without the final extension (`.env` stays `.env`).
pub(crate) fn file_stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_owned(),
        _ => name,
    }
}

/// Make a valid entry/profile name: no `/` or control chars, trimmed, max
/// `max` bytes, `fallback` when empty.
pub(crate) fn sanitize_name(s: &str, max: usize, fallback: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c == '/' { '-' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    out = out.trim().to_owned();
    if out.len() > max {
        let mut cut = max;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out = out.trim().to_owned();
    }
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out
    }
}

/// Split a `/`-separated folder path: first segment → profile, rest → tags.
pub(crate) fn split_folder(path: &str) -> (Option<String>, Vec<String>) {
    let mut parts = path
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let first = parts.next();
    (first, parts.collect())
}

/// Parse a TOTP value: `otpauth://` URI or a bare base32 secret.
pub(crate) fn parse_totp(s: &str) -> Option<TotpConfig> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.to_ascii_lowercase().starts_with("otpauth://") {
        let d = crate::core::totp::parse_otpauth(s).ok()?;
        return Some(TotpConfig {
            secret: Some(d.secret.clone()),
            algorithm: d.algorithm,
            digits: d.digits,
            period: d.period,
            issuer: d.issuer.clone(),
        });
    }
    let secret = crate::core::totp::normalize_secret(s).ok()?;
    Some(TotpConfig {
        secret: Some(secret),
        algorithm: TotpAlgorithm::Sha1,
        digits: 6,
        period: 30,
        issuer: None,
    })
}

/// Accumulates custom fields with unique keys (`key`, `key (2)`, …).
#[derive(Default)]
pub(crate) struct CustomFields(pub Vec<CustomField>);

impl CustomFields {
    pub fn push(&mut self, key: &str, value: &str, hidden: bool) {
        let base = key.trim();
        let base = if base.is_empty() { "Field" } else { base };
        let mut k = base.to_owned();
        let mut n = 2;
        while self.0.iter().any(|c| c.key == k) {
            k = format!("{base} ({n})");
            n += 1;
        }
        self.0.push(CustomField {
            key: k,
            value: Some(value.to_owned()),
            hidden,
        });
    }
}

/// Guess a MIME type from a file name.
pub(crate) fn guess_mime(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "md" | "env" => "text/plain",
        "json" => "application/json",
        "pem" | "crt" | "key" => "application/x-pem-file",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "zip" => "application/zip",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize_name(" a/b\u{7} ", 256, "x"), "a-b");
        assert_eq!(sanitize_name("  ", 256, "x"), "x");
        assert_eq!(sanitize_name("ééé", 3, "x"), "é");
    }

    #[test]
    fn hosts() {
        assert_eq!(
            host_of("https://www.GitHub.com/login").as_deref(),
            Some("github.com")
        );
        assert_eq!(host_of("example.org").as_deref(), Some("example.org"));
        assert_eq!(host_of(""), None);
    }

    #[test]
    fn folders_split() {
        assert_eq!(
            split_folder("Work/Clients/Acme"),
            (Some("Work".into()), vec!["Clients".into(), "Acme".into()])
        );
        assert_eq!(split_folder(""), (None, vec![]));
    }

    #[test]
    fn totp_values() {
        let t = parse_totp("otpauth://totp/X?secret=JBSWY3DPEHPK3PXP&digits=8").unwrap();
        assert_eq!(t.digits, 8);
        let t = parse_totp("jbsw y3dp ehpk 3pxp").unwrap();
        assert_eq!(t.secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
        assert!(parse_totp("not base32 !!").is_none());
    }

    #[test]
    fn custom_fields_unique() {
        let mut c = CustomFields::default();
        c.push("a", "1", false);
        c.push("a", "2", true);
        c.push("", "3", false);
        let keys: Vec<_> = c.0.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["a", "a (2)", "Field"]);
    }

    #[test]
    fn stems() {
        assert_eq!(file_stem(Path::new("/x/.env")), ".env");
        assert_eq!(file_stem(Path::new("/x/secrets.yaml")), "secrets");
        assert_eq!(guess_mime("a.PEM"), "application/x-pem-file");
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn registry_covers_each_format_once_and_routes() {
        let expected = [
            (ImportFormat::Bitwarden, "bitwarden", "bitwarden.json"),
            (ImportFormat::BrowserCsv, "browser_csv", "chrome.csv"),
            (ImportFormat::Dotenv, "dotenv", "sample.env"),
            (ImportFormat::Json, "json", "generic.json"),
            (ImportFormat::KeepassXml, "keepass_xml", "keepass.xml"),
            (
                ImportFormat::LatchArchive,
                "latch_archive",
                "archive.latch.age",
            ),
            (
                ImportFormat::OnePasswordCsv,
                "one_password_csv",
                "1password.csv",
            ),
            (ImportFormat::Pass, "pass", ""),
            (ImportFormat::Yaml, "yaml", "generic.yaml"),
        ];
        let kinds = registered_kinds();
        assert_eq!(kinds.len(), expected.len());
        assert!(kinds.windows(2).all(|pair| pair[0].id < pair[1].id));
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/import");
        for (format, id, fixture) in expected {
            assert_eq!(kinds.iter().filter(|kind| kind.format == format).count(), 1);
            assert_eq!(strategy(format).unwrap().id, id);
            if !fixture.is_empty() {
                let parsed =
                    parse(format, &fixtures.join(fixture), Some("fixture passphrase")).unwrap();
                assert!(!parsed.entries.is_empty(), "{id}");
            }
        }
        let store = tempfile::tempdir().unwrap();
        assert!(
            parse(ImportFormat::Pass, store.path(), None)
                .unwrap()
                .entries
                .is_empty()
        );
        let err = parse(ImportFormat::LatchArchive, store.path(), None).unwrap_err();
        assert!(err.to_string().contains("a passphrase is required"));
    }
}

#[cfg(test)]
mod registry_failure_tests {
    use super::*;

    fn unused_parse(_: &Path, _: Option<&str>) -> Result<Parsed> {
        unreachable!("registry resolution must not parse")
    }

    static KIND: ImportKind = ImportKind {
        format: ImportFormat::Dotenv,
        id: "test",
        parse: unused_parse,
    };

    #[test]
    fn missing_identity_fails_closed() {
        static OTHER: ImportKind = ImportKind {
            format: ImportFormat::Json,
            id: "",
            parse: unused_parse,
        };
        assert!(resolve_strategy(ImportFormat::Dotenv, [&KIND, &OTHER]).is_err());
    }

    #[test]
    fn duplicate_ids_across_formats_fail_closed() {
        static OTHER: ImportKind = ImportKind {
            format: ImportFormat::Json,
            id: "test",
            parse: unused_parse,
        };
        assert!(resolve_strategy(ImportFormat::Dotenv, [&KIND, &OTHER]).is_err());
    }

    #[test]
    fn unrelated_duplicate_formats_fail_closed() {
        static OTHER: ImportKind = ImportKind {
            format: ImportFormat::Json,
            id: "other",
            parse: unused_parse,
        };
        assert!(resolve_strategy(ImportFormat::Dotenv, [&KIND, &OTHER, &OTHER]).is_err());
    }

    #[test]
    fn missing_and_duplicate_strategies_fail_closed() {
        let missing = resolve_strategy(ImportFormat::Dotenv, []).err().unwrap();
        assert!(
            matches!(missing, Error::Internal(ref message) if message == "missing import strategy")
        );
        let duplicate = resolve_strategy(ImportFormat::Dotenv, [&KIND, &KIND])
            .err()
            .unwrap();
        assert!(
            matches!(duplicate, Error::Internal(ref message) if message == "duplicate import strategy")
        );
    }
}
