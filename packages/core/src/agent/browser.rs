//! Running isolated browser sessions, owned by the agent (SPEC §10.1).
//!
//! Contract:
//! - `launch` prepares the RAM user-data dir (restoring the encrypted session
//!   archive), spawns the browser and tracks the process. When the process exits (or
//!   `close` is called) the dir is pruned, archived (tar+zstd), encrypted to
//!   the vault's public recipient — which needs no secret key, so it also
//!   works after the vault locked — stored, and securely deleted.
//! - Process exit and archival are separate: the browser is reaped only
//!   after it exited (`waitid(WNOWAIT)` first), and signals are sent under the
//!   same lock and only while it is unreaped, so a recycled PID is never
//!   signalled. `close` reports archival failures.
//! - A failed archive keeps the RAM dir and its metadata (the recovery
//!   record); the next sweep or launch retries. Launch refuses to replace a
//!   dir that still cannot be archived (reset discards it).
//! - `on_event` receives `BrowserChanged` events.
//! - One session per profile: a second `launch` returns the running session;
//!   while the previous session is still archiving `launch` waits up to
//!   `ManagerConfig::archive_wait`, then fails with a busy (`Conflict`) error.
//! - `new` validates the RAM dir (a real dir owned by us, 0700; created
//!   securely), takes the sessions lock (`.latch-sessions.lock` in the RAM
//!   dir; only its holder sweeps) and sweeps leftover session dirs of a
//!   previous (crashed) agent: archived when their metadata allows it,
//!   otherwise removed. Swept archives always go to the local data dir path
//!   derived from the trusted config (`data_dir`, validated ids); paths in
//!   the metadata are ignored.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::browser as cb;
use crate::core::format::now;
use crate::core::{Error, Result, UnlockedVault, ids, paths};
use crate::protocol::{
    AgentEvent, AgentEventKind, BrowserLaunchParams, BrowserSession, DetectedBrowser,
    SessionArchiveInfo,
};

pub type EventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

/// Lock file in the RAM dir held by the manager that owns (and sweeps) it.
pub const SESSIONS_LOCK_FILE: &str = ".latch-sessions.lock";

/// How long `close` waits for archival after the browser exited.
const CLOSE_ARCHIVE_WAIT: Duration = Duration::from_secs(120);

/// Where sessions live and which binaries to use. [`ManagerConfig::from_env`]
/// is the production configuration; tests point everything at temp dirs.
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// Parent of the per-profile user-data dirs (`paths::ram_dir()`).
    pub ram_dir: PathBuf,
    /// Local archive root (`paths::data_dir()`).
    pub data_dir: PathBuf,
    /// Explicit test/embedder fixture; production never discovers a host browser.
    pub browser_override: Option<PathBuf>,
    /// Legacy fixture configuration; never used to discover installed browsers.
    pub path_var: Option<OsString>,
    /// SIGTERM → SIGKILL grace period.
    pub term_timeout: Duration,
    /// How long a relaunch waits for the previous session of the same
    /// profile to finish archiving before failing as busy.
    pub archive_wait: Duration,
}

impl ManagerConfig {
    pub fn from_env() -> Self {
        ManagerConfig {
            ram_dir: paths::ram_dir(),
            data_dir: paths::data_dir(),
            browser_override: None,
            path_var: None,
            term_timeout: Duration::from_secs(10),
            archive_wait: Duration::from_secs(30),
        }
    }
}

/// Persisted next to the profile data so a crashed agent's dirs can still be
/// archived (public data only). Written by us into our 0700 RAM dir, but the
/// sweep still treats it as untrusted: only ids (validated) and the public
/// recipient are used; `archive`/`stale_archive` are informational.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMeta {
    vault_id: String,
    profile_id: String,
    recipient: String,
    archive: PathBuf,
    /// Archive location not in use (removed after a successful write).
    stale_archive: PathBuf,
    commit: bool,
    browser_pid: u32,
    agent_pid: u32,
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Archival outcome, set once by the watcher.
#[derive(Default)]
struct Done {
    result: Mutex<Option<std::result::Result<(), String>>>,
    cv: Condvar,
}

