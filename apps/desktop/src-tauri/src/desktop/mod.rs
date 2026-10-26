//! Tauri desktop shell.
//!
//! The app embeds the [`Agent`] in-process and is the agent for the user
//! session: it binds the agent socket (see [`socket`]) so the
//! CLI talks to it.
//!
//! Webview surface: exactly one app command, `rpc(method, params)`, which
//! dispatches as `ClientCtx::desktop()` on the blocking pool and resolves
//! with the result or rejects with `{ code, message }`. Agent events are
//! forwarded as `latch://event`; the global shortcut emits
//! `latch://shortcut` `{ action: "palette" }`. Plugin permissions are in
//! `capabilities/default.json` (dialog open/save, clipboard write only).
//!
//! Shell behaviour:
//! - single instance: a second launch focuses the window (`--palette` also
//!   opens the palette; use it for compositor key bindings on Wayland).
//! - tray (SPEC §9.4) when `settings.trayEnabled`; closing the window then
//!   hides it. Without tray, closing quits.
//! - `--minimized` (used by autostart) starts hidden when the tray is on.
//! - global shortcut / autostart follow settings and are re-applied after
//!   `settings.update`.
//! - lock on screen lock/suspend: Linux uses the agent's D-Bus watcher.
//!   macOS (`NSWorkspace` notifications) and Windows (`WTS_SESSION_LOCK`) are
//!   not exposed by Tauri 2; they need a small native hook calling
//!   `Agent::notify_screen_locked` (not implemented yet; idle auto-lock
//!   still applies).
//! - quit: `Agent::shutdown` (lock, close browsers, clear clipboard, unbind).

pub mod clipboard;
pub mod events;
pub mod rpc;
pub mod shortcut;
pub mod socket;
pub mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

use crate::agent::{Agent, AgentOptions};
use crate::protocol::{PinnedTotp, ProfileSummary, Settings, methods as m};

use self::events::{AGENT_EVENT, SHORTCUT_EVENT, ShortcutPayload};
use self::rpc::handle_rpc;
use self::shortcut::{PALETTE_ARG, parse_shortcut};
use self::tray::{TRAY_ID, TrayAction, TrayInput};

pub const MAIN_WINDOW: &str = "main";
/// Passed by autostart: start hidden in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

/// Managed state.
pub struct Shell {
    pub agent: Arc<Agent>,
    rpc_client: Arc<Mutex<latch::rpc::LocalClient>>,
    tray_enabled: AtomicBool,
    quitting: AtomicBool,
    shortcut: Mutex<Option<String>>,
}

