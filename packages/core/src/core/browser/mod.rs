//! Browser detection, isolated user-data dirs and encrypted session archives
//! (SPEC §10, §13). Process management lives in
//! `agent::browser::BrowserManager`; everything here is synchronous and
//! side-effect free except for the paths it is given.
//!
//! Session archive: the pruned user-data dir as tar → zstd → age, encrypted
//! to the vault identity's *public* key (so it can be written after the vault
//! locked), stored with 0600 at
//! `<data>/sessions/<vault-id>/<profile-id>.tar.zst.age`, or
//! `<vault>/sessions/<profile-id>.tar.zst.age` with `[sessions] commit`.
//!
//! Profile bookmarks are imported once (first launch of a fresh profile)
//! through `browser.bookmarks.file`; later bookmark edits in Latch do not
//! propagate to an existing Firefox session.

use std::ffi::OsString;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::Value;

use crate::core::error::IoContext;
use crate::core::{Error, Result, UnlockedVault, paths};
use crate::protocol::{BrowserEngine, BrowserSettings, DetectedBrowser};

pub mod credentials;
pub mod runtime;

/// Session metadata inside the user-data dir (used by the stale-dir sweep).
pub const SESSION_META_FILE: &str = "latch-session-meta.json";
/// Archives larger than this trigger a warning when committed to git.
pub const ARCHIVE_WARN_BYTES: u64 = 20 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Managed Firefox detection
// ---------------------------------------------------------------------------

pub fn detect() -> Vec<DetectedBrowser> {
    detect_at(&paths::data_dir().join("browser-runtime"))
}

fn detect_at(root: &Path) -> Vec<DetectedBrowser> {
    let Ok(Some(path)) = runtime::cached(root) else {
        return vec![];
    };
    vec![DetectedBrowser {
        engine: BrowserEngine::Firefox,
        version: Some(runtime::FIREFOX_VERSION.to_owned()),
        path: path.display().to_string(),
    }]
}

// ---------------------------------------------------------------------------
// Launch arguments and profile files
// ---------------------------------------------------------------------------

/// `latch-<slug of profile name>` for the window class.
pub fn window_class(profile_name: &str, profile_id: &str) -> String {
    let slug: String = profile_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!("latch-{}", if slug.is_empty() { profile_id } else { &slug })
}

/// Schemes/pages accepted as start pages or launch URLs besides http(s).
const ALLOWED_SPECIAL_URLS: &[&str] = &["about:blank", "about:newtab"];

/// Validate and normalize a start page / launch URL: `http`/`https` URLs
/// (parsed and re-serialized by `url::Url`) or one of the explicit
/// `about:` pages. Anything else — in particular strings starting
/// with `-`, which a browser would parse as a command-line switch — is
/// rejected.
pub fn normalize_start_url(u: &str) -> Result<String> {
    let t = u.trim();
    if t.is_empty() || t.starts_with('-') || t.chars().any(char::is_control) {
        return Err(Error::invalid("start pages must be http(s) URLs"));
    }
    if ALLOWED_SPECIAL_URLS.contains(&t) {
        return Ok(t.to_owned());
    }
    let parsed =
        url::Url::parse(t).map_err(|_| Error::invalid("start pages must be http(s) URLs"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(Error::invalid("start pages must be http(s) URLs"));
    }
    let out = parsed.to_string();
    if out.starts_with('-') {
        return Err(Error::invalid("start pages must be http(s) URLs"));
    }
    Ok(out)
}

/// Normalize a list, dropping invalid entries (with a warning). For pages
/// stored before validation existed or synced from another clone.
pub fn safe_start_urls(urls: &[String]) -> Vec<String> {
    urls.iter()
        .filter_map(|u| match normalize_start_url(u) {
            Ok(n) => Some(n),
            Err(_) => {
                tracing::warn!("ignoring an invalid browser start page");
                None
            }
        })
        .collect()
}

/// Command line for an isolated managed Firefox session. `engine` and
/// `allow_extensions` remain in the signature for compatibility with callers
/// that still hold legacy profile values; both are ignored.
pub fn launch_args(
    _engine: BrowserEngine,
    user_data_dir: &Path,
    class: &str,
    _allow_extensions: bool,
    urls: &[String],
) -> Vec<OsString> {
    let urls = safe_start_urls(urls);
    let mut a: Vec<OsString> = vec![
        "-profile".into(),
        user_data_dir.as_os_str().to_owned(),
        "-no-remote".into(),
        "-new-instance".into(),
    ];
    if cfg!(target_os = "linux") {
        a.push("--class".into());
        a.push(class.into());
    }
    for u in urls {
        a.push(u.into());
    }
    a
}

// ---------------------------------------------------------------------------
// Link-safe file access inside session dirs
// ---------------------------------------------------------------------------

/// Largest profile JSON file (Bookmarks, Preferences) we parse.
const MAX_PROFILE_JSON: u64 = 32 * 1024 * 1024;

fn open_nofollow(p: &Path, write: bool) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    if write {
        o.write(true).create_new(true);
    } else {
        o.read(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW);
        if write {
            o.mode(0o600);
        }
    }
    o.open(p)
}

