//! Global shortcut (`settings.globalShortcut`) parsing.
//!
//! Accelerator syntax is the one of `global-hotkey` (used by
//! `tauri-plugin-global-shortcut`): `CmdOrCtrl+Shift+L`, `Alt+Space`, …
//! An empty string disables the shortcut.
//!
//! Wayland: `global-hotkey` grabs keys through X11 only, so registration
//! fails (or never fires) for native Wayland sessions. Bind
//! `latch-desktop --palette` in the compositor instead (Hyprland:
//! `bind = SUPER SHIFT, L, exec, latch-desktop --palette`); the
//! single-instance handler focuses the running app and opens the palette.

use std::str::FromStr;

use tauri_plugin_global_shortcut::Shortcut;

pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+L";

/// Command-line flag that opens the palette (via single instance).
pub const PALETTE_ARG: &str = "--palette";

/// `Ok(None)`: disabled. `Err`: human-readable reason (no secrets involved).
pub fn parse_shortcut(s: &str) -> Result<Option<Shortcut>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    Shortcut::from_str(s)
        .map(Some)
        .map_err(|e| format!("invalid global shortcut {s:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    #[test]
    fn parses_default() {
        let s = parse_shortcut(DEFAULT_SHORTCUT).unwrap().unwrap();
        let expected_mod = if cfg!(target_os = "macos") {
            Modifiers::SUPER
        } else {
            Modifiers::CONTROL
        };
        assert_eq!(
            s,
            Shortcut::new(Some(expected_mod | Modifiers::SHIFT), Code::KeyL)
        );
    }

    #[test]
    fn parses_variants_and_rejects_garbage() {
        assert!(parse_shortcut("Alt+Space").unwrap().is_some());
        assert!(parse_shortcut("ctrl+shift+k").unwrap().is_some());
        assert_eq!(parse_shortcut("").unwrap(), None);
        assert_eq!(parse_shortcut("   ").unwrap(), None);
        assert!(parse_shortcut("Ctrl+").is_err());
        assert!(parse_shortcut("NotAKey+Q").is_err());
    }
}