impl Done {
    fn set(&self, r: std::result::Result<(), String>) {
        *guard(&self.result) = Some(r);
        self.cv.notify_all();
    }
    /// `None` on timeout.
    fn wait(&self, timeout: Duration) -> Option<std::result::Result<(), String>> {
        let g = guard(&self.result);
        let (g, _) = self
            .cv
            .wait_timeout_while(g, timeout, |d| d.is_none())
            .unwrap_or_else(|p| p.into_inner());
        g.clone()
    }
}

/// A spawned browser process. `exited` flips (under the lock) right before
/// the process is reaped; signals are only sent while holding the lock and
/// `!exited`, so the PID cannot have been recycled.
struct Proc {
    pid: u32,
    exited: Mutex<bool>,
    cv: Condvar,
}

impl Proc {
    fn new(pid: u32) -> Self {
        Proc {
            pid,
            exited: Mutex::new(false),
            cv: Condvar::new(),
        }
    }
    fn is_exited(&self) -> bool {
        *guard(&self.exited)
    }
    fn terminate(&self) {
        let g = guard(&self.exited);
        if !*g {
            terminate(self.pid);
        }
    }
    fn kill(&self) {
        let g = guard(&self.exited);
        if !*g {
            // The leader is unreaped, so its pid (= the process group id)
            // is still in use and cannot name another group.
            kill(self.pid);
        }
    }
    fn wait_exit(&self, timeout: Duration) -> bool {
        let g = guard(&self.exited);
        let (g, _) = self
            .cv
            .wait_timeout_while(g, timeout, |e| !*e)
            .unwrap_or_else(|p| p.into_inner());
        *g
    }
    /// Watcher side: block until the child exits, then mark and reap it.
    fn wait_and_reap(&self, mut child: Child) {
        wait_exit_nowait(&child);
        {
            let mut g = guard(&self.exited);
            *g = true;
            let _ = child.wait();
        }
        self.cv.notify_all();
    }
}

/// Wait for `child` to exit without reaping it (unix: `waitid` with
/// `WNOWAIT`). Elsewhere a plain wait.
#[cfg(unix)]
fn wait_exit_nowait(child: &Child) {
    loop {
        // SAFETY: zeroed siginfo_t is a valid out-parameter for waitid.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: waitid on our own (unreaped) child with a valid pointer.
        let r = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if r == 0 {
            return;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return;
        }
    }
}

#[cfg(not(unix))]
fn wait_exit_nowait(_child: &Child) {}

struct Running {
    session: BrowserSession,
    proc: Arc<Proc>,
    done: Arc<Done>,
}

struct Inner {
    cfg: ManagerConfig,
    on_event: EventSink,
    sessions: Mutex<HashMap<String, Running>>,
    /// Held while this manager owns the RAM dir (see module docs).
    _lock: Option<std::fs::File>,
}

pub struct BrowserManager {
    inner: Arc<Inner>,
}

/// Where a swept dir is archived: derived from trusted config only.
fn local_archive(data_dir: &Path, vault_id: &str, profile_id: &str) -> PathBuf {
    data_dir
        .join("sessions")
        .join(vault_id)
        .join(cb::archive_file_name(profile_id))
}

fn modified(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p)
        .ok()
        .filter(|m| m.is_file())
        .and_then(|m| m.modified().ok())
}

impl BrowserManager {
    pub fn new(on_event: EventSink) -> Self {
        Self::with_config(on_event, ManagerConfig::from_env())
    }

    /// Construct with explicit dirs; when the RAM dir is safe and this
    /// manager gets the sessions lock, sweeps stale session dirs.
    pub fn with_config(on_event: EventSink, cfg: ManagerConfig) -> Self {
        let lock = match paths::ensure_secure_dir(&cfg.ram_dir) {
            Ok(()) => take_sessions_lock(&cfg.ram_dir),
            Err(e) => {
                tracing::error!("browser sessions disabled: {e}");
                None
            }
        };
        let sweep = lock.is_some();
        let m = BrowserManager {
            inner: Arc::new(Inner {
                cfg,
                on_event,
                sessions: Mutex::new(HashMap::new()),
                _lock: lock,
            }),
        };
        if sweep {
            m.sweep_stale();
        }
        m
    }