pub fn run() {
    crate::core::crypto::harden_process();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("LATCH_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let context = tauri::generate_context!();
    webkit_workarounds();

    let start_minimized = std::env::args().any(|a| a == MINIMIZED_ARG);

    let app = tauri::Builder::default()
        // Must be first: a second launch exits before anything else runs.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            show_main(app);
            if args.iter().any(|a| a == PALETTE_ARG) {
                let _ = app.emit(SHORTCUT_EVENT, ShortcutPayload::palette());
            }
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg(MINIMIZED_ARG)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        show_main(app);
                        let _ = app.emit(SHORTCUT_EVENT, ShortcutPayload::palette());
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![rpc::rpc])
        .setup(move |app| {
            let handle = app.handle().clone();
            let agent = Agent::new(AgentOptions {
                clipboard: Some(Arc::new(clipboard::TauriClipboard::new(handle.clone()))),
                watch_screen_lock: true,
                ..Default::default()
            });
            let rpc_client = Arc::new(Mutex::new(latch::rpc::LocalClient::new(
                &agent,
                latch::rpc::ClientCtx::desktop(),
            )));
            app.manage(Shell {
                rpc_client,
                agent: agent.clone(),
                tray_enabled: AtomicBool::new(false),
                quitting: AtomicBool::new(false),
                shortcut: Mutex::new(None),
            });
            forward_events(&handle, &agent);
            quit_on_signals(&handle);

            let settings = read_settings(&agent);
            apply_settings(&handle, &settings);

            let show = !(start_minimized && settings.tray_enabled);
            if show || std::env::args().any(|a| a == PALETTE_ARG) {
                show_main(&handle);
            }

            // Binding may wait for a headless agent to exit: off the main thread.
            let h = handle.clone();
            std::thread::spawn(move || match socket::bind_or_take_over(&agent) {
                socket::SocketOutcome::Bound(p) => {
                    tracing::info!("agent socket {}", p.display())
                }
                socket::SocketOutcome::TookOver(p, pid) => {
                    tracing::info!("took over agent socket {} from pid {pid}", p.display())
                }
                socket::SocketOutcome::Unavailable(msg) => {
                    tracing::warn!("{msg}");
                    notify(&h, &msg);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let shell = window.state::<Shell>();
                if shell.tray_enabled.load(Ordering::SeqCst)
                    && !shell.quitting.load(Ordering::SeqCst)
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(context)
        .expect("error while building the Latch desktop app");

    app.run(|app, event| {
        if let RunEvent::Exit = event
            && let Some(shell) = app.try_state::<Shell>()
        {
            shutdown_agent(shell.agent.clone());
        }
    });
}

/// WebKitGTK's DMA-BUF renderer kills the app with "Error 71 (Protocol
/// error) dispatching to Wayland display" on NVIDIA drivers. Disable it
/// there unless the user decided otherwise. Main thread, before any other
/// thread exists.
fn webkit_workarounds() {
    #[cfg(target_os = "linux")]
    {
        const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
        if std::env::var_os(VAR).is_none()
            && std::path::Path::new("/proc/driver/nvidia/version").exists()
        {
            // SAFETY: called from `run` before the Tauri runtime, the agent
            // or any other thread is started.
            unsafe { std::env::set_var(VAR, "1") };
        }
    }
}

/// `Agent::shutdown` may read the clipboard (clear-if-unchanged), which must
/// not happen on the main thread on Linux: run it aside, bounded.
fn shutdown_agent(agent: Arc<Agent>) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        agent.shutdown();
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_secs(5));
}

/// SIGTERM/SIGINT/SIGHUP (e.g. `latch agent stop`, logout) → orderly quit
/// through `RunEvent::Exit` (lock, close browsers, unbind the socket).
fn quit_on_signals(app: &AppHandle) {
    #[cfg(unix)]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let (Ok(mut term), Ok(mut int), Ok(mut hup)) = (
                signal(SignalKind::terminate()),
                signal(SignalKind::interrupt()),
                signal(SignalKind::hangup()),
            ) else {
                return;
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
                _ = hup.recv() => {}
            }
            app.state::<Shell>().quitting.store(true, Ordering::SeqCst);
            app.exit(0);
        });
    }
    #[cfg(not(unix))]
    let _ = app;
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn window_hidden(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN_WINDOW)
        .map(|w| !w.is_visible().unwrap_or(false) || w.is_minimized().unwrap_or(false))
        .unwrap_or(true)
}

fn notify(app: &AppHandle, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title("Latch")
        .body(body)
        .show();
}

fn read_settings(agent: &Arc<Agent>) -> Settings {
    handle_rpc(agent, m::SETTINGS_GET, Value::Null)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_else(crate::agent::state::default_settings)
}

fn forward_events(app: &AppHandle, agent: &Arc<Agent>) {
    let mut client = latch::rpc::LocalClient::new(agent, latch::rpc::ClientCtx::desktop());
    let events = match client.watch_events() {
        Ok(events) => events,
        Err(_) => {
            notify(app, "Could not subscribe to agent events");
            return;
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let _client = client;
        loop {
            let ev = match events.recv() {
                Ok(Ok(ev)) => ev,
                _ => return,
            };
            let _ = app.emit(AGENT_EVENT, &ev);
            let fx = events::effects(&ev);
            if let Some(msg) = fx.notify_when_hidden
                && window_hidden(&app)
            {
                notify(&app, &msg);
            }
            if fx.reload_settings {
                let a = app.state::<Shell>().agent.clone();
                let h = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let s = read_settings(&a);
                    apply_settings(&h, &s);
                });
            } else if fx.rebuild_tray {
                refresh_tray(&app);
            }
        }
    });
}

