//! Tray menu (SPEC §9.4). The menu is first built as a plain [`TrayItem`]
//! tree from agent data ([`build_model`], testable without a display), then
//! turned into a Tauri menu ([`to_menu`]).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

use crate::protocol::{PinnedTotp, ProfileSummary, VaultStatus};

pub const TRAY_ID: &str = "latch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    Lock,
    /// Unlock happens in the window: show and focus it.
    Unlock,
    OpenBrowser(String),
    CopyTotp(String),
    Open,
    Quit,
}

impl TrayAction {
    pub fn id(&self) -> String {
        match self {
            Self::Lock => "lock".into(),
            Self::Unlock => "unlock".into(),
            Self::OpenBrowser(p) => format!("browser:{p}"),
            Self::CopyTotp(e) => format!("totp:{e}"),
            Self::Open => "open".into(),
            Self::Quit => "quit".into(),
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "lock" => Self::Lock,
            "unlock" => Self::Unlock,
            "open" => Self::Open,
            "quit" => Self::Quit,
            _ => {
                if let Some(p) = id.strip_prefix("browser:").filter(|p| !p.is_empty()) {
                    Self::OpenBrowser(p.to_owned())
                } else {
                    Self::CopyTotp(
                        id.strip_prefix("totp:")
                            .filter(|e| !e.is_empty())?
                            .to_owned(),
                    )
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayItem {
    Action {
        action: TrayAction,
        label: String,
    },
    /// Non-clickable caption / placeholder.
    Label(String),
    Separator,
    Submenu {
        label: String,
        items: Vec<TrayItem>,
    },
}

/// Agent data the tray shows. `profiles`/`pinned` are only read when
/// unlocked.
#[derive(Debug, Clone, Default)]
pub struct TrayInput {
    pub initialized: bool,
    pub locked: bool,
    pub profiles: Vec<ProfileSummary>,
    pub pinned: Vec<PinnedTotp>,
}

impl TrayInput {
    pub fn from_status(st: &VaultStatus) -> Self {
        Self {
            initialized: st.initialized,
            locked: st.locked,
            ..Self::default()
        }
    }
}

/// Menu labels treat `&` as a mnemonic marker; show user text literally.
pub fn escape_label(s: &str) -> String {
    s.replace('&', "&&")
}

fn action(action: TrayAction, label: impl Into<String>) -> TrayItem {
    TrayItem::Action {
        action,
        label: label.into(),
    }
}

pub fn build_model(input: &TrayInput) -> Vec<TrayItem> {
    let mut items = vec![];
    if input.initialized {
        if input.locked {
            items.push(action(TrayAction::Unlock, "Unlock…"));
        } else {
            items.push(action(TrayAction::Lock, "Lock"));
            items.push(TrayItem::Separator);
            let mut profiles: Vec<TrayItem> = input
                .profiles
                .iter()
                .map(|p| {
                    let name = escape_label(&p.name);
                    let label = if p.browser_running {
                        format!("{name} — browser running")
                    } else {
                        format!("{name} — Open browser")
                    };
                    action(TrayAction::OpenBrowser(p.id.clone()), label)
                })
                .collect();
            if profiles.is_empty() {
                profiles.push(TrayItem::Label("No profiles".into()));
            }
            items.push(TrayItem::Submenu {
                label: "Profiles".into(),
                items: profiles,
            });
            if !input.pinned.is_empty() {
                items.push(TrayItem::Separator);
                items.push(TrayItem::Label("Copy TOTP code".into()));
                for t in &input.pinned {
                    items.push(action(
                        TrayAction::CopyTotp(t.id.clone()),
                        format!(
                            "{} / {}",
                            escape_label(&t.profile_name),
                            escape_label(&t.name)
                        ),
                    ));
                }
            }
        }
        items.push(TrayItem::Separator);
    }
    items.push(action(TrayAction::Open, "Open Latch"));
    items.push(TrayItem::Separator);
    items.push(action(TrayAction::Quit, "Quit Latch"));
    items
}

/// Turn the model into a Tauri menu.
pub fn to_menu<R: Runtime>(app: &AppHandle<R>, items: &[TrayItem]) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for it in items {
        match it {
            TrayItem::Action { action, label } => menu.append(&MenuItem::with_id(
                app,
                action.id(),
                label,
                true,
                None::<&str>,
            )?)?,
            TrayItem::Label(l) => menu.append(&MenuItem::new(app, l, false, None::<&str>)?)?,
            TrayItem::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
            TrayItem::Submenu { label, items } => {
                let sub = Submenu::new(app, label, true)?;
                for s in items {
                    match s {
                        TrayItem::Action { action, label } => sub.append(&MenuItem::with_id(
                            app,
                            action.id(),
                            label,
                            true,
                            None::<&str>,
                        )?)?,
                        TrayItem::Label(l) => {
                            sub.append(&MenuItem::new(app, l, false, None::<&str>)?)?
                        }
                        TrayItem::Separator => sub.append(&PredefinedMenuItem::separator(app)?)?,
                        // One level is all the tray needs.
                        TrayItem::Submenu { .. } => {}
                    }
                }
                menu.append(&sub)?
            }
        }
    }
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, name: &str, running: bool) -> ProfileSummary {
        ProfileSummary {
            id: id.into(),
            name: name.into(),
            color: "#336699".into(),
            icon: "folder".into(),
            description: String::new(),
            pinned: false,
            entry_count: 0,
            browser_running: running,
        }
    }

    fn actions(items: &[TrayItem]) -> Vec<TrayAction> {
        let mut out = vec![];
        for i in items {
            match i {
                TrayItem::Action { action, .. } => out.push(action.clone()),
                TrayItem::Submenu { items, .. } => out.extend(actions(items)),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn action_ids_round_trip() {
        for a in [
            TrayAction::Lock,
            TrayAction::Unlock,
            TrayAction::OpenBrowser("prof_1".into()),
            TrayAction::CopyTotp("ent:2".into()),
            TrayAction::Open,
            TrayAction::Quit,
        ] {
            assert_eq!(TrayAction::parse(&a.id()), Some(a));
        }
        assert_eq!(TrayAction::parse("browser:"), None);
        assert_eq!(TrayAction::parse("nope"), None);
    }

    #[test]
    fn no_vault() {
        let m = build_model(&TrayInput::default());
        assert_eq!(actions(&m), vec![TrayAction::Open, TrayAction::Quit]);
    }

    #[test]
    fn locked_shows_unlock_only() {
        let m = build_model(&TrayInput {
            initialized: true,
            locked: true,
            // Ignored while locked.
            profiles: vec![profile("p1", "acme", false)],
            pinned: vec![],
        });
        assert_eq!(
            actions(&m),
            vec![TrayAction::Unlock, TrayAction::Open, TrayAction::Quit]
        );
    }

    #[test]
    fn unlocked_lists_profiles_and_pinned_totp() {
        let m = build_model(&TrayInput {
            initialized: true,
            locked: false,
            profiles: vec![
                profile("p1", "acme & co", false),
                profile("p2", "side", true),
            ],
            pinned: vec![PinnedTotp {
                id: "e1".into(),
                profile_name: "acme".into(),
                name: "GitHub".into(),
            }],
        });
        assert_eq!(
            actions(&m),
            vec![
                TrayAction::Lock,
                TrayAction::OpenBrowser("p1".into()),
                TrayAction::OpenBrowser("p2".into()),
                TrayAction::CopyTotp("e1".into()),
                TrayAction::Open,
                TrayAction::Quit,
            ]
        );
        let TrayItem::Submenu { label, items } = &m[2] else {
            panic!("profiles submenu expected, got {:?}", m[2]);
        };
        assert_eq!(label, "Profiles");
        assert_eq!(
            items[0],
            TrayItem::Action {
                action: TrayAction::OpenBrowser("p1".into()),
                label: "acme && co — Open browser".into()
            }
        );
        assert!(matches!(&items[1], TrayItem::Action { label, .. } if label.contains("running")));
        assert!(m.contains(&TrayItem::Label("Copy TOTP code".into())));
    }

    #[test]
    fn unlocked_without_profiles_has_placeholder() {
        let m = build_model(&TrayInput {
            initialized: true,
            locked: false,
            ..TrayInput::default()
        });
        assert!(m.contains(&TrayItem::Submenu {
            label: "Profiles".into(),
            items: vec![TrayItem::Label("No profiles".into())]
        }));
        assert!(!m.contains(&TrayItem::Label("Copy TOTP code".into())));
    }
}