    pub fn config(&self) -> &ManagerConfig {
        &self.inner.cfg
    }

    /// Another handle to the same manager (shares sessions and the lock).
    pub fn clone_handle(&self) -> BrowserManager {
        BrowserManager {
            inner: self.inner.clone(),
        }
    }

    pub fn launch(
        &self,
        vault: &UnlockedVault,
        params: &BrowserLaunchParams,
    ) -> Result<BrowserSession> {
        let pages = match params.url.as_deref().filter(|u| !u.trim().is_empty()) {
            Some(u) => Some(vec![cb::normalize_start_url(u)?]),
            None => None,
        };
        self.launch_pages(vault, params, pages)
    }

    /// Open an action's explicit pages, including in an existing Chromium session.
    pub(crate) fn launch_urls(
        &self,
        vault: &UnlockedVault,
        profile_id: &str,
        urls: &[String],
    ) -> Result<BrowserSession> {
        let pages = urls
            .iter()
            .map(|u| cb::normalize_start_url(u))
            .collect::<Result<Vec<_>>>()?;
        self.launch_pages(
            vault,
            &BrowserLaunchParams {
                profile_id: profile_id.into(),
                engine: None,
                url: None,
            },
            Some(pages),
        )
    }

    fn launch_pages(
        &self,
        vault: &UnlockedVault,
        params: &BrowserLaunchParams,
        pages: Option<Vec<String>>,
    ) -> Result<BrowserSession> {
        ids::check_id(&params.profile_id)?;
        loop {
            let mut map = guard(&self.inner.sessions);
            if let Some(r) = map.get(&params.profile_id) {
                if !r.proc.is_exited() {
                    if pages.as_ref().is_some_and(|urls| !urls.is_empty()) {
                        return Err(Error::Conflict("this workspace browser is already open; close it before opening different action URLs".into()));
                    }
                    return Ok(r.session.clone());
                }
                // Exited, still archiving: the old watcher owns the dir.
                let done = r.done.clone();
                let proc = r.proc.clone();
                drop(map);
                if done.wait(self.inner.cfg.archive_wait).is_none() {
                    return Err(Error::Conflict(
                        "the previous browser session of this profile is still being archived; try again shortly".into(),
                    ));
                }
                let mut map = guard(&self.inner.sessions);
                if map
                    .get(&params.profile_id)
                    .is_some_and(|r| Arc::ptr_eq(&r.proc, &proc))
                {
                    map.remove(&params.profile_id);
                }
                continue;
            }
            let (running, child, meta) = self.start(vault, params, pages.clone())?;
            let session = running.session.clone();
            let proc = running.proc.clone();
            let done = running.done.clone();
            map.insert(params.profile_id.clone(), running);
            drop(map);
            self.spawn_watcher(child, meta, proc, done);
            self.emit(&params.profile_id, None);
            return Ok(session);
        }
    }

