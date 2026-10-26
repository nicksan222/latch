//! The `latch` CLI (SPEC §8). `apps/cli/src/main.rs` calls [`main`].
//!
//! Exit codes: 0 ok, 1 user error, 2 locked, 3 integrity, 4 conflict.
//! Output is never colored; secrets reach stdout only on explicit request
//! (`get --reveal` on a TTY or with `--yes`, `get --field f --reveal`,
//! `env --print`, `totp`).

/// `print!` to stdout that exits quietly (like standard Unix tools, see
/// [`stdout_write`]) when the reader went away (`EPIPE`), e.g.
/// `latch ls | head -1`, instead of panicking.
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::cli::stdout_write(format_args!($($arg)*), false)
    };
}

/// `println!` counterpart of [`out!`].
macro_rules! outln {
    () => {
        $crate::cli::stdout_write(format_args!(""), true)
    };
    ($($arg:tt)*) => {
        $crate::cli::stdout_write(format_args!($($arg)*), true)
    };
}

mod commands;
mod entries;
pub mod session;

/// Write to stdout; on a closed pipe exit immediately with status 0 (the
/// consumer has all it wanted); other errors exit with status 1.
#[doc(hidden)]
pub fn stdout_write(args: std::fmt::Arguments<'_>, newline: bool) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let r = out
        .write_fmt(args)
        .and_then(|_| {
            if newline {
                out.write_all(b"\n")
            } else {
                Ok(())
            }
        })
        .and_then(|_| out.flush());
    if let Err(e) = r {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("latch: cannot write to stdout: {e}");
        std::process::exit(1);
    }
}

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use latch_rpc::types::{ErrorCode, RpcError};

/// CLI error with its exit code. Messages never contain secrets.
#[derive(Debug)]
pub struct CliError {
    pub code: i32,
    pub message: String,
}

impl CliError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn user(message: impl Into<String>) -> Self {
        Self::new(1, message)
    }
}

pub fn exit_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::Locked => 2,
        ErrorCode::Integrity => 3,
        ErrorCode::Conflict => 4,
        _ => 1,
    }
}

impl From<RpcError> for CliError {
    fn from(e: RpcError) -> Self {
        let hint = match e.code {
            ErrorCode::Locked => " (run `latch unlock`)",
            ErrorCode::Integrity => " (run `latch doctor`)",
            ErrorCode::Conflict => " (see `latch sync`, `latch resolve`)",
            _ => "",
        };
        CliError::new(exit_code(e.code), format!("{}{hint}", e.message))
    }
}

impl From<latch::core::Error> for CliError {
    fn from(e: latch::core::Error) -> Self {
        e.to_rpc().into()
    }
}

pub type CliResult<T = ()> = Result<T, CliError>;