/// Make `root/rel` a chain of real directories: symlinks or other non-dirs
/// on the way are removed and replaced (never followed).
fn safe_dir(root: &Path, rel: &Path) -> Result<PathBuf> {
    let mut cur = root.to_path_buf();
    for c in rel.components() {
        match c {
            std::path::Component::Normal(n) => cur.push(n),
            std::path::Component::CurDir => continue,
            _ => return Err(Error::Internal("unsafe session path".into())),
        }
        match std::fs::symlink_metadata(&cur) {
            Ok(m) if m.is_dir() => continue,
            Ok(_) => remove_any(&cur),
            Err(_) => {}
        }
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&cur).ctx(|| format!("create {}", cur.display()))?;
    }
    Ok(cur)
}

/// Write `root/rel` (0600) without following links: parents are made real
/// dirs, any existing entry is removed, the file is created exclusively with
/// `O_NOFOLLOW`.
pub fn write_file_in(root: &Path, rel: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let name = rel
        .file_name()
        .ok_or_else(|| Error::Internal("unsafe session path".into()))?;
    let dir = safe_dir(root, rel.parent().unwrap_or(Path::new("")))?;
    let p = dir.join(name);
    remove_any(&p);
    let mut f = open_nofollow(&p, true).ctx(|| format!("write {}", p.display()))?;
    f.write_all(bytes)
        .ctx(|| format!("write {}", p.display()))?;
    Ok(p)
}

/// Read a JSON document from `p` without following a final symlink;
/// missing, oversized or malformed files read as `None`.
fn read_json(p: &Path) -> Option<Value> {
    let f = open_nofollow(p, false).ok()?;
    if !f.metadata().ok()?.is_file() {
        return None;
    }
    let mut b = vec![];
    f.take(MAX_PROFILE_JSON + 1).read_to_end(&mut b).ok()?;
    if b.len() as u64 > MAX_PROFILE_JSON {
        return None;
    }
    serde_json::from_slice(&b).ok()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Netscape bookmark file for Firefox's one-time import.
pub fn bookmarks_html(settings: &BrowserSettings) -> String {
    let mut s = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks Menu</H1>\n<DL><p>\n    <DT><H3 PERSONAL_TOOLBAR_FOLDER=\"true\">Bookmarks Toolbar</H3>\n    <DL><p>\n",
    );
    for b in &settings.bookmarks {
        s.push_str(&format!(
            "        <DT><A HREF=\"{}\">{}</A>\n",
            html_escape(&b.url),
            html_escape(&b.title)
        ));
    }
    s.push_str("    </DL><p>\n</DL><p>\n");
    s
}

