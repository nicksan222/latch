mod add;
mod list;
mod remove;
mod rotate;

use super::*;

pub(super) fn recipients(s: &mut Session, c: RecipientsCmd) -> CliResult {
    match c {
        RecipientsCmd::Ls { json } => list::list(s, json),
        RecipientsCmd::Add { public_key, label } => add::add(s, public_key, label),
        RecipientsCmd::Rm { label } => remove::remove(s, label),
        RecipientsCmd::Rotate => rotate::rotate(s),
    }
}
