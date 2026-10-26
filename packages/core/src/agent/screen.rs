//! Best-effort OS screen-lock watcher.
//!
//! Linux: spawns `gdbus monitor` (system bus: logind `Session.Lock` and
//! `Manager.PrepareForSleep(true)`; session bus: freedesktop/GNOME
//! `ScreenSaver.ActiveChanged(true)`), falling back to `dbus-monitor` on the
//! session bus. Missing tools or buses are ignored. Other platforms: the
//! desktop shell calls `Agent::notify_screen_locked` from its OS hooks.

use std::sync::Weak;

use super::Agent;

pub(crate) struct Watcher {
    #[allow(dead_code)]
    children: Vec<std::process::Child>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for c in &mut self.children {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Does a monitor output line announce a lock?
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn is_lock_line(line: &str) -> bool {
    let l = line.trim();
    (l.contains("org.freedesktop.login1.Session.Lock") && !l.contains("LockedHint"))
        || l.contains("PrepareForSleep (true")
        || (l.contains("ScreenSaver.ActiveChanged") && l.contains("(true"))
}

#[cfg(target_os = "linux")]
pub(crate) fn start(agent: Weak<Agent>) -> Option<Watcher> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut children = vec![];
    let mut spawn = |cmd: &str, args: &[&str], dbus_monitor: bool| {
        let Ok(mut child) = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let Some(out) = child.stdout.take() else {
            return false;
        };
        let weak = agent.clone();
        let _ = std::thread::Builder::new()
            .name("latch-screen-watch".into())
            .spawn(move || {
                // dbus-monitor prints the signal header, then the argument
                // on the next line ("   boolean true").
                let mut pending_active = false;
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let hit = if dbus_monitor {
                        let hit = pending_active && line.trim() == "boolean true";
                        pending_active = line.contains("member=ActiveChanged");
                        hit
                    } else {
                        is_lock_line(&line)
                    };
                    if hit {
                        match weak.upgrade() {
                            Some(a) => a.notify_screen_locked(),
                            None => return,
                        }
                    }
                }
            });
        children.push(child);
        true
    };
    let have_gdbus = which::which("gdbus").is_ok();
    if have_gdbus {
        spawn(
            "gdbus",
            &["monitor", "--system", "--dest", "org.freedesktop.login1"],
            false,
        );
        spawn(
            "gdbus",
            &[
                "monitor",
                "--session",
                "--dest",
                "org.freedesktop.ScreenSaver",
            ],
            false,
        );
        spawn(
            "gdbus",
            &["monitor", "--session", "--dest", "org.gnome.ScreenSaver"],
            false,
        );
    } else if which::which("dbus-monitor").is_ok() {
        spawn(
            "dbus-monitor",
            &[
                "--session",
                "type='signal',interface='org.freedesktop.ScreenSaver',member='ActiveChanged'",
            ],
            true,
        );
    }
    if children.is_empty() {
        tracing::debug!("no screen-lock watcher available");
        return None;
    }
    Some(Watcher { children })
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_agent: Weak<Agent>) -> Option<Watcher> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_lines() {
        assert!(is_lock_line(
            "/org/freedesktop/login1/session/_32: org.freedesktop.login1.Session.Lock ()"
        ));
        assert!(is_lock_line(
            "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (true,)"
        ));
        assert!(!is_lock_line(
            "/org/freedesktop/login1: org.freedesktop.login1.Manager.PrepareForSleep (false,)"
        ));
        assert!(is_lock_line(
            "/org/freedesktop/ScreenSaver: org.freedesktop.ScreenSaver.ActiveChanged (true,)"
        ));
        assert!(!is_lock_line(
            "/org/freedesktop/ScreenSaver: org.freedesktop.ScreenSaver.ActiveChanged (false,)"
        ));
        assert!(!is_lock_line("org.freedesktop.login1.Session.Unlock ()"));
    }
}
