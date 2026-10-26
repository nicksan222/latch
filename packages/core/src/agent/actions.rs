//! Machine-local action bindings and interactive terminal execution.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::Agent;
use crate::core::{Error, Result, format, paths};
use crate::protocol::{ActionConfigureParams, ActionIdParams, ActionListResult, EnvPair};

const BINDINGS_FILE: &str = "action-directories.json";

#[derive(Default, Serialize, Deserialize)]
struct Bindings {
    // IDs only: project names and action definitions belong in ciphertext.
    directories: BTreeMap<String, BTreeMap<String, String>>,
}

fn load_bindings(config: &Path) -> Result<Bindings> {
    match std::fs::read(config.join(BINDINGS_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
            Error::invalid("malformed local action directories; repair action-directories.json")
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Bindings::default()),
        Err(e) => Err(Error::io("read local action directories", e)),
    }
}

fn project_directory(directory: &Path, vault: &Path) -> Result<PathBuf> {
    if !directory.is_absolute() {
        return Err(Error::invalid(
            "project directory must be an existing absolute path",
        ));
    }
    let dir = directory
        .canonicalize()
        .map_err(|_| Error::invalid("project directory must be an existing absolute path"))?;
    let vault = vault
        .canonicalize()
        .map_err(|e| Error::io("locate vault", e))?;
    if !dir.is_dir() || dir.starts_with(vault) {
        return Err(Error::invalid(
            "project directory must be a folder outside the vault",
        ));
    }
    Ok(dir)
}

impl Agent {
    pub(crate) fn actions_list(&self, profile_id: &str) -> Result<ActionListResult> {
        self.read(|v| {
            let actions = v.list_actions(profile_id)?;
            let binding = load_bindings(&self.config_dir)?;
            Ok(ActionListResult {
                actions,
                working_directory: binding
                    .directories
                    .get(v.id())
                    .and_then(|p| p.get(profile_id))
                    .cloned(),
            })
        })
    }

    pub(crate) fn actions_configure(&self, p: &ActionConfigureParams) -> Result<()> {
        // Serialize local configuration changes with all vault/action writes.
        self.write(|v| {
            v.get_profile(&p.profile_id)?;
            let directory = p
                .working_directory
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| project_directory(Path::new(s), v.path()))
                .transpose()?;
            paths::ensure_private_dir(&self.config_dir)
                .map_err(|e| Error::io("create action config directory", e))?;
            let config = self
                .config_dir
                .canonicalize()
                .map_err(|e| Error::io("locate action config directory", e))?;
            if config.starts_with(
                v.path()
                    .canonicalize()
                    .map_err(|e| Error::io("locate vault", e))?,
            ) {
                return Err(Error::invalid(
                    "action configuration must live outside the vault",
                ));
            }
            let mut binding = load_bindings(&config)?;
            let profiles = binding.directories.entry(v.id().into()).or_default();
            match directory {
                Some(d) => {
                    profiles.insert(p.profile_id.clone(), d.to_string_lossy().into_owned());
                }
                None => {
                    profiles.remove(&p.profile_id);
                }
            }
            let bytes = serde_json::to_vec_pretty(&binding)
                .map_err(|_| Error::Internal("serialize action directories".into()))?;
            format::write_atomic(&config.join(BINDINGS_FILE), &bytes)
        })
    }

    pub(crate) fn actions_run(&self, p: &ActionIdParams) -> Result<()> {
        self.read(|v| {
            let action=v.get_action(p)?;
            // Revalidate even synced definitions and all references before side effects.
            let env=v.action_environment(&p.profile_id,&p.action_id)?;
            let mut terminal=if action.command.trim().is_empty() {None} else {
                let binding=load_bindings(&self.config_dir)?;
                let directory=binding.directories.get(v.id()).and_then(|d|d.get(&p.profile_id)).ok_or_else(||Error::invalid("set this profile's local project directory before running a command"))?;
                let directory=project_directory(Path::new(directory),v.path())?;
                let binary=detect_terminal()?;
                Some(terminal_command(&binary,&action.command,&directory,&env.vars)?)
            };
            // Firefox explicitly disables remote control for isolation. Do not silently swallow URLs.
            if !action.urls.is_empty() && self.browsers().list().iter().any(|b| b.profile_id==p.profile_id && b.engine==crate::protocol::BrowserEngine::Firefox) {
                return Err(Error::Conflict("close this profile's Firefox browser before running an action with URLs".into()));
            }
            if let Some(cmd)=&mut terminal { spawn_terminal(cmd)?; }
            drop(terminal);
            drop(env);
            if !action.urls.is_empty() {
                self.browsers().launch_urls(v,&p.profile_id,&action.urls).map_err(|e| {
                    if action.command.trim().is_empty() {e} else {Error::Conflict(format!("terminal launched, but browser could not open; the command may still be running: {e}"))}
                })?;
            }
            Ok(())
        })
    }
}

fn executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn detect_terminal() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LATCH_TERMINAL") {
        let path = PathBuf::from(path);
        if path.is_absolute() && executable(&path) {
            return Ok(path);
        }
        return Err(Error::invalid(
            "LATCH_TERMINAL must be an absolute path to a supported terminal executable",
        ));
    }
    #[cfg(windows)]
    {
        return Ok(PathBuf::from("powershell.exe"));
    }
    #[cfg(not(windows))]
    {
        let names = if cfg!(target_os = "macos") {
            vec!["kitty", "alacritty"]
        } else {
            vec!["ghostty", "kitty", "foot", "alacritty", "konsole", "xterm"]
        };
        for name in names {
            if let Some(path) = std::env::var_os("PATH").and_then(|p| {
                std::env::split_paths(&p)
                    .map(|d| d.join(name))
                    .find(|p| p.is_absolute() && executable(p))
            }) {
                return Ok(path);
            }
        }
        #[cfg(target_os = "macos")]
        for path in [
            "/Applications/kitty.app/Contents/MacOS/kitty",
            "/Applications/Alacritty.app/Contents/MacOS/alacritty",
        ] {
            if executable(Path::new(path)) {
                return Ok(path.into());
            }
        }
        Err(Error::NotFound("supported terminal; install kitty, foot, alacritty, konsole, xterm or ghostty, or set LATCH_TERMINAL".into()))
    }
}

