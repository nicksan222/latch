mod reset;
mod show;

use super::*;

pub(super) fn trust_cmd(s: &mut Session, c: TrustCmd) -> CliResult {
    match c {
        TrustCmd::Show { json } => show::show(s, json),
        TrustCmd::Reset { yes } => reset::reset(s, yes),
    }
}
