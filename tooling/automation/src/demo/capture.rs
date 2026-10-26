use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PROJECT: &str = "/workspaces/latch";
const OUTPUT: &str = "/output";
const PASSWORD: &str = "correct horse battery staple";
const WINDOW_LEFT: i32 = 160;
const WINDOW_TOP: i32 = 100;

pub(super) fn run() -> Result<()> {
    let fps = env_number("LATCH_DEMO_FPS", 10)?;
    let width = env_number("LATCH_DEMO_WIDTH", 1280)?;
    fs::create_dir_all(OUTPUT)?;
    seed()?;

    let mut window_manager = ChildGuard::spawn("openbox", &[], Stdio::null(), Stdio::null())?;
    let log_file = File::create(format!("{OUTPUT}/app.log"))?;
    let mut app = ChildGuard::spawn(
        required_env("LATCH_DEMO_BINARY")?,
        &[],
        Stdio::from(log_file.try_clone()?),
        Stdio::from(log_file),
    )?;
    let result = (|| -> Result<()> {
        let window = wait_for_window(Duration::from_secs(30))?;
        xdo(&["windowsize", &window, "1600", "1000"])?;
        xdo(&["windowmove", &window, "160", "100"])?;
        xdo(&["windowactivate", "--sync", &window])?;
        thread::sleep(Duration::from_secs(2));
        screenshot("locked")?;

        let input = format!(":99.0+{WINDOW_LEFT},{WINDOW_TOP}");
        let mut recorder = ChildGuard::spawn(
            "ffmpeg",
            &[
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "x11grab",
                "-framerate",
                &fps.to_string(),
                "-video_size",
                "1600x1000",
                "-i",
                &input,
                "-c:v",
                "ffv1",
                "/output/capture.mkv",
            ],
            Stdio::null(),
            Stdio::inherit(),
        )?;
        let capture_result = (|| -> Result<()> {
            thread::sleep(Duration::from_millis(1400));
            xdo(&["type", "--clearmodifiers", "--delay", "32", PASSWORD])?;
            thread::sleep(Duration::from_millis(500));
            xdo(&["key", "Return"])?;
            thread::sleep(Duration::from_secs(3));
            screenshot("unlocked")?;

            click(410, 500, 1200)?;
            screenshot("login")?;
            click(1420, 275, 1200)?;
            click(1448, 275, 1400)?;
            screenshot("secret")?;
            click(90, 160, 1400)?;
            screenshot("profile")?;
            click(90, 125, 1200)?;
            click(410, 225, 1400)?;
            screenshot("environment")?;
            click(900, 240, 1200)?;
            thread::sleep(Duration::from_millis(1500));
            screenshot("environment-run")?;
            Ok(())
        })();
        recorder.interrupt_and_wait(Duration::from_secs(10))?;
        capture_result?;
        encode(fps, width)?;
        Ok(())
    })();
    app.terminate_and_wait(Duration::from_secs(10));
    window_manager.terminate_and_wait(Duration::from_secs(5));
    result?;
    println!("Captured and encoded the real native Latch application.");
    Ok(())
}

fn seed() -> Result<()> {
    let seed = required_env("LATCH_DEMO_SEED")?;
    if !Path::new(&seed).is_file() {
        bail!("seed binary not found: {seed}");
    }
    let mut command = Command::new(seed);
    command.current_dir(PROJECT);
    command.env("GIT_CONFIG_GLOBAL", "/dev/null");
    command.env("GIT_CONFIG_NOSYSTEM", "1");
    for (key, _) in std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("GIT_"))
    {
        if key != "GIT_CONFIG_GLOBAL" && key != "GIT_CONFIG_NOSYSTEM" {
            command.env_remove(key);
        }
    }
    status(command, "seed demo vault")
}

fn wait_for_window(timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let output = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--name", "^Latch$"])
            .output()
            .context("search for Latch window")?;
        if output.status.success()
            && let Some(window) = String::from_utf8_lossy(&output.stdout).lines().next()
        {
            return Ok(window.to_owned());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("Latch window did not appear")
}

fn click(x: i32, y: i32, pause_ms: u64) -> Result<()> {
    let x = WINDOW_LEFT + x;
    let y = WINDOW_TOP + y;
    xdo(&["mousemove", &x.to_string(), &y.to_string(), "click", "1"])?;
    thread::sleep(Duration::from_millis(pause_ms));
    Ok(())
}

fn xdo(args: &[&str]) -> Result<()> {
    let mut command = Command::new("xdotool");
    command.args(args);
    status(command, "drive Latch window")
}

fn screenshot(name: &str) -> Result<()> {
    let mut command = Command::new("scrot");
    command.args(["-o", "-u", &format!("{OUTPUT}/stage-{name}.png")]);
    status(command, "capture demo frame")
}

fn encode(fps: u32, width: u32) -> Result<()> {
    let filter = format!(
        "fps={fps},scale={width}:-2:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=192:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle"
    );
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        "/output/capture.mkv",
        "-vf",
        &filter,
        "-loop",
        "0",
        "/output/demo.gif",
    ]);
    status(command, "encode demo GIF")
}

fn required_env(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("{key} is not set"))
}

fn env_number(key: &str, default: u32) -> Result<u32> {
    std::env::var(key)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{key} must be an integer"))
        })
        .unwrap_or(Ok(default))
}

fn status(mut command: Command, action: &str) -> Result<()> {
    let status = command
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("start process to {action}"))?;
    if !status.success() {
        bail!("process failed to {action}: {status}");
    }
    Ok(())
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn spawn(
        program: impl AsRef<Path>,
        args: &[&str],
        stdout: Stdio,
        stderr: Stdio,
    ) -> Result<Self> {
        let child = Command::new(program.as_ref())
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .with_context(|| format!("start {}", program.as_ref().display()))?;
        Ok(Self(Some(child)))
    }

    fn interrupt_and_wait(&mut self, timeout: Duration) -> Result<()> {
        if let Some(child) = &mut self.0 {
            #[cfg(unix)]
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGINT);
            }
            let status = wait_until(child, timeout)?;
            if status.is_none() {
                child.kill()?;
                let status = child.wait()?;
                bail!("ffmpeg recorder did not stop after SIGINT (status {status})");
            }
            self.0 = None;
        }
        Ok(())
    }

    fn terminate_and_wait(&mut self, timeout: Duration) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = wait_until(child, timeout);
            let _ = child.wait();
            self.0 = None;
        }
    }
}

fn wait_until(child: &mut Child, timeout: Duration) -> Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
