//! `doctor`: integrity verification, manifest rebuild, plaintext scan of
//! tracked/staged files (SPEC §8.3) and the pre-commit hook.
//!
//! The scan rejects any file that is not either an `.age` file with a valid
//! age header or one of the allowed public files (`.latch/version`,
//! `.latch/config.toml`, `.latch/manifest.sig`, `.gitattributes`,
//! `.gitignore`, `README.md`). Allowed plaintext files are additionally
//! checked for key-looking strings (known token patterns and high-entropy
//! tokens). Findings name the file, line and pattern kind, never the match.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::crypto;
use super::error::{Error, Result};
use super::format::*;
use super::git::Git;
use super::secrets;
use super::vault::{UnlockedVault, Vault};
use crate::protocol::{DoctorIssue, DoctorParams, DoctorReport, Severity};

pub const ALLOWED_PLAINTEXT: &[&str] = &[
    VERSION_FILE,
    CONFIG_FILE,
    KEYCHAIN_FILE,
    MANIFEST_SIG_FILE,
    GITATTRIBUTES_FILE,
    GITIGNORE_FILE,
    README_FILE,
];

const HOOK_MARKER: &str = "# latch pre-commit hook";

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("age secret key", r"AGE-SECRET-KEY-1[0-9A-Z]{20,}"),
        ("private key block", r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----"),
        ("AWS access key id", r"\b(AKIA|ASIA)[0-9A-Z]{16}\b"),
        ("GitHub token", r"\bgh[pousr]_[A-Za-z0-9]{30,}"),
        ("GitHub token", r"\bgithub_pat_[A-Za-z0-9_]{40,}"),
        ("Stripe key", r"\b[sr]k_(live|test)_[A-Za-z0-9]{16,}"),
        ("Slack token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
        ("Google API key", r"\bAIza[0-9A-Za-z_\-]{35}"),
        ("JSON web token", r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"),
        (
            "credential assignment",
            r#"(?i)\b(password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key)\b\s*[:=]\s*["']?[^\s"']{8,}"#,
        ),
    ]
    .into_iter()
    .map(|(k, r)| (k, Regex::new(r).expect("valid regex")))
    .collect()
});

static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/=_\-]{32,}").expect("valid regex"));

static SIG_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ed25519:[A-Za-z0-9+/]{85,86}={0,2}\s*$").expect("valid regex"));

/// Shannon entropy in bits per character.
pub fn entropy(s: &str) -> f64 {
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    let n = s.chars().count();
    if n == 0 {
        return 0.0;
    }
    for c in s.chars() {
        *counts.entry(c).or_default() += 1;
    }
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / n as f64;
            -p * p.log2()
        })
        .sum()
}

fn issue(sev: Severity, code: &str, msg: impl Into<String>, path: Option<&str>) -> DoctorIssue {
    DoctorIssue {
        severity: sev,
        code: code.into(),
        message: msg.into(),
        path: path.map(str::to_owned),
    }
}

/// Check one allowed plaintext file's content for key-looking strings.
/// `allow` holds known public tokens (public keys, salt, ids) to ignore.
pub fn check_plaintext_content(path: &str, text: &str, allow: &[String]) -> Vec<DoctorIssue> {
    let mut out = vec![];
    for (no, line) in text.lines().enumerate() {
        let mut scrubbed = line.to_owned();
        for a in allow.iter().filter(|a| !a.is_empty()) {
            scrubbed = scrubbed.replace(a.as_str(), " ");
        }
        for (kind, re) in PATTERNS.iter() {
            if re.is_match(&scrubbed) {
                out.push(issue(
                    Severity::Error,
                    "suspicious_content",
                    format!("line {}: looks like a {kind}", no + 1),
                    Some(path),
                ));
            }
        }
        for m in TOKEN.find_iter(&scrubbed) {
            if entropy(m.as_str()) > 4.3 {
                out.push(issue(
                    Severity::Error,
                    "high_entropy",
                    format!(
                        "line {}: high-entropy string of {} characters looks like a key",
                        no + 1,
                        m.as_str().len()
                    ),
                    Some(path),
                ));
            }
        }
    }
    out
}

