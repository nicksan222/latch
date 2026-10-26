//! OS-process fixture and raw generated client; backend initialization is child-only.
use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use hyper_util::rt::TokioIo;
use latch_rpc::v1;
use prost::Message;
use tonic::codegen::http;
use tonic::transport::{Channel, Endpoint};
use tower::{Service, service_fn};

pub const PASS: &str = "external grpc fixture passphrase";
pub const SECRET: &str = "external-grpc-fictional-secret";
pub type Client = v1::latch_agent_client::LatchAgentClient<ObservedChannel>;

#[derive(Clone)]
pub struct ObservedChannel {
    channel: Channel,
    pub seen: Arc<Mutex<BTreeSet<String>>>,
}
impl Service<http::Request<tonic::body::Body>> for ObservedChannel {
    type Response = <Channel as Service<http::Request<tonic::body::Body>>>::Response;
    type Error = <Channel as Service<http::Request<tonic::body::Body>>>::Error;
    type Future = <Channel as Service<http::Request<tonic::body::Body>>>::Future;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.channel.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
        if let Some(method) = request.uri().path().strip_prefix("/latch.v1.LatchAgent/") {
            self.seen.lock().unwrap().insert(method.into());
        }
        self.channel.call(request)
    }
}

pub fn request<T>(body: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(body);
    request
        .metadata_mut()
        .insert("x-latch-client", "cli".parse().unwrap());
    request
        .metadata_mut()
        .insert("x-latch-version", "external-test/1".parse().unwrap());
    request.set_timeout(Duration::from_secs(30));
    request
}

pub fn expect_error(error: tonic::Status, expected: v1::ErrorCode) {
    assert!(
        !error.message().contains(SECRET) && !error.message().contains(PASS),
        "error echoed request data"
    );
    let details = v1::RpcError::decode(error.details()).expect("typed gRPC error details");
    assert_eq!(details.code, expected as i32);
    assert!(
        !details.message.contains(SECRET) && !details.message.contains(PASS),
        "error details echoed request data"
    );
}

pub fn profile_input(name: &str) -> v1::ProfileInput {
    v1::ProfileInput {
        name: name.into(),
        color: "#2563eb".into(),
        icon: "folder".into(),
        description: String::new(),
        pinned: false,
        browser: Some(v1::BrowserSettings {
            autofill: v1::AutofillScope::All as i32,
            ..Default::default()
        }),
    }
}

