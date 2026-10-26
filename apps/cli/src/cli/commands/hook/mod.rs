mod install;
mod uninstall;
use super::*;
pub(super) fn hook(s: &mut Session, action: HookAction, path: Option<PathBuf>) -> CliResult {
    match action {
        HookAction::Install => install::install(s, path),
        HookAction::Uninstall => uninstall::uninstall(s, path),
    }
}