fn js(s: &str) -> String {
    // Serializing a &str cannot fail; keep a safe fallback anyway.
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Firefox `user.js` (rewritten every launch) and, for a fresh profile, the
/// bookmarks import file.
pub fn write_firefox_profile(udd: &Path, settings: &BrowserSettings) -> Result<()> {
    std::fs::create_dir_all(udd).ctx(|| format!("create {}", udd.display()))?;
    let start_pages = safe_start_urls(&settings.start_pages);
    let mut u = String::from("// Written by Latch at every launch; edits here are overwritten.\n");
    let mut pref = |k: &str, v: String| u.push_str(&format!("user_pref({}, {v});\n", js(k)));
    pref("browser.shell.checkDefaultBrowser", "false".into());
    pref("browser.aboutwelcome.enabled", "false".into());
    pref("browser.startup.homepage_override.mstone", js("ignore"));
    pref(
        "datareporting.policy.dataSubmissionPolicyBypassNotification",
        "true".into(),
    );
    pref("toolkit.telemetry.reportingpolicy.firstRun", "false".into());
    if !start_pages.is_empty() {
        pref("browser.startup.homepage", js(&start_pages.join("|")));
        pref("browser.startup.page", "1".into());
    }
    if !settings.bookmarks.is_empty()
        && std::fs::symlink_metadata(udd.join("places.sqlite")).is_err()
    {
        let f = write_file_in(
            udd,
            Path::new("latch-bookmarks.html"),
            bookmarks_html(settings).as_bytes(),
        )?;
        pref("browser.places.importBookmarksHTML", "true".into());
        pref("browser.bookmarks.file", js(&f.to_string_lossy()));
        pref("browser.toolbars.bookmarks.visibility", js("always"));
    }
    write_file_in(udd, Path::new("user.js"), u.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Session archives
// ---------------------------------------------------------------------------

/// Archive file name for a profile.
pub fn archive_file_name(profile_id: &str) -> String {
    format!("{profile_id}.tar.zst.age")
}

/// `(preferred, alternate)` archive locations for the vault's
/// `[sessions] commit` setting.
pub fn archive_locations(
    data_dir: &Path,
    vault_root: &Path,
    vault_id: &str,
    commit: bool,
    profile_id: &str,
) -> (PathBuf, PathBuf) {
    let local = data_dir
        .join("sessions")
        .join(vault_id)
        .join(archive_file_name(profile_id));
    let repo = vault_root
        .join(crate::core::format::SESSIONS_DIR)
        .join(archive_file_name(profile_id));
    if commit { (repo, local) } else { (local, repo) }
}

/// Directory and file names removed before archiving (anywhere in the tree)
/// and top-level entries that never go into an archive.
pub const PRUNE_NAMES: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "ShaderCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "Crashpad",
    "Crash Reports",
    "crashes",
    "minidumps",
    "cache2",
    "startupCache",
    "component_crx_cache",
];

/// Top-level entries removed before archiving. The session metadata file is
/// NOT removed here (it is the recovery record until the archive is durable)
/// but it is never put into an archive.
pub const PRUNE_TOP: &[&str] = &[
    // Legacy session artifacts must never survive archival.
    "extension",
    "NativeMessagingHosts",
    "latch-credentials.json",
    "latch-credentials.pending",
    "latch-credentials-result.json",
    "lock",
    ".parentlock",
    "parent.lock",
];

fn remove_any(p: &Path) {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {
            let _ = std::fs::remove_dir_all(p);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(p);
        }
        Err(_) => {}
    }
}

fn prune_rec(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if PRUNE_NAMES.contains(&name.as_str()) {
            remove_any(&p);
        } else if e.file_type().is_ok_and(|t| t.is_dir()) {
            prune_rec(&p);
        }
    }
}

/// Remove caches, crash data, locks and legacy extension session artifacts.
pub fn prune(udd: &Path) {
    for n in PRUNE_TOP {
        remove_any(&udd.join(n));
    }
    if let Ok(rd) = std::fs::read_dir(udd) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("Singleton") {
                remove_any(&e.path());
            }
        }
    }
    prune_rec(udd);
}

