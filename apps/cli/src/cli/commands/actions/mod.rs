//! Saved launch actions share the desktop's agent dispatcher.
mod configure;
mod list;
mod run;

use super::*;

pub(super) fn action(s: &mut Session, cmd: ActionCmd) -> CliResult {
    match cmd {
        ActionCmd::Ls { profile } => list::list(s, profile),
        ActionCmd::Run { profile, action } => run::run(s, profile, action),
        ActionCmd::Directory { profile, path } => configure::configure(s, profile, path),
    }
}
