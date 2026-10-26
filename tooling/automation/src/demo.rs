use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, ValueHint};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

mod capture;
mod host;

#[derive(Debug, PartialEq, Eq)]
struct Options {
    output: PathBuf,
    width: u32,
    fps: u32,
    image: String,
    selected_image: bool,
    binary: Option<PathBuf>,
    keep_container: bool,
    help: bool,
}

#[derive(Parser)]
#[command(
    name = "demo",
    about = "Build and record a real native Latch session in an isolated Docker/Xvfb container",
    disable_help_flag = true
)]
struct Cli {
    #[arg(long, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(640..=1920))]
    width: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=30))]
    fps: Option<u32>,
    #[arg(long)]
    image: Option<String>,
    #[arg(long, value_hint = ValueHint::FilePath)]
    binary: Option<PathBuf>,
    #[arg(long)]
    keep_container: bool,
    #[arg(long, short)]
    help: bool,
}

impl Options {
    fn parse<I, T>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let cli = Cli::try_parse_from(args)?;
        let root = crate::repository();
        let environment_image = std::env::var("LATCH_DEMO_IMAGE").ok();
        let selected_image = cli.image.is_some() || environment_image.is_some();
        Ok(Self {
            output: cli
                .output
                .unwrap_or_else(|| root.join(".github/assets/demo.gif")),
            width: cli.width.unwrap_or(1280),
            fps: cli.fps.unwrap_or(10),
            image: cli
                .image
                .or(environment_image)
                .unwrap_or_else(|| "latch-bun-dev".into()),
            selected_image,
            binary: cli
                .binary
                .or_else(|| std::env::var_os("LATCH_DEMO_BINARY").map(PathBuf::from)),
            keep_container: cli.keep_container,
            help: cli.help,
        })
    }
}

/// Run the demo recording workflow. The `--capture` mode is an internal entry
/// point used by the freshly built Rust executable inside the disposable container.
pub fn run(args: Vec<OsString>) -> Result<()> {
    if args.len() == 1 && args[0] == "--capture" {
        return capture::run();
    }
    let mut parse_args = vec![OsString::from("demo")];
    parse_args.extend(args);
    let options = Options::parse(parse_args).context("invalid demo options")?;
    if options.help {
        println!("{}", Cli::command().render_long_help());
        println!(
            "\nThe default path builds .devcontainer/Dockerfile when needed, installs with Bun, and builds the seeder, desktop, and Rust driver inside Docker. Named caches are reused between runs.\nEnvironment overrides: LATCH_DEMO_IMAGE, LATCH_DEMO_BINARY, CONTAINER_ENGINE."
        );
        return Ok(());
    }
    host::run(options)
}

fn container_create_args(image: &str, name: &str) -> Vec<String> {
    [
        "create",
        "--init",
        "--name",
        name,
        "--user",
        "vscode",
        "-e",
        "HOME=/home/vscode",
        "-e",
        "DISPLAY=:99",
        "-e",
        "CARGO_HOME=/home/vscode/.cargo",
        "-e",
        "CARGO_TARGET_DIR=/home/vscode/.cache/latch-targets/demo-recording",
        "-e",
        "LATCH_CONFIG_DIR=/workspaces/latch/.dev/latch/config",
        "-e",
        "LATCH_DATA_DIR=/workspaces/latch/.dev/latch/data",
        "-e",
        "LATCH_RUNTIME_DIR=/workspaces/latch/.dev/latch/runtime",
        "-e",
        "LATCH_RAM_DIR=/workspaces/latch/.dev/latch/ram",
        "-v",
        "latch-demo-cargo:/home/vscode/.cargo",
        "-v",
        "latch-demo-target:/home/vscode/.cache/latch-targets",
        "-v",
        "latch-demo-bun:/home/vscode/.bun/install/cache",
        "--workdir",
        "/workspaces/latch",
        image,
        "tail",
        "-f",
        "/dev/null",
    ]
    .into_iter()
    .map(|arg| arg.to_owned())
    .collect()
}

fn build_commands() -> Vec<Vec<&'static str>> {
    vec![
        vec!["bun", "install", "--frozen-lockfile"],
        vec![
            "cargo",
            "build",
            "--locked",
            "-p",
            "latch-core",
            "--no-default-features",
            "--example",
            "seed_dev",
        ],
        vec![
            "bun",
            "run",
            "--filter",
            "@latch/desktop",
            "tauri",
            "--",
            "build",
            "--debug",
            "--no-bundle",
        ],
    ]
}

fn capture_command(desktop: &str, seed: &str, fps: u32, width: u32) -> Vec<String> {
    [
        "env".to_owned(),
        format!("LATCH_DEMO_BINARY={desktop}"),
        format!("LATCH_DEMO_SEED={seed}"),
        format!("LATCH_DEMO_FPS={fps}"),
        format!("LATCH_DEMO_WIDTH={width}"),
        "dbus-run-session".to_owned(),
        "--".to_owned(),
        "xvfb-run".to_owned(),
        "-n".to_owned(),
        "99".to_owned(),
        "-s".to_owned(),
        "-screen 0 1920x1200x24".to_owned(),
        "/home/vscode/.cache/latch-targets/demo-recording/debug/latch-automation".to_owned(),
        "demo".to_owned(),
        "--capture".to_owned(),
    ]
    .into()
}