fn append_tree<W: Write>(b: &mut tar::Builder<W>, root: &Path, rel: &Path) -> std::io::Result<()> {
    let dir = root.join(rel);
    let mut items: Vec<_> = std::fs::read_dir(&dir)?.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        let ft = e.file_type()?;
        let r = rel.join(e.file_name());
        if rel.as_os_str().is_empty() && e.file_name() == SESSION_META_FILE {
            continue;
        }
        if ft.is_dir() {
            b.append_dir(&r, e.path())?;
            append_tree(b, root, &r)?;
        } else if ft.is_file() {
            let mut f = open_nofollow(&e.path(), false)?;
            b.append_file(&r, &mut f)?;
        }
        // Symlinks, sockets, fifos are not session state.
    }
    Ok(())
}

/// tar + zstd + age (to `recipient`, an `age1…` public key) → `out`,
/// written atomically with 0600. Returns the archive size.
pub fn write_archive(udd: &Path, recipient: &str, out: &Path) -> Result<u64> {
    let r = age::x25519::Recipient::from_str(recipient)
        .map_err(|_| Error::Internal("invalid session archive recipient".into()))?;
    let dir = out
        .parent()
        .ok_or_else(|| Error::Internal("archive path without parent".into()))?;
    std::fs::create_dir_all(dir).ctx(|| format!("create {}", dir.display()))?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".latch-session-")
        .tempfile_in(dir)
        .ctx(|| format!("temp in {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600))
            .ctx(|| "chmod archive".into())?;
    }
    {
        let enc = age::Encryptor::with_recipients(std::iter::once(&r as &dyn age::Recipient))
            .map_err(|e| Error::Internal(format!("age: {e}")))?;
        let w = enc
            .wrap_output(BufWriter::new(tmp.as_file_mut()))
            .ctx(|| "age stream".into())?;
        let z = zstd::Encoder::new(w, 3).ctx(|| "zstd".into())?;
        let mut b = tar::Builder::new(z);
        b.mode(tar::HeaderMode::Deterministic);
        append_tree(&mut b, udd, Path::new("")).ctx(|| format!("archive {}", udd.display()))?;
        let z = b.into_inner().ctx(|| "tar".into())?;
        let w = z.finish().ctx(|| "zstd".into())?;
        let mut bw = w.finish().ctx(|| "age".into())?;
        bw.flush().ctx(|| "flush archive".into())?;
    }
    tmp.as_file().sync_all().ctx(|| "sync archive".into())?;
    let size = tmp.as_file().metadata().map(|m| m.len()).unwrap_or(0);
    tmp.persist(out)
        .map_err(|e| Error::io(format!("rename to {}", out.display()), e.error))?;
    // Make the rename durable before the caller deletes the session dir.
    #[cfg(unix)]
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .ctx(|| format!("sync {}", dir.display()))?;
    Ok(size)
}

/// Limits applied when restoring a session archive (untrusted input: the
/// archive may come from a synced repository).
#[derive(Debug, Clone, Copy)]
pub struct RestoreLimits {
    /// Encrypted archive file size.
    pub max_archive_bytes: u64,
    /// Sum of all extracted file sizes.
    pub max_total_bytes: u64,
    /// One extracted file.
    pub max_entry_bytes: u64,
    /// Number of tar entries.
    pub max_entries: usize,
    /// Path components per entry.
    pub max_depth: usize,
}

impl Default for RestoreLimits {
    fn default() -> Self {
        RestoreLimits {
            max_archive_bytes: 1024 * 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024 * 1024,
            max_entry_bytes: 1024 * 1024 * 1024,
            max_entries: 200_000,
            max_depth: 64,
        }
    }
}

/// Decrypt (vault key, current or previous) and extract an archive.
pub fn restore_archive(vault: &UnlockedVault, archive: &Path, udd: &Path) -> Result<()> {
    restore_archive_with(vault, archive, udd, RestoreLimits::default())
}

