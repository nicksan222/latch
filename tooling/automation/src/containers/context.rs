//! Explicit source inputs only; never send the developer's checkout wholesale to Docker.
use anyhow::Result;
use std::path::Path;

const ROOTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "package.json",
    "bun.lock",
    "tsconfig.json",
    "vite.config.ts",
    "apps",
    "packages",
    "tooling",
];

pub fn snapshot(root: &Path, destination: &Path) -> Result<()> {
    let mut files = std::collections::BTreeSet::new();
    for (selection, tracked) in [("--cached", true), ("--others", false)] {
        let mut command = std::process::Command::new("git");
        // Caller Git overrides must not redirect source enumeration to another index.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        let output = command
            .arg("-C")
            .arg(root)
            .arg("--work-tree")
            .arg(root)
            .args(["ls-files", selection, "--exclude-standard", "-z", "--"])
            .args(ROOTS)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "enumerate declared Git source inputs"
        );
        for name in output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let relative = std::path::PathBuf::from(std::str::from_utf8(name)?);
            if relative
                .components()
                .any(|part| excluded(&part.as_os_str().to_string_lossy()))
            {
                continue;
            }
            if tracked || additional_source(&relative) {
                files.insert(relative);
            }
        }
    }
    for relative in files {
        let mut source = root.to_path_buf();
        let mut present = true;
        for part in relative.components() {
            anyhow::ensure!(
                matches!(part, std::path::Component::Normal(_)),
                "invalid test source path"
            );
            source.push(part);
            match std::fs::symlink_metadata(&source) {
                Ok(metadata) => anyhow::ensure!(
                    !metadata.is_symlink(),
                    "test build context must not contain symlinks"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    present = false;
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        if !present {
            continue;
        } // A locally deleted tracked file stays deleted.
        anyhow::ensure!(source.is_file(), "test source inputs must be regular files");
        let output = destination.join(relative);
        std::fs::create_dir_all(output.parent().expect("context entry parent"))?;
        std::fs::copy(source, output)?;
    }
    Ok(())
}

fn excluded(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules"
                | "target"
                | "dist"
                | "dist-firefox"
                | "gen"
                | "binaries"
                | "test-results"
                | "playwright-report"
        )
        || [".pem", ".key", ".p12", ".pfx", ".log"]
            .iter()
            .any(|extension| name.ends_with(extension))
}

/// New code is usable without staging. New data/config is included only when
/// explicitly tracked, except the repository's synthetic import fixtures.
fn additional_source(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if matches!(name, "Cargo.toml" | "package.json" | "build.rs") {
        return true;
    }
    if path.starts_with("packages/core/fixtures") {
        return true;
    }
    let extension = path
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or_default();
    let code_directory = path
        .components()
        .any(|part| matches!(part.as_os_str().to_str(), Some("src" | "tests")));
    (code_directory
        && matches!(
            extension,
            "rs" | "ts" | "tsx" | "js" | "mjs" | "css" | "html" | "astro"
        ))
        || (path.starts_with("tooling") && matches!(extension, "mjs" | "py" | "sh"))
        || (path.components().any(|part| {
            matches!(
                part.as_os_str().to_str(),
                Some("assets" | "public" | "icons")
            )
        }) && matches!(
            extension,
            "svg" | "png" | "jpg" | "jpeg" | "webp" | "ico" | "ttf" | "woff" | "woff2"
        ))
}

pub fn digest(root: &Path) -> Result<String> {
    fn hash(root: &Path, path: &Path, state: &mut blake3::Hasher) -> Result<()> {
        let mut entries = std::fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                hash(root, &path, state)?;
            } else {
                let relative = path.strip_prefix(root)?.to_string_lossy();
                let bytes = std::fs::read(&path)?;
                state.update(&(relative.len() as u64).to_le_bytes());
                state.update(relative.as_bytes());
                state.update(&(bytes.len() as u64).to_le_bytes());
                state.update(&bytes);
            }
        }
        Ok(())
    }
    let mut state = blake3::Hasher::new();
    hash(root, root, &mut state)?;
    Ok(state.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn init(root: &Path) {
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(root)
                .status()
                .unwrap()
                .success()
        );
    }
    #[test]
    fn snapshot_includes_dirty_source_but_excludes_local_state() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        init(root.path());
        for (name, contents) in [
            ("Cargo.toml", "workspace"),
            ("tooling/patches/example/src/lib.rs", "patched"),
            (
                "packages/core/fixtures/import/fixture.json",
                "synthetic fixture",
            ),
            ("packages/core/src/lib.rs", "local edit"),
            ("packages/core/.env", "synthetic-secret"),
            ("packages/core/target/debug/output", "build"),
            (".dev/vault/secret", "synthetic-secret"),
            (".git/credential-sentinel", "synthetic-secret"),
        ] {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(root.path())
                .args(["add", "Cargo.toml"])
                .status()
                .unwrap()
                .success()
        );
        snapshot(root.path(), output.path()).unwrap();
        assert!(
            output
                .path()
                .join("packages/core/fixtures/import/fixture.json")
                .exists()
        );
        let before = digest(output.path()).unwrap();
        std::fs::write(output.path().join("packages/core/src/lib.rs"), "new edit").unwrap();
        assert_ne!(digest(output.path()).unwrap(), before);
        std::fs::write(output.path().join("packages/core/src/lib.rs"), "local edit").unwrap();
        assert_eq!(
            std::fs::read_to_string(output.path().join("packages/core/src/lib.rs")).unwrap(),
            "local edit"
        );
        assert!(output.path().join("Cargo.toml").is_file());
        assert!(
            output
                .path()
                .join("tooling/patches/example/src/lib.rs")
                .is_file()
        );
        for excluded in ["packages/core/.env", "packages/core/target", ".dev", ".git"] {
            assert!(!output.path().join(excluded).exists());
        }
    }
    #[test]
    fn nested_credentials_vault_and_profile_are_not_build_inputs() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        init(root.path());
        for name in [
            "packages/core/credentials.json",
            "apps/desktop/vault/profiles/one.age",
            "apps/desktop/firefox-profile/prefs.js",
            "packages/core/src/private.json",
        ] {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"synthetic private sentinel").unwrap();
        }
        snapshot(root.path(), output.path()).unwrap();
        for name in [
            "packages/core/credentials.json",
            "apps/desktop/vault",
            "apps/desktop/firefox-profile",
            "packages/core/src/private.json",
        ] {
            assert!(
                !output.path().join(name).exists(),
                "local data must not enter the image context"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_refuses_symlink_escapes() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        init(root.path());
        std::fs::create_dir_all(root.path().join("packages/core/src")).unwrap();
        std::os::unix::fs::symlink(
            "/etc/passwd",
            root.path().join("packages/core/src/escaped.rs"),
        )
        .unwrap();
        assert!(snapshot(root.path(), output.path()).is_err());
    }
}
