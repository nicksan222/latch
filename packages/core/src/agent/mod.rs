//! Agent: holds the unlocked vault and serves RPC (SPEC §7).
//!
//! # Embedding API (desktop shell)
//!
//! The agent is a plain library object; the headless `latch agent run`, the
//! CLI's standalone mode and the Tauri shell all use the same type.
//!
//! ```no_run
//! use std::sync::Arc;
//! use latch::agent::{Agent, AgentOptions, ClientCtx};
//! use latch::rpc::LocalClient;
//! use latch_rpc::sdk::RpcSdk;
//!
//! let agent: Arc<Agent> = Agent::new(AgentOptions::default());
//! agent.serve_socket()?;                    // authenticated local gRPC for CLI
//! let mut client = LocalClient::new(&agent, ClientCtx::desktop());
//! let status = client.vault_status()?;      // generated typed client
//! let events = client.watch_events()?;     // generated gRPC server stream
//! agent.notify_screen_locked();             // OS hook; locks if lockOnScreenLock
//! agent.lock();                             // explicit lock (tray, shortcut)
//! agent.shutdown();                         // on app exit: lock, close browsers,
//!                                           // flush clipboard clears, unbind socket
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! - [`Agent::new`]`(AgentOptions) -> Arc<Agent>` starts one background
//!   timer thread (auto-lock, clipboard clears, browser grace close, headless
//!   idle exit) and, when `watch_screen_lock`, the Linux screen-lock watcher.
//! - [`Agent::dispatch`]`(&self, &ClientCtx, RpcRequest) -> RpcResponse`:
//!   every method of `protocol::methods`. `ClientCtx::desktop()`/`cli()` get
//!   all supported methods. This is backend dispatch machinery; consumers use
//!   generated gRPC clients from the standalone `latch-rpc` interface.
//! - [`Agent::subscribe`]`() -> broadcast::Receiver<AgentEvent>`; forward
//!   to the Tauri event `latch://event`. Sending never needs a runtime.
//! - [`Agent::serve_socket`]`(self: &Arc<Self>) -> core::Result<PathBuf>`
//!   binds `paths::runtime_dir()/agent.sock` (single instance via
//!   `agent.lock`); `Err(AlreadyExists)` when another agent runs.
//! - [`Agent::lock`], [`Agent::notify_screen_locked`], [`Agent::shutdown`],
//!   [`Agent::wait`] (block until shutdown), [`Agent::status`],
//!   [`Agent::integrity_problems`], [`Agent::browsers`].
//! - Clipboard: [`clipboard::ClipboardBackend`]; pass the Tauri clipboard
//!   plugin wrapped in it through `AgentOptions::clipboard`.
//!
//! Locking model: the vault slot is an `RwLock`; reads run concurrently,
//! writes (commits) are serialized. Unlock runs the KDF outside the lock.

mod actions;
pub mod browser;
pub mod client;
pub mod clipboard;
mod dispatch;
mod screen;
#[cfg(unix)]
pub(crate) mod server;
pub mod state;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use zeroize::Zeroizing;

use crate::core::{UnlockedVault, Vault, paths};
use crate::protocol::{AgentEvent, AgentEventKind, ClientKind, Settings, VaultStatus};

use self::browser::BrowserManager;
use self::clipboard::{ArboardClipboard, ClipboardBackend};

#[cfg(unix)]
pub use server::peer_uid;

/// Socket file name inside `paths::runtime_dir()`.
pub const SOCKET_FILE: &str = "agent.sock";
/// Single-instance lock file beside the socket.
pub const LOCK_FILE: &str = "agent.lock";
/// PID of the agent serving the socket (used by `latch agent stop`).
pub const PID_FILE: &str = "agent.pid";
/// Overrides the headless idle-exit delay (seconds).
pub const ENV_IDLE_EXIT_SECS: &str = "LATCH_AGENT_IDLE_EXIT_SECS";