#[derive(Parser, Debug)]
#[command(
    name = "latch",
    version,
    about = "Local, git-backed secret manager",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum ActionCmd {
    /// List a profile's saved actions
    Ls { profile: String },
    /// Launch an action by ID or name in an interactive terminal
    Run { profile: String, action: String },
    /// Set this machine's project directory (empty string clears it)
    Directory { profile: String, path: String },
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Create a new vault (or clone one with --clone)
    Init(InitArgs),
    /// Unlock the vault in the agent
    Unlock {
        /// Auto-lock timeout for this session, e.g. 15m, 1h, 90s
        #[arg(long)]
        timeout: Option<String>,
        /// Unlock with an age identity / SSH key file instead of the passphrase
        #[arg(long)]
        identity: Option<PathBuf>,
        /// Read the passphrase from stdin (first line)
        #[arg(long)]
        passphrase_stdin: bool,
    },
    /// Lock the vault
    Lock,
    /// Show vault status
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Manage profiles
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// List entries
    Ls {
        profile: Option<String>,
        #[arg(long = "type")]
        entry_type: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show an entry (secrets masked unless --reveal)
    Get {
        /// `<profile>/<entry>`
        target: String,
        /// Print a single field (e.g. password, username, vars.KEY)
        #[arg(long)]
        field: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        reveal: bool,
        /// Allow --reveal when stdout is not a terminal
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Create or update an entry from key=value pairs
    Set(SetArgs),
    /// Delete an entry (recoverable from history)
    Rm { target: String },
    /// Move/rename an entry: `<profile>/<entry> <profile>/<newname>`
    Mv { from: String, to: String },
    /// Search entries in all profiles
    Search {
        query: String,
        #[arg(long)]
        json: bool,
    },
    /// Run a command with a profile's env vars, or print them
    Env(EnvArgs),
    /// Run a command with one secret in $LATCH_SECRET
    Exec {
        target: String,
        #[arg(long)]
        field: Option<String>,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Print the current TOTP code
    Totp {
        target: String,
        #[arg(long)]
        copy: bool,
    },
    /// Copy a field to the clipboard (cleared automatically)
    Copy {
        target: String,
        #[arg(long)]
        field: Option<String>,
        /// Clear after this long (e.g. 30s); default from settings
        #[arg(long)]
        clear: Option<String>,
    },
    /// List, run or configure saved project launch actions
    #[command(subcommand)]
    Action(ActionCmd),
    /// Launch or manage isolated browsers
    Browser(BrowserArgs),
    /// Sync with the git remote
    Sync {
        #[arg(long, conflicts_with = "push_only")]
        pull_only: bool,
        #[arg(long)]
        push_only: bool,
        /// Set (or with "" remove) the remote URL instead of syncing
        #[arg(long)]
        set_remote: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Resolve a sync conflict
    Resolve {
        target: String,
        resolution: ResolutionArg,
    },
    /// Show an entry's history
    History {
        target: String,
        #[arg(long)]
        json: bool,
    },
    /// Restore an entry to a commit (also restores deleted entries)
    Restore { target: String, commit: String },
    /// List deleted entries
    Trash {
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Manage vault recipients
    #[command(subcommand)]
    Recipients(RecipientsCmd),
    /// Import from another manager or file
    Import {
        format: String,
        file: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        skip_duplicates: bool,
    },
    /// Export entries
    Export {
        /// json (plaintext), age (encrypted archive) or dotenv
        #[arg(long, default_value = "age")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        profile: Option<String>,
        /// Confirm writing plaintext
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Check vault integrity and scan for plaintext
    Doctor {
        #[arg(long)]
        fix: bool,
        #[arg(long)]
        rebuild_manifest: bool,
        /// Only scan staged files (pre-commit hook)
        #[arg(long)]
        staged: bool,
        #[arg(long)]
        json: bool,
    },
    /// Install or remove the pre-commit hook
    Hook {
        action: HookAction,
        /// Vault path (default: active vault)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Migrate the vault format
    Migrate,
    /// Agent control
    #[command(subcommand)]
    Agent(AgentCmd),
    /// Show or reset this machine's trust in the vault's keys
    #[command(subcommand)]
    Trust(TrustCmd),
    /// Print shell completions
    Completion { shell: clap_complete::Shell },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Vault directory (default ~/latch-vault)
    pub path: Option<PathBuf>,
    /// Clone an existing vault from this git URL
    #[arg(long)]
    pub clone: Option<String>,
    /// Remote URL to configure
    #[arg(long)]
    pub remote: Option<String>,
    /// Generate a recovery key and print the recovery kit
    #[arg(long)]
    pub recovery_kit: bool,
    #[arg(long)]
    pub git_name: Option<String>,
    #[arg(long)]
    pub git_email: Option<String>,
    /// Read the passphrase from stdin (first line)
    #[arg(long)]
    pub passphrase_stdin: bool,
}

#[derive(Args, Debug)]
pub struct SetArgs {
    /// `<profile>/<entry>`
    pub target: String,
    /// key=value pairs (env entries: KEY=VALUE)
    pub pairs: Vec<String>,
    /// login, secret, env, file, note, ssh_key, database, api_key
    #[arg(long = "type")]
    pub entry_type: Option<String>,
    /// Read the main value (or .env content / file content) from a file
    #[arg(long)]
    pub from_file: Option<PathBuf>,
    /// Read the main value from stdin
    #[arg(long)]
    pub stdin: bool,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long)]
    pub notes: Option<String>,
}

#[derive(Args, Debug)]
pub struct EnvArgs {
    pub profile: String,
    /// Env entries to merge (default: all env entries of the profile)
    pub entries: Vec<String>,
    /// Pass only injected vars plus PATH, HOME, TERM
    #[arg(long)]
    pub isolate: bool,
    /// Print instead of running a command
    #[arg(long)]
    pub print: bool,
    #[arg(long, value_enum, default_value = "dotenv")]
    pub format: EnvFormatArg,
    #[arg(last = true)]
    pub command: Vec<String>,
}

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct BrowserArgs {
    #[command(subcommand)]
    pub cmd: Option<BrowserCmd>,
    /// Profile to launch
    pub profile: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum BrowserCmd {
    /// List running browsers
    Ls {
        #[arg(long)]
        json: bool,
    },
    /// Close a profile's browser (session is archived)
    Close { profile: String },
    /// Discard a profile's saved browser session
    Reset { profile: String },
}

#[derive(Subcommand, Debug)]
pub enum ProfileCmd {
    Ls {
        #[arg(long)]
        json: bool,
    },
    Add {
        name: String,
        #[arg(long, default_value = "#2563eb")]
        color: String,
        #[arg(long, default_value = "folder")]
        icon: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    Rm {
        name: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    Rename {
        old: String,
        new: String,
    },
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum RecipientsCmd {
    Ls {
        #[arg(long)]
        json: bool,
    },
    Add {
        public_key: String,
        #[arg(long)]
        label: String,
    },
    Rm {
        label: String,
    },
    Rotate,
}

#[derive(Subcommand, Debug)]
pub enum TrustCmd {
    /// Show the vault fingerprint and trust state
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Forget the pinned key history; the next unlock trusts the
    /// repository's keys again (verify the new fingerprint out of band)
    Reset {
        /// Lock the vault first if it is unlocked
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentCmd {
    /// Run the agent in the foreground
    Run {
        /// Exit after a period without clients while locked
        #[arg(long)]
        headless: bool,
    },
    Stop,
    Status,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum EnvFormatArg {
    Dotenv,
    Export,
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ResolutionArg {
    Mine,
    Theirs,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum HookAction {
    Install,
    Uninstall,
}

/// Parse `15m`, `1h`, `90s`, `2d` or plain seconds.
pub fn parse_duration(s: &str) -> CliResult<u64> {
    let s = s.trim();
    let (num, mult) = match s.char_indices().last() {
        Some((i, 's')) => (&s[..i], 1),
        Some((i, 'm')) => (&s[..i], 60),
        Some((i, 'h')) => (&s[..i], 3600),
        Some((i, 'd')) => (&s[..i], 86400),
        _ => (s, 1),
    };
    let invalid = || CliError::user(format!("invalid duration `{s}` (e.g. 30s, 15m, 1h)"));
    let n = num.trim().parse::<u64>().map_err(|_| invalid())?;
    n.checked_mul(mult)
        .ok_or_else(|| CliError::user(format!("duration `{s}` is too large")))
}

/// [`parse_duration`] for auto-lock timeouts: zero (which would disable
/// auto-lock) and more than 7 days are rejected.
pub fn parse_lock_timeout(s: &str) -> CliResult<u64> {
    let v = parse_duration(s)?;
    if v == 0 {
        return Err(CliError::user(
            "the auto-lock timeout must be at least 1 second",
        ));
    }
    if v > 7 * 24 * 3600 {
        return Err(CliError::user(
            "the auto-lock timeout must be at most 7 days",
        ));
    }
    Ok(v)
}

/// Entry point used by the binary. Returns the process exit code.
pub fn main() -> i32 {
    let cli = Cli::parse();
    match commands::run(cli) {
        Ok(()) => 0,
        Err(e) => {
            if !e.message.is_empty() {
                eprintln!("latch: {}", e.message);
            }
            e.code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90").unwrap(), 90);
        assert_eq!(parse_duration("30s").unwrap(), 30);
        assert_eq!(parse_duration("15m").unwrap(), 900);
        assert_eq!(parse_duration("1h").unwrap(), 3600);
        assert_eq!(parse_duration("2d").unwrap(), 172800);
        assert!(parse_duration("x").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("18446744073709551615d").is_err());
        assert!(parse_lock_timeout("0").is_err());
        assert!(parse_lock_timeout("8d").is_err());
        assert_eq!(parse_lock_timeout("15m").unwrap(), 900);
    }

    #[test]
    fn exit_codes() {
        assert_eq!(exit_code(ErrorCode::Locked), 2);
        assert_eq!(exit_code(ErrorCode::Integrity), 3);
        assert_eq!(exit_code(ErrorCode::Conflict), 4);
        assert_eq!(exit_code(ErrorCode::NotFound), 1);
    }

    #[test]
    fn cli_parses() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
        let c =
            Cli::try_parse_from(["latch", "env", "acme", "a", "b", "--", "sh", "-c", "x"]).unwrap();
        match c.cmd {
            Cmd::Env(e) => {
                assert_eq!(e.entries, vec!["a", "b"]);
                assert_eq!(e.command, vec!["sh", "-c", "x"]);
            }
            _ => panic!(),
        }
        let c = Cli::try_parse_from(["latch", "browser", "acme"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Browser(BrowserArgs {
                cmd: None,
                profile: Some(_),
                ..
            })
        ));
        let c = Cli::try_parse_from(["latch", "browser", "ls"]).unwrap();
        assert!(matches!(
            c.cmd,
            Cmd::Browser(BrowserArgs {
                cmd: Some(BrowserCmd::Ls { .. }),
                ..
            })
        ));
    }
    #[test]
    fn saved_action_commands_parse_names_with_spaces() {
        let cli =
            Cli::try_parse_from(["latch", "action", "run", "Acme", "Start development"]).unwrap();
        assert!(
            matches!(cli.cmd,Cmd::Action(ActionCmd::Run{profile,action}) if profile=="Acme" && action=="Start development")
        );
        assert!(
            Cli::try_parse_from(["latch", "action", "directory", "Acme", "/work/my project"])
                .is_ok()
        );
    }
}
