//! Export only bounded regular test artifacts into a fresh invocation directory.
use anyhow::Result;
use std::path::Path;

const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 512;

fn permitted(path: &Path) -> bool {
    !path.components().any(|part| {
        matches!(
            part.as_os_str().to_str(),
            Some("vault" | "profile" | "profiles" | "ram" | "config" | "data")
        )
    }) && path
        .extension()
        .and_then(|part| part.to_str())
        .is_some_and(|extension| matches!(extension, "zip" | "png" | "webm" | "md"))
}

pub fn archive(root: &Path, destination: &Path) -> Result<()> {
    fn append(
        root: &Path,
        path: &Path,
        builder: &mut tar::Builder<std::fs::File>,
        total: &mut u64,
        count: &mut usize,
    ) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                !metadata.is_symlink(),
                "test artifacts must not contain symlinks"
            );
            let relative = path.strip_prefix(root)?;
            if metadata.is_dir() {
                if !relative.components().any(|part| {
                    matches!(
                        part.as_os_str().to_str(),
                        Some("vault" | "profile" | "profiles" | "ram" | "config" | "data")
                    )
                }) {
                    append(root, &path, builder, total, count)?;
                }
            } else if metadata.is_file() && permitted(relative) {
                *total += metadata.len();
                *count += 1;
                anyhow::ensure!(
                    metadata.len() <= MAX_FILE && *total <= MAX_TOTAL && *count <= MAX_FILES,
                    "test artifacts exceed export limits"
                );
                builder.append_path_with_name(&path, relative)?;
            }
        }
        Ok(())
    }
    let mut builder = tar::Builder::new(std::fs::File::create(destination)?);
    if root.exists() {
        let mut total = 0;
        let mut count = 0;
        append(root, root, &mut builder, &mut total, &mut count)?;
    }
    builder.finish()?;
    Ok(())
}

#[cfg(any(test, feature = "containers"))]
pub fn extract(archive: &Path, destination: &Path) -> Result<()> {
    anyhow::ensure!(
        !std::fs::symlink_metadata(destination)?.is_symlink(),
        "artifact destination must be a private directory"
    );
    let mut reader = tar::Archive::new(std::fs::File::open(archive)?);
    let mut total = 0_u64;
    for (index, entry) in reader.entries()?.enumerate() {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        anyhow::ensure!(
            entry.header().entry_type().is_file(),
            "artifact archive contains a non-file entry"
        );
        anyhow::ensure!(
            path.components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
                && permitted(&path),
            "artifact archive contains an unsafe path"
        );
        total += entry.size();
        anyhow::ensure!(
            entry.size() <= MAX_FILE && total <= MAX_TOTAL && index < MAX_FILES,
            "test artifacts exceed export limits"
        );
        let output = destination.join(path);
        let parent = output.parent().expect("artifact parent");
        std::fs::create_dir_all(parent)?;
        entry.unpack(output)?;
    }
    Ok(())
}

#[cfg(any(test, feature = "containers"))]
pub fn output_directory(root: &Path) -> Result<std::path::PathBuf> {
    for directory in [root.join(".dev"), root.join(".dev/test-results")] {
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.is_symlink(),
                "refuse redirected test artifact directory"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&directory)?
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(root.join(".dev/test-results"))?
        .keep())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn redirected_output_directory_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(".dev")).unwrap();
        assert!(output_directory(root.path()).is_err());
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    }

    #[test]
    fn oversized_artifact_is_rejected_before_copying() {
        let root = tempfile::tempdir().unwrap();
        let file = std::fs::File::create(root.path().join("trace.zip")).unwrap();
        file.set_len(64 * 1024 * 1024 + 1).unwrap();
        assert!(archive(root.path(), &root.path().join("artifacts.tar")).is_err());
    }

    #[test]
    fn artifacts_round_trip_without_exporting_profiles() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("run-container")).unwrap();
        std::fs::write(
            root.path().join("run-container/trace.zip"),
            b"synthetic trace",
        )
        .unwrap();
        std::fs::write(
            root.path().join("run-container/profile.sqlite"),
            b"synthetic private state",
        )
        .unwrap();
        let bundle = root.path().join("artifacts.tar");
        archive(root.path(), &bundle).unwrap();
        extract(&bundle, output.path()).unwrap();
        assert_eq!(
            std::fs::read(output.path().join("run-container/trace.zip")).unwrap(),
            b"synthetic trace"
        );
        assert!(!output.path().join("run-container/profile.sqlite").exists());
    }
    #[test]
    fn traversal_in_artifact_archive_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("output");
        std::fs::create_dir(&destination).unwrap();
        let bundle = root.path().join("malicious.tar");
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_mode(0o600);
        header.as_mut_bytes()[..12].copy_from_slice(b"../escape.md");
        header.set_cksum();
        let mut writer = tar::Builder::new(std::fs::File::create(&bundle).unwrap());
        writer.append(&header, std::io::empty()).unwrap();
        writer.finish().unwrap();
        assert!(extract(&bundle, &destination).is_err());
        assert!(!root.path().join("escape.md").exists());
    }
    #[cfg(unix)]
    #[test]
    fn artifact_symlinks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.path().join("trace.zip")).unwrap();
        assert!(archive(root.path(), &root.path().join("artifacts.tar")).is_err());
    }
}