    fn start(
        &self,
        vault: &UnlockedVault,
        params: &BrowserLaunchParams,
        pages: Option<Vec<String>>,
    ) -> Result<(Running, Child, SessionMeta)> {
        let cfg = &self.inner.cfg;
        let profile = vault.get_profile(&params.profile_id)?;
        // Legacy profile engine/binary fields remain readable, but every product
        // launch uses the sole app-managed provider. Fixture injection is explicit.
        let engine = crate::protocol::BrowserEngine::Firefox;
        let managed = cfg.browser_override.is_none();
        let bin = match &cfg.browser_override {
            Some(fixture) => fixture.clone(),
            None => {
                let root = cfg.data_dir.join("browser-runtime");
                if let Ok(vault_root) = vault.path().canonicalize()
                    && (cfg.data_dir.starts_with(&vault_root)
                        || cfg.ram_dir.starts_with(&vault_root))
                {
                    return Err(Error::Denied(
                        "browser runtime and sessions must be outside the vault".into(),
                    ));
                }
                paths::ensure_secure_dir(&cfg.data_dir)
                    .map_err(|e| Error::io("create private browser data directory", e))?;
                let binary = cb::runtime::ensure(&root)?;
                cb::credentials::install_bootstrap(&binary)?;
                binary
            }
        };

        paths::ensure_secure_dir(&cfg.ram_dir)
            .map_err(|e| Error::io(format!("create {}", cfg.ram_dir.display()), e))?;
        let udd = cfg.ram_dir.join(&profile.id);
        if std::fs::symlink_metadata(&udd).is_ok() {
            // Leftover from a crash or a failed archive: retry; never drop
            // session data that could not be archived.
            if let Err(e) = self.finish_dir(&udd) {
                return Err(Error::Conflict(format!(
                    "the previous browser session of this profile could not be archived ({e}); fix the problem and retry, or reset the browser session"
                )));
            }
            if std::fs::symlink_metadata(&udd).is_ok() {
                remove_leftover(&udd);
            }
        }
        paths::ensure_secure_dir(&udd)
            .map_err(|e| Error::io(format!("create {}", udd.display()), e))?;

        let commit = vault.effective_settings().sessions_commit;
        let (archive, alt) =
            cb::archive_locations(&cfg.data_dir, vault.path(), vault.id(), commit, &profile.id);
        let result = (|| -> Result<(Child, SessionMeta)> {
            // Both may exist after a settings change: restore the newer.
            let existing = match (modified(&archive), modified(&alt)) {
                (Some(a), Some(b)) => Some(if b > a { &alt } else { &archive }),
                (Some(_), None) => Some(&archive),
                (None, Some(_)) => Some(&alt),
                (None, None) => None,
            };
            if let Some(a) = existing {
                cb::restore_archive(vault, a, &udd).map_err(|e| match e {
                    Error::Integrity(_) => Error::integrity(
                        "the browser session archive cannot be restored; reset the session to start fresh",
                    ),
                    e => e,
                })?;
            }
            // Restored archives can predate extension retirement. Drop legacy
            // host manifests, extension tokens and stale process locks.
            cb::prune(&udd);
            cb::write_firefox_profile(&udd, &profile.browser)?;
            let generation = if managed {
                Some(cb::credentials::prepare(vault, &profile.id, &udd)?)
            } else {
                None
            };

            let urls: Vec<String> = match pages {
                Some(urls) => urls,
                None => cb::safe_start_urls(&profile.browser.start_pages),
            };
            let class = cb::window_class(&profile.name, &profile.id);
            let args = cb::launch_args(
                engine,
                &udd,
                &class,
                profile.browser.extensions_allowed,
                &urls,
            );
            let mut cmd = Command::new(&bin);
            cmd.args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            // Persist a conservative recovery record BEFORE starting a process.
            // PID 0 means startup was interrupted and recovery must retain state.
            let mut meta = SessionMeta {
                vault_id: vault.id().to_owned(),
                profile_id: profile.id.clone(),
                recipient: vault.vault_public_key(),
                archive: archive.clone(),
                stale_archive: alt.clone(),
                commit,
                browser_pid: 0,
                agent_pid: std::process::id(),
            };
            let record = |meta: &SessionMeta| -> Result<()> {
                let bytes = serde_json::to_vec(meta)
                    .map_err(|_| Error::Internal("encode browser recovery metadata".into()))?;
                cb::write_file_in(&udd, Path::new(cb::SESSION_META_FILE), &bytes)?;
                Ok(())
            };
            record(&meta)?;
            let mut child = cmd
                .spawn()
                .map_err(|e| Error::io("launch managed Firefox", e))?;
            meta.browser_pid = child.id();
            let startup = record(&meta).and_then(|()| match generation {
                Some(generation) => {
                    cb::credentials::wait(&udd, &generation, Duration::from_secs(30))
                }
                None => Ok(()),
            });
            if let Err(error) = startup {
                kill(child.id());
                let _ = child.wait();
                return Err(error);
            }
            Ok((child, meta))
        })();
        let (child, meta) = match result {
            Ok(v) => v,
            Err(e) => {
                cb::secure_delete(&udd);
                return Err(e);
            }
        };
        let session = BrowserSession {
            profile_id: profile.id.clone(),
            engine,
            pid: child.id(),
            started_at: now(),
            user_data_dir: udd.display().to_string(),
        };
        Ok((
            Running {
                session,
                proc: Arc::new(Proc::new(child.id())),
                done: Arc::new(Done::default()),
            },
            child,
            meta,
        ))
    }