/// [`restore_archive`] with explicit limits. Only regular files and
/// directories with relative, `..`-free paths are extracted; symlinks, hard
/// links, devices, fifos and anything else are rejected (the whole restore
/// fails as `Integrity`). Files are created 0600 with `O_NOFOLLOW` after
/// removing whatever was at the path; directories are real dirs (0700).
pub fn restore_archive_with(
    vault: &UnlockedVault,
    archive: &Path,
    udd: &Path,
    limits: RestoreLimits,
) -> Result<()> {
    let f = open_nofollow(archive, false).ctx(|| format!("read {}", archive.display()))?;
    let len = f
        .metadata()
        .ctx(|| format!("read {}", archive.display()))?
        .len();
    if len > limits.max_archive_bytes {
        return Err(Error::integrity("session archive is too large"));
    }
    let mut ct = Vec::with_capacity(len as usize);
    f.take(limits.max_archive_bytes + 1)
        .read_to_end(&mut ct)
        .ctx(|| format!("read {}", archive.display()))?;
    let pt = vault.decrypt_blob(&ct)?;
    drop(ct);
    extract_session_tar(&pt, udd, limits)
}

fn corrupt(what: &str) -> Error {
    Error::integrity(format!("session archive is corrupt or unsafe ({what})"))
}

/// Extract a zstd-compressed session tar into `udd` (see
/// [`restore_archive_with`]).
pub fn extract_session_tar(compressed: &[u8], udd: &Path, limits: RestoreLimits) -> Result<()> {
    use std::path::Component;
    let z = zstd::Decoder::new(compressed).map_err(|_| corrupt("zstd"))?;
    let mut a = tar::Archive::new(z);
    let mut total: u64 = 0;
    let mut count = 0usize;
    for e in a.entries().map_err(|_| corrupt("tar"))? {
        let mut e = e.map_err(|_| corrupt("tar"))?;
        count += 1;
        if count > limits.max_entries {
            return Err(corrupt("too many entries"));
        }
        let kind = e.header().entry_type();
        if matches!(
            kind,
            tar::EntryType::XGlobalHeader | tar::EntryType::XHeader
        ) {
            continue;
        }
        let path = e.path().map_err(|_| corrupt("path"))?.into_owned();
        let mut rel = PathBuf::new();
        for c in path.components() {
            match c {
                Component::Normal(n) => rel.push(n),
                Component::CurDir => {}
                _ => return Err(corrupt("path escapes the session dir")),
            }
        }
        if rel.components().count() > limits.max_depth {
            return Err(corrupt("path too deep"));
        }
        if rel.as_os_str().is_empty() {
            if kind.is_dir() {
                continue;
            }
            return Err(corrupt("empty path"));
        }
        if kind.is_dir() {
            safe_dir(udd, &rel).map_err(|_| corrupt("directory"))?;
        } else if kind.is_file()
            || kind == tar::EntryType::Continuous
            || kind == tar::EntryType::GNUSparse
        {
            // Logical size (sparse entries expand to it).
            let size = e.size();
            if size > limits.max_entry_bytes {
                return Err(corrupt("entry too large"));
            }
            total = total
                .checked_add(size)
                .ok_or_else(|| corrupt("too large"))?;
            if total > limits.max_total_bytes {
                return Err(corrupt("too large"));
            }
            let name = rel.file_name().ok_or_else(|| corrupt("path"))?.to_owned();
            let dir = safe_dir(udd, rel.parent().unwrap_or(Path::new("")))
                .map_err(|_| corrupt("directory"))?;
            let p = dir.join(name);
            remove_any(&p);
            let mut out = open_nofollow(&p, true).map_err(|_| corrupt("create file"))?;
            let n = std::io::copy(&mut (&mut e).take(size), &mut out)
                .map_err(|_| corrupt("truncated"))?;
            if n != size {
                return Err(corrupt("truncated"));
            }
        } else {
            // Symlinks, hard links, devices, fifos, sparse/unknown types.
            return Err(corrupt("unsupported entry type"));
        }
    }
    Ok(())
}