pub fn socket_path() -> PathBuf {
    paths::runtime_dir().join(SOCKET_FILE)
}

pub fn pid_path() -> PathBuf {
    paths::runtime_dir().join(PID_FILE)
}

pub struct AgentOptions {
    /// Defaults to `paths::config_dir()`.
    pub config_dir: Option<PathBuf>,
    /// Explicit browser paths for isolated embedders; defaults to environment configuration.
    pub browser_config: Option<browser::ManagerConfig>,
    /// Defaults to [`ArboardClipboard`].
    pub clipboard: Option<Arc<dyn ClipboardBackend>>,
    /// Headless mode: shut down after this long locked with no clients.
    pub headless_idle_exit: Option<Duration>,
    /// Start the best-effort OS screen-lock watcher (Linux: gdbus/dbus-monitor).
    pub watch_screen_lock: bool,
    /// Grace period between lock and closing browsers.
    pub browser_lock_grace: Duration,
    /// Timer resolution.
    pub tick: Duration,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            config_dir: None,
            browser_config: None,
            clipboard: None,
            headless_idle_exit: None,
            watch_screen_lock: false,
            browser_lock_grace: Duration::from_secs(30),
            tick: Duration::from_millis(250),
        }
    }
}

/// Who is calling. Created by [`Agent::hello`] for socket clients or with
/// [`ClientCtx::desktop`]/[`ClientCtx::cli`] in-process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientCtx {
    pub kind: ClientKind,
}

impl ClientCtx {
    pub fn desktop() -> Self {
        Self {
            kind: ClientKind::Desktop,
        }
    }
    pub fn cli() -> Self {
        Self {
            kind: ClientKind::Cli,
        }
    }
}

pub(crate) struct Slot {
    pub path: Option<PathBuf>,
    pub unlocked: Option<UnlockedVault>,
}

struct Activity {
    /// Last non-status request (auto-lock).
    last: Instant,
    /// Last request of any kind or connection change (headless exit).
    last_any: Instant,
    timeout_override: Option<u64>,
    /// `LockWarning` already emitted for the current idle period.
    warned: bool,
}

/// Emit `LockWarning` this long before the idle lock (capped at half the
/// timeout for short timeouts).
const IDLE_WARNING_SECS: u64 = 30;

struct PendingClear {
    at: Instant,
    value: Zeroizing<String>,
}

#[derive(Default)]
struct Timers {
    clipboard: Vec<PendingClear>,
}

