// Isolated Firefox sessions with a fake browser script: launch arguments,
// profile files, encrypted archive, restore, prune, close, reset, archive
// after lock, stale-dir sweep and detection.

#[path = "../../../../tests/core_support/mod.rs"]
mod support;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use latch::agent::browser::{BrowserManager, ManagerConfig};
use latch::core::UnlockedVault;
use latch::core::browser as cb;
use latch::protocol::*;
use support::*;

fn dead_pid() -> u32 {
    let mut process = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let pid = process.id();
    process.wait().unwrap();
    pid
}

struct Env {
    _t: tempfile::TempDir,
    root: PathBuf,
    log: PathBuf,
    cfg: ManagerConfig,
    events: Arc<Mutex<Vec<AgentEvent>>>,
}

/// `mode`: "wait" (until SIGTERM) or "exit" (exit right away).
fn fake_browser(dir: &Path, name: &str, log: &Path, mode: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    let script = format!(
        r#"#!/bin/sh
LOG='{log}'
if [ "$1" = "--version" ]; then echo "Fake Browser 1.2.3"; exit 0; fi
printf '%s\n' "$@" > "$LOG/args"
udd=""; prev=""
for a in "$@"; do
  [ "$prev" = "-profile" ] && udd="$a"
  prev="$a"
done
mkdir -p "$udd/cache2" "$udd/storage/default"
echo cachedata > "$udd/cache2/data_0"
n=$(cat "$udd/cookies.sqlite" 2>/dev/null | wc -l | tr -d ' ')
echo "cookie-$n" >> "$udd/cookies.sqlite"
cp "$udd/cookies.sqlite" "$LOG/cookies-seen"
touch "$udd/parent.lock"
ln -sf "host-$$" "$udd/SingletonLock"
touch "$LOG/ready"
if [ "{mode}" = "exit" ]; then exit 0; fi
trap 'echo term >> "$LOG/events"; exit 0' TERM
while :; do sleep 0.05; done
"#,
        log = log.display(),
        mode = mode
    );
    std::fs::write(&p, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn env_with(mode: &str, name: &str) -> Env {
    setup();
    let t = tempfile::tempdir().unwrap();
    let root = t.path().to_path_buf();
    let log = root.join("log");
    std::fs::create_dir_all(&log).unwrap();
    let bin = fake_browser(&root.join("bin"), name, &log, mode);
    let events = Arc::new(Mutex::new(vec![]));
    let cfg = ManagerConfig {
        ram_dir: root.join("ram"),
        data_dir: root.join("data"),
        browser_override: Some(bin),
        path_var: Some(root.join("emptypath").into_os_string()),
        term_timeout: Duration::from_secs(5),
        archive_wait: Duration::from_secs(30),
    };
    Env {
        _t: t,
        root,
        log,
        cfg,
        events,
    }
}

fn env() -> Env {
    env_with("wait", "fake-browser")
}

impl Env {
    fn manager(&self) -> BrowserManager {
        let ev = self.events.clone();
        BrowserManager::with_config(
            Arc::new(move |e| ev.lock().unwrap().push(e)),
            self.cfg.clone(),
        )
    }
    fn vault(&self) -> (UnlockedVault, String) {
        let mut v = new_vault(&self.root.join("vault"));
        let pid = v.create_profile(&profile_input("Acme Corp")).unwrap().id;
        (v, pid)
    }
    fn wait_ready(&self) {
        wait_for(|| self.log.join("ready").exists());
        std::fs::remove_file(self.log.join("ready")).unwrap();
    }
    fn args(&self) -> Vec<String> {
        std::fs::read_to_string(self.log.join("args"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    fn archive(&self, v: &UnlockedVault, pid: &str) -> PathBuf {
        self.cfg
            .data_dir
            .join("sessions")
            .join(v.id())
            .join(format!("{pid}.tar.zst.age"))
    }
}

fn wait_for(f: impl Fn() -> bool) {
    let start = Instant::now();
    while !f() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn launch_params(pid: &str) -> BrowserLaunchParams {
    BrowserLaunchParams {
        profile_id: pid.into(),
        url: None,
        engine: None,
    }
}

/// Paths inside a decrypted archive.
fn archive_listing(v: &UnlockedVault, archive: &Path) -> Vec<String> {
    let pt = v.decrypt_blob(&std::fs::read(archive).unwrap()).unwrap();
    let mut z = zstd::Decoder::new(&pt[..]).unwrap();
    let mut tarb = vec![];
    z.read_to_end(&mut tarb).unwrap();
    let mut a = tar::Archive::new(&tarb[..]);
    a.entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().display().to_string())
        .collect()
}

#[test]
fn launch_close_archive_restore() {
    let e = env();
    let (v, pid) = e.vault();
    let m = e.manager();
    let s = m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    let udd = e.cfg.ram_dir.join(&pid);
    assert_eq!(s.user_data_dir, udd.display().to_string());
    assert_eq!(s.engine, BrowserEngine::Firefox);
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&udd).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    // Arguments
    let args = e.args();
    assert_eq!(args[0], "-profile");
    assert_eq!(args[1], udd.display().to_string());
    assert!(args.contains(&"-no-remote".into()));
    assert!(args.contains(&"-new-instance".into()));
    assert!(args.contains(&"--class".into()));
    assert!(args.contains(&"latch-acme-corp".into()));
    assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    assert_eq!(args.last().unwrap(), "https://example.com/");
    assert!(!args.iter().any(|a| a == "--"));

    // Profile files
    let userjs = std::fs::read_to_string(udd.join("user.js")).unwrap();
    assert!(userjs.contains("browser.startup.homepage"));
    let bookmarks = std::fs::read_to_string(udd.join("latch-bookmarks.html")).unwrap();
    assert!(bookmarks.contains("https://acme.atlassian.net"));
    assert!(!udd.join("extension").exists());
    assert!(!udd.join("NativeMessagingHosts").exists());

    // One session per profile
    assert!(m.is_running(&pid));
    let again = m.launch(&v, &launch_params(&pid)).unwrap();
    assert_eq!(again.pid, s.pid);
    assert_eq!(m.list().len(), 1);

    // Close → archived, encrypted, dir gone
    m.close(&pid).unwrap();
    assert!(!m.is_running(&pid));
    assert!(m.list().is_empty());
    assert!(!udd.exists());
    assert_eq!(
        std::fs::read_to_string(e.log.join("events"))
            .unwrap()
            .trim(),
        "term"
    );
    let archive = e.archive(&v, &pid);
    let bytes = std::fs::read(&archive).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1\n"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("cookie-0") && !text.contains("ustar"));
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&archive).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Prune list applied
    let listing = archive_listing(&v, &archive);
    assert!(listing.iter().any(|p| p == "cookies.sqlite"), "{listing:?}");
    for bad in [
        "Cache",
        "extension",
        "SingletonLock",
        "NativeMessagingHosts",
        cb::SESSION_META_FILE,
    ] {
        assert!(
            !listing.iter().any(|p| p.split('/').any(|c| c == bad)),
            "{bad} in {listing:?}"
        );
    }
    let info = m.session_info(&v, &pid).unwrap();
    assert!(info.exists && info.size_bytes > 0 && !info.committed && info.updated_at.is_some());
    assert!(
        e.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.kind == AgentEventKind::BrowserChanged)
            .count()
            >= 2
    );

    // Restore on next launch round-trips the cookie file
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    let cookies = std::fs::read_to_string(e.log.join("cookies-seen")).unwrap();
    assert_eq!(cookies, "cookie-0\ncookie-1\n");
    m.close(&pid).unwrap();

    // Reset deletes the archive
    m.reset(&v, &pid).unwrap();
    assert!(!archive.exists());
    assert!(!m.session_info(&v, &pid).unwrap().exists);
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    assert_eq!(
        std::fs::read_to_string(e.log.join("cookies-seen")).unwrap(),
        "cookie-0\n"
    );
    // Reset of a running session closes it first.
    m.reset(&v, &pid).unwrap();
    assert!(!m.is_running(&pid));
    assert!(!archive.exists());
}

#[test]
fn url_param_overrides_start_pages() {
    let e = env();
    let (v, pid) = e.vault();
    let m = e.manager();
    m.launch(
        &v,
        &BrowserLaunchParams {
            profile_id: pid.clone(),
            url: Some("https://only.test".into()),
            engine: None,
        },
    )
    .unwrap();
    e.wait_ready();
    let args = e.args();
    assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    assert_eq!(args.last().unwrap(), "https://only.test/");
    assert!(!args.contains(&"https://example.com/".to_string()));
    m.close_all();
    assert!(m.list().is_empty());
    assert!(e.archive(&v, &pid).exists());
}

#[test]
fn archive_written_after_vault_locked() {
    let e = env();
    let (v, pid) = e.vault();
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    let locked = v.lock();
    m.close(&pid).unwrap();
    assert!(e.archive_path_for(&locked, &pid).exists());
    let v = locked.unlock_passphrase(PASS).unwrap();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    assert_eq!(
        std::fs::read_to_string(e.log.join("cookies-seen")).unwrap(),
        "cookie-0\ncookie-1\n"
    );
    m.close(&pid).unwrap();
}

impl Env {
    fn archive_path_for(&self, v: &latch::core::Vault, pid: &str) -> PathBuf {
        self.cfg
            .data_dir
            .join("sessions")
            .join(v.id())
            .join(format!("{pid}.tar.zst.age"))
    }
}

#[test]
fn browser_exit_triggers_archive() {
    let e = env_with("exit", "fake-browser");
    let (v, pid) = e.vault();
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    let archive = e.archive(&v, &pid);
    wait_for(|| !m.is_running(&pid) && archive.exists() && !e.cfg.ram_dir.join(&pid).exists());
    // Events: launch + exit.
    wait_for(|| e.events.lock().unwrap().len() >= 2);
}

#[test]
fn committed_sessions_live_in_the_repo() {
    let e = env();
    let (mut v, pid) = e.vault();
    v.set_vault_settings(false, false, true).unwrap();
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    m.close(&pid).unwrap();
    let in_repo = v.path().join("sessions").join(format!("{pid}.tar.zst.age"));
    assert!(in_repo.exists());
    assert!(!e.archive(&v, &pid).exists());
    let info = m.session_info(&v, &pid).unwrap();
    assert!(info.exists && info.committed);
    // Switching back moves the archive to local storage on the next close.
    v.set_vault_settings(false, false, false).unwrap();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    assert_eq!(
        std::fs::read_to_string(e.log.join("cookies-seen")).unwrap(),
        "cookie-0\ncookie-1\n"
    );
    m.close(&pid).unwrap();
    assert!(e.archive(&v, &pid).exists());
    assert!(!in_repo.exists());
}

#[test]
fn corrupt_archive_fails_closed() {
    let e = env();
    let (v, pid) = e.vault();
    let archive = e.archive(&v, &pid);
    std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
    std::fs::write(&archive, b"age-encryption.org/v1\ngarbage").unwrap();
    let m = e.manager();
    let err = m.launch(&v, &launch_params(&pid)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Integrity);
    assert!(!e.cfg.ram_dir.join(&pid).exists());
    assert!(!m.is_running(&pid));
    assert!(archive.exists(), "never overwrite an unreadable archive");
}

#[test]
fn stale_dirs_are_swept() {
    let e = env();
    let (v, pid) = e.vault();
    let ram = &e.cfg.ram_dir;
    // A crashed agent's session with metadata → archived.
    let udd = ram.join(&pid);
    std::fs::create_dir_all(udd.join("Default")).unwrap();
    std::fs::write(udd.join("cookies.sqlite"), "stale-cookie\n").unwrap();
    let archive = e.archive(&v, &pid);
    let meta = serde_json::json!({
        "vault_id": v.id(), "profile_id": pid, "recipient": v.vault_public_key(),
        "archive": archive, "stale_archive": e.root.join("none"), "commit": false,
        "browser_pid": dead_pid(), "agent_pid": 0,
    });
    std::fs::write(udd.join(cb::SESSION_META_FILE), meta.to_string()).unwrap();
    // Missing metadata is retained: a crash may have left a live browser.
    let other = ram.join("aaaaaaaaaaaaaaaaaaaaaaaaaa");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("x"), "x").unwrap();
    let foreign = ram.join("not-a-session");
    std::fs::create_dir_all(&foreign).unwrap();

    let m = e.manager();
    assert!(!udd.exists() && other.exists() && foreign.exists());
    assert!(archive.exists());
    assert!(archive_listing(&v, &archive).contains(&"cookies.sqlite".to_string()));
    // And it restores.
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    assert_eq!(
        std::fs::read_to_string(e.log.join("cookies-seen")).unwrap(),
        "stale-cookie\ncookie-1\n"
    );
    m.close(&pid).unwrap();
}

#[test]
fn firefox_launch() {
    let e = env_with("wait", "firefox");
    let (v, pid) = e.vault();
    let m = e.manager();
    let s = m.launch(&v, &launch_params(&pid)).unwrap();
    assert_eq!(s.engine, BrowserEngine::Firefox);
    e.wait_ready();
    let udd = e.cfg.ram_dir.join(&pid);
    let args = e.args();
    assert_eq!(args[0], "-profile");
    assert_eq!(args[1], udd.display().to_string());
    assert!(args.contains(&"-no-remote".into()) && args.contains(&"-new-instance".into()));
    assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    let userjs = std::fs::read_to_string(udd.join("user.js")).unwrap();
    assert!(userjs.contains("https://example.com"));
    assert!(!udd.join("extension").exists());
    m.close(&pid).unwrap();
    assert!(e.archive(&v, &pid).exists());
}

/// Managed Firefox smoke test. It uses an already cached runtime and never downloads.
#[test]
#[ignore]
fn real_managed_firefox_smoke() {
    let runtime_root = latch::core::paths::data_dir().join("browser-runtime");
    let Ok(Some(firefox)) = cb::runtime::cached(&runtime_root) else {
        return;
    };
    let mut e = env();
    e.cfg.browser_override = Some(firefox);
    let (v, pid) = e.vault();
    let m = e.manager();
    m.launch(
        &v,
        &BrowserLaunchParams {
            profile_id: pid.clone(),
            url: Some("about:blank".into()),
            engine: None,
        },
    )
    .unwrap();
    let udd = e.cfg.ram_dir.join(&pid);
    wait_for(|| udd.join("cookies.sqlite").exists());
    std::thread::sleep(Duration::from_secs(2));
    m.close(&pid).unwrap();
    assert!(!udd.exists());
    let listing = archive_listing(&v, &e.archive(&v, &pid));
    assert!(listing.iter().any(|p| p == "cookies.sqlite"));
    assert!(!listing.iter().any(|p| p.contains("cache2/")));
}

// ---------------------------------------------------------------------------
// Review regressions (Codex probes)
// ---------------------------------------------------------------------------

/// zstd(tar) built by `f`, encrypted to the vault key, as a session archive.
fn write_crafted_archive(
    v: &UnlockedVault,
    archive: &Path,
    f: impl FnOnce(&mut tar::Builder<Vec<u8>>),
) {
    let mut b = tar::Builder::new(Vec::new());
    f(&mut b);
    let pt = zstd::encode_all(b.into_inner().unwrap().as_slice(), 1).unwrap();
    let r: age::x25519::Recipient = v.vault_public_key().parse().unwrap();
    let ct = latch::core::crypto::encrypt_to(&[&r], &pt).unwrap();
    std::fs::create_dir_all(archive.parent().unwrap()).unwrap();
    std::fs::write(archive, ct).unwrap();
}

fn tar_with(kind: tar::EntryType, path: &str, link: Option<&Path>, data: &[u8]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_gnu();
    h.set_entry_type(kind);
    h.set_mode(0o644);
    h.set_size(data.len() as u64);
    // Bypass the builder's path sanitizing to craft hostile names.
    {
        let gnu = h.as_gnu_mut().unwrap();
        gnu.name[..path.len()].copy_from_slice(path.as_bytes());
    }
    if let Some(l) = link {
        h.set_link_name(l).unwrap();
    }
    h.set_cksum();
    b.append(&h, data).unwrap();
    zstd::encode_all(b.into_inner().unwrap().as_slice(), 1).unwrap()
}

#[test]
fn restored_archive_symlink_is_rejected() {
    let e = env();
    let (v, pid) = e.vault();
    let outside = e.root.join("outside");
    std::fs::write(&outside, b"DO NOT OVERWRITE").unwrap();
    write_crafted_archive(&v, &e.archive(&v, &pid), |b| {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        h.set_mode(0o777);
        b.append_link(&mut h, "user.js", &outside).unwrap();
    });
    let m = e.manager();
    let err = m.launch(&v, &launch_params(&pid)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Integrity);
    assert_eq!(std::fs::read(&outside).unwrap(), b"DO NOT OVERWRITE");
    assert!(!e.cfg.ram_dir.join(&pid).exists());
}

#[test]
fn session_tar_extraction_rejects_unsafe_entries() {
    use cb::{RestoreLimits, extract_session_tar};
    let t = tempfile::tempdir().unwrap();
    let outside = t.path().join("outside");
    let lim = RestoreLimits::default();
    for (kind, path, link) in [
        (tar::EntryType::Regular, "../escape", None),
        (tar::EntryType::Regular, "/abs/escape", None),
        (tar::EntryType::Regular, "a/../../escape", None),
        (tar::EntryType::Symlink, "lnk", Some(outside.as_path())),
        (tar::EntryType::Link, "hard", Some(outside.as_path())),
        (tar::EntryType::Fifo, "fifo", None),
        (tar::EntryType::Char, "dev", None),
        (tar::EntryType::Block, "blk", None),
    ] {
        let udd = t.path().join(format!("udd-{}", path.replace('/', "_")));
        std::fs::create_dir_all(&udd).unwrap();
        let z = tar_with(kind, path, link, b"x");
        let r = extract_session_tar(&z, &udd, lim);
        assert!(r.is_err(), "{kind:?} {path} accepted");
        assert_eq!(r.unwrap_err().code(), ErrorCode::Integrity);
    }
    assert!(!t.path().join("escape").exists() && !outside.exists());

    // A pre-existing symlink in the target is replaced, never followed.
    let udd = t.path().join("udd-pre");
    std::fs::create_dir_all(&udd).unwrap();
    std::fs::write(&outside, b"keep").unwrap();
    std::os::unix::fs::symlink(&outside, udd.join("f")).unwrap();
    extract_session_tar(
        &tar_with(tar::EntryType::Regular, "f", None, b"new"),
        &udd,
        lim,
    )
    .unwrap();
    assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
    assert_eq!(std::fs::read(udd.join("f")).unwrap(), b"new");

    // GNU sparse entries (written by tar::Builder for sparse files) restore.
    let src = t.path().join("sparse-src");
    let f = std::fs::File::create(&src).unwrap();
    f.set_len(4 * 1024 * 1024).unwrap();
    drop(f);
    let mut b = tar::Builder::new(Vec::new());
    b.append_path_with_name(&src, "sparse").unwrap();
    let z = zstd::encode_all(b.into_inner().unwrap().as_slice(), 1).unwrap();
    let udd = t.path().join("udd-sparse");
    std::fs::create_dir_all(&udd).unwrap();
    extract_session_tar(&z, &udd, lim).unwrap();
    assert_eq!(
        std::fs::metadata(udd.join("sparse")).unwrap().len(),
        4 * 1024 * 1024
    );

    // Size, count and depth limits.
    let small = RestoreLimits {
        max_entry_bytes: 4,
        ..lim
    };
    let z = tar_with(tar::EntryType::Regular, "big", None, b"0123456789");
    assert!(extract_session_tar(&z, &t.path().join("udd-big"), small).is_err());
    let mut b = tar::Builder::new(Vec::new());
    for i in 0..20 {
        let mut h = tar::Header::new_gnu();
        h.set_size(1);
        h.set_mode(0o600);
        h.set_cksum();
        b.append_data(&mut h, format!("f{i}"), &b"x"[..]).unwrap();
    }
    let z = zstd::encode_all(b.into_inner().unwrap().as_slice(), 1).unwrap();
    let few = RestoreLimits {
        max_entries: 10,
        ..lim
    };
    assert!(extract_session_tar(&z, &t.path().join("udd-many"), few).is_err());
    let total = RestoreLimits {
        max_total_bytes: 5,
        ..lim
    };
    assert!(extract_session_tar(&z, &t.path().join("udd-total"), total).is_err());
}

#[test]
fn profile_writes_do_not_follow_links() {
    let t = tempfile::tempdir().unwrap();
    let outside = t.path().join("outside");
    std::fs::write(&outside, b"DO NOT OVERWRITE").unwrap();
    let udd = t.path().join("udd");
    std::fs::create_dir_all(&udd).unwrap();
    std::os::unix::fs::symlink(&outside, udd.join("user.js")).unwrap();
    cb::write_firefox_profile(&udd, &profile_input("x").browser).unwrap();
    assert_eq!(std::fs::read(&outside).unwrap(), b"DO NOT OVERWRITE");
    assert!(
        !std::fs::symlink_metadata(udd.join("user.js"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let bookmarks = udd.join("latch-bookmarks.html");
    std::fs::remove_file(&bookmarks).unwrap();
    std::os::unix::fs::symlink(&outside, &bookmarks).unwrap();
    cb::write_firefox_profile(&udd, &profile_input("x").browser).unwrap();
    assert_eq!(std::fs::read(&outside).unwrap(), b"DO NOT OVERWRITE");
    assert!(bookmarks.is_file());
    assert!(
        !std::fs::symlink_metadata(bookmarks)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn stale_metadata_paths_are_ignored() {
    let e = env();
    let (v, pid) = e.vault();
    let victim = e.root.join("unrelated-important-file");
    std::fs::write(&victim, b"important").unwrap();
    let elsewhere = e.root.join("elsewhere.age");
    let udd = e.cfg.ram_dir.join(&pid);
    std::fs::create_dir_all(&udd).unwrap();
    std::fs::write(udd.join("cookies.sqlite"), "c\n").unwrap();
    let meta = serde_json::json!({
        "vault_id": v.id(), "profile_id": pid, "recipient": v.vault_public_key(),
        "archive": elsewhere, "stale_archive": victim, "commit": false,
        "browser_pid": dead_pid(), "agent_pid": 0,
    });
    std::fs::write(udd.join(cb::SESSION_META_FILE), meta.to_string()).unwrap();
    // A dir whose metadata names another profile id is not trusted.
    let other = e.cfg.ram_dir.join("bbbbbbbbbbbbbbbbbbbbbbbbba");
    std::fs::create_dir_all(&other).unwrap();
    let meta2 = serde_json::json!({
        "vault_id": v.id(), "profile_id": pid, "recipient": v.vault_public_key(),
        "archive": elsewhere, "stale_archive": victim, "commit": false,
        "browser_pid": dead_pid(), "agent_pid": 0,
    });
    std::fs::write(other.join(cb::SESSION_META_FILE), meta2.to_string()).unwrap();

    let _m = e.manager();
    assert_eq!(std::fs::read(&victim).unwrap(), b"important");
    assert!(!elsewhere.exists());
    assert!(
        e.archive(&v, &pid).exists(),
        "archived to the trusted local path"
    );
    assert!(!udd.exists() && other.exists());
}

#[test]
fn failed_archive_keeps_recovery_metadata_and_retries() {
    let e = env();
    let (v, pid) = e.vault();
    let udd = e.cfg.ram_dir.join(&pid);
    std::fs::create_dir_all(&udd).unwrap();
    std::fs::write(udd.join("cookies.sqlite"), "c\n").unwrap();
    let meta = serde_json::json!({
        "vault_id": v.id(), "profile_id": pid, "recipient": v.vault_public_key(),
        "archive": e.archive(&v, &pid), "stale_archive": e.root.join("x"), "commit": false,
        "browser_pid": dead_pid(), "agent_pid": 0,
    });
    std::fs::write(udd.join(cb::SESSION_META_FILE), meta.to_string()).unwrap();
    // Make the archive destination unwritable: a file where the dir goes.
    let sessions = e.cfg.data_dir.join("sessions");
    std::fs::create_dir_all(&e.cfg.data_dir).unwrap();
    std::fs::write(&sessions, b"not a dir").unwrap();
    {
        let m = e.manager();
        assert!(udd.join("cookies.sqlite").exists());
        assert!(
            udd.join(cb::SESSION_META_FILE).exists(),
            "recovery record kept"
        );
        // A launch must not discard the unarchived session either.
        let err = m.launch(&v, &launch_params(&pid)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::Conflict);
        assert!(udd.join("cookies.sqlite").exists());
    }
    // Fixed: the next sweep archives it.
    std::fs::remove_file(&sessions).unwrap();
    let _m = e.manager();
    assert!(!udd.exists());
    assert!(e.archive(&v, &pid).exists());
}

#[test]
fn close_reports_archive_failure() {
    let e = env();
    let (v, pid) = e.vault();
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    let sessions = e.cfg.data_dir.join("sessions");
    std::fs::create_dir_all(&e.cfg.data_dir).unwrap();
    std::fs::write(&sessions, b"not a dir").unwrap();
    let err = m.close(&pid).unwrap_err();
    assert!(err.to_string().contains("archiving"), "{err}");
    let udd = e.cfg.ram_dir.join(&pid);
    assert!(udd.join(cb::SESSION_META_FILE).exists());
    // Reset discards it explicitly.
    std::fs::remove_file(&sessions).unwrap();
    m.reset(&v, &pid).unwrap();
    assert!(!udd.exists());
}

#[test]
fn relaunch_is_busy_while_previous_session_archives() {
    let mut e = env();
    e.cfg.archive_wait = Duration::from_millis(1);
    let (v, pid) = e.vault();
    let m = e.manager();
    let s = m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    // A large file makes archiving take a while (written, not sparse).
    {
        use std::io::Write;
        let mut big = std::fs::File::create(e.cfg.ram_dir.join(&pid).join("large.bin")).unwrap();
        let chunk = vec![0x5au8; 1024 * 1024];
        for _ in 0..384 {
            big.write_all(&chunk).unwrap();
        }
    }
    // SAFETY: kill(2) on the fake browser this test launched.
    unsafe {
        libc::kill(s.pid as i32, libc::SIGKILL);
    }
    wait_for(|| !m.is_running(&pid));
    let err = m.launch(&v, &launch_params(&pid)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Conflict, "{err}");
    let archive = e.archive(&v, &pid);
    let start = Instant::now();
    while !archive.exists() {
        assert!(start.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(50));
    }
    wait_for(|| !e.cfg.ram_dir.join(&pid).exists());
}

#[test]
fn legacy_extension_setting_does_not_add_browser_flags() {
    let e = env();
    let mut v = new_vault(&e.root.join("vault"));
    let mut input = profile_input("Locked Down");
    input.browser.extensions_allowed = false;
    let pid = v.create_profile(&input).unwrap().id;
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    let udd = e.cfg.ram_dir.join(&pid);
    let args = e.args();
    assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    assert!(!args.iter().any(|a| a.starts_with("--user-data-dir")));
    assert!(!args.iter().any(|a| a == "--disable-extensions"));
    assert!(!udd.join("extension").exists());
    m.close(&pid).unwrap();
}

#[test]
fn start_pages_cannot_inject_switches() {
    let e = env();
    let mut v = new_vault(&e.root.join("vault"));
    let mut p = profile_input("bad");
    p.browser.start_pages = vec!["--user-data-dir=/outside".into()];
    assert_eq!(
        v.create_profile(&p).unwrap_err().code(),
        ErrorCode::InvalidParams
    );
    p.browser.start_pages = vec!["file:///etc/passwd".into()];
    assert_eq!(
        v.create_profile(&p).unwrap_err().code(),
        ErrorCode::InvalidParams
    );
    let args = cb::launch_args(
        BrowserEngine::Firefox,
        Path::new("/private"),
        "class",
        true,
        &["--user-data-dir=/outside".into(), "https://ok.test".into()],
    );
    let args: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(!args.iter().any(|x| x == "--user-data-dir=/outside"));
    assert!(!args.iter().any(|x| x == "--"));
    assert_eq!(args.last().unwrap(), "https://ok.test/");

    let pid = v.create_profile(&profile_input("good")).unwrap().id;
    let m = e.manager();
    let err = m
        .launch(
            &v,
            &BrowserLaunchParams {
                profile_id: pid.clone(),
                url: Some("--remote-debugging-port=9222".into()),
                engine: None,
            },
        )
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidParams);
    assert!(!m.is_running(&pid));
}

#[test]
fn foreign_or_symlinked_ram_dir_is_refused() {
    let mut e = env();
    let target = e.root.join("real-ram");
    std::fs::create_dir_all(target.join("aaaaaaaaaaaaaaaaaaaaaaaaaa")).unwrap();
    let link = e.root.join("ram-link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    e.cfg.ram_dir = link;
    let (v, pid) = e.vault();
    let m = e.manager();
    // Not swept through the symlink, and no session is created in it.
    assert!(target.join("aaaaaaaaaaaaaaaaaaaaaaaaaa").exists());
    assert!(m.launch(&v, &launch_params(&pid)).is_err());
    assert!(!target.join(&pid).exists());
}

#[test]
fn only_the_lock_holder_sweeps() {
    let e = env();
    let (v, pid) = e.vault();
    let first = e.manager();
    let udd = e.cfg.ram_dir.join(&pid);
    std::fs::create_dir_all(&udd).unwrap();
    std::fs::write(udd.join("x"), "x").unwrap();
    let meta = serde_json::json!({
        "vault_id": v.id(), "profile_id": pid, "recipient": v.vault_public_key(),
        "archive": e.archive(&v, &pid), "stale_archive": e.root.join("old"),
        "commit": false, "browser_pid": dead_pid(), "agent_pid": 0,
    });
    std::fs::write(udd.join(cb::SESSION_META_FILE), meta.to_string()).unwrap();
    let second = e.manager();
    assert!(udd.exists(), "a second manager must not sweep");
    drop(second);
    drop(first);
    let _third = e.manager();
    assert!(!udd.exists());
    let _ = v;
}

#[test]
fn legacy_session_restores_cookies_without_extension_or_native_host() {
    let e = env();
    let (v, pid) = e.vault();
    let legacy = e.root.join("legacy-session");
    std::fs::create_dir_all(legacy.join("Default")).unwrap();
    std::fs::create_dir_all(legacy.join("extension")).unwrap();
    std::fs::create_dir_all(legacy.join("NativeMessagingHosts")).unwrap();
    std::fs::write(legacy.join("cookies.sqlite"), "legacy-cookie\n").unwrap();
    std::fs::write(legacy.join("extension/manifest.json"), "{}").unwrap();
    std::fs::write(legacy.join("extension/latch-session.json"), "legacy-token").unwrap();
    std::fs::write(
        legacy.join("NativeMessagingHosts/dev.latch.native_host.json"),
        "{}",
    )
    .unwrap();
    cb::write_archive(&legacy, &v.vault_public_key(), &e.archive(&v, &pid)).unwrap();
    let m = e.manager();
    m.launch(&v, &launch_params(&pid)).unwrap();
    e.wait_ready();
    assert_eq!(
        std::fs::read_to_string(e.log.join("cookies-seen")).unwrap(),
        "legacy-cookie\ncookie-1\n"
    );
    let udd = e.cfg.ram_dir.join(&pid);
    assert!(!udd.join("extension").exists());
    assert!(!udd.join("NativeMessagingHosts").exists());
    assert!(!e.args().iter().any(|a| a.starts_with("--load-extension")));
    m.close(&pid).unwrap();
    let listing = archive_listing(&v, &e.archive(&v, &pid));
    assert!(listing.iter().any(|p| p == "cookies.sqlite"));
    assert!(
        !listing
            .iter()
            .any(|p| p.starts_with("extension") || p.starts_with("NativeMessagingHosts"))
    );
}