struct Host {
    child: Child,
    output: Option<std::thread::JoinHandle<()>>,
}
impl Host {
    fn start(root: &Path) -> Self {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "host::grpc_test_host",
                "--nocapture",
                "--test-threads=1",
            ])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command.env_clear().env("LANG", "C.UTF-8");
        command
            .env("LATCH_GRPC_TEST_ROOT", root)
            .env("LATCH_CONFIG_DIR", root.join("config"))
            .env("LATCH_DATA_DIR", root.join("data"))
            .env("LATCH_RUNTIME_DIR", root.join("r"))
            .env("LATCH_RAM_DIR", root.join("ram"))
            .env("LATCH_ALLOW_WEAK_KDF", "1")
            .env("LATCH_TERMINAL", root.join("bin/xterm"))
            .env("PATH", root.join("bin"))
            .env("XDG_CONFIG_HOME", root.join("xdg-config"))
            .env("XDG_DATA_HOME", root.join("xdg-data"))
            .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TEMPLATE_DIR", root.join("git-template"))
            .env("GIT_TERMINAL_PROMPT", "0");
        if root.join("native-browser").is_file() {
            command
                .env("MOZ_HEADLESS", "1")
                .env("HOME", root.join("home"));
        }
        let mut child = command.spawn().expect("start external server process");
        let output = child.stdout.take().unwrap();
        let (ready, receiver) = std::sync::mpsc::channel();
        let output = std::thread::spawn(move || {
            for line in std::io::BufReader::new(output)
                .lines()
                .map_while(Result::ok)
            {
                if line.contains("LATCH_GRPC_TEST_READY") {
                    let _ = ready.send(());
                }
            }
        });
        let mut host = Self {
            child,
            output: Some(output),
        };
        if receiver.recv_timeout(Duration::from_secs(15)).is_err() {
            host.stop();
            panic!("external gRPC host did not become ready");
        }
        host
    }
    fn stop(&mut self) {
        if let Some(mut stdin) = self.child.stdin.take() {
            let _ = stdin.write_all(b"shutdown\n");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(thread) = self.output.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct Harness {
    pub client: Client,
    pub seen: Arc<Mutex<BTreeSet<String>>>,
    host: Host,
    root: tempfile::TempDir,
}
impl Harness {
    pub async fn start() -> Self {
        Self::start_mode(false).await
    }
    pub async fn start_native() -> Self {
        Self::start_mode(true).await
    }
    async fn start_mode(native: bool) -> Self {
        let root = tempfile::Builder::new()
            .prefix("gx")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        fixtures(root.path());
        if native {
            std::fs::write(root.path().join("native-browser"), b"native").unwrap();
            std::fs::create_dir_all(root.path().join("home")).unwrap();
        }
        let host = Host::start(root.path());
        let seen = Arc::new(Mutex::new(BTreeSet::new()));
        let client = connect(root.path(), seen.clone()).await;
        Self {
            client,
            seen,
            host,
            root,
        }
    }
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.path().join(relative)
    }
    pub async fn init(&mut self) -> v1::InitResult {
        self.client
            .vault_init(request(v1::InitParams {
                path: self.path("vault").display().to_string(),
                passphrase: PASS.into(),
                kdf: Some(v1::KdfParams {
                    memory_kib: 256,
                    iterations: 1,
                    parallelism: 1,
                }),
                git_name: Some("External RPC tests".into()),
                git_email: Some("grpc@example.invalid".into()),
                recovery_kit: true,
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner()
    }
    pub async fn profile(&mut self, name: &str) -> v1::Profile {
        self.client
            .profiles_create(request(profile_input(name)))
            .await
            .unwrap()
            .into_inner()
    }
    pub async fn restart(&mut self) {
        self.host.stop();
        self.host = Host::start(self.root.path());
        self.client = connect(self.root.path(), self.seen.clone()).await;
    }
}

async fn connect(root: &Path, seen: Arc<Mutex<BTreeSet<String>>>) -> Client {
    let socket = root.join("r/agent.sock");
    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30))
        .connect_with_connector(service_fn(move |_| {
            let socket = socket.clone();
            async move {
                tokio::net::UnixStream::connect(socket)
                    .await
                    .map(TokioIo::new)
            }
        }))
        .await
        .expect("connect generated client to external server");
    v1::latch_agent_client::LatchAgentClient::new(ObservedChannel { channel, seen })
        .max_decoding_message_size(16 * 1024 * 1024)
        .max_encoding_message_size(32 * 1024 * 1024)
}

fn fixtures(root: &Path) {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir(root.join("git-template")).unwrap();
    std::fs::write(root.join("gitconfig"), "[user]\nname = External RPC tests\nemail = grpc@example.invalid\n[commit]\ngpgsign = false\n[tag]\ngpgsign = false\n").unwrap();
    for name in ["git", "mkdir", "sleep", "sh", "env", "true"] {
        let executable = [
            PathBuf::from("/usr/bin").join(name),
            PathBuf::from("/bin").join(name),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .expect("fixture executable");
        symlink(executable, bin.join(name)).unwrap();
    }
    std::fs::write(
        bin.join("chromium"),
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then printf 'Chromium fixture 1\n'; exit 0; fi
profile=''; previous=''
for argument in "$@"; do
 [ "$previous" = '-profile' ] && profile="$argument"
 previous="$argument"
done
[ -n "$profile" ] || exit 0
mkdir -p "$profile"
printf 'fictional-session' > "$profile/cookies.sqlite"
trap 'exit 0' TERM INT
while :; do sleep 0.1; done
"##,
    )
    .unwrap();
    std::fs::write(
        bin.join("xterm"),
        r##"#!/bin/sh
while [ "$#" -gt 0 ] && [ "$1" != '-e' ]; do shift; done
[ "$#" -gt 0 ] && shift
"$@"
status=$?
sleep 0.25
exit "$status"
"##,
    )
    .unwrap();
    for name in ["chromium", "xterm"] {
        std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}