pub struct Agent {
    config_dir: PathBuf,
    opts: AgentOptions,
    pub(crate) slot: RwLock<Slot>,
    unlocked: AtomicBool,
    /// Lifecycle generation: bumped (under the slot write lock) by lock,
    /// shutdown and vault switches. An unlock publishes its keys only if the
    /// generation did not change while its KDF ran.
    pub(crate) generation: AtomicU64,
    /// An idle-lock worker is waiting for browser termination and archival.
    locking_in_background: Arc<AtomicBool>,
    activity: Mutex<Activity>,
    settings: Mutex<Settings>,
    events: broadcast::Sender<AgentEvent>,
    browsers: BrowserManager,
    clipboard: Arc<dyn ClipboardBackend>,
    timers: Mutex<Timers>,
    stopped: Mutex<bool>,
    wake: Condvar,
    shutting_down: AtomicBool,
    connections: AtomicUsize,
    #[cfg(unix)]
    server: Mutex<Option<server::ServerState>>,
    screen: Mutex<Option<screen::Watcher>>,
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("config_dir", &self.config_dir)
            .field("unlocked", &self.is_unlocked())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Agent {
    pub fn new(mut opts: AgentOptions) -> Arc<Agent> {
        let config_dir = opts.config_dir.clone().unwrap_or_else(paths::config_dir);
        let (events, _) = broadcast::channel(256);
        let sink_tx = events.clone();
        let sink: browser::EventSink = Arc::new(move |e: AgentEvent| {
            let _ = sink_tx.send(e);
        });
        let browsers = match opts.browser_config.take() {
            Some(config) => BrowserManager::with_config(sink, config),
            None => BrowserManager::new(sink),
        };
        let clipboard = opts
            .clipboard
            .take()
            .unwrap_or_else(|| Arc::new(ArboardClipboard::new()));
        let vaults = state::VaultsFile::load(&config_dir);
        let settings = state::load_settings(&config_dir);
        let now = Instant::now();
        let agent = Arc::new(Agent {
            slot: RwLock::new(Slot {
                path: vaults.active_path(),
                unlocked: None,
            }),
            unlocked: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            locking_in_background: Arc::new(AtomicBool::new(false)),
            activity: Mutex::new(Activity {
                last: now,
                last_any: now,
                timeout_override: None,
                warned: false,
            }),
            settings: Mutex::new(settings),
            events,
            browsers,
            clipboard,
            timers: Mutex::new(Timers::default()),
            stopped: Mutex::new(false),
            wake: Condvar::new(),
            shutting_down: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
            #[cfg(unix)]
            server: Mutex::new(None),
            screen: Mutex::new(None),
            config_dir,
            opts,
        });
        let weak = Arc::downgrade(&agent);
        let tick = agent.opts.tick;
        std::thread::Builder::new()
            .name("latch-agent-timer".into())
            .spawn(move || timer_loop(weak, tick))
            .expect("spawn timer thread");
        if agent.opts.watch_screen_lock {
            *lock(&agent.screen) = screen::start(Arc::downgrade(&agent));
        }
        agent
    }

    /// Subscribe to agent events (lock state, data changes, browsers).
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.events.subscribe()
    }

    pub fn browsers(&self) -> &BrowserManager {
        &self.browsers
    }

    pub fn config_dir(&self) -> &std::path::Path {
        &self.config_dir
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked.load(Ordering::SeqCst)
    }

    /// Current vault status (with `locksInSecs`).
    pub fn status(&self) -> VaultStatus {
        self.vault_status()
    }

    /// Messages of integrity problems blocking the unlocked vault (empty
    /// when locked or healthy). Never contains secret content.
    pub fn integrity_problems(&self) -> Vec<String> {
        let s = self.slot.read().unwrap_or_else(|p| p.into_inner());
        s.unlocked
            .as_ref()
            .map(dispatch::problem_strings)
            .unwrap_or_default()
    }

    pub(crate) fn emit(
        &self,
        kind: AgentEventKind,
        profile_id: Option<String>,
        entry_id: Option<String>,
        message: Option<String>,
    ) {
        let _ = self.events.send(AgentEvent {
            kind,
            profile_id,
            entry_id,
            message,
        });
    }