fn known_public_tokens(config_text: &str) -> Vec<String> {
    let Ok(c) = toml::from_str::<ConfigDoc>(config_text) else {
        return vec![];
    };
    let mut v = vec![
        c.kdf.salt.clone(),
        c.vault.id.clone(),
        c.vault.signing_public_key.clone(),
    ];
    if let Some(k) = c.vault.signing_public_key.strip_prefix("ed25519:") {
        v.push(k.to_owned());
    }
    for r in &c.recipients {
        v.push(r.public_key.clone());
        v.extend(r.public_key.split_whitespace().map(str::to_owned));
    }
    v.sort_by_key(|s| std::cmp::Reverse(s.len()));
    v
}

/// Public tokens (salt, ids, public keys) of the config at `root`, which
/// the content scan must not flag.
pub fn public_tokens(root: &Path) -> Vec<String> {
    known_public_tokens(&std::fs::read_to_string(root.join(CONFIG_FILE)).unwrap_or_default())
}

/// Classify and check one file. Only vault paths ([`is_vault_path`]) are
/// allowed; this is the same allowlist Latch commits enforce.
pub fn check_file(path: &str, content: &[u8], public_tokens: &[String]) -> Vec<DoctorIssue> {
    if !is_vault_path(path) {
        return vec![if path.ends_with(".age") {
            issue(
                Severity::Error,
                "unexpected_file",
                "`.age` file outside the vault layout",
                Some(path),
            )
        } else {
            issue(
                Severity::Error,
                "plaintext_file",
                "file is neither ciphertext nor an allowed public file",
                Some(path),
            )
        }];
    }
    if path.ends_with(".age") {
        if crypto::has_valid_age_header(content) {
            return vec![];
        }
        return vec![issue(
            Severity::Error,
            "bad_age_header",
            "`.age` file without a valid age header",
            Some(path),
        )];
    }
    if !ALLOWED_PLAINTEXT.contains(&path) {
        return vec![issue(
            Severity::Error,
            "plaintext_file",
            "file is neither ciphertext nor an allowed public file",
            Some(path),
        )];
    }
    let Ok(text) = std::str::from_utf8(content) else {
        return vec![issue(
            Severity::Error,
            "binary_file",
            "allowed plaintext file is not UTF-8",
            Some(path),
        )];
    };
    match path {
        KEYCHAIN_FILE => match KeychainDoc::parse(content) {
            Ok(_) => vec![],
            Err(_) => vec![issue(
                Severity::Error,
                "bad_keychain",
                "keychain.json is not a Latch keychain",
                Some(path),
            )],
        },
        MANIFEST_SIG_FILE => {
            if SIG_LINE.is_match(text.trim()) {
                vec![]
            } else {
                vec![issue(
                    Severity::Error,
                    "bad_signature_file",
                    "manifest.sig is not a single ed25519 signature",
                    Some(path),
                )]
            }
        }
        VERSION_FILE => {
            if text.trim().parse::<u32>().is_ok() {
                vec![]
            } else {
                vec![issue(
                    Severity::Error,
                    "bad_version_file",
                    ".latch/version must be an integer",
                    Some(path),
                )]
            }
        }
        CONFIG_FILE => {
            let mut v = vec![];
            if toml::from_str::<ConfigDoc>(text).is_err() {
                v.push(issue(
                    Severity::Error,
                    "bad_config",
                    "config.toml does not parse as a Latch config",
                    Some(path),
                ));
            }
            let mut allow = known_public_tokens(text);
            allow.extend(public_tokens.iter().cloned());
            v.extend(check_plaintext_content(path, text, &allow));
            v
        }
        _ => check_plaintext_content(path, text, public_tokens),
    }
}