fn terminal_command(
    binary: &Path,
    source: &str,
    directory: &Path,
    env: &[EnvPair],
) -> Result<Command> {
    let mut cmd = Command::new(binary);
    #[cfg(not(windows))]
    {
        let name = binary.file_name().and_then(|s| s.to_str()).unwrap_or("");
        match name {
            "ghostty" => {
                cmd.args([
                    "--gtk-single-instance=false",
                    "--wait-after-command=true",
                    "-e",
                ]);
            }
            "kitty" => {
                cmd.arg("--hold");
            }
            "foot" => {
                cmd.args(["--hold", "--"]);
            }
            "alacritty" => {
                cmd.args(["--hold", "-e"]);
            }
            "konsole" => {
                cmd.args(["--separate", "--hold", "-e"]);
            }
            "xterm" => {
                cmd.args(["-hold", "-e"]);
            }
            _ => {
                return Err(Error::invalid(
                    "unsupported terminal; use kitty, foot, alacritty, konsole, xterm or ghostty",
                ));
            }
        }
        // Source is an explicit user command, while secrets are environment-only.
        cmd.args(["/bin/sh", "-c", source]);
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A short-lived broker creates PowerShell with its own console I/O.
        // Redirecting the broker avoids leaking command output to the agent;
        // Start-Process itself opens a new window with inherited environment.
        cmd.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]);
        cmd.arg(powershell_launch_script(binary, source, directory));
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW (broker only)
    }
    cmd.current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for p in env {
        cmd.env(&p.key, &p.value);
    }
    Ok(cmd)
}

#[cfg(any(windows, test))]
fn powershell_launch_script(binary: &Path, source: &str, directory: &Path) -> String {
    use base64::Engine;
    // Encode the user-authored command so PowerShell's launcher cannot reparse it.
    let utf16: Vec<u8> = source.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let binary = binary.to_string_lossy().replace('\'', "''");
    let directory = directory.to_string_lossy().replace('\'', "''");
    format!(
        "Start-Process -FilePath '{binary}' -ArgumentList '-NoLogo -NoExit -EncodedCommand {encoded}' -WorkingDirectory '{directory}' -ErrorAction Stop"
    )
}

fn spawn_terminal(cmd: &mut Command) -> Result<()> {
    let mut child = cmd.spawn().map_err(|e| Error::io("launch terminal", e))?;
    // Catch immediate argument/display failures without holding RPC for the command's lifetime.
    std::thread::sleep(std::time::Duration::from_millis(150));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| Error::io("check terminal launch", e))?
    {
        if !status.success() {
            return Err(Error::invalid(
                "terminal exited before opening; check your display and terminal configuration",
            ));
        }
    } else {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::EnvPair;
    use std::path::Path;

    #[test]
    #[cfg(unix)]
    fn terminal_argv_keeps_secret_values_out_of_shell_source() {
        let env = vec![EnvPair {
            key: "TOKEN".into(),
            value: "$(touch /bad) ' secret\n".into(),
        }];
        let cmd = terminal_command(
            Path::new("/usr/bin/kitty"),
            "printf '%s' \"$TOKEN\"",
            Path::new("/project with spaces"),
            &env,
        )
        .unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"printf '%s' \"$TOKEN\"".to_owned()));
        assert!(!args.join(" ").contains("touch /bad"));
        assert_eq!(
            cmd.get_current_dir(),
            Some(Path::new("/project with spaces"))
        );
        assert_eq!(
            cmd.get_envs()
                .find(|(k, _)| *k == "TOKEN")
                .unwrap()
                .1
                .unwrap(),
            "$(touch /bad) ' secret\n"
        );
    }

    #[test]
    fn windows_launcher_encodes_commands_and_creates_a_separate_console() {
        let script = powershell_launch_script(
            Path::new("C:/Program Files/PowerShell/pwsh.exe"),
            "Write-Host 'a; b'",
            Path::new("C:/Jane's project"),
        );
        assert!(
            script.starts_with("Start-Process -FilePath 'C:/Program Files/PowerShell/pwsh.exe'")
        );
        assert!(script.contains("-NoExit -EncodedCommand"));
        assert!(script.contains("-WorkingDirectory 'C:/Jane''s project'"));
        assert!(!script.contains("Write-Host"));
        assert!(!script.contains("-NoNewWindow"));
    }

    #[test]
    fn project_directory_rejects_nested_and_symlinked_vault_paths() {
        let t = tempfile::tempdir().unwrap();
        let vault = t.path().join("vault");
        std::fs::create_dir(&vault).unwrap();
        let sub = vault.join("nested");
        std::fs::create_dir(&sub).unwrap();
        assert!(project_directory(&vault, &vault).is_err());
        assert!(project_directory(&sub, &vault).is_err());
        assert!(project_directory(Path::new("relative"), &vault).is_err());
        #[cfg(unix)]
        {
            let link = t.path().join("link");
            std::os::unix::fs::symlink(&sub, &link).unwrap();
            assert!(project_directory(&link, &vault).is_err());
        }
        assert_eq!(
            project_directory(t.path(), &vault).unwrap(),
            t.path().canonicalize().unwrap()
        );
    }
}
