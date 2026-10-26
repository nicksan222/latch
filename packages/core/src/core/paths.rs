//! Per-user directories with `LATCH_*` overrides (see AGENTS.md).
//!
//! | What    | Default (Linux)             | Override            |
//! |---------|-----------------------------|---------------------|
//! | config  | `~/.config/latch`           | `LATCH_CONFIG_DIR`  |
//! | data    | `~/.local/share/latch`      | `LATCH_DATA_DIR`    |
//! | runtime | `$XDG_RUNTIME_DIR/latch`    | `LATCH_RUNTIME_DIR` |
//! | RAM dir | `/dev/shm/latch-<uid>`      | `LATCH_RAM_DIR`     |

use std::path::PathBuf;

pub const ENV_CONFIG_DIR: &str = "LATCH_CONFIG_DIR";
pub const ENV_DATA_DIR: &str = "LATCH_DATA_DIR";
pub const ENV_RUNTIME_DIR: &str = "LATCH_RUNTIME_DIR";
pub const ENV_RAM_DIR: &str = "LATCH_RAM_DIR";

const APP: &str = "latch";

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn fallback_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(std::env::temp_dir)
}

/// Expand a vault path's leading `~` component against the actual home.
/// Other users' `~name` syntax is unsupported rather than treated literally.
/// Plain relative paths stay relative; vault operations canonicalize normally.
pub(crate) fn expand_vault_home(path: &std::path::Path) -> super::error::Result<PathBuf> {
    let home = dirs::home_dir();
    expand_vault_home_with(path, home.as_deref())
}

fn expand_vault_home_with(
    path: &std::path::Path,
    home: Option<&std::path::Path>,
) -> super::error::Result<PathBuf> {
    use super::error::Error;
    use std::path::Component;
    let mut components = path.components();
    if let Some(Component::Normal(first)) = components.next() {
        if first == "~" {
            let home = home.filter(|p| p.is_absolute()).ok_or_else(|| {
                Error::invalid("home directory unavailable; use an absolute vault path")
            })?;
            return Ok(home.join(components.as_path()));
        }
        if first.as_encoded_bytes().starts_with(b"~") {
            return Err(Error::invalid(
                "named-user home paths are unsupported; use ~/ or an absolute vault path",
            ));
        }
    }
    Ok(path.to_path_buf())
}

/// Config dir (settings.json, vaults.json).
pub fn config_dir() -> PathBuf {
    env_path(ENV_CONFIG_DIR).unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| fallback_home().join(".config"))
            .join(APP)
    })
}

/// Data dir (session archives).
pub fn data_dir() -> PathBuf {
    env_path(ENV_DATA_DIR).unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| fallback_home().join(".local/share"))
            .join(APP)
    })
}

/// Encrypted browser session archives for a vault (outside the repo).
pub fn sessions_dir(vault_id: &str) -> PathBuf {
    data_dir().join("sessions").join(vault_id)
}

/// Runtime dir (agent.sock, agent.lock).
pub fn runtime_dir() -> PathBuf {
    if let Some(p) = env_path(ENV_RUNTIME_DIR) {
        return p;
    }
    if let Some(p) = dirs::runtime_dir() {
        return p.join(APP);
    }
    // No XDG_RUNTIME_DIR: a per-user dir in the user's own cache dir (never a
    // predictable name in a shared temp dir). Verified 0700/ours on use.
    dirs::cache_dir()
        .unwrap_or_else(|| fallback_home().join(".cache"))
        .join(APP)
        .join("run")
}

/// RAM-backed dir for decrypted browser sessions.
pub fn ram_dir() -> PathBuf {
    if let Some(p) = env_path(ENV_RAM_DIR) {
        return p;
    }
    #[cfg(target_os = "linux")]
    {
        let shm = std::path::Path::new("/dev/shm");
        if shm.is_dir() {
            return shm.join(format!("{APP}-{}", uid()));
        }
    }
    std::env::temp_dir().join(format!("{APP}-{}", uid()))
}

/// Current user id (0 on non-unix, only used to namespace temp dirs).
pub fn uid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: getuid has no preconditions and cannot fail.
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn insecure(dir: &std::path::Path, why: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!("refusing to use {}: {why}", dir.display()),
    )
}