/// Plaintext scan. `staged`: index contents of staged files (pre-commit);
/// otherwise tracked + untracked (non-ignored) working-tree files. Works on
/// a locked vault. Non-git directories are walked.
pub fn scan(root: &Path, staged: bool) -> Result<Vec<DoctorIssue>> {
    let git = Git::new(root);
    let config_text = std::fs::read_to_string(root.join(CONFIG_FILE)).unwrap_or_default();
    let public = known_public_tokens(&config_text);
    let mut out = vec![];
    if git.is_repo() {
        if staged {
            for f in git.staged_files()? {
                let content = git.show_spec(&format!(":{f}"))?.unwrap_or_default();
                out.extend(check_file(&f, &content, &public));
            }
        } else {
            let mut files = git.ls_files()?;
            files.extend(git.untracked_files()?);
            files.sort();
            files.dedup();
            for f in files {
                let Ok(content) = std::fs::read(root.join(&f)) else {
                    continue; // deleted in the work tree
                };
                out.extend(check_file(&f, &content, &public));
            }
        }
    } else {
        for f in walk(root) {
            let content = std::fs::read(root.join(&f)).unwrap_or_default();
            out.extend(check_file(&f, &content, &public));
        }
    }
    Ok(out)
}

fn walk(root: &Path) -> Vec<String> {
    fn rec(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_name() == ".git" {
                continue;
            }
            if p.is_dir() {
                rec(base, &p, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = vec![];
    rec(root, root, &mut out);
    out.sort();
    out
}

fn report(issues: Vec<DoctorIssue>, fixed: Vec<String>) -> DoctorReport {
    DoctorReport {
        ok: !issues.iter().any(|i| i.severity == Severity::Error),
        issues,
        fixed,
    }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

fn hook_path(git: &Git) -> Result<PathBuf> {
    Ok(git.hooks_dir()?.join("pre-commit"))
}

pub fn hook_script(latch_bin: Option<&Path>) -> String {
    match latch_bin {
        Some(bin) => format!(
            "#!/bin/sh\n{HOOK_MARKER} (managed by Latch; `latch hook uninstall` removes it)\nexec '{}' doctor --staged\n",
            bin.display().to_string().replace('\'', "'\\''")
        ),
        None => format!(
            "#!/bin/sh\n{HOOK_MARKER} (managed by Latch; `latch hook uninstall` removes it)\n\
if command -v latch >/dev/null 2>&1; then\n  exec latch doctor --staged\nfi\n\
echo \"latch: 'latch' not found in PATH; refusing to commit unscanned changes (git commit --no-verify bypasses)\" >&2\n\
exit 1\n"
        ),
    }
}

pub fn hook_installed(git: &Git) -> bool {
    hook_path(git)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|s| s.contains(HOOK_MARKER))
}

/// Install `.git/hooks/pre-commit` running `latch doctor --staged`. Refuses
/// to overwrite a foreign hook.
pub fn install_hook(git: &Git, latch_bin: Option<&Path>) -> Result<PathBuf> {
    let p = hook_path(git)?;
    if let Ok(existing) = std::fs::read_to_string(&p)
        && !existing.contains(HOOK_MARKER)
    {
        return Err(Error::AlreadyExists(format!(
            "a pre-commit hook not managed by Latch at {}",
            p.display()
        )));
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| Error::io(format!("create {}", dir.display()), e))?;
    }
    std::fs::write(&p, hook_script(latch_bin))
        .map_err(|e| Error::io(format!("write {}", p.display()), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| Error::io(format!("chmod {}", p.display()), e))?;
    }
    Ok(p)
}

/// Remove the hook if it is ours. Returns whether something was removed.
pub fn uninstall_hook(git: &Git) -> Result<bool> {
    if !hook_installed(git) {
        return Ok(false);
    }
    let p = hook_path(git)?;
    std::fs::remove_file(&p).map_err(|e| Error::io(format!("remove {}", p.display()), e))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Locked + unlocked doctor
// ---------------------------------------------------------------------------

impl Vault {
    /// Plaintext scan only (no keys needed): what the pre-commit hook runs.
    pub fn scan(&self, staged: bool) -> Result<DoctorReport> {
        Ok(report(scan(self.path(), staged)?, vec![]))
    }

    pub fn install_hook(&self, latch_bin: Option<&Path>) -> Result<PathBuf> {
        install_hook(&Git::new(self.path()), latch_bin)
    }

    pub fn uninstall_hook(&self) -> Result<bool> {
        uninstall_hook(&Git::new(self.path()))
    }
}

/// Result of decrypting one vault file during a rebuild.
pub(crate) struct Rebuilt {
    pub issues: Vec<DoctorIssue>,
    /// Files re-encrypted from a previous vault key to the current one.
    pub reencrypted: usize,
}

impl UnlockedVault {
    pub fn install_hook(&self, latch_bin: Option<&Path>) -> Result<PathBuf> {
        install_hook(&self.git, latch_bin)
    }

    pub fn uninstall_hook(&self) -> Result<bool> {
        uninstall_hook(&self.git)
    }

    /// Decrypt with the current key; fall back to previous keys (flagging
    /// the file for re-encryption).
    pub(crate) fn decrypt_tracking(
        &self,
        ct: &[u8],
    ) -> std::result::Result<(zeroize::Zeroizing<Vec<u8>>, bool), crypto::DecryptFailure> {
        match crypto::decrypt_with(&[&*self.keys.identity as &dyn age::Identity], ct) {
            Ok(pt) => Ok((pt, false)),
            Err(crypto::DecryptFailure::NoMatchingKey) if !self.keys.previous.is_empty() => {
                let prev: Vec<&dyn age::Identity> = self
                    .keys
                    .previous
                    .iter()
                    .map(|(_, i)| &**i as &dyn age::Identity)
                    .collect();
                crypto::decrypt_with(&prev, ct).map(|pt| (pt, true))
            }
            Err(e) => Err(e),
        }
    }

    /// Rebuild the manifest from the files on disk. Undecryptable or
    /// inconsistent files are reported and left out. With `reencrypt`,
    /// files under a previous vault key are re-encrypted to the current one.
    /// Duplicate entry ids (e.g. a move racing an edit) keep the newest.
    pub(crate) fn rebuild_manifest_from_disk(&mut self, reencrypt: bool) -> Result<Rebuilt> {
        let mut issues = vec![];
        let mut reencrypted = 0usize;
        let old_order: Vec<String> = self
            .manifest
            .profiles
            .iter()
            .map(|p| p.id.clone())
            .collect();
        let old_entry_order: Vec<String> = self
            .manifest
            .profiles
            .iter()
            .flat_map(|p| p.entries.iter().map(|e| e.id.clone()))
            .collect();
        let mut profiles: Vec<(ProfileDoc, String)> = vec![];
        let mut entry_docs: Vec<(EntryDoc, String)> = vec![];

        let files = super::vault::list_vault_files(&self.root);
        for rel in &files {
            let Some(pid) = parse_profile_path(rel) else {
                continue;
            };
            let ct = read_file(&self.root.join(rel))?;
            match self.decrypt_tracking(&ct) {
                Ok((pt, old)) => match ProfileDoc::from_json(&pt) {
                    Ok(doc) if doc.id == pid => {
                        let hash = if old && reencrypt {
                            reencrypted += 1;
                            self.write_profile_doc(&doc)?
                        } else {
                            crypto::hash(&ct)
                        };
                        profiles.push((doc, hash));
                    }
                    _ => issues.push(issue(
                        Severity::Error,
                        "id_mismatch",
                        "profile document does not match its path",
                        Some(rel),
                    )),
                },
                Err(_) => issues.push(issue(
                    Severity::Error,
                    "decrypt_failed",
                    "profile cannot be decrypted with the vault key",
                    Some(rel),
                )),
            }
        }
        for rel in &files {
            let Some((pid, eid)) = parse_entry_path(rel) else {
                continue;
            };
            let ct = read_file(&self.root.join(rel))?;
            match self.decrypt_tracking(&ct) {
                Ok((pt, old)) => match EntryDoc::from_json(&pt) {
                    Ok(doc) if doc.id == eid && doc.profile_id == pid => {
                        if !profiles.iter().any(|(p, _)| p.id == pid) {
                            issues.push(issue(
                                Severity::Error,
                                "orphan_entry",
                                "entry belongs to a missing profile",
                                Some(rel),
                            ));
                            continue;
                        }
                        let hash = if old && reencrypt {
                            reencrypted += 1;
                            self.write_entry_doc(&doc)?
                        } else {
                            crypto::hash(&ct)
                        };
                        if let Some(i) = entry_docs.iter().position(|(d, _)| d.id == doc.id) {
                            // Same entry in two profiles: keep the newest.
                            let (other, _) = &entry_docs[i];
                            let (keep_new, drop_rel) = if doc.updated_at >= other.updated_at {
                                (true, entry_file(&other.profile_id, &other.id))
                            } else {
                                (false, rel.clone())
                            };
                            let _ = std::fs::remove_file(self.root.join(&drop_rel));
                            if keep_new {
                                entry_docs[i] = (doc, hash);
                            }
                            continue;
                        }
                        entry_docs.push((doc, hash));
                    }
                    _ => issues.push(issue(
                        Severity::Error,
                        "id_mismatch",
                        "entry document does not match its path",
                        Some(rel),
                    )),
                },
                Err(_) => issues.push(issue(
                    Severity::Error,
                    "decrypt_failed",
                    "entry cannot be decrypted with the vault key",
                    Some(rel),
                )),
            }
        }
        let pos =
            |order: &[String], id: &str| order.iter().position(|x| x == id).unwrap_or(usize::MAX);
        profiles.sort_by(|(a, _), (b, _)| {
            pos(&old_order, &a.id)
                .cmp(&pos(&old_order, &b.id))
                .then(a.created_at.cmp(&b.created_at))
                .then(a.id.cmp(&b.id))
        });
        entry_docs.sort_by(|(a, _), (b, _)| {
            pos(&old_entry_order, &a.id)
                .cmp(&pos(&old_entry_order, &b.id))
                .then(a.created_at.cmp(&b.created_at))
                .then(a.id.cmp(&b.id))
        });
        self.manifest.profiles = profiles
            .iter()
            .map(|(p, h)| ManifestProfile {
                id: p.id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                icon: p.icon.clone(),
                description: p.description.clone(),
                pinned: p.pinned,
                hash: h.clone(),
                entries: entry_docs
                    .iter()
                    .filter(|(e, _)| e.profile_id == p.id)
                    .map(|(e, h)| secrets::manifest_entry(e, h.clone()))
                    .collect(),
            })
            .collect();
        self.save_manifest()?;
        Ok(Rebuilt {
            issues,
            reencrypted,
        })
    }

    /// Full doctor run.
    ///
    /// Repairs never launder untrusted input: nothing is re-signed when the
    /// key history fails the trust anchor, and the manifest is only rebuilt
    /// for a config.toml this clone authenticated before (or that the
    /// current signed manifest vouches for).
    pub fn doctor(&mut self, params: &DoctorParams) -> Result<DoctorReport> {
        if params.staged {
            // Pre-commit mode: only the plaintext scan of the index.
            return Ok(report(scan(&self.root, true)?, vec![]));
        }
        // Fresh verification from disk, trust anchor included.
        if let Err(e) = self.reload() {
            let mut issues = vec![issue(
                Severity::Error,
                "untrusted_state",
                e.to_string(),
                None,
            )];
            issues.extend(scan(&self.root, false)?);
            return Ok(report(issues, vec![]));
        }
        let problem_issues = |v: &UnlockedVault| -> Vec<DoctorIssue> {
            v.problems
                .iter()
                .map(|p| {
                    issue(
                        Severity::Error,
                        p.code,
                        p.message.clone(),
                        p.path.as_deref(),
                    )
                })
                .collect()
        };
        let mut issues = problem_issues(self);
        let mut fixed = vec![];

        // Deep check: every listed file decrypts and matches its path.
        if self.problems.is_empty() {
            for p in self.manifest.profiles.clone() {
                if let Err(e) = self.read_profile_doc(&p.id) {
                    issues.push(issue(
                        Severity::Error,
                        "profile_unreadable",
                        e.to_string(),
                        Some(&profile_file(&p.id)),
                    ));
                }
                for e in &p.entries {
                    if let Err(err) = self.read_entry_doc(&e.id) {
                        issues.push(issue(
                            Severity::Error,
                            "entry_unreadable",
                            err.to_string(),
                            Some(&entry_file(&p.id, &e.id)),
                        ));
                    }
                }
            }
        }

        let repo = self.git.is_repo();
        // Config authenticity gate.
        const MANIFEST_CODES: [&str; 4] = [
            "manifest_missing",
            "manifest_signature",
            "manifest_decrypt",
            "config_mismatch",
        ];
        let manifest_ok = !self
            .problems
            .iter()
            .any(|p| MANIFEST_CODES.contains(&p.code));
        let mut config_ok = manifest_ok || self.config_trusted(&self.config.security_hash());
        if !config_ok
            && params.fix
            && repo
            && let Ok(Some(head)) = self.git.show("HEAD", CONFIG_FILE)
            && let Ok(c) = ConfigDoc::parse(&String::from_utf8_lossy(&head))
            && self.config_trusted(&c.security_hash())
        {
            write_atomic(&self.root.join(CONFIG_FILE), &head)?;
            fixed.push(format!("restored {CONFIG_FILE} from HEAD"));
            let _ = self.reload();
            config_ok = true;
            issues = problem_issues(self);
        }
        if !config_ok {
            issues.push(issue(
                Severity::Error,
                "config_untrusted",
                "config.toml is not a configuration this machine authenticated; refusing to \
                 re-sign it (restore it from git history)",
                Some(CONFIG_FILE),
            ));
        }
        if self.rotation_pending() {
            issues.push(issue(
                Severity::Warning,
                "rotation_incomplete",
                "a key rotation was interrupted; run doctor --fix (or rotate) to finish it",
                Some(VAULTKEY_FILE),
            ));
        }

        let needs_rebuild = issues.iter().any(|i| i.severity == Severity::Error);
        if params.fix && repo && config_ok {
            // Restore damaged tracked files from HEAD when HEAD decrypts.
            for i in issues.iter() {
                let Some(path) = i.path.as_deref() else {
                    continue;
                };
                if !(parse_entry_path(path).is_some() || parse_profile_path(path).is_some()) {
                    continue;
                }
                let Ok(Some(head)) = self.git.show("HEAD", path) else {
                    continue;
                };
                let now_bytes = std::fs::read(self.root.join(path)).unwrap_or_default();
                if head != now_bytes
                    && crypto::decrypt_with(&self.keys.all_identities(), &head).is_ok()
                {
                    write_atomic(&self.root.join(path), &head)?;
                    fixed.push(format!("restored {path} from HEAD"));
                }
            }
        }
        let rotate = params.fix && config_ok && self.rotation_pending();
        if config_ok && (rotate || params.rebuild_manifest || (params.fix && needs_rebuild)) {
            if repo && self.git.rebase_in_progress() {
                return Err(Error::Conflict(
                    "a sync is in progress; resolve it first".into(),
                ));
            }
            let mut extra = vec![];
            if rotate {
                let archives = self.session_archives()?;
                self.complete_rotation(&archives)?;
                fixed.push("completed an interrupted key rotation".into());
                if repo {
                    self.commit("vault: rotate vault key")?;
                }
            } else {
                let r = self.rebuild_manifest_from_disk(params.fix)?;
                fixed.push("rebuilt and re-signed the manifest".into());
                if r.reencrypted > 0 {
                    fixed.push(format!(
                        "re-encrypted {} file(s) to the current vault key",
                        r.reencrypted
                    ));
                }
                if repo {
                    self.commit("vault: rebuild manifest")?;
                }
                extra = r.issues;
            }
            self.load_and_verify();
            // Replace pre-fix integrity issues with the post-fix state.
            issues = extra;
            issues.extend(problem_issues(self));
        }

        // Plaintext scan of the work tree.
        let scan_issues = scan(&self.root, false)?;
        if params.fix && repo {
            let in_head: std::collections::BTreeSet<String> = if self.git.head()?.is_some() {
                self.git.tree_paths("HEAD")?.into_iter().collect()
            } else {
                Default::default()
            };
            let mut removes = vec![];
            for i in &scan_issues {
                if i.code != "plaintext_file" && i.code != "unexpected_file" {
                    continue;
                }
                let Some(path) = i.path.as_deref() else {
                    continue;
                };
                if self.git.ls_files()?.iter().any(|f| f == path) {
                    self.git.run(&["rm", "--cached", "--quiet", "--", path])?;
                }
                if in_head.contains(path) {
                    removes.push(path.to_owned());
                }
                let exclude = self.git.git_path("info/exclude")?;
                let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
                let line = format!("/{path}");
                if !text.lines().any(|l| l == line) {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&line);
                    text.push('\n');
                    if let Some(d) = exclude.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    std::fs::write(&exclude, text)
                        .map_err(|e| Error::io("write info/exclude", e))?;
                }
                fixed.push(format!("untracked and excluded {path}"));
            }
            if !removes.is_empty() {
                self.commit_vault_paths("doctor: untrack plaintext files", &[], &removes)?;
            }
            issues.extend(scan(&self.root, false)?);
        } else {
            issues.extend(scan_issues);
        }

        if repo && self.config.hooks.pre_commit_scan && !hook_installed(&self.git) {
            if params.fix {
                match install_hook(&self.git, None) {
                    Ok(_) => fixed.push("installed pre-commit hook".into()),
                    Err(e) => issues.push(issue(
                        Severity::Warning,
                        "hook_missing",
                        e.to_string(),
                        None,
                    )),
                }
            } else {
                issues.push(issue(
                    Severity::Warning,
                    "hook_missing",
                    "pre-commit scan hook is not installed",
                    None,
                ));
            }
        }
        if let Some(h) = crypto::hardening_status()
            && !h.memory_locked
        {
            issues.push(issue(
                Severity::Warning,
                "memory_not_locked",
                "process memory could not be locked (mlockall); decrypted secrets may be \
                 swapped to disk. Raise RLIMIT_MEMLOCK (e.g. LimitMEMLOCK=infinity) or use \
                 encrypted swap",
                None,
            ));
        }
        if !repo {
            issues.push(issue(
                Severity::Info,
                "not_a_repo",
                "vault directory is not a git repository (read-only)",
                None,
            ));
        }
        Ok(report(issues, fixed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_values() {
        assert_eq!(entropy(""), 0.0);
        assert_eq!(entropy("aaaa"), 0.0);
        assert!((entropy("ab") - 1.0).abs() < 1e-9);
        assert!(entropy("Zx8q2LmN0pRt5VwY7sKd3HfJ9gBc1AeU") > 4.3);
        assert!(entropy("0123456789abcdef0123456789abcdef01234567") <= 4.3);
    }

    #[test]
    fn classifies_files() {
        let id = age::x25519::Identity::generate();
        let ct = crypto::encrypt_to_identity(&id, b"x").unwrap();
        let (p, e) = (super::super::ids::new_id(), super::super::ids::new_id());
        let entry = entry_file(&p, &e);
        assert!(check_file(&entry, &ct, &[]).is_empty());
        assert_eq!(check_file(&entry, b"plain", &[])[0].code, "bad_age_header");
        // `.age` outside the layout is not allowed either.
        assert_eq!(
            check_file("profiles/a/entries/b.age", &ct, &[])[0].code,
            "unexpected_file"
        );
        assert!(check_file("sessions/x.tar.zst.age", &ct, &[]).is_empty());
        assert_eq!(
            check_file(KEYCHAIN_FILE, b"{}", &[])[0].code,
            "bad_keychain"
        );
        assert_eq!(
            check_file("secrets.txt", b"x", &[])[0].code,
            "plaintext_file"
        );
        assert!(check_file(VERSION_FILE, b"1\n", &[]).is_empty());
        assert_eq!(
            check_file(VERSION_FILE, b"one", &[])[0].code,
            "bad_version_file"
        );
        assert!(check_file(GITIGNORE_FILE, GITIGNORE.as_bytes(), &[]).is_empty());
        assert!(check_file(GITATTRIBUTES_FILE, GITATTRIBUTES.as_bytes(), &[]).is_empty());
        assert!(
            check_file(
                README_FILE,
                readme(&super::super::ids::new_id()).as_bytes(),
                &[]
            )
            .is_empty()
        );
        let k = crypto::generate_signing_key();
        let sig = crypto::sign(&k, b"m");
        assert!(check_file(MANIFEST_SIG_FILE, format!("{sig}\n").as_bytes(), &[]).is_empty());
        assert!(!check_file(MANIFEST_SIG_FILE, b"hello", &[]).is_empty());
    }

    #[test]
    fn detects_key_looking_strings_without_echo() {
        let cases = [
            "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE",
            "token: ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "key AGE-SECRET-KEY-1QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ",
            "stripe sk_live_51HxYzAbCdEfGhIjKlMn",
            "x = Zx8q2LmN0pRt5VwY7sKd3HfJ9gBc1AeU",
            "password = hunter2hunter2",
        ];
        for c in cases {
            let issues = check_plaintext_content(README_FILE, &format!("# notes\n{c}\n"), &[]);
            assert!(!issues.is_empty(), "{c}");
            assert!(issues.iter().all(|i| i.message.starts_with("line 2")));
            for i in &issues {
                assert!(!i.message.contains("AKIA") && !i.message.contains("hunter2"));
            }
        }
        assert!(check_plaintext_content(README_FILE, "just words here\n", &[]).is_empty());
        // Allow-listed public tokens are ignored.
        let tok = "Zx8q2LmN0pRt5VwY7sKd3HfJ9gBc1AeU".to_owned();
        assert!(check_plaintext_content(README_FILE, &format!("k {tok}\n"), &[tok]).is_empty());
    }

    #[test]
    fn hook_script_shapes() {
        assert!(hook_script(None).contains("latch doctor --staged"));
        assert!(hook_script(Some(Path::new("/opt/it's/latch"))).contains("'/opt/it'\\''s/latch'"));
    }

    #[test]
    fn hook_install_uninstall() {
        let t = tempfile::tempdir().unwrap();
        let g = Git::new(t.path());
        g.init(Some("T"), Some("t@example.com")).unwrap();
        assert!(!hook_installed(&g));
        install_hook(&g, None).unwrap();
        assert!(hook_installed(&g));
        install_hook(&g, None).unwrap(); // idempotent
        assert!(uninstall_hook(&g).unwrap());
        assert!(!uninstall_hook(&g).unwrap());
        let p = hook_path(&g).unwrap();
        std::fs::write(&p, "#!/bin/sh\necho mine\n").unwrap();
        assert!(matches!(
            install_hook(&g, None),
            Err(Error::AlreadyExists(_))
        ));
        assert!(!uninstall_hook(&g).unwrap());
    }

    #[test]
    fn scan_non_git_dir() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join(".latch")).unwrap();
        std::fs::write(t.path().join(VERSION_FILE), "1\n").unwrap();
        std::fs::write(t.path().join("leak.env"), "A=1\n").unwrap();
        let issues = scan(t.path(), false).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path.as_deref(), Some("leak.env"));
    }
}
