use super::*;

pub(super) fn exec_secret(
    s: &mut Session,
    target: &str,
    field: Option<String>,
    command: Vec<String>,
) -> CliResult {
    let (_, e) = resolve_entry(s, target)?;
    let field = match field {
        Some(f) => f,
        None => default_field(e.entry_type)?.to_owned(),
    };
    let r: RevealResult = s.entries_reveal(RevealParams {
        id: e.id,
        field: field.clone(),
    })?;
    let mut value = Zeroizing::new(r.value);
    if e.entry_type == EntryType::File && (field == "content" || field == "contentBase64") {
        use base64::Engine;
        let bytes = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(value.as_bytes())
                .map_err(|_| CliError::user("file content is not valid base64"))?,
        );
        value = Zeroizing::new(String::from_utf8(bytes.to_vec()).map_err(|_| {
            CliError::user("file content is binary; cannot pass it as $LATCH_SECRET")
        })?);
    }
    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..]).env("LATCH_SECRET", value.as_str());
    run_child(s, cmd)
}

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Cmd};
    use clap::Parser;
    #[test]
    fn requires_child_command_and_preserves_child_flags() {
        let cli = Cli::try_parse_from([
            "latch",
            "exec",
            "Acme/token",
            "--field",
            "value",
            "--",
            "node",
            "--version",
        ])
        .unwrap();
        assert!(
            matches!(cli.cmd, Cmd::Exec { target, field, command } if target == "Acme/token" && field.as_deref() == Some("value") && command == ["node", "--version"])
        );
        assert!(Cli::try_parse_from(["latch", "exec", "Acme/token"]).is_err());
    }
}
