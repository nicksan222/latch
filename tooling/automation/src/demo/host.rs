use super::{
    Options, build_commands, capture_command, container_create_args, include_snapshot_path,
};
use anyhow::{Context, Result, bail};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn run(options: Options) -> Result<()> {
    let root = crate::repository();
    let engine = std::env::var_os("CONTAINER_ENGINE")
        .map(PathBuf::from)
        .or_else(|| find_executable("docker"))
        .or_else(|| find_executable("podman"))
        .context("Docker or Podman is required")?;

    if let Some(binary) = &options.binary
        && !binary.is_file()
    {
        bail!("desktop binary not found: {}", binary.display());
    }
    if options.selected_image {
        run_status(
            &engine,
            [
                OsStr::new("image"),
                OsStr::new("inspect"),
                OsStr::new(&options.image),
            ],
        )
        .with_context(|| format!("container image not found: {}", options.image))?;
    } else {
        println!("Building {} from .devcontainer/Dockerfile", options.image);
        run_status(
            &engine,
            [
                OsStr::new("build"),
                OsStr::new("--tag"),
                OsStr::new(&options.image),
                OsStr::new("--file"),
                root.join(".devcontainer/Dockerfile").as_os_str(),
                root.as_os_str(),
            ],
        )?;
    }

    let output = if options.output.is_absolute() {
        options.output.clone()
    } else {
        std::env::current_dir()?.join(&options.output)
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }

    let staging = tempfile::Builder::new()
        .prefix("latch-demo-source-")
        .tempdir()?;
    let snapshot = staging.path().join("latch");
    println!("Creating a disposable checkout snapshot");
    snapshot_checkout(&root, &snapshot)?;

    let name = format!("latch-demo-{}", std::process::id());
    let mut created = false;
    let result = (|| -> Result<()> {
        let create_args = container_create_args(&options.image, &name);
        run_status(&engine, create_args.iter().map(OsStr::new)).context("create demo container")?;
        created = true;
        run_status(&engine, [OsStr::new("start"), OsStr::new(&name)])
            .context("start demo container")?;
        docker_exec(
            &engine,
            &name,
            &["mkdir", "-p", "/workspaces/latch", "/output"],
            true,
        )?;
        run_status(
            &engine,
            [
                OsStr::new("cp"),
                snapshot.join(".").as_os_str(),
                OsStr::new(&format!("{name}:/workspaces/latch")),
            ],
        )
        .context("copy source snapshot into demo container")?;
        docker_exec(
            &engine,
            &name,
            &[
                "chown",
                "-R",
                "vscode:vscode",
                "/home/vscode/.cargo",
                "/home/vscode/.cache/latch-targets",
                "/home/vscode/.bun/install/cache",
                "/workspaces/latch",
                "/output",
            ],
            true,
        )?;

        let mut desktop = "/home/vscode/.cache/latch-targets/demo-recording/debug/latch-desktop";
        let mut seed = "/home/vscode/.cache/latch-targets/demo-recording/debug/examples/seed_dev";
        if let Some(binary) = &options.binary {
            let sibling_seed = binary
                .parent()
                .context("desktop binary has no parent directory")?
                .join("examples/seed_dev");
            if !sibling_seed.is_file() {
                bail!(
                    "accelerated binary requires its matching seed executable: {}",
                    sibling_seed.display()
                );
            }
            docker_copy_in(&engine, binary, &name, "/opt/latch-desktop")?;
            docker_copy_in(&engine, &sibling_seed, &name, "/opt/seed_dev")?;
            desktop = "/opt/latch-desktop";
            seed = "/opt/seed_dev";
        } else {
            for command in build_commands() {
                docker_exec(&engine, &name, &command, false)
                    .context("build demo application and Rust driver")?;
            }
        }
        docker_exec(
            &engine,
            &name,
            &[
                "cargo",
                "build",
                "--locked",
                "-p",
                "latch-automation",
                "--bin",
                "latch-automation",
            ],
            false,
        )
        .context("build Rust demo driver")?;

        let capture = capture_command(desktop, seed, options.fps, options.width);
        docker_exec(&engine, &name, &capture, false).context("record and encode demo GIF")?;
        run_status(
            &engine,
            [
                OsStr::new("cp"),
                OsStr::new(&format!("{name}:/output/demo.gif")),
                output.as_os_str(),
            ],
        )
        .context("copy recorded GIF from demo container")?;
        println!("Wrote {}", output.display());
        Ok(())
    })();

    if created {
        if options.keep_container {
            println!("Kept container {name}");
        } else {
            run_status(
                &engine,
                [OsStr::new("rm"), OsStr::new("--force"), OsStr::new(&name)],
            )
            .context("remove demo container")?;
        }
    }
    result
}

fn docker_exec(
    engine: &Path,
    container: &str,
    args: &[impl AsRef<OsStr>],
    root: bool,
) -> Result<()> {
    let user = if root { "root" } else { "vscode" };
    let mut command = vec![
        OsString::from("exec"),
        OsString::from("--user"),
        OsString::from(user),
        OsString::from(container),
    ];
    command.extend(args.iter().map(|arg| arg.as_ref().to_os_string()));
    run_status(engine, command).with_context(|| format!("run command in {container}"))
}

fn docker_copy_in(engine: &Path, source: &Path, container: &str, destination: &str) -> Result<()> {
    let target = format!("{container}:{destination}");
    run_status(
        engine,
        [OsStr::new("cp"), source.as_os_str(), OsStr::new(&target)],
    )
    .with_context(|| format!("copy {} into demo container", source.display()))
}

fn run_status<I, S>(program: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("execute {}", program.display()))?;
    if !status.success() {
        bail!("{} exited with status {status}", program.display());
    }
    Ok(())
}

fn find_executable(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn snapshot_checkout(root: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    let listing = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(root)
        .output()
        .context("list source files for demo snapshot")?;
    if !listing.status.success() {
        bail!("git ls-files failed while preparing demo snapshot");
    }
    for relative in listing
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let relative = Path::new(std::ffi::OsStr::from_bytes(relative));
        if !include_snapshot_path(relative) {
            continue;
        }
        let source = root.join(relative);
        let target = destination.join(relative);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("inspect {}", source.display()));
            }
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(&source)?, &target)?;
        } else if metadata.is_file() {
            fs::copy(&source, &target)
                .with_context(|| format!("copy {} into snapshot", source.display()))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

#[cfg(not(unix))]
compile_error!("demo source snapshots currently require a Unix host");
