//! Pinned, app-owned Firefox runtime. The archive is verified before it is
//! extracted, and the completed tree is published with one rename.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::core::error::IoContext;
use crate::core::{Error, Result};

const REVISION: &str = "1555";
/// Firefox version of the pinned Playwright build.
pub const FIREFOX_VERSION: &str = "157.0";
const INSTALL_DIR: &str = "firefox-1555";
const CDN_BASE: &str = "https://cdn.playwright.dev/dbazure/download/playwright/builds/firefox/1555";
const MAX_ARCHIVE_BYTES: u64 = 250 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 30_000;

/// Map a target triple and Linux distribution to an archive from Playwright's
/// Firefox revision 1555. Other Linux distributions use its Debian 12 build.
/// Only glibc Linux is supported by the caller.
pub fn platform_archive(os: &str, arch: &str, distro: &str) -> Result<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") if distro == "ubuntu" => Ok("firefox-ubuntu-22.04.zip"),
        ("linux", "aarch64") if distro == "ubuntu" => Ok("firefox-ubuntu-22.04-arm64.zip"),
        ("linux", "x86_64") => Ok("firefox-debian-12.zip"),
        ("linux", "aarch64") => Ok("firefox-debian-12-arm64.zip"),
        ("macos", "x86_64") => Ok("firefox-mac.zip"),
        ("macos", "aarch64") => Ok("firefox-mac-arm64.zip"),
        _ => Err(Error::invalid(format!(
            "managed Firefox is unavailable on {os}/{arch}"
        ))),
    }
}

fn current_archive() -> Result<&'static str> {
    if cfg!(all(target_os = "linux", not(target_env = "gnu"))) {
        return Err(Error::invalid("managed Firefox requires glibc Linux"));
    }
    let distro = if cfg!(target_os = "linux") {
        fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines().find_map(|line| {
                    line.strip_prefix("ID=")
                        .map(|v| v.trim_matches('"').to_owned())
                })
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    platform_archive(std::env::consts::OS, std::env::consts::ARCH, &distro)
}

fn executable_relative_path() -> &'static str {
    if cfg!(target_os = "macos") {
        "firefox/Nightly.app/Contents/MacOS/firefox"
    } else {
        "firefox/firefox"
    }
}

/// Location of Firefox installation-wide AutoConfig files for a managed build.
pub fn config_root(executable: &Path) -> PathBuf {
    let parent = executable.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().is_some_and(|name| name == "MacOS")
        && parent
            .parent()
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "Contents"))
    {
        return parent.parent().unwrap().join("Resources");
    }
    parent.to_path_buf()
}

#[cfg(any(test, target_os = "macos"))]
fn app_bundle(executable: &Path) -> Option<&Path> {
    let macos = executable.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension()? != "app" {
        return None;
    }
    Some(bundle)
}

/// Re-sign a macOS managed bundle after installation-wide AutoConfig files
/// have been written. Call this before launching the browser.
pub fn finalize_bootstrap(executable: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let bundle = app_bundle(executable)
            .ok_or_else(|| Error::invalid("managed Firefox executable is outside an app bundle"))?;
        let status = std::process::Command::new("codesign")
            .args([
                "--force",
                "--deep",
                "--sign",
                "-",
                "--preserve-metadata=entitlements,flags,runtime",
            ])
            .arg(bundle)
            .status()
            .ctx(|| "ad-hoc sign managed Firefox app bundle".to_owned())?;
        if !status.success() {
            return Err(Error::Internal(
                "ad-hoc signing managed Firefox failed".to_owned(),
            ));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = executable;
    Ok(())
}

/// Return a complete pinned install, without touching the network.
pub fn cached(root: &Path) -> Result<Option<PathBuf>> {
    current_archive()?;
    let install = root.join(INSTALL_DIR);
    let marker = install.join(".complete");
    let exe = install.join(executable_relative_path());
    let application_ini = config_root(&exe).join("application.ini");
    if fs::read_to_string(marker).ok().as_deref() != Some(REVISION) {
        return Ok(None);
    }
    if !exe.is_file()
        || exe
            .metadata()
            .ctx(|| "inspect managed Firefox executable".to_owned())?
            .len()
            == 0
    {
        return Ok(None);
    }
    if !application_ini.is_file()
        || application_ini
            .metadata()
            .ctx(|| "inspect managed Firefox application.ini".to_owned())?
            .len()
            == 0
    {
        return Ok(None);
    }
    Ok(Some(exe))
}

/// Install or reuse Firefox in the app-owned data directory `root`.
/// The caller must supply a location outside any vault repository.
pub fn ensure(root: &Path) -> Result<PathBuf> {
    let archive = current_archive()?;
    fs::create_dir_all(root).ctx(|| "create managed Firefox cache".to_owned())?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(".install.lock"))
        .ctx(|| "open managed Firefox install lock".to_owned())?;
    lock.lock_exclusive()
        .ctx(|| "lock managed Firefox install".to_owned())?;
    let result = ensure_locked(root, archive);
    let _ = lock.unlock();
    result
}