    fn spawn_watcher(&self, child: Child, meta: SessionMeta, proc: Arc<Proc>, done: Arc<Done>) {
        let inner = self.inner.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("latch-browser-{}", meta.profile_id))
            .spawn(move || {
                proc.wait_and_reap(child);
                let udd = inner.cfg.ram_dir.join(&meta.profile_id);
                let res = finish(
                    &udd,
                    &meta.recipient,
                    &meta.archive,
                    Some(&meta.stale_archive),
                    meta.commit,
                );
                {
                    let mut map = guard(&inner.sessions);
                    if map
                        .get(&meta.profile_id)
                        .is_some_and(|r| Arc::ptr_eq(&r.proc, &proc))
                    {
                        map.remove(&meta.profile_id);
                    }
                }
                let msg = match &res {
                    Ok(m) => m.clone(),
                    Err(e) => Some(format!("archiving the browser session failed: {e}")),
                };
                done.set(res.map(|_| ()));
                (inner.on_event)(AgentEvent {
                    kind: AgentEventKind::BrowserChanged,
                    profile_id: Some(meta.profile_id.clone()),
                    entry_id: None,
                    message: msg,
                });
            });
        if let Err(e) = spawned {
            tracing::error!("cannot spawn browser watcher: {e}");
        }
    }

    fn emit(&self, profile_id: &str, message: Option<String>) {
        (self.inner.on_event)(AgentEvent {
            kind: AgentEventKind::BrowserChanged,
            profile_id: Some(profile_id.to_owned()),
            entry_id: None,
            message,
        });
    }

    /// Reconcile active managed profiles after a successful vault mutation.
    /// A failed refresh closes the browser so stale copied logins stay unusable.
    pub fn refresh_credentials(&self, vault: &UnlockedVault) {
        if self.inner.cfg.browser_override.is_some() {
            return;
        }
        for session in self.list() {
            let profile = PathBuf::from(&session.user_data_dir);
            let result = cb::credentials::prepare(vault, &session.profile_id, &profile).and_then(
                |generation| cb::credentials::wait(&profile, &generation, Duration::from_secs(10)),
            );
            if result.is_err() {
                let _ = self.close(&session.profile_id);
                self.emit(
                    &session.profile_id,
                    Some(
                        "Browser closed because workspace logins could not be synchronized".into(),
                    ),
                );
            }
        }
    }

    pub fn list(&self) -> Vec<BrowserSession> {
        let map = guard(&self.inner.sessions);
        let mut v: Vec<BrowserSession> = map
            .values()
            .filter(|r| !r.proc.is_exited())
            .map(|r| r.session.clone())
            .collect();
        v.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        v
    }

    pub fn is_running(&self, profile_id: &str) -> bool {
        let map = guard(&self.inner.sessions);
        map.get(profile_id).is_some_and(|r| !r.proc.is_exited())
    }

    /// Graceful terminate (SIGKILL after `term_timeout`), then wait for the
    /// archive. Blocks until archived; archival failures are returned.
    pub fn close(&self, profile_id: &str) -> Result<()> {
        let (proc, done) = {
            let map = guard(&self.inner.sessions);
            match map.get(profile_id) {
                Some(r) => (r.proc.clone(), r.done.clone()),
                None => return Ok(()),
            }
        };
        proc.terminate();
        if !proc.wait_exit(self.inner.cfg.term_timeout) {
            proc.kill();
            if !proc.wait_exit(Duration::from_secs(10)) {
                return Err(Error::Internal("the browser did not exit".into()));
            }
        }
        match done.wait(CLOSE_ARCHIVE_WAIT) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(Error::Internal(format!(
                "archiving the browser session failed (kept in memory for a retry): {e}"
            ))),
            None => Err(Error::Internal(
                "browser session did not finish archiving in time".into(),
            )),
        }
    }

    /// Used on lock (after the grace period) and on agent shutdown.
    pub fn close_all(&self) {
        let ids: Vec<String> = guard(&self.inner.sessions).keys().cloned().collect();
        std::thread::scope(|s| {
            for id in &ids {
                s.spawn(move || {
                    if let Err(e) = self.close(id) {
                        tracing::warn!("closing browser session failed: {e}");
                    }
                });
            }
        });
    }

    /// Close the session if running (an archival failure is ignored: the
    /// session is being discarded), remove any leftover session dir, then
    /// delete its archive (both locations).
    pub fn reset(&self, vault: &UnlockedVault, profile_id: &str) -> Result<()> {
        ids::check_id(profile_id)?;
        if let Err(e) = self.close(profile_id) {
            tracing::warn!("reset: {e}");
        }
        if guard(&self.inner.sessions)
            .get(profile_id)
            .is_some_and(|r| !r.proc.is_exited())
        {
            return Err(Error::Conflict(
                "the browser session is still running".into(),
            ));
        }
        let udd = self.inner.cfg.ram_dir.join(profile_id);
        if std::fs::symlink_metadata(&udd).is_ok() {
            remove_leftover(&udd);
        }
        let (a, b) = cb::archive_locations(
            &self.inner.cfg.data_dir,
            vault.path(),
            vault.id(),
            vault.effective_settings().sessions_commit,
            profile_id,
        );
        for p in [a, b] {
            if std::fs::symlink_metadata(&p).is_ok() {
                std::fs::remove_file(&p)
                    .map_err(|e| Error::io(format!("remove {}", p.display()), e))?;
            }
        }
        self.emit(profile_id, None);
        Ok(())
    }

    pub fn session_info(
        &self,
        vault: &UnlockedVault,
        profile_id: &str,
    ) -> Result<SessionArchiveInfo> {
        ids::check_id(profile_id)?;
        let (pref, alt) = cb::archive_locations(
            &self.inner.cfg.data_dir,
            vault.path(),
            vault.id(),
            vault.effective_settings().sessions_commit,
            profile_id,
        );
        let in_repo = |p: &Path| p.starts_with(vault.path());
        for p in [pref, alt] {
            if let Ok(m) = std::fs::metadata(&p) {
                let updated_at = m.modified().ok().and_then(|t| {
                    time::OffsetDateTime::from(t)
                        .format(&time::format_description::well_known::Rfc3339)
                        .ok()
                });
                return Ok(SessionArchiveInfo {
                    exists: true,
                    size_bytes: m.len(),
                    updated_at,
                    committed: in_repo(&p),
                });
            }
        }
        Ok(SessionArchiveInfo {
            exists: false,
            size_bytes: 0,
            updated_at: None,
            committed: false,
        })
    }

    pub fn detect() -> Vec<DetectedBrowser> {
        cb::detect()
    }

    /// Validated metadata of a leftover dir named `name`.
    fn read_meta(udd: &Path) -> Option<SessionMeta> {
        let name = udd.file_name()?.to_str()?.to_owned();
        let p = udd.join(cb::SESSION_META_FILE);
        let m = std::fs::symlink_metadata(&p).ok()?;
        if !m.is_file() || m.len() > 64 * 1024 {
            return None;
        }
        let meta: SessionMeta = serde_json::from_slice(&std::fs::read(&p).ok()?).ok()?;
        (meta.profile_id == name
            && ids::is_valid_id(&meta.profile_id)
            && ids::is_valid_id(&meta.vault_id))
        .then_some(meta)
    }

    /// Archive (when possible) and delete one leftover dir. Dirs without
    /// usable metadata are deleted; with metadata the archive goes to the
    /// local data-dir location (paths in the metadata are never used).
    /// On failure the dir and its metadata stay for a later retry.
    fn finish_dir(&self, udd: &Path) -> std::result::Result<(), String> {
        match Self::read_meta(udd) {
            Some(m) => {
                stop_orphan(&m, udd)?;
                let archive = local_archive(&self.inner.cfg.data_dir, &m.vault_id, &m.profile_id);
                finish(udd, &m.recipient, &archive, None, false).map(|_| ())
            }
            None => Err("browser recovery metadata is missing or unreadable; close any old browser before resetting this session".into()),
        }
    }

    /// Handle session dirs left behind by a crashed agent.
    fn sweep_stale(&self) {
        let Ok(rd) = std::fs::read_dir(&self.inner.cfg.ram_dir) else {
            return;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !ids::is_valid_id(&name) || !e.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let udd = e.path();
            if let Some(m) = Self::read_meta(&udd)
                && m.agent_pid != std::process::id()
                && pid_alive(m.agent_pid)
            {
                continue; // possibly owned by another live agent
            }
            tracing::info!("cleaning up stale browser session dir {}", udd.display());
            if let Err(e) = self.finish_dir(&udd) {
                tracing::error!("stale browser session kept for a retry: {e}");
            }
        }
    }
}

