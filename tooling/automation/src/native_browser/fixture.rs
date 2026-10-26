//! Isolated native Firefox fixture; scenario bodies contain only test actions.
use super::marionette;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const FORM: &str = "<form action=\"/login\" method=\"POST\"><label>User<input name=\"username\" autocomplete=\"username\"></label><label>Password<input type=\"password\" name=\"password\" autocomplete=\"current-password\"></label><button>Sign in</button></form>";
const READY_TIMEOUT: Duration = Duration::from_secs(45);
const ACK_TIMEOUT: Duration = Duration::from_secs(20);
const PROFILE_PREFS: &str = "user_pref(\"marionette.port\", 0);\nuser_pref(\"signon.autofillForms\", true);\nuser_pref(\"signon.autofillForms.http\", true);\nuser_pref(\"signon.rememberSignons\", true);\nuser_pref(\"browser.shell.checkDefaultBrowser\", false);\nuser_pref(\"browser.startup.page\", 0);\nuser_pref(\"browser.tabs.warnOnClose\", false);\n";

fn seed(
    profile: &Path,
    generation: &mut u64,
    credential: Option<(&str, &str, &str)>,
) -> Result<String> {
    let credentials = credential
        .map(|(origin, username, password)| {
            json!([{"id":"fixture-login","origin":origin,"username":username,"password":password}])
        })
        .unwrap_or_else(|| json!([]));
    seed_rows(profile, generation, credentials)
}

fn seed_rows(profile: &Path, generation: &mut u64, credentials: Value) -> Result<String> {
    *generation += 1;
    let generation = format!("fixture-{generation}");
    let bytes =
        serde_json::to_vec(&json!({"version":1,"generation":generation,"credentials":credentials}))
            .context("encode synthetic native-browser test snapshot")?;
    write_private(&profile.join("latch-credentials.json"), &bytes)?;
    Ok(generation)
}

fn wait_ack(
    profile: &Path,
    generation: &str,
    success: bool,
    managed: Option<u64>,
) -> Result<Value> {
    let ack = profile.join("latch-credentials-result.json");
    let end = Instant::now() + ACK_TIMEOUT;
    loop {
        if let Ok(bytes) = std::fs::read(&ack) {
            let value: Value =
                serde_json::from_slice(&bytes).context("decode native-browser acknowledgement")?;
            if value.get("generation").and_then(Value::as_str) == Some(generation) {
                let _ = std::fs::remove_file(&ack);
                if value.get("ok").and_then(Value::as_bool) != Some(success) {
                    bail!("native-browser synchronization had an unexpected result");
                }
                if let Some(expected) = managed
                    && value.get("managed").and_then(Value::as_u64) != Some(expected)
                {
                    bail!("native-browser synchronization returned an unexpected login count");
                }
                return Ok(value);
            }
        }
        if Instant::now() >= end {
            bail!("native-browser synchronization acknowledgement timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("pending");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .context("open private browser fixture file")?;
    file.write_all(bytes)
        .context("write private browser fixture file")?;
    file.sync_all()
        .context("flush private browser fixture file")?;
    std::fs::rename(temporary, path).context("publish complete private browser fixture")?;
    Ok(())
}

fn native_cache() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LATCH_NATIVE_BROWSER_CACHE") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("locate home directory for managed Firefox cache")?;
    Ok(home.join(".cache/latch-browser-tests"))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

struct FixtureServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureServer {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .context("bind local native-browser fixture server")?;
        let port = listener
            .local_addr()
            .context("read fixture server port")?
            .port();
        listener
            .set_nonblocking(true)
            .context("configure fixture server")?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = std::thread::Builder::new()
            .name("latch-native-browser-http".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => serve_fixture(&mut stream),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => break,
                    }
                }
            })
            .context("start native-browser fixture server")?;
        Ok(Self {
            port,
            stop,
            thread: Some(thread),
        })
    }

    fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_fixture(stream: &mut TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = [0; 4096];
    let _ = stream.read(&mut request);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        FORM.len(),
        FORM
    );
    let _ = stream.write_all(response.as_bytes());
}

struct NativeFirefox {
    child: FirefoxProcess,
    stream: TcpStream,
    next_id: u32,
    closed: bool,
}

