//! Agent events → webview events and shell side effects (pure parts).

use serde::Serialize;

use crate::protocol::{AgentEvent, AgentEventKind};

/// Every `AgentEvent` is forwarded unchanged under this name.
pub const AGENT_EVENT: &str = "latch://event";
/// Desktop-only: the global shortcut fired (payload [`ShortcutPayload`]).
pub const SHORTCUT_EVENT: &str = "latch://shortcut";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShortcutPayload {
    pub action: &'static str,
}

impl ShortcutPayload {
    pub const fn palette() -> Self {
        Self { action: "palette" }
    }
}

/// What the shell does besides forwarding an event to the webview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects {
    /// Rebuild the tray menu (lock state, profiles, pinned TOTP changed).
    pub rebuild_tray: bool,
    /// Show an OS notification with this text when the window is hidden.
    pub notify_when_hidden: Option<String>,
    /// Re-read settings (vault-scoped settings may have changed).
    pub reload_settings: bool,
}

pub fn effects(ev: &AgentEvent) -> Effects {
    use AgentEventKind as K;
    match ev.kind {
        K::Locked | K::Unlocked | K::ProfilesChanged | K::EntriesChanged | K::BrowserChanged => {
            Effects {
                rebuild_tray: true,
                ..Effects::default()
            }
        }
        K::VaultChanged => Effects {
            rebuild_tray: true,
            reload_settings: true,
            ..Effects::default()
        },
        K::LockWarning => Effects {
            notify_when_hidden: Some(
                ev.message
                    .clone()
                    .unwrap_or_else(|| "Latch will lock soon".to_owned()),
            ),
            ..Effects::default()
        },
        K::SyncChanged => Effects::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(kind: AgentEventKind, message: Option<&str>) -> AgentEvent {
        AgentEvent {
            kind,
            profile_id: None,
            entry_id: None,
            message: message.map(str::to_owned),
        }
    }

    #[test]
    fn forwarded_payload_is_the_wire_agent_event() {
        let e = AgentEvent {
            kind: AgentEventKind::EntriesChanged,
            profile_id: Some("p1".into()),
            entry_id: Some("e1".into()),
            message: None,
        };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            json!({ "kind": "entries_changed", "profileId": "p1", "entryId": "e1" })
        );
        assert_eq!(
            serde_json::to_value(ShortcutPayload::palette()).unwrap(),
            json!({ "action": "palette" })
        );
    }

    #[test]
    fn effects_mapping() {
        for k in [
            AgentEventKind::Locked,
            AgentEventKind::Unlocked,
            AgentEventKind::ProfilesChanged,
            AgentEventKind::EntriesChanged,
            AgentEventKind::BrowserChanged,
        ] {
            let e = effects(&ev(k, None));
            assert!(e.rebuild_tray, "{k:?}");
            assert!(e.notify_when_hidden.is_none());
            assert!(!e.reload_settings);
        }
        let v = effects(&ev(AgentEventKind::VaultChanged, None));
        assert!(v.rebuild_tray && v.reload_settings);
        assert_eq!(
            effects(&ev(AgentEventKind::SyncChanged, None)),
            Effects::default()
        );
        let w = effects(&ev(
            AgentEventKind::LockWarning,
            Some("browsers close in 30 s"),
        ));
        assert_eq!(
            w.notify_when_hidden.as_deref(),
            Some("browsers close in 30 s")
        );
        assert!(!w.rebuild_tray);
        let w = effects(&ev(AgentEventKind::LockWarning, None));
        assert!(w.notify_when_hidden.is_some());
    }
}