fn ensure_locked(root: &Path, archive: &str) -> Result<PathBuf> {
    if let Some(exe) = cached(root)? {
        return Ok(exe);
    }
    let staging = tempfile::Builder::new()
        .prefix(".firefox-install-")
        .tempdir_in(root)
        .ctx(|| "create managed Firefox staging directory".to_owned())?;
    let archive_path = staging.path().join("archive.zip");
    download_archive(archive, &archive_path)?;
    let unpacked = staging.path().join("install");
    fs::create_dir(&unpacked).ctx(|| "create managed Firefox extraction directory".to_owned())?;
    extract_archive(&archive_path, &unpacked)?;
    let exe = unpacked.join(executable_relative_path());
    if !exe.is_file()
        || exe
            .metadata()
            .ctx(|| "inspect extracted Firefox executable".to_owned())?
            .len()
            == 0
    {
        return Err(Error::integrity(
            "managed Firefox archive has no executable",
        ));
    }
    fs::write(unpacked.join(".complete"), REVISION)
        .ctx(|| "mark managed Firefox install complete".to_owned())?;
    let final_path = root.join(INSTALL_DIR);
    if final_path.exists() {
        fs::remove_dir_all(&final_path)
            .ctx(|| "remove incomplete managed Firefox install".to_owned())?;
    }
    fs::rename(&unpacked, &final_path).ctx(|| "publish managed Firefox install".to_owned())?;
    Ok(final_path.join(executable_relative_path()))
}

fn archive_sha256(archive: &str) -> Option<&'static str> {
    match archive {
        "firefox-ubuntu-22.04.zip" => {
            Some("0a32f1f8d7e4170573435b4219eff8317e558750612b6355f18e9a4cb27e594b")
        }
        "firefox-ubuntu-22.04-arm64.zip" => {
            Some("dd80674501471999418e585e0feac8d7e568f1f88d528bb446b9e574091a9f89")
        }
        "firefox-debian-12.zip" => {
            Some("e9e37ebc321e481cf5162f935caec9f5332b14b30852c6d7f5d14fc60cac655a")
        }
        "firefox-debian-12-arm64.zip" => {
            Some("cc84e3b4354d9cd8398a0f9d2281d78bf4a917507f33619b2eae066f9f59f2de")
        }
        "firefox-mac.zip" => {
            Some("ae2c6e477d4c5f59e76be39ca0db6c003e9bcf9bb9b3c4e13cf545275d6be388")
        }
        "firefox-mac-arm64.zip" => {
            Some("72eb32289f368789786d16d252e8e4fef7b62ae721d57f82d3d6d9221d3fd2af")
        }
        _ => None,
    }
}

fn download_archive(archive: &str, destination: &Path) -> Result<()> {
    let expected = archive_sha256(archive)
        .ok_or_else(|| Error::invalid("managed Firefox archive has no pinned checksum"))?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| Error::Internal("construct managed Firefox downloader".to_owned()))?;
    let url = format!("{CDN_BASE}/{archive}");
    let mut response = client
        .get(&url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| Error::Internal("download managed Firefox failed".to_owned()))?;
    if response
        .content_length()
        .is_some_and(|len| len > MAX_ARCHIVE_BYTES)
    {
        return Err(Error::integrity(
            "managed Firefox archive exceeds size limit",
        ));
    }
    let mut file = File::create(destination).ctx(|| "create managed Firefox archive".to_owned())?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .ctx(|| "read managed Firefox download".to_owned())?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_ARCHIVE_BYTES {
            return Err(Error::integrity(
                "managed Firefox archive exceeds size limit",
            ));
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .ctx(|| "write managed Firefox archive".to_owned())?;
    }
    if format!("{:x}", hasher.finalize()) != expected {
        return Err(Error::integrity(
            "managed Firefox archive checksum mismatch",
        ));
    }
    Ok(())
}