fn take_sessions_lock(ram_dir: &Path) -> Option<std::fs::File> {
    use fs2::FileExt;
    let p = ram_dir.join(SESSIONS_LOCK_FILE);
    let mut o = std::fs::OpenOptions::new();
    o.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW).mode(0o600);
    }
    match o.open(&p) {
        Ok(f) => {
            // A just-released lock can still be shared briefly by a child
            // forked (not yet exec'd) elsewhere in this process: retry a
            // little before concluding another process owns the dir.
            for _ in 0..10 {
                if f.try_lock_exclusive().is_ok() {
                    return Some(f);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            tracing::info!("another Latch process owns the browser sessions dir; not sweeping");
            None
        }
        Err(e) => {
            tracing::warn!("cannot open {}: {e}", p.display());
            None
        }
    }
}

/// Remove a session dir (a symlink is removed itself, never followed).
fn remove_leftover(udd: &Path) {
    match std::fs::symlink_metadata(udd) {
        Ok(m) if m.is_dir() => cb::secure_delete(udd),
        Ok(_) => {
            let _ = std::fs::remove_file(udd);
        }
        Err(_) => {}
    }
}

/// Prune, archive, then securely delete. The metadata file stays until the
/// archive is durable. Returns a warning message, if any.
fn finish(
    udd: &Path,
    recipient: &str,
    archive: &Path,
    stale: Option<&Path>,
    commit: bool,
) -> std::result::Result<Option<String>, String> {
    match std::fs::symlink_metadata(udd) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            remove_leftover(udd);
            return Ok(None);
        }
        Err(_) => return Ok(None),
    }
    cb::prune(udd);
    match cb::write_archive(udd, recipient, archive) {
        Ok(size) => {
            if let Some(s) = stale
                && s != archive
                && std::fs::symlink_metadata(s).is_ok_and(|m| m.is_file())
            {
                let _ = std::fs::remove_file(s);
            }
            cb::secure_delete(udd);
            if commit && size > cb::ARCHIVE_WARN_BYTES {
                let msg = format!(
                    "browser session archive is {} MiB and committed to git; consider git LFS or disabling session commits",
                    size / (1024 * 1024)
                );
                tracing::warn!("{msg}");
                return Ok(Some(msg));
            }
            Ok(None)
        }
        Err(e) => {
            // Keep the (RAM-backed) dir and its metadata so a later sweep
            // or launch can retry.
            tracing::error!("archiving browser session failed: {e}");
            Err(e.to_string())
        }
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    // SAFETY: signal 0 only checks for existence/permission.
    (unsafe { libc::kill(pid as libc::pid_t, 0) == 0 })
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn terminate(pid: u32) {
    // SAFETY: kill(2) on our unreaped child (see `Proc`).
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
}

#[cfg(unix)]
fn kill(pid: u32) {
    // SAFETY: as above; the browser runs in its own process group.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn terminate(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn kill(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// A browser orphaned by a crashed agent: stop it before archiving, but only
/// when the pid still belongs to a process using this user-data dir. On
/// Linux the process is pinned with a pidfd before its command line is
/// checked, and signalled through that pidfd (never a recycled pid).
fn stop_orphan(meta: &SessionMeta, udd: &Path) -> std::result::Result<(), String> {
    const RETAIN: &str = "browser may still be running; close it before retrying session recovery";
    if meta.browser_pid == 0 || meta.browser_pid > i32::MAX as u32 {
        return Err(RETAIN.into());
    }
    if !pid_alive(meta.browser_pid) {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let pid = meta.browser_pid as libc::pid_t;
        // SAFETY: pidfd_open returns a new descriptor or -1.
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
        if raw < 0 {
            return Err(RETAIN.into());
        }
        // SAFETY: this is the descriptor just opened and owned by us.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let needle = udd.as_os_str().as_encoded_bytes();
        if !cmdline.split(|byte| *byte == 0).any(|arg| arg == needle) {
            return Err(RETAIN.into());
        }
        let send = |signal: i32| {
            // SAFETY: signal addresses the pinned process, never a recycled PID.
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    fd.as_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                ) == 0
            }
        };
        let exited = |timeout_ms| {
            let mut descriptor = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: descriptor is valid and points to one initialized pollfd.
            unsafe {
                libc::poll(&mut descriptor, 1, timeout_ms) > 0
                    && descriptor.revents & libc::POLLIN != 0
            }
        };
        if exited(0) {
            return Ok(());
        }
        if !send(libc::SIGTERM) {
            return Err(RETAIN.into());
        }
        if exited(5000) {
            return Ok(());
        }
        if !send(libc::SIGKILL) || !exited(5000) {
            return Err(RETAIN.into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        // macOS has no Linux pidfd equivalent here. Never signal an unpinned
        // PID or archive/delete a profile while its owner might still be alive.
        let _ = udd;
        Err(RETAIN.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_retains_a_directory_when_a_live_pid_cannot_be_authenticated() {
        let dir = tempfile::tempdir().unwrap();
        let meta = SessionMeta {
            vault_id: ids::new_id(),
            profile_id: ids::new_id(),
            recipient: String::new(),
            archive: dir.path().join("archive"),
            stale_archive: dir.path().join("old"),
            commit: false,
            browser_pid: std::process::id(),
            agent_pid: 0,
        };
        assert!(stop_orphan(&meta, dir.path()).is_err());
        assert!(dir.path().exists());
    }

    #[test]
    fn recovery_never_deletes_a_profile_without_readable_metadata() {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("aaaaaaaaaaaaaaaaaaaaaaaaaa");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("cookies.sqlite"), b"fictional session").unwrap();
        let manager = BrowserManager::with_config(
            Arc::new(|_| {}),
            ManagerConfig {
                ram_dir: root.path().join("ram"),
                data_dir: root.path().join("data"),
                browser_override: None,
                path_var: None,
                term_timeout: Duration::from_millis(10),
                archive_wait: Duration::from_millis(10),
            },
        );
        assert!(manager.finish_dir(&profile).is_err());
        assert!(profile.join("cookies.sqlite").is_file());
    }

    #[test]
    fn done_waits() {
        let d = Arc::new(Done::default());
        assert!(d.wait(Duration::from_millis(10)).is_none());
        let d2 = d.clone();
        std::thread::spawn(move || d2.set(Err("x".into())));
        assert_eq!(d.wait(Duration::from_secs(5)), Some(Err("x".to_string())));
    }

    #[cfg(unix)]
    #[test]
    fn proc_never_signals_after_reap() {
        let child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        let p = Arc::new(Proc::new(child.id()));
        let p2 = p.clone();
        let h = std::thread::spawn(move || p2.wait_and_reap(child));
        assert!(p.wait_exit(Duration::from_secs(10)));
        h.join().unwrap();
        // Reaped: these must be no-ops (the pid may already be recycled).
        assert!(p.is_exited());
        p.terminate();
        p.kill();
    }
}
