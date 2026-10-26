//! Native packaging uses Rust filesystem, archive, and checksum APIs.
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn stable_version(value: &str) -> Result<()> {
    let version = semver::Version::parse(value).context("invalid release version")?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty() && version.to_string() == value,
        "expected a stable release version"
    );
    Ok(())
}
fn run(command: &mut Command) -> Result<()> {
    ensure!(
        command.status().context("start release command")?.success(),
        "release command failed"
    );
    Ok(())
}
fn host_triple() -> Result<String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .context("read Rust host")?;
    ensure!(output.status.success(), "cannot read Rust host");
    String::from_utf8(output.stdout)?
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .context("missing Rust host")
}
fn target_directory(root: &Path) -> PathBuf {
    let path = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn prepare_checkout(root: &Path, version: &str) -> Result<()> {
    stable_version(version)?;
    // The public build entrypoint validates that this is a disposable CI checkout.
    run(Command::new("node")
        .current_dir(root)
        .env("CI", "true")
        .env("LATCH_RELEASE_BUILD", "1")
        .arg("tooling/release/prepare-version.mjs")
        .arg(version))?;
    run(Command::new("bun")
        .current_dir(root)
        .args(["install", "--frozen-lockfile"]))
}

pub fn build(version: &str) -> Result<()> {
    stable_version(version)?;
    ensure!(
        std::env::var("CI").as_deref() == Ok("true")
            && std::env::var("LATCH_RELEASE_BUILD").as_deref() == Ok("1"),
        "release packaging requires a disposable CI checkout"
    );
    let root = crate::repository();
    let triple = host_triple()?;
    ensure!(
        triple == "x86_64-unknown-linux-gnu",
        "Linux release packaging requires x86_64 Linux"
    );
    ensure!(
        !root.join("release-artifacts").try_exists()?,
        "refusing to reuse artifact directory"
    );
    prepare_checkout(&root, version)?;
    run(Command::new("bun").current_dir(&root).args([
        "run",
        "desktop:bundle",
        "--",
        "--bundles",
        "deb,appimage",
        "--",
        "--locked",
    ]))?;
    run(Command::new(root.join("apps/desktop/src-tauri/binaries/latch")).arg("--version"))?;
    let output = package_existing(&root, version, &triple, &target_directory(&root))?;
    println!("Release artifacts ready at {}", output.display());
    Ok(())
}

pub fn prepare_desktop_bundle() -> Result<()> {
    run(Command::new("bun")
        .current_dir(crate::repository())
        .args(["run", "build:desktop"]))?;
    sidecar()
}

pub fn sidecar() -> Result<()> {
    let root = crate::repository();
    let triple = std::env::var("TAURI_ENV_TARGET_TRIPLE")
        .map(Ok)
        .unwrap_or_else(|_| host_triple())?;
    ensure!(
        matches!(
            triple.as_str(),
            "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu" | "aarch64-apple-darwin"
        ),
        "unsupported CLI packaging target"
    );
    let debug = std::env::var("TAURI_ENV_DEBUG").as_deref() == Ok("true");
    let target = target_directory(&root);
    let mut command = Command::new("cargo");
    command
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", &target)
        .args(["build", "--locked", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args([
            "--no-default-features",
            "-p",
            "latch-cli",
            "--target",
            &triple,
            "--bin",
            "latch",
        ]);
    if !debug {
        command.arg("--release");
    }
    run(&mut command)?;
    let source = target
        .join(&triple)
        .join(if debug { "debug" } else { "release" })
        .join("latch");
    let output = root.join("apps/desktop/src-tauri/binaries");
    fs::create_dir_all(&output)?;
    fs::copy(source, output.join("latch"))?;
    println!("CLI ready for {triple}");
    Ok(())
}

fn bundle(target: &Path, kind: &str, extension: &str) -> Result<PathBuf> {
    let directory = target.join("release/bundle").join(kind);
    let candidates = fs::read_dir(&directory)
        .with_context(|| format!("missing {kind} bundle directory"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|value| value == extension))
        .collect::<Vec<_>>();
    ensure!(candidates.len() == 1, "expected one {kind} bundle");
    let path = &candidates[0];
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "bundle must be a regular file"
    );
    Ok(path.clone())
}
fn package_existing(root: &Path, version: &str, triple: &str, target: &Path) -> Result<PathBuf> {
    stable_version(version)?;
    let output = root.join("release-artifacts");
    ensure!(
        fs::symlink_metadata(&output).is_err(),
        "refusing to reuse artifact directory"
    );
    let bundles = [
        bundle(target, "deb", "deb")?,
        bundle(target, "appimage", "AppImage")?,
    ];
    let cli = root.join("apps/desktop/src-tauri/binaries/latch");
    for path in [&cli, &root.join("LICENSE")] {
        ensure!(
            fs::symlink_metadata(path)?.is_file(),
            "archive input must be a regular file"
        );
    }
    let staging = tempfile::Builder::new()
        .prefix(".latch-release-")
        .tempdir_in(root)?;
    let archive_name = format!("latch-{version}-{triple}.tar.gz");
    let compressed = flate2::write::GzEncoder::new(
        File::create(staging.path().join(&archive_name))?,
        flate2::Compression::default(),
    );
    let mut archive = tar::Builder::new(compressed);
    archive.append_path_with_name(cli, "latch")?;
    archive.append_path_with_name(root.join("LICENSE"), "LICENSE")?;
    archive.into_inner()?.finish()?;
    let mut names = vec![archive_name];
    for path in bundles {
        let name = path
            .file_name()
            .context("missing bundle filename")?
            .to_str()
            .context("invalid bundle filename")?
            .to_owned();
        ensure!(!name.contains(['\n', '\r']), "invalid bundle filename");
        fs::copy(&path, staging.path().join(&name))?;
        names.push(name);
    }
    names.sort();
    let mut checksums = String::new();
    for name in names {
        let mut file = File::open(staging.path().join(&name))?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65_536];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        checksums.push_str(&format!("{:x}  {name}\n", digest.finalize()));
    }
    fs::write(staging.path().join("SHA256SUMS"), checksums)?;
    fs::rename(staging.path(), &output).context("publish artifact directory")?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use sha2::{Digest, Sha256};
    use std::fs;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("apps/desktop/src-tauri/binaries")).unwrap();
        fs::write(
            root.path().join("apps/desktop/src-tauri/binaries/latch"),
            b"fixture CLI",
        )
        .unwrap();
        fs::write(root.path().join("LICENSE"), b"Fixture license\n").unwrap();
        for (kind, name) in [
            ("deb", "Latch_0.1.1_amd64.deb"),
            ("appimage", "Latch_0.1.1_amd64.AppImage"),
        ] {
            let dir = root.path().join("target/release/bundle").join(kind);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(name), kind).unwrap();
        }
        root
    }

    fn preparation_fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("tooling/release")).unwrap();
        fs::create_dir_all(root.path().join("apps/desktop/src-tauri")).unwrap();
        fs::create_dir_all(root.path().join("dependency")).unwrap();
        for name in ["prepare-version.mjs", "release-plan.mjs"] {
            fs::copy(
                crate::repository().join("tooling/release").join(name),
                root.path().join("tooling/release").join(name),
            )
            .unwrap();
        }
        fs::write(root.path().join("package.json"), r#"{"name":"release-fixture","version":"0.1.0","private":true,"devDependencies":{"fixture-dep":"file:./dependency"}}"#).unwrap();
        fs::write(
            root.path().join("dependency/package.json"),
            r#"{"name":"fixture-dep","version":"1.0.0"}"#,
        )
        .unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            r#"[workspace.package]
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.path().join("Cargo.lock"),
            r#"version = 4

[[package]]
name = "latch-automation"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.path().join("apps/desktop/src-tauri/tauri.conf.json"),
            r#"{"version":"0.1.0"}"#,
        )
        .unwrap();
        run(Command::new("bun")
            .current_dir(root.path())
            .args(["install", "--lockfile-only"]))
        .unwrap();
        assert!(!root.path().join("node_modules/fixture-dep").exists());
        root
    }

    #[test]
    fn clean_release_checkout_prepares_versions_and_installs_dependencies() {
        let root = preparation_fixture();
        prepare_checkout(root.path(), "0.1.1").unwrap();
        let config: serde_json::Value = serde_json::from_slice(
            &fs::read(root.path().join("apps/desktop/src-tauri/tauri.conf.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(config["version"], "0.1.1");
        assert!(
            fs::read_to_string(root.path().join("Cargo.lock"))
                .unwrap()
                .contains(r#"version = "0.1.1""#)
        );
        assert!(
            root.path()
                .join("node_modules/fixture-dep/package.json")
                .is_file()
        );
    }
    #[test]
    fn invalid_versions_fail_before_installing_any_dependency() {
        let root = preparation_fixture();
        assert!(prepare_checkout(root.path(), "invalid-version").is_err());
        assert!(!root.path().join("node_modules/fixture-dep").exists());
    }
    #[test]
    fn package_has_exact_assets_license_and_valid_checksums() {
        let root = fixture();
        let output = package_existing(
            root.path(),
            "0.1.1",
            "x86_64-unknown-linux-gnu",
            &root.path().join("target"),
        )
        .unwrap();
        assert_eq!(fs::read_dir(&output).unwrap().count(), 4);
        let archive = output.join("latch-0.1.1-x86_64-unknown-linux-gnu.tar.gz");
        let mut tar = tar::Archive::new(GzDecoder::new(fs::File::open(archive).unwrap()));
        let names = tar
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["latch", "LICENSE"]);
        let checksums = fs::read_to_string(output.join("SHA256SUMS")).unwrap();
        for line in checksums.lines() {
            let (digest, name) = line.split_once("  ").unwrap();
            assert_eq!(
                digest,
                format!("{:x}", Sha256::digest(fs::read(output.join(name)).unwrap()))
            );
        }
        assert_eq!(checksums.lines().count(), 3);
    }

    #[test]
    fn duplicate_bundles_do_not_create_partial_artifacts() {
        let root = fixture();
        fs::write(
            root.path().join("target/release/bundle/deb/duplicate.deb"),
            b"duplicate",
        )
        .unwrap();
        assert!(
            package_existing(
                root.path(),
                "0.1.1",
                "x86_64-unknown-linux-gnu",
                &root.path().join("target")
            )
            .is_err()
        );
        assert!(!root.path().join("release-artifacts").exists());
    }
    #[test]
    fn existing_release_artifacts_are_preserved() {
        let root = fixture();
        fs::create_dir(root.path().join("release-artifacts")).unwrap();
        fs::write(root.path().join("release-artifacts/keep"), b"keep").unwrap();
        assert!(
            package_existing(
                root.path(),
                "0.1.1",
                "x86_64-unknown-linux-gnu",
                &root.path().join("target")
            )
            .is_err()
        );
        assert_eq!(
            fs::read(root.path().join("release-artifacts/keep")).unwrap(),
            b"keep"
        );
    }
    #[test]
    fn releases_require_canonical_stable_versions() {
        for value in ["1.2", "01.2.3", "v1.2.3", "1.2.3-rc.1", "1.2.3+metadata"] {
            assert!(stable_version(value).is_err());
        }
        assert!(stable_version("0.1.1").is_ok());
    }
    #[test]
    fn missing_bundles_do_not_create_partial_artifacts() {
        let root = fixture();
        fs::remove_file(
            root.path()
                .join("target/release/bundle/appimage/Latch_0.1.1_amd64.AppImage"),
        )
        .unwrap();
        assert!(
            package_existing(
                root.path(),
                "0.1.1",
                "x86_64-unknown-linux-gnu",
                &root.path().join("target")
            )
            .is_err()
        );
        assert!(!root.path().join("release-artifacts").exists());
    }
}