    pub(crate) fn settings_snapshot(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub(crate) fn set_machine_settings(&self, s: Settings) {
        *lock(&self.settings) = s;
    }

    /// Record a request for auto-lock (`counts`) and idle-exit purposes.
    pub(crate) fn touch(&self, counts: bool) {
        let mut a = lock(&self.activity);
        let now = Instant::now();
        a.last_any = now;
        if counts {
            a.last = now;
            a.warned = false;
        }
    }

    /// Reset idle/auto-lock state after the slot was filled. The caller has
    /// already published `unlocked` under the slot write lock.
    pub(crate) fn set_unlocked(&self, timeout_override: Option<u64>) {
        {
            let mut a = lock(&self.activity);
            a.last = Instant::now();
            a.warned = false;
            a.timeout_override = timeout_override;
        }
    }

    pub(crate) fn set_timeout_override(&self, t: Option<u64>) {
        if t.is_some() {
            lock(&self.activity).timeout_override = t;
        }
    }

    fn effective_timeout(&self) -> u64 {
        let o = lock(&self.activity).timeout_override;
        o.unwrap_or_else(|| lock(&self.settings).auto_lock_secs as u64)
    }

    /// Seconds until auto-lock (None when locked or auto-lock disabled).
    pub(crate) fn locks_in_secs(&self) -> Option<u64> {
        if !self.is_unlocked() {
            return None;
        }
        let t = self.effective_timeout();
        if t == 0 {
            return None;
        }
        let el = lock(&self.activity).last.elapsed().as_secs();
        Some(t.saturating_sub(el))
    }

    /// Close copied-credential browser sessions, zeroize vault keys and emit
    /// `Locked`. No-op when already locked.
    pub fn lock(&self) {
        self.lock_with_reason(None);
    }

    fn lock_with_reason(&self, reason: Option<&str>) -> bool {
        let taken = {
            let mut s = self.slot.write().unwrap_or_else(|p| p.into_inner());
            // Recheck under the exclusive vault slot: a queued idle worker must
            // not act after a request has renewed activity or disabled auto-lock.
            if reason == Some("idle timeout") && self.locks_in_secs() != Some(0) {
                return false;
            }
            // Browser password stores hold copied credentials. Close them while
            // the vault slot is exclusive, before completing the lock transition.
            self.browsers.close_all();
            // Cancels any unlock whose KDF is still running.
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.unlocked.store(false, Ordering::SeqCst);
            s.unlocked.take()
        };
        let Some(v) = taken else {
            return false;
        };
        drop(v.lock());
        lock(&self.activity).timeout_override = None;
        self.emit(
            AgentEventKind::Locked,
            None,
            None,
            reason.map(str::to_owned),
        );
        true
    }

    /// OS screen lock / suspend hook: locks when `lockOnScreenLock` is set.
    pub fn notify_screen_locked(&self) {
        if lock(&self.settings).lock_on_screen_lock {
            self.lock_with_reason(Some("screen locked"));
        }
    }

    /// Lock, close browsers, clear pending clipboard values, unbind the
    /// socket and stop the timer. Idempotent.
    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return;
        }
        self.lock_with_reason(Some("agent shutting down"));
        self.browsers.close_all();
        let pending = std::mem::take(&mut lock(&self.timers).clipboard);
        for p in pending {
            self.clear_clipboard_if(&p.value);
        }
        #[cfg(unix)]
        if let Some(s) = lock(&self.server).take() {
            s.stop();
        }
        lock(&self.screen).take();
        *lock(&self.stopped) = true;
        self.wake.notify_all();
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Block until [`Agent::shutdown`] has run.
    pub fn wait(&self) {
        let mut g = lock(&self.stopped);
        while !*g {
            g = self.wake.wait(g).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// Bind the agent socket and serve clients on background threads.
    #[cfg(unix)]
    pub fn serve_socket(self: &Arc<Self>) -> crate::core::Result<PathBuf> {
        let mut g = lock(&self.server);
        if let Some(s) = g.as_ref() {
            return Ok(s.path().to_owned());
        }
        let s = server::ServerState::start(Arc::downgrade(self))?;
        let p = s.path().to_owned();
        *g = Some(s);
        Ok(p)
    }

    #[cfg(not(unix))]
    pub fn serve_socket(self: &Arc<Self>) -> crate::core::Result<PathBuf> {
        Err(crate::core::Error::Internal(
            "the agent socket (named pipe) is not supported on this platform yet".into(),
        ))
    }

    pub(crate) fn connection_opened(&self) {
        self.connections.fetch_add(1, Ordering::SeqCst);
        self.touch(false);
    }

    pub(crate) fn connection_closed(&self) {
        self.connections.fetch_sub(1, Ordering::SeqCst);
        self.touch(false);
    }

    /// Put `value` on the clipboard and schedule a clear-if-unchanged.
    pub(crate) fn copy_to_clipboard(
        &self,
        value: Zeroizing<String>,
        clear_after: u32,
    ) -> crate::core::Result<()> {
        self.clipboard
            .set_text(&value)
            .map_err(crate::core::Error::Internal)?;
        if clear_after > 0 {
            lock(&self.timers).clipboard.push(PendingClear {
                at: Instant::now() + Duration::from_secs(clear_after as u64),
                value,
            });
        }
        Ok(())
    }

    fn clear_clipboard_if(&self, value: &str) {
        if let Some(cur) = self.clipboard.get_text() {
            let cur = Zeroizing::new(cur);
            if cur.as_str() == value {
                let _ = self.clipboard.clear();
            }
        }
    }

    /// One timer tick: auto-lock, clipboard clears, browser close, idle exit.
    fn tick(self: &Arc<Self>) {
        let now = Instant::now();
        if self.is_unlocked() {
            let t = self.effective_timeout();
            let (last, warned) = {
                let a = lock(&self.activity);
                (a.last, a.warned)
            };
            let idle = now.duration_since(last);
            if t > 0 && idle >= Duration::from_secs(t) {
                self.lock_in_background();
            } else if t > 0 && !warned {
                let warn_at = t.saturating_sub(IDLE_WARNING_SECS.min(t / 2));
                if idle >= Duration::from_secs(warn_at) {
                    lock(&self.activity).warned = true;
                    let left = Duration::from_secs(t).saturating_sub(idle).as_secs();
                    self.emit(
                        AgentEventKind::LockWarning,
                        None,
                        None,
                        Some(format!("vault auto-locks in {left} s")),
                    );
                }
            }
        }
        let due: Vec<PendingClear> = {
            let mut tm = lock(&self.timers);
            let (due, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut tm.clipboard)
                .into_iter()
                .partition(|p| p.at <= now);
            tm.clipboard = keep;
            due
        };
        for p in due {
            self.clear_clipboard_if(&p.value);
        }
        if let Some(idle) = self.opts.headless_idle_exit
            && !self.is_unlocked()
            && self.connections.load(Ordering::SeqCst) == 0
            && self.browsers.list().is_empty()
            && lock(&self.activity).last_any.elapsed() >= idle
        {
            self.shutdown();
        }
    }
}

impl Agent {
    /// Locking waits for browser termination and archival. Keep that wait off
    /// the timer thread so clipboard expiry remains independent.
    fn lock_in_background(self: &Arc<Self>) {
        if self.locking_in_background.swap(true, Ordering::SeqCst) {
            return;
        }
        let agent = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("latch-idle-lock".into())
            .spawn(move || {
                agent.lock_with_reason(Some("idle timeout"));
                agent.locking_in_background.store(false, Ordering::SeqCst);
            });
        if spawned.is_err() {
            self.locking_in_background.store(false, Ordering::SeqCst);
            // Fail closed even when the OS cannot create another thread.
            self.lock_with_reason(Some("idle timeout"));
        }
    }
}

fn timer_loop(agent: Weak<Agent>, tick: Duration) {
    loop {
        let Some(a) = agent.upgrade() else {
            return;
        };
        if a.is_shutting_down() {
            return;
        }
        a.tick();
        {
            let g = lock(&a.stopped);
            if *g {
                return;
            }
            let _ = a.wake.wait_timeout(g, tick);
        }
        drop(a);
    }
}

/// Default delay before a headless agent exits (locked, no clients).
pub fn default_idle_exit() -> Duration {
    std::env::var(ENV_IDLE_EXIT_SECS)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// Open the configured vault (locked handle), for callers that only need
/// public config (e.g. the pre-commit hook).
pub fn open_active_vault(config_dir: &std::path::Path) -> crate::core::Result<Vault> {
    let v = state::VaultsFile::load(config_dir);
    let p = v
        .active_path()
        .ok_or_else(|| crate::core::Error::NoVault("(none configured)".into()))?;
    Vault::open(p)
}

#[cfg(test)]
#[path = "tests/idle_lock.rs"]
mod idle_lock_tests;