fn extract_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive_path).ctx(|| "open managed Firefox archive".to_owned())?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|_| Error::integrity("managed Firefox archive is invalid"))?;
    if zip.len() > MAX_FILES {
        return Err(Error::integrity(
            "managed Firefox archive has too many entries",
        ));
    }
    let mut seen = HashSet::new();
    let mut expanded = 0u64;
    let mut links = Vec::new();
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|_| Error::integrity("managed Firefox archive entry is invalid"))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| Error::integrity("managed Firefox archive contains unsafe path"))?
            .to_path_buf();
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || !seen.insert(relative.clone())
        {
            return Err(Error::integrity(
                "managed Firefox archive contains unsafe or duplicate path",
            ));
        }
        expanded = expanded.checked_add(entry.size()).ok_or_else(|| {
            Error::integrity("managed Firefox archive exceeds expanded size limit")
        })?;
        if expanded > MAX_EXPANDED_BYTES {
            return Err(Error::integrity(
                "managed Firefox archive exceeds expanded size limit",
            ));
        }
        let target = destination.join(&relative);
        let mode = entry.unix_mode().unwrap_or(0);
        if (mode & 0o170000) == 0o120000 {
            let mut link_target = String::new();
            entry
                .take(4097)
                .read_to_string(&mut link_target)
                .map_err(|_| Error::integrity("managed Firefox archive has invalid symlink"))?;
            if link_target.len() > 4096 || !safe_relative_link(&relative, Path::new(&link_target)) {
                return Err(Error::integrity(
                    "managed Firefox archive has unsafe symlink",
                ));
            }
            links.push((target, link_target));
        } else if entry.is_dir() {
            fs::create_dir_all(&target).ctx(|| "create managed Firefox directory".to_owned())?;
        } else if (mode & 0o170000) == 0 || (mode & 0o170000) == 0o100000 {
            fs::create_dir_all(target.parent().unwrap())
                .ctx(|| "create managed Firefox directory".to_owned())?;
            let mut output =
                File::create(&target).ctx(|| "create managed Firefox file".to_owned())?;
            std::io::copy(&mut entry, &mut output)
                .ctx(|| "extract managed Firefox file".to_owned())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                    .ctx(|| "set managed Firefox file mode".to_owned())?;
            }
        } else {
            return Err(Error::integrity(
                "managed Firefox archive has unsupported entry",
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let canonical_root = destination
            .canonicalize()
            .ctx(|| "inspect managed Firefox extraction directory".to_owned())?;
        for (path, target) in links {
            let parent = path
                .parent()
                .unwrap()
                .canonicalize()
                .ctx(|| "inspect managed Firefox symlink parent".to_owned())?;
            if !parent.starts_with(&canonical_root) {
                return Err(Error::integrity(
                    "managed Firefox symlink leaves extraction directory",
                ));
            }
            symlink(target, &path).ctx(|| "create managed Firefox symlink".to_owned())?;
        }
        for path in seen {
            let actual = destination
                .join(path)
                .canonicalize()
                .ctx(|| "inspect managed Firefox extracted path".to_owned())?;
            if !actual.starts_with(&canonical_root) {
                return Err(Error::integrity(
                    "managed Firefox symlink leaves extraction directory",
                ));
            }
        }
    }
    #[cfg(not(unix))]
    if !links.is_empty() {
        return Err(Error::integrity(
            "managed Firefox archive has unsupported symlinks",
        ));
    }
    Ok(())
}

fn safe_relative_link(location: &Path, target: &Path) -> bool {
    let mut depth = location.components().count().saturating_sub(1);
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_matrix_selects_pinned_archives() {
        let cases = [
            ("linux", "x86_64", "ubuntu", "firefox-ubuntu-22.04.zip"),
            (
                "linux",
                "aarch64",
                "ubuntu",
                "firefox-ubuntu-22.04-arm64.zip",
            ),
            ("linux", "x86_64", "debian", "firefox-debian-12.zip"),
            ("linux", "aarch64", "arch", "firefox-debian-12-arm64.zip"),
            ("macos", "x86_64", "", "firefox-mac.zip"),
            ("macos", "aarch64", "", "firefox-mac-arm64.zip"),
        ];
        for (os, arch, distro, archive) in cases {
            assert_eq!(platform_archive(os, arch, distro).unwrap(), archive);
        }
        for (os, arch) in [
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "riscv64"),
            ("freebsd", "x86_64"),
        ] {
            assert!(platform_archive(os, arch, "").is_err());
        }
    }

    #[test]
    fn missing_or_partial_cache_is_not_accepted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cached(dir.path()).unwrap().is_none());
        let exe = dir
            .path()
            .join("firefox-1555")
            .join(executable_relative_path());
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"binary").unwrap();
        assert!(cached(dir.path()).unwrap().is_none());
        std::fs::write(dir.path().join("firefox-1555").join(".complete"), b"1555").unwrap();
        assert!(cached(dir.path()).unwrap().is_none());
        let config = config_root(&exe);
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("application.ini"), b"[App]").unwrap();
        assert_eq!(cached(dir.path()).unwrap(), Some(exe));
    }

    #[test]
    fn config_root_matches_platform_layout() {
        assert_eq!(
            config_root(Path::new("Firefox.app/Contents/MacOS/firefox")),
            Path::new("Firefox.app/Contents/Resources")
        );
        assert_eq!(
            config_root(Path::new("firefox/firefox")),
            Path::new("firefox")
        );
    }

    #[test]
    fn app_bundle_is_derived_from_macos_executable() {
        assert_eq!(
            app_bundle(Path::new("firefox/Nightly.app/Contents/MacOS/firefox")),
            Some(Path::new("firefox/Nightly.app"))
        );
        assert_eq!(app_bundle(Path::new("firefox/firefox")), None);
    }

    #[test]
    fn extraction_rejects_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bad.zip");
        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        let destination = dir.path().join("output");
        std::fs::create_dir(&destination).unwrap();
        assert!(extract_archive(&archive, &destination).is_err());
        assert!(!dir.path().join("escape").exists());
    }

    #[test]
    #[ignore = "downloads the pinned Firefox archive"]
    fn downloads_and_reuses_pinned_firefox() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure(dir.path()).unwrap();
        assert!(first.is_file());
        assert_eq!(cached(dir.path()).unwrap(), Some(first.clone()));
        assert_eq!(ensure(dir.path()).unwrap(), first);
    }
}
