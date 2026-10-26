//! Explicit Rust quality scopes shared by developer commands and CI.
use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use std::process::Command;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Action {
    Format,
    FormatCheck,
    Check,
    Clippy,
    Docs,
    Test,
    Quality,
    All,
}
#[derive(Clone, Copy)]
enum Step {
    Format,
    FormatCheck,
    Check,
    Clippy,
    Docs,
    Test,
}

fn arguments(step: Step, desktop: bool) -> Vec<String> {
    let format = matches!(step, Step::Format | Step::FormatCheck);
    let mut args = vec![
        match step {
            Step::Format | Step::FormatCheck => "fmt",
            Step::Check => "check",
            Step::Clippy => "clippy",
            Step::Docs => "doc",
            Step::Test => "test",
        }
        .to_owned(),
    ];
    if !format {
        args.push("--locked".into());
    }
    if desktop {
        args.extend(["--package".into(), "latch-desktop".into()]);
    } else if format {
        for name in [
            "latch-automation",
            "latch-rpc",
            "latch-core",
            "latch-cli",
            "latch-web-bridge",
        ] {
            args.extend(["--package".into(), name.into()]);
        }
    } else {
        args.extend(
            [
                "--workspace",
                "--exclude",
                "latch-desktop",
                "--no-default-features",
            ]
            .map(str::to_owned),
        );
    }
    match step {
        Step::Format => {}
        Step::FormatCheck => args.extend(["--", "--check"].map(str::to_owned)),
        Step::Check => args.push("--all-targets".into()),
        Step::Clippy => args.extend(["--all-targets", "--", "-D", "warnings"].map(str::to_owned)),
        Step::Docs => args.extend(["--no-deps", "--lib"].map(str::to_owned)),
        Step::Test => {}
    }
    args
}
fn execute(command: &mut Command) -> Result<()> {
    ensure!(
        command.status().context("start quality command")?.success(),
        "quality command failed"
    );
    Ok(())
}
pub fn run(action: Action, desktop: bool) -> Result<()> {
    let quality = [Step::FormatCheck, Step::Check, Step::Clippy, Step::Docs];
    let steps = match action {
        Action::Format => vec![Step::Format],
        Action::FormatCheck => vec![Step::FormatCheck],
        Action::Check => vec![Step::Check],
        Action::Clippy => vec![Step::Clippy],
        Action::Docs => vec![Step::Docs],
        Action::Test => vec![Step::Test],
        Action::Quality => quality.to_vec(),
        Action::All => [quality.to_vec(), vec![Step::Test]].concat(),
    };
    for step in steps {
        let args = arguments(step, desktop);
        println!("cargo {}", args.join(" "));
        let mut command = if desktop
            && matches!(step, Step::Test)
            && cfg!(target_os = "linux")
            && std::env::var_os("DISPLAY").is_none()
        {
            let mut command = Command::new("xvfb-run");
            command.args(["-a", "dbus-run-session", "--", "cargo"]);
            command
        } else {
            Command::new("cargo")
        };
        command.current_dir(crate::repository()).args(args);
        if matches!(step, Step::Docs) {
            command.env(
                "RUSTDOCFLAGS",
                format!(
                    "{} -D warnings",
                    std::env::var("RUSTDOCFLAGS").unwrap_or_default()
                ),
            );
        }
        execute(&mut command)?;
    }
    Ok(())
}
pub fn desktop_ci() -> Result<()> {
    let root = crate::repository();
    execute(
        Command::new("bun")
            .current_dir(&root)
            .args(["install", "--frozen-lockfile"]),
    )?;
    execute(
        Command::new("bun")
            .current_dir(&root)
            .args(["run", "build:desktop"]),
    )?;
    execute(Command::new("cargo").current_dir(&root).args([
        "build",
        "--locked",
        "-p",
        "latch-desktop",
    ]))?;
    run(Action::All, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headless_checks_exclude_the_native_shell_and_default_features() {
        for step in [Step::Check, Step::Clippy, Step::Docs, Step::Test] {
            let args = arguments(step, false);
            assert!(
                args.windows(2)
                    .any(|pair| pair == ["--exclude", "latch-desktop"])
            );
            assert!(args.iter().any(|arg| arg == "--no-default-features"));
            assert!(args.iter().any(|arg| arg == "--locked"));
        }
    }
    #[test]
    fn checking_format_never_selects_a_mutating_format_command() {
        let args = arguments(Step::FormatCheck, false);
        assert_eq!(&args[args.len() - 2..], ["--", "--check"]);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--package", "latch-automation"])
        );
        assert!(!args.iter().any(|arg| arg == "--locked"));
    }
}