/// True when `p` is on a RAM-backed filesystem (no overwrite needed).
pub fn is_tmpfs(p: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else {
            return false;
        };
        let mut s: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is a valid NUL-terminated path, `s` a valid out-pointer.
        if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
            return false;
        }
        const TMPFS_MAGIC: i64 = 0x0102_1994;
        const RAMFS_MAGIC: i64 = 0x8584_58f6;
        #[allow(clippy::unnecessary_cast)]
        let t = s.f_type as i64;
        t == TMPFS_MAGIC || t == RAMFS_MAGIC
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = p;
        false
    }
}

fn overwrite_files(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            overwrite_files(&e.path());
        } else if ft.is_file()
            && let Ok(len) = e.metadata().map(|m| m.len())
            && let Ok(mut f) = open_existing_nofollow(&e.path())
        {
            let zeros = [0u8; 64 * 1024];
            let mut left = len;
            while left > 0 {
                let n = left.min(zeros.len() as u64) as usize;
                if f.write_all(&zeros[..n]).is_err() {
                    break;
                }
                left -= n as u64;
            }
            let _ = f.sync_all();
        }
    }
}

fn open_existing_nofollow(p: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW);
    }
    o.open(p)
}

/// Delete a session dir: overwrite files with zeros first unless on tmpfs.
pub fn secure_delete(dir: &Path) {
    if !dir.exists() {
        return;
    }
    if !is_tmpfs(dir) {
        overwrite_files(dir);
    }
    if let Err(e) = std::fs::remove_dir_all(dir) {
        tracing::warn!("could not remove session dir {}: {e}", dir.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Bookmark;

    fn settings() -> BrowserSettings {
        BrowserSettings {
            engine: BrowserEngine::Firefox,
            binary: None,
            start_pages: vec!["https://a.test".into(), "https://b.test".into()],
            bookmarks: vec![Bookmark {
                title: "Jira <x>".into(),
                url: "https://j.test/?a=1&b=2".into(),
            }],
            extensions_allowed: true,
            autofill: crate::protocol::AutofillScope::All,
            autofill_entry_ids: vec![],
        }
    }

    #[test]
    fn detection_uses_only_a_cached_managed_firefox() {
        let root = tempfile::tempdir().unwrap();
        assert!(detect_at(root.path()).is_empty());

        let executable = if cfg!(target_os = "macos") {
            root.path()
                .join("firefox-1555/firefox/Nightly.app/Contents/MacOS/firefox")
        } else if cfg!(target_os = "windows") {
            root.path().join("firefox-1555/firefox/firefox.exe")
        } else {
            root.path().join("firefox-1555/firefox/firefox")
        };
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, b"managed firefox").unwrap();
        let config = runtime::config_root(&executable);
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("application.ini"), b"[App]").unwrap();
        std::fs::write(root.path().join("firefox-1555/.complete"), b"1555").unwrap();

        let detected = detect_at(root.path());
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].engine, BrowserEngine::Firefox);
        assert_eq!(
            detected[0].version.as_deref(),
            Some(runtime::FIREFOX_VERSION)
        );
        assert_eq!(detected[0].path, executable.display().to_string());
    }

    #[test]
    fn class_slug() {
        assert_eq!(window_class("Acme Corp!", "id"), "latch-acme-corp");
        assert_eq!(window_class("日本", "abc"), "latch-abc");
    }

    #[test]
    fn launch_args_always_use_an_isolated_firefox_profile() {
        let a = launch_args(
            BrowserEngine::Chrome,
            Path::new("/u"),
            "latch-x",
            true,
            &["https://a".into(), "--evil".into()],
        );
        let s: Vec<String> = a.iter().map(|x| x.to_string_lossy().into_owned()).collect();
        assert_eq!(&s[..4], ["-profile", "/u", "-no-remote", "-new-instance"]);
        assert_eq!(s.last().unwrap(), "https://a/");
        assert!(!s.contains(&"--evil".to_string()));
        let args = launch_args(
            BrowserEngine::Firefox,
            Path::new("/u"),
            "latch-x",
            true,
            &[],
        );
        let s: Vec<String> = args
            .iter()
            .map(|x| x.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&s[..4], ["-profile", "/u", "-no-remote", "-new-instance"]);
        assert!(!s.iter().any(|x| x.starts_with("--user-data-dir")));
        assert!(!s.iter().any(|x| x.starts_with("--disable-extensions")));
    }

    #[test]
    fn firefox_profile() {
        let t = tempfile::tempdir().unwrap();
        write_firefox_profile(t.path(), &settings()).unwrap();
        let u = std::fs::read_to_string(t.path().join("user.js")).unwrap();
        assert!(u.contains(
            r#"user_pref("browser.startup.homepage", "https://a.test/|https://b.test/");"#
        ));
        assert!(u.contains("browser.bookmarks.file"));
        let h = std::fs::read_to_string(t.path().join("latch-bookmarks.html")).unwrap();
        assert!(h.contains("https://j.test/?a=1&amp;b=2") && h.contains("Jira &lt;x&gt;"));
        std::fs::write(t.path().join("places.sqlite"), b"x").unwrap();
        write_firefox_profile(t.path(), &settings()).unwrap();
        let u = std::fs::read_to_string(t.path().join("user.js")).unwrap();
        assert!(!u.contains("browser.bookmarks.file"));
    }

    #[test]
    fn prune_removes_caches() {
        let t = tempfile::tempdir().unwrap();
        let u = t.path();
        for d in [
            "Default/Cache/x",
            "Default/Code Cache/js",
            "GPUCache",
            "Crashpad/r",
            "extension",
            "cache2/e",
            "Default/Local Storage",
        ] {
            std::fs::create_dir_all(u.join(d)).unwrap();
        }
        std::fs::write(u.join("cookies.sqlite"), b"c").unwrap();
        std::fs::write(u.join("SingletonCookie"), b"c").unwrap();
        std::fs::write(u.join("latch-credentials.json"), b"pending").unwrap();
        std::fs::write(u.join("latch-credentials.pending"), b"partial").unwrap();
        std::fs::write(u.join("latch-credentials-result.json"), b"result").unwrap();
        std::fs::write(u.join("latch-login-ownership.json"), b"[\"guid\"]").unwrap();
        std::fs::write(u.join(SESSION_META_FILE), b"{}").unwrap();
        prune(u);
        // Kept as the recovery record until the archive is durable.
        assert!(u.join(SESSION_META_FILE).exists());
        assert!(u.join("cookies.sqlite").exists());
        assert!(u.join("latch-login-ownership.json").exists());
        assert!(u.join("Default/Local Storage").exists());
        for gone in [
            "Default/Cache",
            "Default/Code Cache",
            "GPUCache",
            "Crashpad",
            "extension",
            "cache2",
            "SingletonCookie",
            "latch-credentials.json",
            "latch-credentials.pending",
            "latch-credentials-result.json",
        ] {
            assert!(!u.join(gone).exists(), "{gone}");
        }
    }

    #[test]
    fn start_urls() {
        assert_eq!(
            normalize_start_url("https://Example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(normalize_start_url("about:blank").unwrap(), "about:blank");
        for bad in [
            "--user-data-dir=/x",
            "-profile",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "example.com",
            "",
            "chrome://settings",
            "https://a.test\n--x",
        ] {
            assert!(normalize_start_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn secure_delete_removes() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("s");
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a/f"), vec![7u8; 100_000]).unwrap();
        secure_delete(&d);
        assert!(!d.exists());
        assert!(!is_tmpfs(Path::new("/nonexistent/path")));
    }
}
