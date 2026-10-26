use super::*;

pub(super) fn completion(shell: clap_complete::Shell) -> CliResult {
    completion_to(shell, &mut std::io::stdout());
    Ok(())
}

fn completion_to(shell: clap_complete::Shell, output: &mut dyn std::io::Write) {
    use clap::CommandFactory;
    clap_complete::generate(shell, &mut Cli::command(), "latch", output);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completions_include_nested_actions_and_preserve_the_binary_name() {
        let mut output = Vec::new();
        completion_to(clap_complete::Shell::Bash, &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("latch"));
        assert!(output.contains("recipients"));
        assert!(output.contains("passphrase-stdin"));
    }
}
