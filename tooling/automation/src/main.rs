use clap::{Parser, Subcommand};
use std::ffi::OsString;

#[derive(Parser)]
#[command(about = "Latch development and release automation")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}
#[derive(Subcommand)]
enum Task {
    /// Exercise the managed browser through Rust gRPC and native UI drivers.
    TestBrowser,
    /// Run a Linux test suite in its declared Docker environment.
    #[cfg(feature = "containers")]
    TestContainer {
        #[arg(value_enum)]
        suite: latch_automation::containers::Suite,
    },
    /// Regenerate the README usage GIF in an isolated desktop.
    #[command(disable_help_flag = true)]
    Demo {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Build and checksum the Linux release artifacts in a disposable CI checkout.
    ReleaseBuild { version: String },
    /// Build the CLI bundled with a supported native desktop target.
    ReleaseSidecar,
    /// Prepare frontend and CLI assets for Tauri packaging.
    DesktopBundlePrep,
    Rust {
        #[arg(value_enum)]
        action: latch_automation::rust_checks::Action,
        #[arg(long)]
        desktop: bool,
    },
    /// Install, build, and verify the native Linux desktop inside the devcontainer.
    DesktopCi,
}
fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        #[cfg(feature = "containers")]
        Task::TestContainer { suite } => latch_automation::containers::run(suite),
        Task::TestBrowser => latch_automation::native_browser::run(),
        Task::Demo { args } => latch_automation::demo::run(args),
        Task::ReleaseBuild { version } => latch_automation::release::build(&version),
        Task::ReleaseSidecar => latch_automation::release::sidecar(),
        Task::DesktopBundlePrep => latch_automation::release::prepare_desktop_bundle(),
        Task::Rust { action, desktop } => latch_automation::rust_checks::run(action, desktop),
        Task::DesktopCi => latch_automation::rust_checks::desktop_ci(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "containers")]
    #[test]
    fn desktop_container_suite_is_selectable() {
        assert!(Cli::try_parse_from(["automation", "test-container", "desktop"]).is_ok());
    }
    #[test]
    fn unknown_quality_actions_are_rejected() {
        assert!(Cli::try_parse_from(["automation", "rust", "typo"]).is_err());
    }
    #[test]
    fn demo_options_reach_the_recorder_parser() {
        let cli = Cli::try_parse_from(["automation", "demo", "--help"]).unwrap();
        assert!(matches!(cli.command, Task::Demo { args } if args == [OsString::from("--help")]));
    }
}
