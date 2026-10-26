mod run;
mod status;
mod stop;

use super::*;

pub(super) fn agent_cmd(c: AgentCmd) -> CliResult {
    match c {
        AgentCmd::Run { headless } => run::run(headless),
        AgentCmd::Stop => stop::stop(),
        AgentCmd::Status => status::status(),
    }
}