/// Verify (without creating or changing anything) that `dir` is a real
/// directory (not a symlink), owned by the current user, with no group or
/// other permission bits. Used by clients before trusting a runtime dir.
pub fn check_secure_dir(dir: &std::path::Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(dir)?;
    if m.file_type().is_symlink() {
        return Err(insecure(dir, "it is a symlink"));
    }
    if !m.is_dir() {
        return Err(insecure(dir, "not a directory"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if m.uid() != uid() {
            return Err(insecure(dir, "owned by another user"));
        }
        if m.mode() & 0o077 != 0 {
            return Err(insecure(
                dir,
                "accessible by other users (mode must be 0700)",
            ));
        }
    }
    Ok(())
}

/// Create `dir` securely (parents normally, `dir` itself 0700) or validate an
/// existing one: must be a real directory owned by us. Group/other bits on a
/// dir we own are removed. Foreign or symlinked dirs are refused (an attacker
/// could pre-create a predictable path such as `/dev/shm/latch-<uid>`).
pub fn ensure_secure_dir(dir: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    match b.create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    match check_secure_dir(dir) {
        Ok(()) => Ok(()),
        #[cfg(unix)]
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let m = std::fs::symlink_metadata(dir)?;
            if m.is_dir() && !m.file_type().is_symlink() && m.uid() == uid() {
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
                check_secure_dir(dir)
            } else {
                Err(e)
            }
        }
        Err(e) => Err(e),
    }
}

/// Create `dir` (and parents) with 0700 permissions on unix.
pub fn ensure_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests read env overrides; they do not mutate the process
    // environment (unsafe in edition 2024 and racy across test threads).
    #[test]
    fn vault_home_expansion_is_pure_and_preserves_path_components() {
        let home = std::path::Path::new("/home/test-user");
        for (input, expected) in [
            ("~", "/home/test-user"),
            ("~/latch-vault", "/home/test-user/latch-vault"),
            ("~/.user/keys", "/home/test-user/.user/keys"),
            ("~//nested/vault", "/home/test-user/nested/vault"),
            ("relative/vault", "relative/vault"),
            ("/absolute/vault", "/absolute/vault"),
            ("./~/literal", "./~/literal"),
        ] {
            assert_eq!(
                expand_vault_home_with(std::path::Path::new(input), Some(home)).unwrap(),
                std::path::PathBuf::from(expected)
            );
        }
        for input in ["~alice/vault", "~.user/vault", "~alice"] {
            assert!(expand_vault_home_with(std::path::Path::new(input), Some(home)).is_err());
        }
        assert!(expand_vault_home_with(std::path::Path::new("~/vault"), None).is_err());
        assert!(
            expand_vault_home_with(
                std::path::Path::new("~/vault"),
                Some(std::path::Path::new("relative-home"))
            )
            .is_err()
        );
        assert_eq!(
            expand_vault_home_with(std::path::Path::new("plain"), None).unwrap(),
            std::path::PathBuf::from("plain")
        );
    }

    #[test]
    fn defaults_end_in_latch() {
        if std::env::var_os(ENV_CONFIG_DIR).is_none() {
            assert!(config_dir().ends_with("latch"));
        }
        if std::env::var_os(ENV_DATA_DIR).is_none() {
            assert!(data_dir().ends_with("latch"));
        }
        if std::env::var_os(ENV_RUNTIME_DIR).is_none() {
            let r = runtime_dir();
            // Headless containers have no XDG runtime directory and use the
            // deliberately private cache/latch/run fallback.
            if let Some(runtime) = dirs::runtime_dir() {
                assert_eq!(r, runtime.join("latch"));
            } else {
                assert!(r.ends_with("latch/run"));
            }
        }
        if std::env::var_os(ENV_RAM_DIR).is_none() {
            let r = ram_dir();
            assert!(
                r.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("latch-")
            );
        }
    }

    #[test]
    fn env_path_ignores_empty() {
        assert_eq!(env_path("LATCH_TEST_DEFINITELY_UNSET_VAR"), None);
    }

    #[test]
    fn sessions_under_data() {
        assert!(sessions_dir("abc").starts_with(data_dir()));
    }

    #[cfg(unix)]
    #[test]
    fn secure_dir_rules() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("run");
        ensure_secure_dir(&d).unwrap();
        check_secure_dir(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_secure_dir(&d).is_err());
        ensure_secure_dir(&d).unwrap(); // ours: repaired
        check_secure_dir(&d).unwrap();
        let l = t.path().join("link");
        std::os::unix::fs::symlink(&d, &l).unwrap();
        assert!(check_secure_dir(&l).is_err());
        assert!(ensure_secure_dir(&l).is_err());
        assert!(check_secure_dir(&t.path().join("missing")).is_err());
    }

    #[test]
    fn private_dir() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("a/b");
        ensure_private_dir(&d).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&d).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }
}
