use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixtureConfig {
    Browser(BrowserConfig),
    Terminal(TerminalConfig),
    Gpg(GpgConfig),
    GitCallback(GitCallbackConfig),
    ExitStatus { status: i32 },
    UserHook(UserHookConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub args_path: Option<PathBuf>,
    pub populate_profile: bool,
    pub cookie_prefix: Option<String>,
    pub cookie_snapshot: Option<PathBuf>,
    pub ready_path: Option<PathBuf>,
    /// Hold startup at the ready marker until a test creates this path.
    pub race_marker: Option<PathBuf>,
    pub term_marker: Option<PathBuf>,
    pub ignore_term: bool,
    pub exit_after_start: Option<i32>,
    pub fail_if_exists: Option<PathBuf>,
    pub fail_status: i32,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            args_path: None,
            populate_profile: true,
            cookie_prefix: Some("cookie".into()),
            cookie_snapshot: None,
            ready_path: None,
            race_marker: None,
            term_marker: None,
            ignore_term: false,
            exit_after_start: None,
            fail_if_exists: None,
            fail_status: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerminalConfig {
    pub args_path: Option<PathBuf>,
    pub delay_ms: u64,
    pub execute_command: bool,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpgConfig {
    pub output: String,
    pub repeat_output: bool,
    pub exit_status: i32,
    pub gnupghome: Option<PathBuf>,
    pub delegate: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCallbackConfig {
    pub marker_path: PathBuf,
    pub exit_status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserHookConfig {
    pub marker_path: PathBuf,
    pub exit_status: i32,
}

/// Install a configured test executable at `path`. The JSON config is stored
/// alongside it as `<path>.json` and the executable reads that config on start.
pub fn install(config: FixtureConfig, path: &Path) -> Result<PathBuf> {
    let source = fixture_binary()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::copy(&source, path)
        .with_context(|| format!("copy fixture executable to {}", path.display()))?;
    set_executable(path)?;
    let config_path = config_path(path);
    fs::write(&config_path, serde_json::to_vec(&config)?)
        .with_context(|| format!("write fixture config {}", config_path.display()))?;
    Ok(path.to_path_buf())
}

pub fn run_installed() -> Result<i32> {
    let executable = std::env::current_exe().context("locate fixture executable")?;
    let config: FixtureConfig =
        serde_json::from_slice(&fs::read(config_path(&executable)).context("read fixture config")?)
            .context("parse fixture config")?;
    match config {
        FixtureConfig::Browser(config) => run_browser(config),
        FixtureConfig::Terminal(config) => run_terminal(config),
        FixtureConfig::Gpg(config) => run_gpg(config),
        FixtureConfig::GitCallback(config) => {
            fs::write(config.marker_path, b"hostile").context("write callback marker")?;
            Ok(config.exit_status)
        }
        FixtureConfig::ExitStatus { status } => Ok(status),
        FixtureConfig::UserHook(config) => {
            fs::write(config.marker_path, b"user hook ran")?;
            Ok(config.exit_status)
        }
    }
}

pub fn main_entry() {
    let status = run_installed().unwrap_or(125);
    std::process::exit(status.clamp(0, 255));
}

fn run_browser(config: BrowserConfig) -> Result<i32> {
    install_term_handlers(config.ignore_term);
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(path) = config.args_path {
        fs::write(
            path,
            args.iter()
                .map(|arg| format!("{}\n", arg.to_string_lossy()))
                .collect::<String>(),
        )?;
    }
    if config
        .fail_if_exists
        .as_ref()
        .is_some_and(|path| path.exists())
    {
        return Ok(config.fail_status);
    }
    if args.first().is_some_and(|arg| arg == "--version") {
        println!("Fixture Browser 1.0");
        return Ok(0);
    }
    if let Some(profile) = profile_arg(&args) {
        if config.populate_profile {
            fs::create_dir_all(profile.join("cache2"))?;
            fs::create_dir_all(profile.join("storage/default"))?;
            fs::write(profile.join("cache2/data_0"), b"cachedata")?;
            fs::write(profile.join("parent.lock"), b"")?;
            #[cfg(unix)]
            {
                let lock = profile.join("SingletonLock");
                let _ = fs::remove_file(&lock);
                std::os::unix::fs::symlink(format!("host-{}", std::process::id()), lock)?;
            }
        }
        if let Some(prefix) = config.cookie_prefix {
            let cookies = profile.join("cookies.sqlite");
            let count = fs::read_to_string(&cookies)
                .unwrap_or_default()
                .lines()
                .count();
            use std::io::Write;
            writeln!(
                File::options().create(true).append(true).open(&cookies)?,
                "{prefix}-{count}"
            )?;
            if let Some(snapshot) = config.cookie_snapshot {
                fs::copy(cookies, snapshot)?;
            }
        }
    }
    if let Some(marker) = config.ready_path {
        fs::write(marker, b"ready")?;
    }
    if let Some(marker) = config.race_marker {
        while !marker.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    if let Some(status) = config.exit_after_start {
        return Ok(status);
    }
    wait_for_term(config.term_marker.as_deref());
    Ok(0)
}

fn profile_arg(args: &[std::ffi::OsString]) -> Option<PathBuf> {
    let position = args
        .iter()
        .position(|arg| arg == "-profile" || arg == "--user-data-dir")?;
    args.get(position + 1).map(PathBuf::from)
}

#[cfg(unix)]
static TERMINATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn term_handler(_: libc::c_int) {
    TERMINATE.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(unix)]
extern "C" fn ignore_handler(_: libc::c_int) {}

fn install_term_handlers(ignore: bool) {
    #[cfg(unix)]
    unsafe {
        TERMINATE.store(false, std::sync::atomic::Ordering::Relaxed);
        let handler = if ignore {
            ignore_handler as *const () as libc::sighandler_t
        } else {
            term_handler as *const () as libc::sighandler_t
        };
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }
    #[cfg(not(unix))]
    let _ = ignore;
}

fn wait_for_term(marker: Option<&Path>) {
    loop {
        #[cfg(unix)]
        if TERMINATE.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(marker) = marker {
                let _ = fs::write(marker, b"term\n");
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn run_terminal(config: TerminalConfig) -> Result<i32> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(path) = config.args_path {
        fs::write(
            path,
            args.iter()
                .map(|arg| format!("{}\n", arg.to_string_lossy()))
                .collect::<String>(),
        )?;
    }
    let mut status = if config.execute_command {
        let command_index = args
            .iter()
            .position(|arg| arg == "-e")
            .map(|index| index + 1);
        if let Some(index) = command_index.filter(|index| *index < args.len()) {
            Command::new(&args[index])
                .args(&args[index + 1..])
                .status()?
                .code()
                .unwrap_or(1)
        } else {
            2
        }
    } else {
        config.exit_status.unwrap_or(0)
    };
    if config.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(config.delay_ms));
    }
    if let Some(exit_status) = config.exit_status {
        status = exit_status;
    }
    Ok(status)
}

fn run_gpg(config: GpgConfig) -> Result<i32> {
    if let Some(delegate) = config.delegate {
        let mut command = Command::new(delegate);
        command.args(std::env::args_os().skip(1));
        if let Some(home) = config.gnupghome {
            command.env("GNUPGHOME", home);
        }
        return Ok(command.status()?.code().unwrap_or(1));
    }
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if config.repeat_output {
        loop {
            stdout.write_all(config.output.as_bytes())?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    stdout.write_all(config.output.as_bytes())?;
    stdout.flush()?;
    Ok(config.exit_status)
}

fn config_path(executable: &Path) -> PathBuf {
    let mut path = executable.as_os_str().to_os_string();
    path.push(".json");
    PathBuf::from(path)
}

fn fixture_binary() -> Result<PathBuf> {
    static BINARY: std::sync::OnceLock<std::result::Result<PathBuf, String>> =
        std::sync::OnceLock::new();
    match BINARY.get_or_init(|| fixture_binary_once().map_err(|error| format!("{error:#}"))) {
        Ok(path) => Ok(path.clone()),
        Err(error) => bail!("{error}"),
    }
}

fn fixture_binary_once() -> Result<PathBuf> {
    let cache =
        PathBuf::from(std::env::var_os("HOME").context("locate home directory for fixture build")?)
            .join(".cache/latch-targets/sharedlatch-fixtures");
    fs::create_dir_all(&cache)?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(cache.join("build.lock"))?;
    lock.lock_exclusive()?;
    let binary_name = if cfg!(windows) {
        "latch-test-fixture.exe"
    } else {
        "latch-test-fixture"
    };
    let binary = cache.join("debug").join(binary_name);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args(["build", "--locked", "--manifest-path"])
        .arg(manifest)
        .args([
            "-p",
            "latch-automation",
            "--bin",
            "latch-test-fixture",
            "--target-dir",
        ])
        .arg(&cache)
        .output()
        .context("build latch-test-fixture")?;
    if !output.status.success() {
        bail!(
            "building latch-test-fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    if !binary.is_file() {
        bail!(
            "cargo did not create fixture binary at {}",
            binary.display()
        );
    }
    Ok(binary)
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_exit_fixture_returns_configured_status() {
        let dir = tempfile::tempdir().unwrap();
        let executable = install(
            FixtureConfig::ExitStatus { status: 37 },
            &dir.path().join("exit-fixture"),
        )
        .unwrap();
        let status = Command::new(executable).status().unwrap();
        assert_eq!(status.code(), Some(37));
    }

    #[test]
    fn browser_fixture_records_arguments_and_profile_cookies() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("browser-fixture");
        let args_path = dir.path().join("args");
        let profile = dir.path().join("profile");
        let snapshot = dir.path().join("cookies-seen");
        let config = BrowserConfig {
            args_path: Some(args_path.clone()),
            cookie_snapshot: Some(snapshot.clone()),
            exit_after_start: Some(0),
            ..Default::default()
        };
        install(FixtureConfig::Browser(config), &executable).unwrap();
        let status = Command::new(executable)
            .args([
                "-profile",
                profile.to_str().unwrap(),
                "https://example.test",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            fs::read_to_string(args_path).unwrap(),
            format!("-profile\n{}\nhttps://example.test\n", profile.display())
        );
        assert_eq!(fs::read_to_string(snapshot).unwrap(), "cookie-0\n");
    }

    #[test]
    fn browser_fixture_waits_for_the_configured_race_marker() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("browser-fixture");
        let ready = dir.path().join("ready");
        let release = dir.path().join("release");
        let config = BrowserConfig {
            ready_path: Some(ready.clone()),
            race_marker: Some(release.clone()),
            exit_after_start: Some(0),
            ..BrowserConfig::default()
        };
        install(FixtureConfig::Browser(config), &executable).unwrap();
        let mut child = Command::new(executable).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !ready.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture did not start"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(child.try_wait().unwrap().is_none());
        fs::write(release, b"release").unwrap();
        assert!(child.wait().unwrap().success());
    }
}
