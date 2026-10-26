use super::*;

fn env_ids(s: &mut Session, profile_id: &str, names: &[String]) -> CliResult<Option<Vec<String>>> {
    if names.is_empty() {
        return Ok(None);
    }
    let list = list_entries(s, profile_id)?;
    names
        .iter()
        .map(|n| {
            find_in(&list, n)
                .map(|e| e.id)
                .ok_or_else(|| CliError::user(format!("no entry `{n}`")))
        })
        .collect::<CliResult<Vec<_>>>()
        .map(Some)
}

pub(super) fn env(s: &mut Session, a: EnvArgs) -> CliResult {
    let prof = resolve_profile(s, &a.profile)?;
    let ids = env_ids(s, &prof.id, &a.entries)?;
    if a.print {
        let format = match a.format {
            EnvFormatArg::Dotenv => EnvFormat::Dotenv,
            EnvFormatArg::Export => EnvFormat::Export,
            EnvFormatArg::Json => EnvFormat::Json,
        };
        let t: TextResult = s.env_format(EnvFormatParams {
            profile_id: prof.id,
            entry_ids: ids.map(|items| OptionalStringList { items }),
            format: format as i32,
        })?;
        let text = Zeroizing::new(t.text);
        out!("{}", text.as_str());
        if !text.ends_with('\n') && !text.is_empty() {
            outln!();
        }
        return Ok(());
    }
    if a.command.is_empty() {
        return Err(CliError::user("give a command after `--`, or use --print"));
    }
    let r: EnvResolveResult = s.env_resolve(EnvResolveParams {
        profile_id: prof.id,
        entry_ids: ids.map(|items| OptionalStringList { items }),
    })?;
    let mut cmd = std::process::Command::new(&a.command[0]);
    cmd.args(&a.command[1..]);
    if a.isolate {
        cmd.env_clear();
        for k in ["PATH", "HOME", "TERM"] {
            if let Some(v) = std::env::var_os(k) {
                cmd.env(k, v);
            }
        }
    }
    for p in &r.vars {
        cmd.env(&p.key, &p.value);
    }
    drop(r);
    run_child(s, cmd)
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn separates_entry_selection_from_child_flags() {
        let cli = Cli::try_parse_from([
            "latch",
            "env",
            "Acme",
            "development",
            "--isolate",
            "--",
            "node",
            "--version",
        ])
        .unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Env(a) if a.profile == "Acme" && a.entries == ["development"] && a.isolate && a.command == ["node", "--version"])
        );
    }

    #[test]
    fn print_format_is_validated_by_clap() {
        let cli =
            Cli::try_parse_from(["latch", "env", "Acme", "--print", "--format", "json"]).unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Env(a) if a.print && matches!(a.format, crate::cli::EnvFormatArg::Json) && a.command.is_empty())
        );
        assert!(
            Cli::try_parse_from(["latch", "env", "Acme", "--print", "--format", "xml"]).is_err()
        );
    }
}