impl NativeFirefox {
    fn start(executable: &Path, profile: &Path) -> Result<Self> {
        write_private(&profile.join("user.js"), PROFILE_PREFS.as_bytes())?;
        let mut command = Command::new(executable);
        command
            .args(["--marionette", "-no-remote", "-profile"])
            .arg(profile)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if std::env::var("LATCH_BROWSER_HEADLESS").as_deref() == Ok("1") {
            command.arg("--headless");
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = FirefoxProcess(
            command
                .spawn()
                .context("launch managed Firefox for native-browser acceptance")?,
        );
        let port_file = profile.join("MarionetteActivePort");
        let deadline = Instant::now() + READY_TIMEOUT;
        let mut stream = loop {
            if let Ok(contents) = std::fs::read_to_string(&port_file)
                && let Ok(port) = contents.trim().parse::<u16>()
                && let Ok(stream) = TcpStream::connect_timeout(
                    &SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into(),
                    Duration::from_millis(300),
                )
            {
                break stream;
            }
            if child
                .0
                .try_wait()
                .context("check managed Firefox startup")?
                .is_some()
            {
                let _ = std::fs::remove_file(&port_file);
                bail!("managed Firefox exited before Marionette became available");
            }
            if Instant::now() >= deadline {
                let _ = std::fs::remove_file(&port_file);
                bail!("managed Firefox Marionette startup timed out");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .context("set Marionette read timeout")?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .context("set Marionette write timeout")?;
        let hello = read_hello(&mut stream)?;
        if hello.get("applicationType").and_then(Value::as_str) != Some("gecko")
            || hello
                .get("marionetteProtocol")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                < 3
        {
            bail!("managed Firefox returned an unsupported Marionette handshake");
        }
        let _ = std::fs::remove_file(port_file);
        let mut browser = Self {
            child,
            stream,
            next_id: 0,
            closed: false,
        };
        browser.command("WebDriver:NewSession", json!({"capabilities": {}}))?;
        Ok(browser)
    }

    fn command(&mut self, name: &str, params: Value) -> Result<Value> {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        marionette::write_packet(&mut self.stream, &json!([0, id, name, params]))?;
        let packet = marionette::read_packet(&mut self.stream)?;
        marionette::response(packet, id)
    }

    fn navigate(&mut self, url: &str) -> Result<()> {
        self.command("WebDriver:Navigate", json!({"url":url}))?;
        Ok(())
    }

    fn element(&mut self, selector: &str) -> Result<String> {
        let value = self.command(
            "WebDriver:FindElement",
            json!({"using":"css selector","value":selector}),
        )?;
        element_id(&value).context("Marionette did not return an element reference")
    }

    fn click(&mut self, selector: &str) -> Result<()> {
        let id = self.element(selector)?;
        self.command("WebDriver:ElementClick", json!({"id":id}))?;
        Ok(())
    }

    fn property(&mut self, selector: &str, property: &str) -> Result<String> {
        let id = self.element(selector)?;
        let value = self.command(
            "WebDriver:GetElementProperty",
            json!({"id":id,"name":property}),
        )?;
        string_property(&value).context("Marionette element property was not a string")
    }

    fn wait_property(
        &mut self,
        selector: &str,
        property: &str,
        expected: &str,
        timeout: Duration,
    ) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut property_read = false;
        let mut non_empty = false;
        loop {
            if let Ok(value) = self.property(selector, property) {
                property_read = true;
                non_empty = !value.is_empty();
                if value == expected {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                bail!(
                    "managed Firefox form value did not reach the expected state (readable: {property_read}, non-empty: {non_empty})"
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn close(&mut self) -> Result<()> {
        if !self.closed {
            let _ = self.command("WebDriver:DeleteSession", json!({}));
            self.child.terminate();
            self.closed = true;
        }
        Ok(())
    }
}

impl Drop for NativeFirefox {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct FirefoxProcess(Child);

impl FirefoxProcess {
    fn terminate(&mut self) {
        let child = &mut self.0;
        #[cfg(unix)]
        {
            let group = -(child.id() as i32);
            // Firefox owns helper processes; request a clean shutdown for its group.
            // SAFETY: this is the private process group created for this child.
            unsafe { libc::kill(group, libc::SIGTERM) };
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            // Remove any helper left behind after the browser process exits.
            // SAFETY: the isolated process group belongs to this Firefox process.
            unsafe { libc::kill(group, libc::SIGKILL) };
        }
        #[cfg(not(unix))]
        {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
        }
        let _ = child.wait();
    }
}

impl Drop for FirefoxProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn read_hello<R: Read>(reader: &mut R) -> Result<Value> {
    let mut first = [0; 1];
    reader
        .read_exact(&mut first)
        .context("read Marionette handshake")?;
    let mut bytes = Vec::with_capacity(64);
    if first[0] == b'{' {
        bytes.push(first[0]);
        loop {
            let mut byte = [0; 1];
            reader
                .read_exact(&mut byte)
                .context("read Marionette handshake")?;
            bytes.push(byte[0]);
            if byte[0] == b'}' {
                break;
            }
            if bytes.len() > 4096 {
                bail!("Marionette handshake exceeded the size limit");
            }
        }
    } else if first[0].is_ascii_digit() {
        let mut length = vec![first[0]];
        loop {
            let mut byte = [0; 1];
            reader
                .read_exact(&mut byte)
                .context("read Marionette handshake length")?;
            match byte[0] {
                b':' => break,
                b'0'..=b'9' if length.len() < 10 => length.push(byte[0]),
                _ => bail!("invalid Marionette handshake length"),
            }
        }
        let length: usize = std::str::from_utf8(&length)
            .context("decode Marionette handshake length")?
            .parse()
            .context("parse Marionette handshake length")?;
        if length > 4096 {
            bail!("Marionette handshake exceeded the size limit");
        }
        bytes.resize(length, 0);
        reader
            .read_exact(&mut bytes)
            .context("read Marionette handshake body")?;
    } else {
        bail!("invalid Marionette handshake prefix");
    }
    serde_json::from_slice(&bytes).context("decode Marionette handshake")
}

fn element_id(value: &Value) -> Option<String> {
    std::iter::once(value)
        .chain(value.get("value"))
        .find_map(|item| {
            item.get("element-6066-11e4-a52e-4f735466cecf")
                .or_else(|| item.get("ELEMENT"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn string_property(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value.get("value").and_then(Value::as_str))
        .map(str::to_owned)
}

fn prepare_runtime() -> Result<PathBuf> {
    let cache = native_cache()?;
    std::fs::create_dir_all(&cache).context("create managed Firefox cache")?;
    let output = Command::new("cargo")
        .args([
            "run",
            "--locked",
            "-q",
            "-p",
            "latch-core",
            "--no-default-features",
            "--example",
            "managed_browser",
            "--",
        ])
        .arg(&cache)
        .current_dir(repository_root())
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .context("prepare the pinned managed Firefox runtime")?;
    if !output.status.success() {
        bail!("managed Firefox runtime preparation failed");
    }
    let path = String::from_utf8(output.stdout)
        .context("read managed Firefox executable path")?
        .trim()
        .to_owned();
    let executable = PathBuf::from(path);
    if !executable.is_file() {
        bail!("managed Firefox runtime preparation returned no executable");
    }
    Ok(executable)
}

/// Owns the browser before its temporary profile and server are removed.
pub struct BrowserFixture {
    browser: Option<NativeFirefox>,
    profile: tempfile::TempDir,
    server: FixtureServer,
    executable: PathBuf,
    generation: u64,
}
impl BrowserFixture {
    pub fn start() -> Result<Self> {
        static RUNTIME: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
        let executable = RUNTIME
            .get_or_init(|| prepare_runtime().map_err(|e| format!("{e:#}")))
            .clone()
            .map_err(anyhow::Error::msg)?;
        let profile = tempfile::Builder::new()
            .prefix("latch-firefox-native-")
            .tempdir()?;
        let server = FixtureServer::start()?;
        let browser = Some(NativeFirefox::start(&executable, profile.path())?);
        Ok(Self {
            browser,
            profile,
            server,
            executable,
            generation: 0,
        })
    }
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.server.port())
    }
    pub fn seed_login(&mut self, username: &str, password: &str) -> Result<()> {
        let origin = self.origin();
        let generation = seed(
            self.profile.path(),
            &mut self.generation,
            Some((&origin, username, password)),
        )?;
        wait_ack(self.profile.path(), &generation, true, Some(1))?;
        anyhow::ensure!(
            !self.profile.path().join("latch-credentials.json").exists(),
            "credential batch was not consumed"
        );
        Ok(())
    }
    pub fn reject_batch(&mut self, rows: Value) -> Result<()> {
        let generation = seed_rows(self.profile.path(), &mut self.generation, rows)?;
        wait_ack(self.profile.path(), &generation, false, None)?;
        Ok(())
    }
    pub fn delete_login(&mut self) -> Result<()> {
        let generation = seed(self.profile.path(), &mut self.generation, None)?;
        wait_ack(self.profile.path(), &generation, true, Some(0))?;
        Ok(())
    }
    pub fn navigate_to_login(&mut self) -> Result<()> {
        let origin = self.origin();
        let browser = self.browser.as_mut().context("browser is closed")?;
        browser.navigate(&origin)?;
        browser.click("[name=username]")
    }
    pub fn navigate_to_other_origin(&mut self) -> Result<()> {
        let origin = format!("http://localhost:{}", self.server.port());
        self.browser
            .as_mut()
            .context("browser is closed")?
            .navigate(&origin)
    }
    pub fn assert_password(&mut self, expected: &str) -> Result<()> {
        self.browser
            .as_mut()
            .context("browser is closed")?
            .wait_property("[name=password]", "value", expected, ACK_TIMEOUT)
    }
    pub fn reopen_with_login(&mut self, username: &str, password: &str) -> Result<()> {
        drop(self.browser.take());
        let origin = self.origin();
        let generation = seed(
            self.profile.path(),
            &mut self.generation,
            Some((&origin, username, password)),
        )?;
        self.browser = Some(NativeFirefox::start(&self.executable, self.profile.path())?);
        wait_ack(self.profile.path(), &generation, true, Some(1))?;
        Ok(())
    }
    pub fn revoke_ownership(&self) -> Result<()> {
        std::fs::write(
            self.profile.path().join("latch-login-ownership.json"),
            b"[]",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    #[test]
    fn fixture_publication_never_exposes_partial_json() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("snapshot.json");
        let bytes = serde_json::to_vec(&json!({"synthetic": "x".repeat(65536)})).unwrap();
        write_private(&path, &bytes).unwrap();
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                for _ in 0..100 {
                    write_private(&path, &bytes).unwrap();
                }
            });
            while !writer.is_finished() {
                let snapshot = std::fs::read(&path).unwrap();
                assert!(
                    serde_json::from_slice::<Value>(&snapshot).is_ok(),
                    "fixture readers must see a complete snapshot"
                );
            }
            writer.join().unwrap();
        });
    }

    #[test]
    fn private_browser_fixture_files_are_restricted() {
        let root = TempDir::new().unwrap();
        write_private(&root.path().join("fixture.json"), b"synthetic")
            .expect("write private browser fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(root.path().join("fixture.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn local_fixture_server_serves_login_form() {
        let server = FixtureServer::start().unwrap();
        let mut stream =
            TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, server.port)).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("autocomplete=\"username\""));
        assert!(response.contains("autocomplete=\"current-password\""));
    }

    #[test]
    fn marionette_handshake_accepts_framed_and_unframed_json() {
        let hello = br#"{"applicationType":"gecko","marionetteProtocol":3}"#;
        let mut unframed = Cursor::new(hello);
        let mut framed =
            Cursor::new([format!("{}:", hello.len()).into_bytes(), hello.to_vec()].concat());

        let plain = read_hello(&mut unframed).unwrap();
        let length_prefixed = read_hello(&mut framed).unwrap();

        assert_eq!(plain, length_prefixed);
        assert_eq!(plain["marionetteProtocol"], 3);
    }

    #[test]
    fn web_element_id_accepts_marionette_element_references() {
        assert_eq!(
            element_id(&json!({"element-6066-11e4-a52e-4f735466cecf":"id"})).as_deref(),
            Some("id")
        );
        assert_eq!(
            element_id(&json!({"value":{"ELEMENT":"legacy-id"}})).as_deref(),
            Some("legacy-id")
        );
        assert!(element_id(&json!({"value":"not-an-element"})).is_none());
    }

    #[test]
    fn element_property_accepts_marionette_result_wrapper() {
        assert_eq!(
            string_property(&json!("current")).as_deref(),
            Some("current")
        );
        assert_eq!(
            string_property(&json!({"value":"current"})).as_deref(),
            Some("current")
        );
        assert!(string_property(&json!({"value":null})).is_none());
    }
}