fn include_snapshot_path(path: &Path) -> bool {
    !path.components().next().is_some_and(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | ".dev" | "node_modules" | "target")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_portable_recording_settings() {
        let options = Options::parse([OsString::from("latch-automation")]).unwrap();
        assert_eq!(
            options.output,
            crate::repository().join(".github/assets/demo.gif")
        );
        assert_eq!(options.width, 1280);
        assert_eq!(options.fps, 10);
        assert_eq!(options.image, "latch-bun-dev");
        assert!(!options.keep_container);
        assert!(options.binary.is_none());
    }

    #[test]
    fn parses_output_dimensions_image_binary_and_keep_container() {
        let options = Options::parse([
            "latch-automation",
            "--output",
            "/tmp/example.gif",
            "--width",
            "1440",
            "--fps",
            "12",
            "--image",
            "latch-test",
            "--binary",
            "/tmp/latch-desktop",
            "--keep-container",
        ])
        .unwrap();
        assert_eq!(options.output, PathBuf::from("/tmp/example.gif"));
        assert_eq!(options.width, 1440);
        assert_eq!(options.fps, 12);
        assert_eq!(options.image, "latch-test");
        assert_eq!(options.binary, Some(PathBuf::from("/tmp/latch-desktop")));
        assert!(options.keep_container);
    }

    #[test]
    fn rejects_out_of_range_dimensions_and_frame_rates() {
        assert!(Options::parse(["demo", "--width", "639"]).is_err());
        assert!(Options::parse(["demo", "--fps", "31"]).is_err());
    }

    #[test]
    fn container_uses_named_caches_and_private_state_directories() {
        let args = container_create_args("latch-bun-dev", "latch-demo-test");
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--name", "latch-demo-test"])
        );
        for value in [
            "CARGO_HOME=/home/vscode/.cargo",
            "latch-demo-cargo:/home/vscode/.cargo",
            "latch-demo-target:/home/vscode/.cache/latch-targets",
            "latch-demo-bun:/home/vscode/.bun/install/cache",
            "LATCH_CONFIG_DIR=/workspaces/latch/.dev/latch/config",
            "LATCH_DATA_DIR=/workspaces/latch/.dev/latch/data",
            "LATCH_RUNTIME_DIR=/workspaces/latch/.dev/latch/runtime",
            "LATCH_RAM_DIR=/workspaces/latch/.dev/latch/ram",
        ] {
            assert!(args.iter().any(|arg| arg == value), "missing {value}");
        }
        assert!(args.iter().all(|arg| !arg.contains("/home/nicksan222")));
        assert!(args.iter().all(|arg| !arg.contains("5900")));
        assert!(!args.iter().any(|arg| arg == "--mount" || arg == "-p"));
    }

    #[test]
    fn source_snapshot_excludes_state_and_dependency_directories() {
        for excluded in [
            ".git/config",
            ".dev/latch/data/key",
            "node_modules/pkg/a.js",
            "target/debug/app",
        ] {
            assert!(
                !include_snapshot_path(Path::new(excluded)),
                "included {excluded}"
            );
        }
        for included in [
            "Cargo.toml",
            "tooling/automation/src/demo.rs",
            "apps/desktop/src/main.ts",
        ] {
            assert!(
                include_snapshot_path(Path::new(included)),
                "excluded {included}"
            );
        }
    }

    #[test]
    fn build_commands_install_and_build_seed_and_embedded_desktop() {
        assert_eq!(
            build_commands(),
            vec![
                vec!["bun", "install", "--frozen-lockfile"],
                vec![
                    "cargo",
                    "build",
                    "--locked",
                    "-p",
                    "latch-core",
                    "--no-default-features",
                    "--example",
                    "seed_dev"
                ],
                vec![
                    "bun",
                    "run",
                    "--filter",
                    "@latch/desktop",
                    "tauri",
                    "--",
                    "build",
                    "--debug",
                    "--no-bundle"
                ],
            ]
        );
    }

    #[test]
    fn capture_command_runs_the_rust_driver_in_dbus_and_xvfb() {
        let command = capture_command("/opt/latch-desktop", "/opt/seed_dev", 10, 1280);
        assert!(
            command
                .windows(3)
                .any(|args| args == ["dbus-run-session", "--", "xvfb-run"])
        );
        assert!(
            command
                .windows(2)
                .any(|args| args == ["-s", "-screen 0 1920x1200x24"])
        );
        assert!(command.ends_with(&[
            "/home/vscode/.cache/latch-targets/demo-recording/debug/latch-automation".into(),
            "demo".into(),
            "--capture".into(),
        ]));
    }
}