/// Apply machine settings to the shell: tray, global shortcut, autostart.
pub fn apply_settings(app: &AppHandle, s: &Settings) {
    let shell = app.state::<Shell>();
    let was = shell.tray_enabled.swap(s.tray_enabled, Ordering::SeqCst);
    if s.tray_enabled {
        if (!was || app.tray_by_id(TRAY_ID).is_none())
            && let Err(e) = create_tray(app)
        {
            tracing::warn!("tray unavailable: {e}");
            shell.tray_enabled.store(false, Ordering::SeqCst);
        }
        refresh_tray(app);
    } else if was {
        let _ = app.remove_tray_by_id(TRAY_ID);
    }

    {
        let mut cur = shell.shortcut.lock().unwrap_or_else(|p| p.into_inner());
        if cur.as_deref() != Some(s.global_shortcut.as_str()) {
            let gs = app.global_shortcut();
            let _ = gs.unregister_all();
            match parse_shortcut(&s.global_shortcut) {
                Ok(Some(sc)) => {
                    if let Err(e) = gs.register(sc) {
                        tracing::warn!(
                            "global shortcut {:?} not registered: {e} (on Wayland bind `latch-desktop {PALETTE_ARG}` in the compositor)",
                            s.global_shortcut
                        );
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("{e}"),
            }
            *cur = Some(s.global_shortcut.clone());
        }
    }

    let al = app.autolaunch();
    match al.is_enabled() {
        Ok(on) if on != s.autostart => {
            let r = if s.autostart {
                al.enable()
            } else {
                al.disable()
            };
            if let Err(e) = r {
                tracing::warn!("autostart: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("autostart state: {e}"),
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray::to_menu(app, &tray::build_model(&TrayInput::default()))?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Latch")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| on_tray_action(app, ev.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

/// Rebuild the tray menu from agent data (off the main thread: dispatch
/// may contend with a commit).
fn refresh_tray(app: &AppHandle) {
    let shell = app.state::<Shell>();
    if !shell.tray_enabled.load(Ordering::SeqCst) {
        return;
    }
    let agent = shell.agent.clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let input = tray_input(&agent);
        let items = tray::build_model(&input);
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(t) = h.tray_by_id(TRAY_ID) {
                match tray::to_menu(&h, &items) {
                    Ok(menu) => {
                        let _ = t.set_menu(Some(menu));
                    }
                    Err(e) => tracing::warn!("tray menu: {e}"),
                }
            }
        });
    });
}

fn tray_input(agent: &Arc<Agent>) -> TrayInput {
    let status = handle_rpc(agent, m::VAULT_STATUS, Value::Null)
        .ok()
        .and_then(|value| serde_json::from_value::<crate::protocol::VaultStatus>(value).ok());
    let mut input = status
        .as_ref()
        .map(TrayInput::from_status)
        .unwrap_or_else(|| TrayInput {
            locked: true,
            ..Default::default()
        });
    if input.initialized && !input.locked {
        input.profiles = handle_rpc(agent, m::PROFILES_LIST, Value::Null)
            .ok()
            .and_then(|v| serde_json::from_value::<Vec<ProfileSummary>>(v).ok())
            .unwrap_or_default();
        input.pinned = handle_rpc(agent, m::TOTP_PINNED, Value::Null)
            .ok()
            .and_then(|v| serde_json::from_value::<Vec<PinnedTotp>>(v).ok())
            .unwrap_or_default();
    }
    input
}

fn on_tray_action(app: &AppHandle, id: &str) {
    let Some(action) = TrayAction::parse(id) else {
        return;
    };
    let agent = app.state::<Shell>().agent.clone();
    match action {
        TrayAction::Lock => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = handle_rpc(&agent, m::VAULT_LOCK, Value::Null) {
                    notify(&app, &error.message);
                }
            });
        }
        TrayAction::Unlock | TrayAction::Open => show_main(app),
        TrayAction::Quit => {
            app.state::<Shell>().quitting.store(true, Ordering::SeqCst);
            app.exit(0);
        }
        TrayAction::OpenBrowser(profile_id) => {
            let h = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = handle_rpc(
                    &agent,
                    m::BROWSER_LAUNCH,
                    json!({ "profileId": profile_id }),
                ) {
                    notify(&h, &format!("Could not open the browser: {}", e.message));
                }
            });
        }
        TrayAction::CopyTotp(entry_id) => {
            let h = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                match handle_rpc(
                    &agent,
                    m::CLIPBOARD_COPY,
                    json!({ "id": entry_id, "field": "totp" }),
                ) {
                    Ok(_) => notify(&h, "TOTP code copied"),
                    Err(e) => notify(&h, &format!("Could not copy the code: {}", e.message)),
                }
            });
        }
    }
}
